/**
 * El catálogo de iconos y su documentación no pueden divergir.
 *
 * Es el mismo problema que resuelve `check:bindings` entre Rust y el puente, y
 * se resuelve igual: se permite la repetición y se prohíbe la deriva. Un icono
 * en el código y no en el documento es un icono que nadie sabe que existe;
 * al revés, es una promesa que no se puede pintar.
 */

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import { CUANTOS_ICONOS, ICONOS } from './iconos';

const DOC = readFileSync(resolve(import.meta.dirname, '../../../docs/design/icons.md'), 'utf8');

/**
 * Nombres que la documentación lista, uno por fila de tabla.
 *
 * La regla es estricta a propósito: **cualquier** fila de tabla cuya primera
 * celda sea solo un nombre entre comillas se considera un icono prometido. Una
 * tabla de este documento que hable de otra cosa —la marca, por ejemplo— no
 * pone el nombre en la primera celda, y así no hay forma de prometer sin querer
 * un icono que no existe.
 */
const documentados = new Set(
  Array.from(DOC.matchAll(/^\| `([a-zA-Z]+)` \|/gm)).map(([, nombre]) => nombre as string)
);

describe('el catálogo de iconos', () => {
  it('documenta todos los que existen', () => {
    for (const nombre of Object.keys(ICONOS)) {
      expect(documentados.has(nombre), `${nombre} no está en docs/design/icons.md`).toBe(true);
    }
  });

  it('no promete ninguno que no exista', () => {
    for (const nombre of documentados) {
      expect(nombre in ICONOS, `${nombre} está documentado y no existe`).toBe(true);
    }
  });

  it('ninguno se sale del área segura de 20', () => {
    // Es lo que hace que dos iconos puestos uno al lado del otro pesen lo mismo
    // aunque uno sea un círculo y el otro un rectángulo. Sin esta comprobación,
    // el que se salga se ve más grande y no se sabe por qué.
    for (const [nombre, trazos] of Object.entries(ICONOS)) {
      for (const circulo of trazos.matchAll(/<circle cx="([\d.]+)" cy="([\d.]+)" r="([\d.]+)"/g)) {
        const [cx, cy, r] = circulo.slice(1).map(Number) as [number, number, number];
        expect(cx - r, `${nombre}`).toBeGreaterThanOrEqual(1.9);
        expect(cx + r, `${nombre}`).toBeLessThanOrEqual(22.1);
        expect(cy - r, `${nombre}`).toBeGreaterThanOrEqual(1.9);
        expect(cy + r, `${nombre}`).toBeLessThanOrEqual(22.1);
      }

      for (const rect of trazos.matchAll(
        /<rect x="([\d.]+)" y="([\d.]+)" width="([\d.]+)" height="([\d.]+)"/g
      )) {
        const [x, y, w, h] = rect.slice(1).map(Number) as [number, number, number, number];
        expect(x, `${nombre}`).toBeGreaterThanOrEqual(1.9);
        expect(y, `${nombre}`).toBeGreaterThanOrEqual(1.9);
        expect(x + w, `${nombre}`).toBeLessThanOrEqual(22.1);
        expect(y + h, `${nombre}`).toBeLessThanOrEqual(22.1);
      }

      // Las cifras sueltas de los paths delatan un trazo que toca el borde.
      const cifras = trazos.replace(/<[^>]*>/g, ' ').match(/-?\d+\.?\d*/g) ?? [];
      for (const cifra of cifras) {
        expect(Number(cifra), `${nombre}: coordenada fuera de la rejilla`).toBeLessThanOrEqual(
          22.6
        );
      }
    }
  });

  it('todos hablan el mismo lenguaje visual', () => {
    for (const [nombre, trazos] of Object.entries(ICONOS)) {
      // Ni color propio ni relleno: un icono que los lleva deja de cambiar con
      // el tema y en alto contraste se queda como estaba.
      expect(trazos, `${nombre} lleva color propio`).not.toMatch(
        /#[0-9a-f]{3,8}|rgb|fill="(?!none)/i
      );
      expect(trazos, `${nombre} fija su propio trazo`).not.toContain('stroke-width');
      expect(trazos.length, `${nombre} está vacío`).toBeGreaterThan(10);
    }
  });

  it('tiene los que la interfaz necesita hoy', () => {
    // Los destinos de ADR-013 y las acciones del lector. Si alguno
    // desapareciera, una barra se quedaría con un hueco.
    for (const imprescindible of [
      'inicio',
      'biblioteca',
      'descubrir',
      'lector',
      'estudio',
      'ajustes',
      'menu',
      'buscar',
      'nota',
      'resaltar',
      'marcador',
      'indice',
      'tipografia',
      'inmersion',
      'pantallaCompleta'
    ]) {
      expect(imprescindible in ICONOS, `falta ${imprescindible}`).toBe(true);
    }
  });

  it('la documentación dice cuántos hay', () => {
    expect(CUANTOS_ICONOS).toBe(documentados.size);
  });
});
