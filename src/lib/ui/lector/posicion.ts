/**
 * Dónde estás dentro del documento, dicho de las tres formas que la gente usa.
 *
 * # Por qué existe
 *
 * Porque el lector **no decía la posición en ninguna parte**. La
 * cabecera daba el nombre del capítulo y nada más: ni cuánto llevas, ni cuánto
 * queda, ni por qué capítulo de cuántos vas. El dato existía —el runtime ancla
 * con Locator y `totalProgression` viaja en cada posición—, pero no
 * se pintaba, así que leer en NB Books era leer sin saber dónde estás.
 *
 * Los cuatro lectores de referencia lo enseñan en una barra inferior, y la queja
 * más repetida contra el visor de Calibre 4 no fue que escondiera los controles:
 * fue que **quitó el número de página** y dejó solo el porcentaje. De ahí sale
 * la regla de este módulo: se dan todas las formas que el formato conozca y no
 * se elige por el usuario. Un EPUB reflowable no tiene páginas y da porcentaje;
 * un PDF tiene página y la da; no se presenta su índice de páginas como capítulos.
 *
 * # Por qué es un módulo aparte y puro
 *
 * Porque es aritmética con casos límite —un índice vacío, un capítulo que no
 * está en el índice, una progresión que el formato no calcula— y esos casos se
 * comprueban mejor con una tabla de entradas que mirando una pantalla.
 */

import { porcentajeLeido } from '$core/progreso';
import type { Locator, NavigationEntry } from '$lib/bindings';

/** La posición, ya lista para pintar. Un `null` significa "el formato no lo sabe". */
export interface Posicion {
  /** Título del capítulo en curso. */
  capitulo: string | null;
  /** Cuál de las entradas del índice, empezando por uno, y cuántas hay. */
  capituloNumero: number | null;
  capitulosTotales: number | null;
  /** Progresión dentro de la obra entera, de 0 a 100 y ya redondeada. */
  porcentaje: number | null;
  /** Página absoluta, cuando el formato la tiene. Empieza en 1, como en Readium. */
  pagina: number | null;
  /** Cuántas páginas tiene, cuando el índice del documento **son** sus páginas. */
  paginasTotales: number | null;
}

/**
 * Aplana el índice, que es un árbol, en el orden en que se lee.
 *
 * Hace falta para contar: "capítulo 3 de 40" no significa nada si solo se
 * cuentan las entradas de primer nivel de un libro con partes y capítulos
 * dentro. Se recorre en profundidad y por orden, que es el orden del documento.
 */
export function indicePlano(entradas: readonly NavigationEntry[]): NavigationEntry[] {
  const salida: NavigationEntry[] = [];
  for (const entrada of entradas) {
    salida.push(entrada);
    salida.push(...indicePlano(entrada.children));
  }
  return salida;
}

/** Nombres inequívocos de capítulos: una página o un recurso compartido no identifica uno. */
export function capitulosPorRecurso(entradas: readonly NavigationEntry[]): Record<string, string> {
  const nombres: Record<string, string> = Object.create(null);
  const ambiguos = new Set<string>();
  for (const entrada of indicePlano(entradas)) {
    if (!entrada.title || typeof entrada.locator.locations.position === 'number') continue;
    const href = entrada.locator.href;
    if (Object.hasOwn(nombres, href)) {
      delete nombres[href];
      ambiguos.add(href);
    } else if (!ambiguos.has(href)) nombres[href] = entrada.title;
  }
  return nombres;
}

/**
 * Dónde está la lectura, según el locator y el índice del documento.
 *
 * El capítulo se busca por `href` y no por progresión: el `href` es lo que
 * identifica el recurso, y dos entradas del índice pueden compartir progresión
 * si el formato no la calcula. Cuando el recurso no está en el índice —ocurre:
 * hay EPUB cuyo índice se salta recursos— el número de capítulo se omite en
 * lugar de inventarse, y el título cae al que trae el propio locator.
 */
export function posicionDe(locator: Locator, entradas: readonly NavigationEntry[]): Posicion {
  const plano = indicePlano(entradas);
  const indice = plano.findIndex((e) => e.locator.href === locator.href);

  const total = locator.locations.totalProgression;
  const porcentaje = porcentajeLeido(total);

  const posicion = locator.locations.position ?? null;
  const pagina = typeof posicion === 'number' && Number.isFinite(posicion) ? posicion : null;

  /*
   * Un documento de páginas -un PDF- trae por índice sus propias páginas, todas
   * con el mismo `href`. Buscando por `href` caía siempre en la primera, y la
   * barra decía "Página 1 · Capítulo 1 de 28 · Página 2": tres datos, dos falsos
   * (segunda auditoría, 2026-11-13). Ahí el índice no son capítulos: se dice la
   * página y cuántas hay, y nada más.
   */
  const sonPaginas =
    pagina !== null &&
    plano.length > 0 &&
    plano.every((e) => typeof e.locator.locations.position === 'number');
  if (sonPaginas) {
    return {
      capitulo: null,
      capituloNumero: null,
      capitulosTotales: null,
      porcentaje,
      pagina,
      paginasTotales: plano.length
    };
  }

  return {
    capitulo: plano[indice]?.locator.title ?? locator.title ?? null,
    capituloNumero: indice >= 0 ? indice + 1 : null,
    capitulosTotales: plano.length > 0 ? plano.length : null,
    porcentaje,
    pagina,
    paginasTotales: null
  };
}

/**
 * Si hay algo que enseñar en la barra de posición.
 *
 * Un documento del que no se sabe ni el capítulo ni el porcentaje ni la página
 * no necesita una barra vacía debajo: se omite, que es la misma regla que
 * gobierna los controles del lector.
 */
export function hayPosicion(posicion: Posicion): boolean {
  return (
    posicion.capitulo !== null ||
    posicion.porcentaje !== null ||
    posicion.pagina !== null ||
    posicion.capituloNumero !== null
  );
}

/** Textos traducidos que necesita la locución de la posición. */
export interface EtiquetasDePosicion {
  capitulo: string;
  de: string;
  pagina: string;
}

/**
 * Frase para lectores de pantalla que anuncia la posición actual.
 *
 * Anuncia el capítulo, el encabezado más cercano y la posición (página o porcentaje),
 * sin redundancias y omitiendo las partes desconocidas.
 */
export function anuncioDePosicion(
  posicion: Posicion,
  encabezado: string | null,
  etiquetas: EtiquetasDePosicion
): string {
  const partes: string[] = [];

  // 1. Capítulo
  const partesCapitulo: string[] = [];
  if (posicion.capitulo) {
    partesCapitulo.push(posicion.capitulo);
  }
  if (posicion.capituloNumero !== null && posicion.capitulosTotales !== null) {
    partesCapitulo.push(
      `${etiquetas.capitulo} ${posicion.capituloNumero} ${etiquetas.de} ${posicion.capitulosTotales}`
    );
  }
  if (partesCapitulo.length > 0) {
    partes.push(partesCapitulo.join('. '));
  }

  // 2. Encabezado más cercano (si no es idéntico al capítulo ya anunciado)
  if (encabezado && encabezado !== posicion.capitulo) {
    partes.push(encabezado);
  }

  // 3. Posición (página y/o porcentaje)
  const partesPosicion: string[] = [];
  if (posicion.pagina !== null) {
    partesPosicion.push(`${etiquetas.pagina} ${posicion.pagina}`);
  }
  if (posicion.porcentaje !== null) {
    partesPosicion.push(`${posicion.porcentaje} %`);
  }
  if (partesPosicion.length > 0) {
    partes.push(partesPosicion.join(', '));
  }

  return partes.join('. ');
}
