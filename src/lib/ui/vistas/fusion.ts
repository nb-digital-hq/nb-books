/**
 * Fundir los resultados de la búsqueda léxica y de la búsqueda por significado en
 * una sola lista, sin perder de dónde salió cada uno.
 *
 * # Cómo se funde
 *
 * Por **fusión de rangos recíprocos**: cada resultado suma `1 / (K + posición)` por
 * cada lista en la que aparece. Es lo que se usa cuando las dos puntuaciones no se
 * pueden comparar —el rango de FTS5 y un coseno no están en la misma escala— y
 * solo se puede confiar en el orden de cada una. Lo que sale en las dos sube.
 *
 * # Por qué se funde aquí y no en el runtime
 *
 * Porque el criterio es que, con la IA caída, la búsqueda léxica no
 * se degrade, y la manera de asegurarlo es que **no cambie**: la léxica llega por
 * su comando de siempre y se pinta en cuanto llega, y esta función solo se aplica
 * cuando la de significado contesta. Si no contesta, no se llama.
 */

import type { Hit, Locator, SemanticHit } from '$lib/bindings';

/** Dónde se encontró un resultado. Es lo que la interfaz distingue a la vista. */
export type Origen = 'exacta' | 'significado' | 'ambas';

export interface Coincidencia {
  origen: Origen;
  editionId: string;
  fileId: string | null;
  locator: Locator;
  /** El pasaje: el léxico, con la coincidencia marcada, si lo hay. */
  snippet: string;
  /** De qué obra es. Vacío para lo que solo vino por significado. */
  titulo: string;
  /**
   * Cuántas ediciones más traen el mismo pasaje. Con varias traducciones o
   * reediciones del mismo libro, el mismo párrafo salía una vez por edición y
   * llenaba la lista de copias.
   */
  otras: number;
}

/** Las marcas del runtime alrededor de la coincidencia (`search::MARCA_INICIO`). */
const MARCA_INICIO = '\u2039';
const MARCA_FIN = '\u203a';

/**
 * El pasaje partido en trozos, marcados o no, para pintarlo **como texto**.
 *
 * El runtime envuelve la coincidencia en ‹ › y no en `<mark>` para que el
 * pasaje nunca se interprete como marcado: viene de un libro que escribió un
 * desconocido. Antes la interfaz enseñaba las marcas tal cual
 *; esto las convierte en trozos que el componente pinta con `<mark>`.
 */
export function trozosDelPasaje(pasaje: string): Array<{ texto: string; marcado: boolean }> {
  const trozos: Array<{ texto: string; marcado: boolean }> = [];
  let resto = pasaje;
  while (resto.length > 0) {
    const inicio = resto.indexOf(MARCA_INICIO);
    if (inicio < 0) {
      trozos.push({ texto: resto, marcado: false });
      break;
    }
    if (inicio > 0) trozos.push({ texto: resto.slice(0, inicio), marcado: false });
    const fin = resto.indexOf(MARCA_FIN, inicio + 1);
    if (fin < 0) {
      trozos.push({ texto: resto.slice(inicio + 1), marcado: false });
      break;
    }
    trozos.push({ texto: resto.slice(inicio + 1, fin), marcado: true });
    resto = resto.slice(fin + 1);
  }
  // Dos marcados seguidos separados solo por espacio se leen como uno.
  return trozos.filter((t) => t.texto.length > 0);
}

/** El pasaje sin marcas ni espacios repetidos, para reconocer el mismo dos veces. */
function huella(pasaje: string): string {
  return pasaje
    .replaceAll(MARCA_INICIO, '')
    .replaceAll(MARCA_FIN, '')
    .replace(/\s+/g, ' ')
    .trim()
    .toLowerCase();
}

/**
 * La constante de la fusión. Sesenta es el valor del trabajo que la propuso
 * (Cormack, Clarke y Büttcher, 2009) y el que se usa por defecto: suaviza la
 * ventaja de las primeras posiciones sin anularla.
 */
export const K = 60;

/** Un fragmento es el mismo en las dos listas si lleva el mismo locator. */
function clave(editionId: string, locator: Locator): string {
  return `${editionId}|${JSON.stringify(locator)}`;
}

export function fusionar(
  lexicos: readonly Hit[],
  semanticos: readonly SemanticHit[]
): Coincidencia[] {
  const fundidos = new Map<string, Coincidencia & { puntos: number }>();

  lexicos.forEach((hit, i) => {
    fundidos.set(clave(hit.editionId, hit.locator), {
      origen: 'exacta',
      editionId: hit.editionId,
      fileId: hit.fileId,
      locator: hit.locator,
      snippet: hit.snippet,
      titulo: hit.title,
      otras: 0,
      puntos: 1 / (K + i + 1)
    });
  });

  semanticos.forEach((hit, i) => {
    const puntos = 1 / (K + i + 1);
    const k = clave(hit.editionId, hit.locator);
    const ya = fundidos.get(k);
    if (ya) {
      ya.origen = 'ambas';
      ya.puntos += puntos;
      // El pasaje léxico se queda: lleva la coincidencia marcada.
      if (!ya.snippet) ya.snippet = hit.snippet;
    } else {
      fundidos.set(k, {
        origen: 'significado',
        editionId: hit.editionId,
        fileId: hit.fileId,
        locator: hit.locator,
        snippet: hit.snippet,
        titulo: '',
        otras: 0,
        puntos
      });
    }
  });

  // `sort` es estable: a igual puntuación se respeta el orden de llegada, que pone
  // lo léxico delante.
  const ordenados = [...fundidos.values()].sort((a, b) => b.puntos - a.puntos);
  // El mismo pasaje en otra edición no es otro resultado: se queda el mejor y
  // se cuenta cuántas más lo traen.
  const vistos = new Map<string, Coincidencia & { puntos: number }>();
  const unicos: Array<Coincidencia & { puntos: number }> = [];
  for (const c of ordenados) {
    const h = huella(c.snippet);
    const ya = h ? vistos.get(h) : undefined;
    if (ya) {
      ya.otras += 1;
      continue;
    }
    if (h) vistos.set(h, c);
    unicos.push(c);
  }
  return unicos.map(({ puntos: _puntos, ...coincidencia }) => coincidencia);
}
