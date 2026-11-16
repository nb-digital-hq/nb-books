/**
 * Qué decirle al usuario sobre la búsqueda de un documento.
 *
 * La mitad que vive en el frontend. El runtime dice **por qué** un
 * documento no se puede buscar con un código canónico; aquí se decide qué
 * significa eso para quien está mirando la pantalla.
 *
 * Presentar un documento como buscable cuando no lo es genera desconfianza en la
 * función completa: el usuario busca una frase que sabe que está en el libro, no
 * la encuentra, y a partir de ahí no se fía de ninguna búsqueda.
 *
 * Contrato: `docs/architecture/search.md`, sección 4.
 */

import type { SearchStatus } from '$lib/bindings';

/**
 * Qué puede hacer el usuario ante un documento que no se puede buscar.
 *
 * Tres respuestas y no una, porque son tres situaciones distintas:
 *
 * * `esperar`: todavía no se ha indexado, o el volumen está desconectado. Se
 *   arregla solo y no hay nada que hacer.
 * * `ocr`: es un escaneo sin capa de texto. Tiene arreglo, y es el **único**
 *   caso en el que ofrecer OCR significa algo.
 * * `ninguna`: es un cómic, o esta instalación no puede abrir ese formato. No
 *   hay nada que ofrecer, y fingir que sí lo hay es peor que no ofrecer nada.
 */
export type AccionSobreBusqueda = 'esperar' | 'ocr' | 'ninguna';

/** Clave del mensaje que la interfaz muestra. La traducción le corresponde a otro módulo. */
export type MensajeDeBusqueda =
  | 'busqueda.pendiente'
  | 'busqueda.disponible'
  | 'busqueda.sin-texto-imagen'
  | 'busqueda.sin-capa-de-texto'
  | 'busqueda.archivo-no-disponible'
  | 'busqueda.sin-motor';

/**
 * Indica si el documento se puede buscar **ahora mismo**.
 *
 * Un documento pendiente devuelve `false` y no es lo mismo que uno no buscable:
 * para eso está `mensajeDeBusqueda`.
 */
export function sePuedeBuscar(estado: SearchStatus): boolean {
  return estado.state === 'searchable';
}

/**
 * Mensaje que corresponde a un estado de búsqueda.
 */
export function mensajeDeBusqueda(estado: SearchStatus): MensajeDeBusqueda {
  if (estado.state === 'searchable') return 'busqueda.disponible';
  if (estado.state === 'pending') return 'busqueda.pendiente';

  switch (estado.reason) {
    case 'IMAGE_ONLY':
      return 'busqueda.sin-texto-imagen';
    case 'NO_TEXT_LAYER':
      return 'busqueda.sin-capa-de-texto';
    case 'FILE_UNAVAILABLE':
      return 'busqueda.archivo-no-disponible';
    case 'NO_ENGINE':
      return 'busqueda.sin-motor';
    default:
      // Un motivo que esta versión de la interfaz no conoce. Se trata como
      // no buscable sin explicación concreta en lugar de romper la pantalla:
      // un runtime más nuevo que el frontend es una situación normal durante
      // una actualización.
      return 'busqueda.sin-capa-de-texto';
  }
}

/**
 * Qué se le puede ofrecer al usuario ante ese estado.
 */
export function accionSobreBusqueda(estado: SearchStatus): AccionSobreBusqueda {
  if (estado.state === 'searchable') return 'ninguna';
  if (estado.state === 'pending') return 'esperar';

  switch (estado.reason) {
    case 'NO_TEXT_LAYER':
      return 'ocr';
    case 'FILE_UNAVAILABLE':
      return 'esperar';
    default:
      return 'ninguna';
  }
}
