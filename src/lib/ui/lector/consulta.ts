/**
 * La consulta sobre la selección, como datos y funciones puras.
 *
 * # Por qué hay que partir la selección en palabras
 *
 * Porque en NB Books **se selecciona por bloques**, no por caracteres: es la
 * decisión, y viene de que la navegación por cursor de texto no se
 * comporta igual en los tres webviews. Eso hace que la selección de partida sea
 * casi siempre un párrafo entero, y un párrafo entero no está en ningún
 * diccionario.
 *
 * Así que la consulta se hace en dos niveles, que es lo que ya distinguía el
 * contrato y aquí encuentra su motivo:
 *
 * * **El término** es una palabra. Es lo que busca un diccionario.
 * * **El contexto** es el párrafo. Es lo que usa un traductor, y lo que sirve
 *   para desambiguar.
 *
 * # Por qué se ofrecen todas las palabras y no solo la primera
 *
 * Porque quien consulta sabe cuál es la palabra que no entiende y el programa
 * no. Adivinarla sería la clase de decisión que obliga al usuario a corregirla
 * cada vez. Se consulta la primera para que el panel diga algo al abrirse, y las
 * demás quedan a un tabulador, sin ratón.
 */

import type { LookupOutcome } from '$core/ipc';

/** Lo que el panel de consulta tiene que enseñar. */
export interface EstadoDeConsulta {
  /** La palabra consultada. */
  termino: string;
  /** El párrafo del que salió, que es lo que se ofrece para elegir otra. */
  seleccion: string;
  resultado: LookupOutcome | null;
  cargando: boolean;
  /** Código canónico del fallo, nunca un mensaje. */
  error: string | null;
}

/**
 * Cuántas palabras se ofrecen como mucho.
 *
 * Un párrafo largo tiene doscientas, y doscientos botones no son una elección:
 * son una lista que nadie recorre. Con las primeras se cubre el caso real, que
 * es consultar algo de la frase que se acaba de leer.
 */
export const MAXIMO_PALABRAS = 24;

/**
 * Palabras consultables de un texto, en orden y sin repetir.
 *
 * Se conservan los guiones y los apóstrofos **dentro** de la palabra —`sobre-`
 * `todo`, `l'aigua`— porque en varios idiomas forman parte de ella, y se
 * descarta lo que no tiene letras: un número suelto no está en ningún
 * diccionario.
 */
export function palabrasDe(texto: string, maximo: number = MAXIMO_PALABRAS): string[] {
  const encontradas = texto.match(/[\p{L}\p{M}\p{N}]+(?:['’-][\p{L}\p{M}\p{N}]+)*/gu) ?? [];

  const vistas = new Set<string>();
  const palabras: string[] = [];
  for (const bruta of encontradas) {
    // Una palabra sin ninguna letra es un número o una referencia: no se
    // consulta.
    if (!/\p{L}/u.test(bruta)) continue;
    const clave = bruta.toLocaleLowerCase();
    if (vistas.has(clave)) continue;
    vistas.add(clave);
    palabras.push(bruta);
    if (palabras.length >= maximo) break;
  }
  return palabras;
}

/**
 * Qué palabra se consulta al abrir el panel sobre una selección.
 *
 * Cadena vacía cuando no hay ninguna, y entonces no se consulta nada: preguntar
 * por un párrafo de puntuación no devolvería más que un panel vacío.
 */
export function terminoInicial(seleccion: string): string {
  return palabrasDe(seleccion, 1)[0] ?? '';
}
