import { describe, expect, it } from 'vitest';

import type { AudioMark, Locator } from '$lib/bindings';

import { marcaDesde, marcaEn } from './preparado';

function locator(href: string, progression: number, position?: number): Locator {
  return {
    href,
    type: 'application/xhtml+xml',
    locations: { progression, ...(position === undefined ? {} : { position }) }
  } as unknown as Locator;
}

function marcas(): AudioMark[] {
  return [
    { startMs: 0, endMs: 4000, locator: locator('c1.xhtml', 0) },
    { startMs: 4000, endMs: 9000, locator: locator('c1.xhtml', 0.5) },
    { startMs: 9000, endMs: 15000, locator: locator('c2.xhtml', 0) },
    { startMs: 15000, endMs: 20000, locator: locator('c2.xhtml', 0.6) }
  ];
}

describe('seguir el texto mientras suena lo preparado', () => {
  it('dice qué fragmento suena en cada momento, también en los bordes', () => {
    const m = marcas();
    expect(marcaEn(m, 0)).toBe(0);
    expect(marcaEn(m, 3999)).toBe(0);
    expect(marcaEn(m, 4000)).toBe(1);
    expect(marcaEn(m, 14999)).toBe(2);
    expect(marcaEn(m, 19999)).toBe(3);
    expect(marcaEn(m, 20000)).toBe(-1);
    expect(marcaEn([], 10)).toBe(-1);
  });

  it('empieza por el fragmento que contiene donde se va leyendo', () => {
    const m = marcas();
    expect(marcaDesde(m, locator('c1.xhtml', 0))).toBe(0);
    // A mitad del segundo fragmento del capítulo dos: ese, no el siguiente.
    expect(marcaDesde(m, locator('c2.xhtml', 0.7))).toBe(3);
    expect(marcaDesde(m, locator('c2.xhtml', 0.3))).toBe(2);
    expect(marcaDesde(m, locator('c1.xhtml', 0.5))).toBe(1);
  });

  it('si lo que se lee no está en lo preparado, empieza por el principio', () => {
    expect(marcaDesde(marcas(), locator('c9.xhtml', 0.2))).toBe(0);
  });

  it('en un PDF manda la página', () => {
    const m: AudioMark[] = [
      { startMs: 0, endMs: 1000, locator: locator('doc.pdf', 0, 3) },
      { startMs: 1000, endMs: 2000, locator: locator('doc.pdf', 0.1, 5) }
    ];
    expect(marcaDesde(m, locator('doc.pdf', 0.1, 4))).toBe(1);
  });
});
