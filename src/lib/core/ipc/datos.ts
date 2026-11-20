/**
 * Los comandos de borrado de datos del usuario desde Ajustes.
 *
 * # Por qué están aparte y no en `index.ts`
 *
 * Porque `index.ts` entra en el paquete de arranque (`performance-budgets.md`,
 * sección 4.11, tope de 250 KB) y estos tres comandos solo los invoca la sección
 * `Dónde Viven tus Libros` de Ajustes, que se carga al abrirla.
 *
 * **La regla del puente se mantiene intacta**: toda invocación pasa por `call`, con
 * su traducción de errores y su registro.
 *
 * Contrato: `docs/audit/privacy.md`, sección 4.
 */

import type { LegalNotices } from '$lib/bindings';

import { call } from './index';

/**
 * Borra el historial de lectura y por dónde iba cada documento (`reading_sessions`
 * y `progress`). Conserva el catálogo, las anotaciones, las notas, las preferencias
 * y los archivos del usuario en el disco.
 */
export function clearReadingHistory(): Promise<void> {
  return call<void>('clear_reading_history');
}

/**
 * Borra todas las anotaciones, las notas, sus enlaces y los tableros. Conserva el
 * catálogo, el historial de lectura, el vocabulario, las preferencias y los archivos
 * del usuario en el disco.
 */
export function clearAnnotationsAndNotes(): Promise<void> {
  return call<void>('clear_annotations_and_notes');
}

/**
 * Vacía la biblioteca entera en la base de datos. Conserva las preferencias, los
 * diccionarios, los catálogos OPDS y los permisos de red, y **nunca borra los
 * archivos del usuario en el disco**.
 */
export function clearLibrary(): Promise<void> {
  return call<void>('clear_library');
}

/**
 * Devuelve `LICENSE`, `NOTICE` y `THIRD-PARTY-NOTICES.md` para la sección `Acerca de`
 * de Ajustes.
 */
export function legalNotices(): Promise<LegalNotices> {
  return call<LegalNotices>('legal_notices');
}
