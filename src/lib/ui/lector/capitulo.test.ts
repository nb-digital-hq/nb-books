import { describe, expect, it } from 'vitest';

import type { Locator, NavigationEntry } from '$lib/bindings';

import { capituloDe, PAGINAS_SIN_INDICE } from './capitulo';

function locator(href: string, position: number | null = null): Locator {
  return {
    version: 1,
    href,
    mediaType: 'application/xhtml+xml',
    locations: { fragments: [], progression: 0, totalProgression: null, position }
  } as unknown as Locator;
}

function entrada(title: string, loc: Locator, children: NavigationEntry[] = []): NavigationEntry {
  return { title, locator: loc, children };
}

describe('el capítulo de las funciones de estudio', () => {
  it('en un EPUB es el recurso en el que se lee, sin la parte de ancla', () => {
    const indice = [
      entrada('Uno', locator('c1.xhtml')),
      entrada('Dos', locator('c2.xhtml#inicio'), [entrada('Dos punto uno', locator('c2.xhtml#s1'))])
    ];
    const c = capituloDe(indice, locator('c2.xhtml#s1'));
    expect(c.hrefs).toEqual(['c2.xhtml']);
    expect(c.titulo).toBe('Dos');
    expect(c.desde).toBeNull();
  });

  it('en un PDF son las páginas hasta la siguiente entrada del índice, no el libro entero', () => {
    const indice = [
      entrada('Introducción', locator('documento', 1)),
      entrada('Capítulo 1', locator('documento', 12), [entrada('1.1', locator('documento', 15))]),
      entrada('Capítulo 2', locator('documento', 40))
    ];
    const c = capituloDe(indice, locator('documento', 13));
    expect(c).toEqual({ hrefs: ['documento'], titulo: 'Capítulo 1', desde: 12, hasta: 15 });
    expect(capituloDe(indice, locator('documento', 41)).hasta).toBeNull();
  });

  it('en un PDF sin índice es un tramo acotado desde la página actual', () => {
    const c = capituloDe([], locator('documento', 7));
    expect(c.desde).toBe(7);
    expect(c.hasta).toBe(7 + PAGINAS_SIN_INDICE);
  });
});
