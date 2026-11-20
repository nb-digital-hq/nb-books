/**
 * La capa de internacionalización.
 *
 * Un solo punto de entrada: los componentes importan de `$lib/i18n` y nunca de
 * `locales/`. Es la misma regla que `$core/ipc` con el puente, y por el mismo
 * motivo: un sitio donde mirar cuando algo del idioma se comporta raro.
 *
 * Contrato: `docs/architecture/i18n.md`.
 */

export {
  type Diccionario,
  esIdioma,
  IDIOMA_BASE,
  IDIOMAS,
  type Idioma,
  type MensajesDeError,
  NOMBRE_DEL_IDIOMA
} from './diccionario';
export { codigoDeError, esCodigoDeError, mensajeDeError, traducirError } from './errores';
export { cargarIdioma, idioma, textos, usarIdioma } from './idioma.svelte';
