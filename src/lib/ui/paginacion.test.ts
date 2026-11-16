/**
 * El presupuesto de nodos y la aritmética de las páginas.
 *
 * La propiedad que hay que demostrar cambió de forma, y a mejor:
 * antes había que probar que **para todo par (total, desplazamiento)** la
 * ventana estaba acotada, porque el número de elementos pintados salía de una
 * cuenta. Ahora sale de una lista cerrada, así que basta con recorrerla. Una
 * garantía que se comprueba enumerando es más fuerte que una que se comprueba
 * razonando.
 */

import { describe, expect, it } from 'vitest';

import {
  acotarPagina,
  NODOS_POR_ELEMENTO,
  nodosDe,
  numerosVisibles,
  PRESUPUESTO_DE_NODOS,
  paginaEscrita,
  paginas,
  peticionDe,
  TAMANO_POR_DEFECTO,
  TAMANOS_DE_PAGINA
} from './paginacion';

/** El corpus real de pruebas. No es un número redondo a propósito. */
const CATALOGO_REAL = 152_080;

describe('el tamaño de página', () => {
  it('nunca deja pintar más nodos de los que el presupuesto admite', () => {
    for (const tamano of TAMANOS_DE_PAGINA) {
      expect(nodosDe(tamano), `${tamano} por página`).toBeLessThanOrEqual(PRESUPUESTO_DE_NODOS);
    }
  });

  it('ofrece el que se usa de partida', () => {
    expect(TAMANOS_DE_PAGINA).toContain(TAMANO_POR_DEFECTO);
  });

  it('va de menor a mayor, que es como se lee un desplegable', () => {
    const ordenados = [...TAMANOS_DE_PAGINA].sort((a, b) => a - b);
    expect(TAMANOS_DE_PAGINA).toEqual(ordenados);
  });
});

describe('cuántas páginas hay', () => {
  it('reparte el catálogo entero sin perder ni repetir ninguna obra', () => {
    for (const tamano of TAMANOS_DE_PAGINA) {
      const cuantas = paginas(CATALOGO_REAL, tamano);
      const ultima = peticionDe(cuantas, tamano);
      // La última página empieza antes del final y lo alcanza o lo pasa: si
      // empezara después, habría obras que ninguna página enseña.
      expect(ultima.offset).toBeLessThan(CATALOGO_REAL);
      expect(ultima.offset + ultima.limit).toBeGreaterThanOrEqual(CATALOGO_REAL);
      // Y con una menos no llegaría: no sobra ninguna página vacía al final.
      expect(peticionDe(cuantas - 1, tamano).offset + tamano).toBeLessThan(CATALOGO_REAL);
    }
  });

  it('una biblioteca vacía sigue teniendo una página, no cero', () => {
    // "1 de 0" se lee como un error de la aplicación, y no lo es: es que no hay
    // nada, que es un estado normal de una biblioteca recién instalada.
    expect(paginas(0, 25)).toBe(1);
  });

  it('un tamaño absurdo no rompe la cuenta', () => {
    expect(paginas(100, 0)).toBe(100);
    expect(paginas(-5, 25)).toBe(1);
  });
});

describe('acotar la página', () => {
  it('devuelve al rango que existe cuando el filtro encoge la biblioteca', () => {
    // Es el caso real: estás en la página 400 de "Todos" y pulsas "Empezados",
    // que tiene doce obras. Sin acotar, se pediría la 400 de doce.
    expect(acotarPagina(400, 12, 25)).toBe(1);
  });

  it('nunca baja de uno ni sube de la última', () => {
    expect(acotarPagina(-3, 1000, 25)).toBe(1);
    expect(acotarPagina(1e9, 1000, 25)).toBe(40);
  });

  it('un número que no es un número vuelve a la primera', () => {
    expect(acotarPagina(Number.NaN, 1000, 25)).toBe(1);
  });
});

describe('qué trozo del orden global es una página', () => {
  it('la primera empieza en cero', () => {
    expect(peticionDe(1, 25)).toEqual({ offset: 0, limit: 25 });
  });

  it('cada página empieza donde acaba la anterior', () => {
    for (let numero = 1; numero < 50; numero += 1) {
      const actual = peticionDe(numero, 25);
      const siguiente = peticionDe(numero + 1, 25);
      expect(siguiente.offset).toBe(actual.offset + actual.limit);
    }
  });

  it('nunca pide más de un tamaño de página', () => {
    for (const tamano of TAMANOS_DE_PAGINA) {
      expect(peticionDe(3000, tamano).limit).toBe(tamano);
    }
  });
});

describe('los números que se pintan', () => {
  it('con tres mil páginas no se pintan tres mil botones', () => {
    const visibles = numerosVisibles(1500, 3042);
    expect(visibles.length).toBeLessThanOrEqual(9);
  });

  it('siempre lleva a la primera y a la última', () => {
    const visibles = numerosVisibles(1500, 3042);
    expect(visibles[0]).toBe(1);
    expect(visibles.at(-1)).toBe(3042);
  });

  it('incluye la actual y sus vecinas', () => {
    expect(numerosVisibles(1500, 3042)).toEqual([1, null, 1499, 1500, 1501, null, 3042]);
  });

  it('no pone una elipsis donde solo falta una página', () => {
    // Entre la 1 y la 3 falta la 2, y la elipsis ocuparía lo mismo que el botón.
    // Enseñar un hueco en lugar del destino que cabe es cobrar sin dar nada.
    expect(numerosVisibles(3, 5)).toEqual([1, 2, 3, 4, 5]);
  });

  it('con pocas páginas las pinta todas y sin huecos', () => {
    expect(numerosVisibles(1, 3)).toEqual([1, 2, 3]);
    expect(numerosVisibles(1, 1)).toEqual([1]);
  });

  it('nunca repite un número', () => {
    for (const [actual, total] of [
      [1, 1],
      [1, 2],
      [2, 3],
      [1, 3042],
      [3042, 3042]
    ] as const) {
      const numeros = numerosVisibles(actual, total).filter((n): n is number => n !== null);
      expect(new Set(numeros).size, `${actual} de ${total}`).toBe(numeros.length);
    }
  });
});

describe('el reparto de nodos por elemento', () => {
  it('es el que el componente declara en su cabecera', () => {
    // Si alguien añade un elemento a la tarjeta y no toca esta constante, el
    // presupuesto se calcula sobre una cuenta que ya no es cierta.
    expect(NODOS_POR_ELEMENTO).toBe(9);
  });
});

describe('los números de página cuando el pie tiene sitio', () => {
  it('llena el sitio desde la primera página, como se pidió', () => {
    expect(numerosVisibles(1, 1521, 11)).toEqual([
      1,
      2,
      3,
      4,
      5,
      null,
      1517,
      1518,
      1519,
      1520,
      1521
    ]);
  });

  it('en mitad de la biblioteca da contexto a la actual y a los dos extremos', () => {
    expect(numerosVisibles(700, 1521, 11)).toEqual([
      1,
      2,
      null,
      698,
      699,
      700,
      701,
      702,
      null,
      1520,
      1521
    ]);
  });

  it('nunca pinta más casillas de las que caben', () => {
    for (const casillas of [5, 7, 9, 11, 15, 21]) {
      for (const actual of [1, 2, 6, 700, 1516, 1520, 1521]) {
        const visibles = numerosVisibles(actual, 1521, casillas);
        expect(visibles.length, `${actual} con ${casillas}`).toBeLessThanOrEqual(casillas);
        expect(visibles).toContain(actual);
        expect(visibles[0]).toBe(1);
        expect(visibles.at(-1)).toBe(1521);
      }
    }
  });
});

describe('el campo de ir a una página', () => {
  it('solo deja cifras', () => {
    expect(paginaEscrita('1a2-3', 1521)).toBe('123');
    expect(paginaEscrita('abc', 1521)).toBe('');
    expect(paginaEscrita(' 7 ', 1521)).toBe('7');
  });

  it('nunca pasa de la última ni baja de la primera', () => {
    expect(paginaEscrita('1522', 1521)).toBe('1521');
    expect(paginaEscrita('99999999999999999999999', 1521)).toBe('1521');
    expect(paginaEscrita('0', 1521)).toBe('');
    expect(paginaEscrita('007', 1521)).toBe('7');
  });
});
