/**
 * La barra de posición dice dónde estás, y los casos que importan son los que no
 * se ven mirando un EPUB bien formado.
 */

import { describe, expect, it } from 'vitest';

import type { Locator, NavigationEntry } from '$lib/bindings';

import {
  anuncioDePosicion,
  capitulosPorRecurso,
  hayPosicion,
  indicePlano,
  posicionDe
} from './posicion';

const locator = (href: string, extra: Partial<Locator['locations']> = {}): Locator => ({
  version: 1,
  href,
  mediaType: 'application/xhtml+xml',
  title: null,
  locations: { fragments: [`#${href}`], ...extra },
  text: null
});

const entrada = (
  title: string,
  href: string,
  children: NavigationEntry[] = []
): NavigationEntry => ({
  title,
  locator: { ...locator(href), title },
  children
});

describe('dónde está la lectura', () => {
  it('cuenta también los capítulos anidados, no solo las partes', () => {
    // Un libro con partes y capítulos dentro: "capítulo 2 de 2" sería mentira.
    const indice = [
      entrada('Primera parte', 'p1.xhtml', [
        entrada('Uno', 'c1.xhtml'),
        entrada('Dos', 'c2.xhtml')
      ]),
      entrada('Segunda parte', 'p2.xhtml', [entrada('Tres', 'c3.xhtml')])
    ];

    expect(indicePlano(indice)).toHaveLength(5);

    const posicion = posicionDe(locator('c2.xhtml'), indice);
    expect(posicion.capituloNumero).toBe(3);
    expect(posicion.capitulosTotales).toBe(5);
  });

  it('el orden es el del documento, no el del árbol por niveles', () => {
    const indice = [
      entrada('Primera parte', 'p1.xhtml', [entrada('Uno', 'c1.xhtml')]),
      entrada('Segunda parte', 'p2.xhtml')
    ];

    expect(indicePlano(indice).map((e) => e.locator.href)).toEqual([
      'p1.xhtml',
      'c1.xhtml',
      'p2.xhtml'
    ]);
  });

  it('un recurso que el índice no lista no inventa número de capítulo', () => {
    // Ocurre de verdad: hay EPUB cuyo índice se salta recursos.
    const posicion = posicionDe(locator('suelto.xhtml'), [entrada('Uno', 'c1.xhtml')]);

    expect(posicion.capituloNumero).toBeNull();
    expect(posicion.capitulosTotales).toBe(1);
  });

  it('sin título en el índice, se dice el que trae el propio locator', () => {
    const suelto = { ...locator('suelto.xhtml'), title: 'Epílogo' };
    expect(posicionDe(suelto, []).capitulo).toBe('Epílogo');
  });

  it('un formato que no calcula la progresión no enseña un porcentaje falso', () => {
    expect(posicionDe(locator('c1.xhtml'), []).porcentaje).toBeNull();
  });

  it('la progresión se trunca y se acota al rango que se puede pintar', () => {
    // Truncar y no redondear, con la misma regla que el resto de la aplicación:
    // redondeando, el lector decía 43 % y la barra lateral 42 %.
    expect(posicionDe(locator('c1.xhtml', { totalProgression: 0.426 }), []).porcentaje).toBe(42);
    // Un valor fuera de rango es un defecto del proveedor, y una barra al 140 %
    // se sale de su caja: se acota aquí, que es donde se pinta.
    expect(posicionDe(locator('c1.xhtml', { totalProgression: 1.4 }), []).porcentaje).toBe(100);
    expect(posicionDe(locator('c1.xhtml', { totalProgression: -0.2 }), []).porcentaje).toBe(0);
  });

  it('la página se enseña donde el formato la tiene, y solo ahí', () => {
    expect(posicionDe(locator('p12', { position: 12 }), []).pagina).toBe(12);
    expect(posicionDe(locator('c1.xhtml'), []).pagina).toBeNull();
  });

  it('un documento del que no se sabe nada no pinta una barra vacía', () => {
    const sinNada = posicionDe(locator('x'), []);

    expect(sinNada.capitulo).toBeNull();
    expect(sinNada.capituloNumero).toBeNull();
    expect(sinNada.capitulosTotales).toBeNull();
    expect(hayPosicion(sinNada)).toBe(false);
  });

  it('con solo el capítulo ya hay algo que decir', () => {
    expect(hayPosicion(posicionDe(locator('c1.xhtml'), [entrada('Uno', 'c1.xhtml')]))).toBe(true);
  });
});

describe('anuncio accesible de dónde estoy', () => {
  const etiquetas = {
    capitulo: 'Capítulo',
    de: 'de',
    pagina: 'Página'
  };

  it('anuncia capítulo, número de capítulos y posición completa', () => {
    const posicion = {
      capitulo: 'El coronel Aureliano Buendía',
      capituloNumero: 1,
      capitulosTotales: 20,
      pagina: 5,
      paginasTotales: null,
      porcentaje: 25
    };

    const texto = anuncioDePosicion(posicion, 'Macondo al amanecer', etiquetas);
    expect(texto).toBe(
      'El coronel Aureliano Buendía. Capítulo 1 de 20. Macondo al amanecer. Página 5, 25 %'
    );
  });

  it('omite el encabezado cuando es idéntico al capítulo para no repetir', () => {
    const posicion = {
      capitulo: 'Capítulo 1',
      capituloNumero: 1,
      capitulosTotales: 10,
      pagina: null,
      paginasTotales: null,
      porcentaje: 10
    };

    const texto = anuncioDePosicion(posicion, 'Capítulo 1', etiquetas);
    expect(texto).toBe('Capítulo 1. Capítulo 1 de 10. 10 %');
  });

  it('anuncia sólo página cuando no hay porcentaje ni capítulo', () => {
    const posicion = {
      capitulo: null,
      capituloNumero: null,
      capitulosTotales: null,
      pagina: 42,
      paginasTotales: null,
      porcentaje: null
    };

    const texto = anuncioDePosicion(posicion, 'Introducción al Álgebra', etiquetas);
    expect(texto).toBe('Introducción al Álgebra. Página 42');
  });

  it('anuncia sólo porcentaje cuando no hay página ni encabezado', () => {
    const posicion = {
      capitulo: 'Prólogo',
      capituloNumero: null,
      capitulosTotales: null,
      pagina: null,
      paginasTotales: null,
      porcentaje: 50
    };

    const texto = anuncioDePosicion(posicion, null, etiquetas);
    expect(texto).toBe('Prólogo. 50 %');
  });

  it('devuelve cadena vacía cuando no hay ningún dato', () => {
    const posicion = {
      capitulo: null,
      capituloNumero: null,
      capitulosTotales: null,
      pagina: null,
      paginasTotales: null,
      porcentaje: null
    };

    const texto = anuncioDePosicion(posicion, null, etiquetas);
    expect(texto).toBe('');
  });
});

describe('un documento de páginas', () => {
  it('dice la página y cuántas hay, y no la confunde con un capítulo', () => {
    // El índice de un PDF son sus páginas, todas con el mismo recurso. Buscando
    // por recurso caía siempre en la primera: "Página 1 · Capítulo 1 de 3".
    const pagina = (n: number): NavigationEntry => ({
      title: `Página ${n}`,
      locator: { ...locator('/', { position: n }), title: `Página ${n}` },
      children: []
    });
    const posicion = posicionDe(locator('/', { position: 2, totalProgression: 0.34 }), [
      pagina(1),
      pagina(2),
      pagina(3)
    ]);
    expect(posicion).toEqual({
      capitulo: null,
      capituloNumero: null,
      capitulosTotales: null,
      porcentaje: 34,
      pagina: 2,
      paginasTotales: 3
    });
  });
});

describe('nombres de resultados del documento abierto', () => {
  it('conserva el capítulo inequívoco y omite recursos con varios títulos', () => {
    const indice = [
      entrada('Capítulo único', '/uno'),
      entrada('Parte', '/dos'),
      entrada('Otro capítulo', '/dos')
    ];
    expect(capitulosPorRecurso(indice)).toEqual({ '/uno': 'Capítulo único' });
  });
  it('los recursos que coinciden con propiedades de objeto siguen siendo nombres del documento', () => {
    const nombres = capitulosPorRecurso([entrada('Prólogo', '__proto__')]);
    expect(Object.getOwnPropertyDescriptor(nombres, '__proto__')?.value).toBe('Prólogo');
    expect(nombres.constructor).toBeUndefined();
  });
  it('no presenta todas las coincidencias de un PDF como si fueran de su primera página', () => {
    const paginas = [1, 2].map((n) => ({
      ...entrada(`Página ${n}`, '/'),
      locator: locator('/', { position: n })
    }));
    expect(capitulosPorRecurso(paginas)).toEqual({});
  });
});
