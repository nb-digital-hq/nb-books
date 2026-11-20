import { describe, expect, it } from 'vitest';

import type { AnnotationKind, Locator, NavigationEntry, OutlineEntry } from '$lib/bindings';

import { CUBOS, mapaDelLibro } from './mapa';

const locator = (href: string, total: number | null): Locator => ({
  version: 1,
  href,
  mediaType: 'application/xhtml+xml',
  title: null,
  locations: { fragments: ['#char=0'], progression: 0, totalProgression: total ?? undefined },
  text: { highlight: 'algo' }
});

const capitulo = (n: number, total: number | null): NavigationEntry => ({
  title: `Capítulo ${n}`,
  locator: locator(`c${n}.xhtml`, total),
  children: []
});

const anotacion = (
  id: string,
  kind: AnnotationKind,
  total: number,
  lost = false
): OutlineEntry => ({
  annotation: {
    id,
    editionId: 'e0',
    kind,
    body: null,
    target: locator('c1.xhtml', total),
    createdAt: '2026-09-23T00:00:00Z',
    supersedes: null,
    deletedAt: null
  } as OutlineEntry['annotation'],
  position: null,
  precision: 'exact',
  relocated: false,
  warn: false,
  lost
});

describe('el mapa del libro', () => {
  it('reparte la franja por capítulos con su progresión, y el último llega al final', () => {
    const mapa = mapaDelLibro([capitulo(1, 0), capitulo(2, 0.25), capitulo(3, 0.7)], [], null);
    expect(mapa.tramos.map((t) => [t.desde, t.hasta])).toEqual([
      [0, 0.25],
      [0.25, 0.7],
      [0.7, 1]
    ]);
  });

  it('sin progresión en el índice, reparte a partes iguales', () => {
    const mapa = mapaDelLibro([capitulo(1, null), capitulo(2, null)], [], null);
    expect(mapa.tramos.map((t) => t.desde)).toEqual([0, 0.5]);
  });

  it('marca el capítulo donde se está y cuánto va de lectura', () => {
    const mapa = mapaDelLibro([capitulo(1, 0), capitulo(2, 0.5)], [], locator('c2.xhtml', 0.62));
    expect(mapa.tramos.map((t) => t.actual)).toEqual([false, true]);
    expect(mapa.leido).toBe(0.62);
  });

  it('cada anotación suelta lleva exactamente a su sitio', () => {
    const mapa = mapaDelLibro(
      [capitulo(1, 0)],
      [anotacion('a', 'note', 0.1), anotacion('b', 'highlight', 0.8)],
      null
    );
    expect(
      mapa.grupos.map((g) => [g.posicion, g.clase, g.locator.locations.totalProgression])
    ).toEqual([
      [0.1, 'note', 0.1],
      [0.8, 'highlight', 0.8]
    ]);
  });

  it('las que caen juntas se reúnen, llevan a la primera del libro y dicen cuántas son', () => {
    const mapa = mapaDelLibro(
      [capitulo(1, 0)],
      [
        anotacion('b', 'highlight', 0.505),
        anotacion('a', 'highlight', 0.501),
        anotacion('c', 'note', 0.509)
      ],
      null
    );
    expect(mapa.grupos).toHaveLength(1);
    expect(mapa.grupos[0]?.posicion).toBe(0.501);
    expect(mapa.grupos[0]?.cuantas).toEqual({ highlight: 2, note: 1 });
    expect(mapa.grupos[0]?.clase).toBe('highlight');
  });

  it('una anotación perdida no se pinta en un sitio que no es el suyo', () => {
    const mapa = mapaDelLibro([capitulo(1, 0)], [anotacion('a', 'note', 0.3, true)], null);
    expect(mapa.grupos).toHaveLength(0);
  });

  it('lo que se pinta no crece con lo anotado ni con los capítulos', () => {
    const muchas = Array.from({ length: 1600 }, (_, i) =>
      anotacion(`a${i}`, 'highlight', i / 1600)
    );
    const muchos = Array.from({ length: 900 }, (_, i) => capitulo(i, i / 900));
    const mapa = mapaDelLibro(muchos, muchas, null);
    expect(mapa.grupos.length).toBeLessThanOrEqual(CUBOS);
    expect(mapa.tramos.length).toBeLessThanOrEqual(CUBOS);
    // Y no se pierde ninguna: se reúnen, no se tiran.
    const total = mapa.grupos.reduce((s, g) => s + (g.cuantas.highlight ?? 0), 0);
    expect(total).toBe(1600);
    expect(mapa.tramos.reduce((s, t) => s + t.cuantos, 0)).toBe(900);
  });
});
