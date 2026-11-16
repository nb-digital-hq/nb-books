/**
 * El capítulo en el que se está leyendo, como alcance de las funciones de estudio
 *.
 *
 * Lo calcula la interfaz y no el runtime porque es quien tiene el índice y la
 * posición, y porque la respuesta depende del formato de una forma que el
 * contrato ya separa:
 *
 * - **En un EPUB**, un capítulo es su recurso: el `href` de la posición. Un
 *   capítulo repartido en dos recursos se estudia por partes, que es lo mismo que
 *   ve quien lo lee.
 * - **En un PDF**, todo el documento es un solo recurso, así que el capítulo son
 *   páginas: desde la entrada del índice en la que se está hasta la siguiente. Sin
 *   índice, un tramo de páginas desde la actual, para no mandar el libro entero.
 */

import type { Locator, NavigationEntry } from '$lib/bindings';

/** Páginas que se toman de un PDF sin índice. */
export const PAGINAS_SIN_INDICE = 20;

export interface Capitulo {
  hrefs: string[];
  titulo: string;
  desde: number | null;
  hasta: number | null;
}

function aplanar(entradas: readonly NavigationEntry[]): NavigationEntry[] {
  return entradas.flatMap((e) => [e, ...aplanar(e.children)]);
}

function sinAncla(href: string): string {
  return href.split('#')[0] ?? href;
}

/** El capítulo de una posición. */
export function capituloDe(entradas: readonly NavigationEntry[], actual: Locator): Capitulo {
  const planas = aplanar(entradas);
  const pagina = actual.locations.position ?? null;

  if (pagina === null) {
    const propia = planas.filter((e) => sinAncla(e.locator.href) === sinAncla(actual.href));
    return {
      hrefs: [sinAncla(actual.href)],
      titulo: propia[0]?.title ?? '',
      desde: null,
      hasta: null
    };
  }

  const conPagina = planas
    .filter((e) => typeof e.locator.locations.position === 'number')
    .map((e) => ({ titulo: e.title, pagina: e.locator.locations.position as number }))
    .sort((a, b) => a.pagina - b.pagina);
  const anteriores = conPagina.filter((e) => e.pagina <= pagina);
  const actualEntrada = anteriores[anteriores.length - 1];
  if (!actualEntrada) {
    return {
      hrefs: [actual.href],
      titulo: '',
      desde: pagina,
      hasta: pagina + PAGINAS_SIN_INDICE
    };
  }
  const siguiente = conPagina.find((e) => e.pagina > actualEntrada.pagina);
  return {
    hrefs: [actual.href],
    titulo: actualEntrada.titulo,
    desde: actualEntrada.pagina,
    hasta: siguiente?.pagina ?? null
  };
}
