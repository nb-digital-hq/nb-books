import { describe, expect, it } from 'vitest';

import type { Hit, Locator, SemanticHit } from '$lib/bindings';

import { fusionar, trozosDelPasaje } from './fusion';

function locator(href: string): Locator {
  return {
    version: 1,
    href,
    mediaType: 'application/xhtml+xml',
    title: null,
    locations: { fragments: [`#${href}`], progression: 0 },
    text: null
  } as unknown as Locator;
}

const lexico = (href: string): Hit => ({
  editionId: 'e',
  fileId: 'f',
  locator: locator(href),
  snippet: `‹${href}›`,
  rank: -1,
  title: 'Una obra'
});

const semantico = (href: string, similarity = 0.8): SemanticHit => ({
  editionId: 'e',
  fileId: 'f',
  locator: locator(href),
  snippet: `pasaje de ${href}`,
  similarity
});

describe('fusionar las dos búsquedas', () => {
  it('sin resultados por significado, la lista léxica queda igual y en su orden', () => {
    const hits = [lexico('a'), lexico('b'), lexico('c')];
    const fundidos = fusionar(hits, []);
    expect(fundidos.map((c) => c.locator.href)).toEqual(['a', 'b', 'c']);
    expect(fundidos.every((c) => c.origen === 'exacta')).toBe(true);
    expect(fundidos.map((c) => c.snippet)).toEqual(hits.map((h) => h.snippet));
  });

  it('lo que sale en las dos sube, se marca como de las dos y conserva el pasaje léxico', () => {
    const fundidos = fusionar([lexico('a'), lexico('b')], [semantico('b'), semantico('z')]);
    const primero = fundidos[0];
    expect(primero?.locator.href).toBe('b');
    expect(primero?.origen).toBe('ambas');
    expect(primero?.snippet).toBe('‹b›');
    expect(fundidos.find((c) => c.locator.href === 'z')?.origen).toBe('significado');
    expect(fundidos.find((c) => c.locator.href === 'a')?.origen).toBe('exacta');
  });

  it('no repite un fragmento aunque aparezca en las dos listas', () => {
    const fundidos = fusionar([lexico('a')], [semantico('a')]);
    expect(fundidos).toHaveLength(1);
  });

  it('a igual posición en su lista, lo léxico va delante de lo que solo es por significado', () => {
    const fundidos = fusionar([lexico('a')], [semantico('z')]);
    expect(fundidos.map((c) => c.origen)).toEqual(['exacta', 'significado']);
  });
});

describe('el pasaje de un resultado', () => {
  it('las marcas del runtime se convierten en trozos marcados y no se enseñan', () => {
    expect(trozosDelPasaje('…‹molinos› de ‹viento› y…')).toEqual([
      { texto: '…', marcado: false },
      { texto: 'molinos', marcado: true },
      { texto: ' de ', marcado: false },
      { texto: 'viento', marcado: true },
      { texto: ' y…', marcado: false }
    ]);
  });

  it('el mismo pasaje en otra edición no es otro resultado: se cuenta', () => {
    const a = { ...lexico('a'), snippet: '‹Molinos› de viento', editionId: 'e1' };
    const b = { ...lexico('b'), snippet: 'molinos de  viento', editionId: 'e2' };
    const c = { ...lexico('c'), snippet: 'otra cosa', editionId: 'e3' };
    const fundidos = fusionar([a, b, c], []);
    expect(fundidos.map((x) => x.editionId)).toEqual(['e1', 'e3']);
    expect(fundidos[0]?.otras).toBe(1);
    expect(fundidos[0]?.titulo).toBe('Una obra');
  });
});
