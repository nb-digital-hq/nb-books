/**
 * El mapa del libro: el libro entero de un vistazo.
 *
 * Contrato: ADR-004 -cada marca lleva a su sitio por `Locator`, nunca por número- y
 * `docs/design/reading-experience/README.md`, sección 2.2.
 *
 * # Qué enseña
 *
 * Una franja que es el libro de principio a fin, con sus capítulos por tramos, lo
 * que va de lectura, dónde se está y las anotaciones. Es lo que un libro de papel
 * enseña cerrado sobre la mesa -el grosor de lo leído, las tiras que asoman- y lo
 * que un lector digital suele esconder detrás de un porcentaje.
 *
 * # Por qué agrupa
 *
 * Porque lo que se pinta no puede depender de cuánto haya anotado alguien: un libro
 * con mil seiscientos párrafos subrayados serían mil seiscientos botones, y el
 * presupuesto de la carcasa entera es de mil quinientos nodos. Las marcas se reúnen
 * en como mucho `CUBOS` grupos a lo largo de la franja, y los capítulos, en como
 * mucho otros tantos tramos. Con pocas anotaciones cada grupo es una sola, y lleva
 * exactamente a ella; con muchas, lleva a la primera del grupo y dice cuántas hay.
 *
 * Pura a propósito: la aritmética de los tramos tiene casos límite -un índice sin
 * progresión, un capítulo sin anotaciones, todo al final- que se comprueban mejor
 * con una tabla que mirando una pantalla.
 */

import type { AnnotationKind, Locator, NavigationEntry, OutlineEntry } from '$lib/bindings';

/** Cuántos grupos como mucho, de marcas y de capítulos. */
export const CUBOS = 100;

/** Un tramo de la franja: uno o varios capítulos seguidos. */
export interface Tramo {
  /** Dónde empieza y acaba, de 0 a 1. */
  desde: number;
  hasta: number;
  /** El título del primero, y cuántos reúne. */
  titulo: string;
  cuantos: number;
  locator: Locator;
  /** Si la lectura está en él. */
  actual: boolean;
}

/** Las anotaciones que caen en un mismo punto de la franja. */
export interface Grupo {
  /** Dónde, de 0 a 1: el de la primera. */
  posicion: number;
  /** A dónde lleva: la primera del grupo. */
  locator: Locator;
  cuantas: Partial<Record<AnnotationKind, number>>;
  /** La clase que más hay, que es la que se dibuja. */
  clase: AnnotationKind;
}

export interface Mapa {
  tramos: Tramo[];
  grupos: Grupo[];
  /** Cuánto va de lectura, de 0 a 1, o nulo si el formato no lo dice. */
  leido: number | null;
}

const progresion = (locator: Locator): number | null => {
  const valor = locator.locations.totalProgression;
  return typeof valor === 'number' && Number.isFinite(valor)
    ? Math.min(1, Math.max(0, valor))
    : null;
};

/**
 * Los capítulos del primer nivel, con dónde empieza cada uno.
 *
 * Solo el primer nivel: un mapa con las secciones de cada capítulo sería el índice
 * otra vez, y el índice ya está debajo. Si el formato no da progresión, se reparten
 * a partes iguales, que es mentir menos que no pintarlos.
 */
function capitulos(
  entradas: readonly NavigationEntry[]
): { entrada: NavigationEntry; desde: number }[] {
  const conProgresion = entradas.every((e) => progresion(e.locator) !== null);
  return entradas.map((entrada, i) => ({
    entrada,
    desde: conProgresion ? (progresion(entrada.locator) ?? 0) : i / Math.max(1, entradas.length)
  }));
}

/** Los tramos de la franja, reuniendo capítulos seguidos si son más que los cubos. */
function tramosDe(
  entradas: readonly NavigationEntry[],
  actual: Locator | null,
  cubos: number
): Tramo[] {
  const lista = capitulos(entradas);
  const tramos: Tramo[] = [];
  const paso = Math.max(1, Math.ceil(lista.length / cubos));
  for (let i = 0; i < lista.length; i += paso) {
    const primero = lista[i];
    if (!primero) continue;
    const hasta = lista[i + paso]?.desde ?? 1;
    const reunidos = lista.slice(i, i + paso);
    tramos.push({
      desde: primero.desde,
      hasta: Math.max(primero.desde, hasta),
      titulo: primero.entrada.title,
      cuantos: reunidos.length,
      locator: primero.entrada.locator,
      actual: actual !== null && reunidos.some((c) => c.entrada.locator.href === actual.href)
    });
  }
  return tramos;
}

type Acumulado = Omit<Grupo, 'clase'>;

/** Suma una anotación a su cubo, que lleva a la primera en el orden del libro. */
function sumar(grupo: Acumulado, kind: AnnotationKind, posicion: number, target: Locator) {
  grupo.cuantas[kind] = (grupo.cuantas[kind] ?? 0) + 1;
  if (posicion < grupo.posicion) {
    grupo.posicion = posicion;
    grupo.locator = target;
  }
}

/** La clase que más hay en un grupo, que es la que se dibuja. */
function dominante(cuantas: Grupo['cuantas']): AnnotationKind {
  return (Object.entries(cuantas) as [AnnotationKind, number][]).reduce((a, b) =>
    b[1] > a[1] ? b : a
  )[0];
}

/** Las anotaciones reunidas en como mucho `cubos` grupos a lo largo de la franja. */
function gruposDe(anotaciones: readonly OutlineEntry[], cubos: number): Grupo[] {
  const porCubo = new Map<number, Acumulado>();
  for (const { annotation, lost } of anotaciones) {
    // Una anotación que no ancla no se pinta en un sitio que no es el suyo: el
    // panel de anotaciones ya la enseña como perdida.
    const posicion = lost ? null : progresion(annotation.target);
    if (posicion === null) continue;
    const cubo = Math.min(cubos - 1, Math.floor(posicion * cubos));
    const grupo = porCubo.get(cubo);
    if (grupo) sumar(grupo, annotation.kind, posicion, annotation.target);
    else
      porCubo.set(cubo, {
        posicion,
        locator: annotation.target,
        cuantas: { [annotation.kind]: 1 }
      });
  }
  return [...porCubo.entries()]
    .sort(([a], [b]) => a - b)
    .map(([, g]) => ({ ...g, clase: dominante(g.cuantas) }));
}

/** El mapa de un libro, con como mucho `cubos` tramos y `cubos` grupos. */
export function mapaDelLibro(
  entradas: readonly NavigationEntry[],
  anotaciones: readonly OutlineEntry[],
  actual: Locator | null,
  cubos = CUBOS
): Mapa {
  return {
    tramos: tramosDe(entradas, actual, cubos),
    grupos: gruposDe(anotaciones, cubos),
    leido: actual ? progresion(actual) : null
  };
}
