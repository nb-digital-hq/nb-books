/**
 * De código canónico a mensaje que se le puede enseñar a alguien.
 *
 * # Por qué la traducción vive en el frontend y no en Rust
 *
 * Porque el runtime no sabe en qué idioma está la interfaz, y no debe saberlo:
 * `i18n.md`, sección 3, y el contrato de errores. Rust devuelve un código en
 * mayúsculas, estable, y quien lo pinta decide cómo se dice. Eso además permite
 * que el mismo código se diga distinto según dónde aparezca sin tocar el
 * runtime.
 *
 * # Por qué nunca se enseña el código
 *
 * `DOCUMENT_CORRUPTED` no le dice nada a nadie que no haya escrito el runtime.
 * Un código sin traducir es un fallo de esta capa, no información para el
 * usuario, y por eso lo que llega sin catalogar cae en `desconocido` en lugar
 * de pintarse en crudo.
 */

import { ERROR_CODES, type ErrorCode } from '$lib/bindings/errorCodes';

import { textos } from './idioma.svelte';

/** Si una cadena es uno de los códigos que el runtime puede devolver. */
export function esCodigoDeError(valor: unknown): valor is ErrorCode {
  return typeof valor === 'string' && (ERROR_CODES as readonly string[]).includes(valor);
}

/**
 * Mensaje localizado de un error del runtime.
 *
 * Acepta lo que sea: un `NbError` trae el código, pero por el puente puede
 * llegar cualquier cosa si algo va muy mal, y esta función es la última entre
 * ese "cualquier cosa" y la pantalla.
 */
export function traducirError(codigo: unknown): string {
  const mensajes = textos().errores;
  if (esCodigoDeError(codigo)) {
    const mensaje = mensajes[codigo];
    if (mensaje) return mensaje;
  }
  return mensajes.desconocido;
}

/**
 * Mensaje localizado de lo que se haya lanzado.
 *
 * Reconoce un error del puente por su campo `code` en lugar de por su clase, y
 * no es un atajo: importar `NbError` aquí ataría la capa de idioma a la del
 * puente, y esta función tiene que servir también donde el error viene de otro
 * sitio. Lo que no traiga código cae en `desconocido`.
 *
 * **Nunca devuelve `error.message`.** En un `NbError` el mensaje *es* el código,
 * así que enseñarlo sería enseñar `STORAGE_FAILURE` en mitad de la pantalla; y
 * en cualquier otro error puede llevar rutas del usuario, que es justo lo que la
 * política de registro prohíbe que salga.
 */
export function mensajeDeError(causa: unknown): string {
  if (typeof causa === 'object' && causa !== null && 'code' in causa) {
    return traducirError((causa as { code: unknown }).code);
  }
  return traducirError(undefined);
}

/**
 * El código canónico de lo que se haya lanzado, si lo trae.
 *
 * Existe para el segundo renglón de un aviso de fallo: el primero dice qué pasó
 * en palabras y este dice **qué fue**, que es lo único que sirve para buscarlo
 * en la documentación o para contarlo en un informe. No sustituye al mensaje ni
 * se enseña solo: un código a secas no le dice nada a nadie.
 *
 * Devuelve `null` para lo que llegue sin catalogar, para no inventar un código
 * que el runtime no ha emitido.
 */
export function codigoDeError(causa: unknown): string | null {
  if (typeof causa === 'object' && causa !== null && 'code' in causa) {
    const codigo = (causa as { code: unknown }).code;
    if (esCodigoDeError(codigo)) return codigo;
  }
  return null;
}
