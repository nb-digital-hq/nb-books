/**
 * **El rendimiento en WebKitGTK, en la suite.**
 *
 * Contrato: `docs/architecture/performance-budgets.md` §4.3.0 y §2.
 *
 * # Por qué existe
 *
 * Porque la suite de interfaz corre en Chromium y la aplicación corre en WebKitGTK.
 * La comprobación ejecuta un recorrido sobre el WebKitGTK del sistema y
 * mide los tres recursos deterministas:
 *   - Nodos DOM (< 1500 por vista)
 *   - Píxeles descodificados acotados (miniaturas y no originales que inflan a 183 MB)
 *   - Capas de composición permanentes (0 elementos con will-change en reposo)
 *
 * Registra además los fotogramas sin decidir aprobado/suspenso (I14), y se salta
 * limpiamente con explicación donde no hay WebKitGTK disponible (I13).
 */

import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it, vi } from 'vitest';

const RAIZ = fileURLToPath(new URL('..', import.meta.url));
const SCRIPT = join(RAIZ, 'scripts', 'recorrido-webkitgtk.py');

vi.setConfig({ testTimeout: 60_000 });

function ejecutarScript(argumentos: string[]): { salida: string; codigo: number } {
  try {
    const salida = execFileSync('python3', [SCRIPT, ...argumentos], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe']
    });
    return { salida, codigo: 0 };
  } catch (error) {
    const fallo = error as { status: number; stdout: string; stderr: string };
    return { salida: `${fallo.stdout}${fallo.stderr}`, codigo: fallo.status };
  }
}

describe('recorrido de rendimiento en WebKitGTK', () => {
  it('ejecuta el recorrido o se salta limpiamente si no hay WebKitGTK', () => {
    const { salida, codigo } = ejecutarScript(['--json']);
    expect(codigo, `el script terminó con error: ${salida}`).toBe(0);

    // Si se omitió por falta de WebKitGTK, GTK o servidor gráfico:
    if (salida.startsWith('Omitiendo')) {
      expect(salida).toMatch(/Omitiendo recorrido de rendimiento en WebKitGTK:/);
      return;
    }

    // Si corrió, debe ser un JSON válido con las tres vistas
    const datos = JSON.parse(salida);
    expect(datos).toHaveProperty('inicio');
    expect(datos).toHaveProperty('biblioteca');
    expect(datos).toHaveProperty('lector');

    // Inicio
    expect(datos.inicio.nodos).toBeLessThanOrEqual(1500);
    expect(datos.inicio.capasWillChange).toBe(0);
    expect(datos.inicio.pixelesDescodificados).toBeLessThanOrEqual(15_000_000);
    expect(datos.inicio.fotogramas).toHaveProperty('mediana');
    expect(datos.inicio.fotogramas).toHaveProperty('largos');

    // Biblioteca
    expect(datos.biblioteca.nodos).toBeLessThanOrEqual(1500);
    expect(datos.biblioteca.capasWillChange).toBe(0);
    expect(datos.biblioteca.pixelesDescodificados).toBeLessThanOrEqual(20_000_000);
    expect(datos.biblioteca.fotogramas).toHaveProperty('mediana');
    expect(datos.biblioteca.fotogramas).toHaveProperty('largos');

    // Lector
    expect(datos.lector.nodos).toBeLessThanOrEqual(1500);
    expect(datos.lector.capasWillChange).toBe(0);
    expect(datos.lector.pixelesDescodificados).toBeLessThanOrEqual(5_000_000);
    expect(datos.lector.fotogramas).toHaveProperty('mediana');
    expect(datos.lector.fotogramas).toHaveProperty('largos');
  });

  it('formatea el reporte textual y el resumen', () => {
    const { salida, codigo } = ejecutarScript(['--resumen']);
    expect(codigo).toBe(0);
    if (!salida.startsWith('Omitiendo')) {
      expect(salida).toContain(
        'WebKitGTK: Inicio, Biblioteca y Lector cumplen los presupuestos de nodos, píxeles y capas.'
      );
    }
  });

  describe('comprobado rompiéndolo (hace cumplir los presupuestos)', () => {
    const baseValida = {
      inicio: {
        nodos: 450,
        pixelesDescodificados: 1_200_000,
        capasWillChange: 0,
        fotogramas: { total: 60, mediana: 16.6, p95: 18, peor: 22, largos: 1 }
      },
      biblioteca: {
        nodos: 220,
        pixelesDescodificados: 960_000,
        capasWillChange: 0,
        fotogramas: { total: 60, mediana: 16.6, p95: 17, peor: 18, largos: 0 }
      },
      lector: {
        nodos: 330,
        pixelesDescodificados: 480_000,
        capasWillChange: 0,
        fotogramas: { total: 60, mediana: 16.6, p95: 20, peor: 22, largos: 2 }
      }
    };

    it('falla si los nodos pasan del presupuesto de 1500', () => {
      const datosRotos = JSON.parse(JSON.stringify(baseValida));
      datosRotos.biblioteca.nodos = 1501;

      const { salida, codigo } = ejecutarScript(['--datos-simulados', JSON.stringify(datosRotos)]);
      if (salida.startsWith('Omitiendo')) return;

      expect(codigo).toBe(1);
      expect(salida).toContain('Biblioteca: 1501 nodos excede el presupuesto (1500)');
    });

    it('falla si hay capas will-change permanentes en reposo', () => {
      const datosRotos = JSON.parse(JSON.stringify(baseValida));
      datosRotos.inicio.capasWillChange = 2;

      const { salida, codigo } = ejecutarScript(['--datos-simulados', JSON.stringify(datosRotos)]);
      if (salida.startsWith('Omitiendo')) return;

      expect(codigo).toBe(1);
      expect(salida).toContain('Inicio: 2 elementos con will-change permanente en reposo');
    });

    it('falla si los píxeles descodificados pasan del presupuesto', () => {
      const datosRotos = JSON.parse(JSON.stringify(baseValida));
      datosRotos.biblioteca.pixelesDescodificados = 25_000_000;

      const { salida, codigo } = ejecutarScript(['--datos-simulados', JSON.stringify(datosRotos)]);
      if (salida.startsWith('Omitiendo')) return;

      expect(codigo).toBe(1);
      expect(salida).toContain(
        'Biblioteca: 25,000,000 píxeles descodificados excede el presupuesto'
      );
    });
  });
});
