/**
 * **Un `:hover` no desplaza la caja del elemento que lo recibe**, comprobado
 * rompiéndolo.
 *
 * La regla vive en `scripts/check-ui.mjs` desde el 2026-09-02, cuando el hover de
 * las tarjetas temblaba en Inicio, en la biblioteca y en Descubrir: levantar la
 * caja la sacaba de debajo del ratón, el hover se apagaba, bajaba y se encendía
 * otra vez. La revisión la rompió a propósito el 2026-09-11 y **pasó**:
 * el guardián tomaba el espacio que Prettier deja antes de la llave por un
 * combinador, así que ninguna regla formateada contaba. Y al escribir estas
 * pruebas apareció la segunda mitad: `translate(0, -2px)` pasaba por quieto.
 *
 * Se fijan las dos cosas: que lo que mueve la caja se rechaza, y que lo que no la
 * mueve se sigue admitiendo. Una regla que rechaza de más se acaba apagando.
 */

import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, describe, expect, it, vi } from 'vitest';

// Lo que cada prueba crea en el temporal se borra al acabar: sin esto se
// acumulaba una carpeta por ejecución.
const creados: string[] = [];
afterAll(() => {
  for (const d of creados) rmSync(d, { recursive: true, force: true });
});

const RAIZ = fileURLToPath(new URL('..', import.meta.url));
const AVISO = 'un :hover que se desplaza';

/*
 * Cada prueba lanza el guardián en otro proceso, y la primera le da la interfaz
 * entera: con la máquina ocupada por el resto de `pnpm verify` eso pasa de los
 * cinco segundos de fábrica y la prueba falla sin que nadie haya roto la regla.
 * Una prueba que depende de lo ocupado que esté el equipo no mide lo que dice.
 */
vi.setConfig({ testTimeout: 30_000 });

function guardian(fuentes: string): { salida: number; texto: string } {
  try {
    const texto = execFileSync('node', [join(RAIZ, 'scripts', 'check-ui.mjs'), fuentes], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe']
    });
    return { salida: 0, texto };
  } catch (error) {
    const fallo = error as { status: number; stdout: string; stderr: string };
    return { salida: fallo.status, texto: `${fallo.stdout}${fallo.stderr}` };
  }
}

/** Un árbol con un componente y la hoja base, que el guardián lee siempre. */
function arbol(estilo: string): string {
  const directorio = mkdtempSync(join(tmpdir(), 'nb-hover-'));
  creados.push(directorio);
  mkdirSync(join(directorio, 'lib'), { recursive: true });
  writeFileSync(join(directorio, 'app.css'), ':root {\n  color-scheme: dark;\n}\n');
  writeFileSync(
    join(directorio, 'lib', 'Tarjeta.svelte'),
    [
      '<div class="nb-tarjeta"><span class="nb-portada">Portada</span></div>',
      '',
      '<style>',
      '  .nb-tarjeta {',
      '    color: var(--m3c-on-surface);',
      '  }',
      '',
      '  .nb-portada {',
      '    color: var(--m3c-primary);',
      '  }',
      '',
      estilo,
      '</style>',
      ''
    ].join('\n')
  );
  return directorio;
}

describe('un :hover no desplaza la caja del elemento que lo recibe', () => {
  it('la interfaz real no tiene ninguno', () => {
    const { salida, texto } = guardian(join(RAIZ, 'src'));
    expect(texto).not.toContain(AVISO);
    expect(salida).toBe(0);
  });

  it('se rechaza escrito como lo deja Prettier, con espacio antes de la llave', () => {
    // **Regresión**: es exactamente la forma que pasaba.
    const { salida, texto } = guardian(
      arbol('  .nb-tarjeta:hover {\n    transform: translateY(-2px);\n  }')
    );
    expect(salida).toBe(1);
    expect(texto).toContain(AVISO);
  });

  it('se rechaza en todas las formas de mover la caja', () => {
    for (const estilo of [
      '  .nb-tarjeta:hover{transform: translateX(4px);}',
      '  .nb-tarjeta:hover:not(:disabled) {\n    transform: translateY(-1px) scale(1.02);\n  }',
      // **Regresión**: con el primer argumento a cero pasaba por quieto.
      '  .nb-tarjeta:hover {\n    transform: translate(0, -2px);\n  }',
      '  .nb-tarjeta:hover {\n    translate: 0 -2px;\n  }'
    ]) {
      expect(guardian(arbol(estilo)).texto, estilo).toContain(AVISO);
    }
  });

  it('lo que no mueve el área que recibe el hover se sigue admitiendo', () => {
    for (const estilo of [
      // Mueve a un descendiente: el área de la tarjeta no cambia.
      '  .nb-tarjeta:hover .nb-portada {\n    transform: translateY(-2px);\n  }',
      // Agrandar solo puede ampliar el área, así que el ciclo no puede darse.
      '  .nb-tarjeta:hover {\n    transform: scale(1.02);\n  }',
      // Devolver a su sitio no es moverse.
      '  .nb-tarjeta:hover {\n    transform: translate(0, 0);\n    translate: none;\n  }'
    ]) {
      expect(guardian(arbol(estilo)).texto, estilo).not.toContain(AVISO);
    }
  });
});
