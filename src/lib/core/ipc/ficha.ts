/**
 * Comandos que **no hacen falta para arrancar**: corregir y quitar desde la ficha, y
 * saber si las actualizaciones las hace NB Launcher.
 *
 * Aparte de `index.ts` por lo mismo que `voz.ts`: lo que hay en `index.ts` entra en
 * el paquete de arranque, que tiene un presupuesto de 250 KB, y estos solo los usan
 * la ficha y Ajustes, que se cargan al abrirlos. La regla no cambia: todo pasa por
 * `call`, con su contrato de errores.
 *
 * Contrato: `docs/architecture/ipc-contracts.md`.
 */

import type { Actualizacion, BrowseQuery, DocumentRemoval, LetterAnchor } from '$lib/bindings';

import { call } from './index';

/** Si las actualizaciones de esta copia las hace NB Launcher. */
export function actualizacionDelLauncher(): Promise<boolean> {
  return call<boolean>('update_managed_by_launcher');
}

/** Corrige a mano un campo de la ficha. Lo escrito por el usuario gana siempre. */
export function editarCampoDeEdicion(
  editionId: string,
  field: string,
  value: string | null
): Promise<void> {
  return call<void>('edit_edition_field', { editionId, field, value });
}

/**
 * Quita un documento de la biblioteca. El archivo referenciado no se toca; la
 * copia administrada solo se borra si se pide.
 */
export function quitarDocumento(
  editionId: string,
  deleteManagedCopy: boolean
): Promise<DocumentRemoval> {
  return call<DocumentRemoval>('delete_document', { editionId, deleteManagedCopy });
}

/** Las iniciales de la lista ordenada por título o autor, con su posición. */
export function anclasAlfabeticas(query: BrowseQuery): Promise<LetterAnchor[]> {
  return call<LetterAnchor[]>('browse_letter_anchors', { query });
}

/**
 * La última versión publicada, si es más nueva que la instalada. Lo dispara el
 * usuario: es el consentimiento para consultar las publicaciones del proyecto.
 */
export function actualizacionBuscar(): Promise<Actualizacion | null> {
  return call<Actualizacion | null>('update_check');
}

/** Descarga, comprueba la firma, instala la versión nueva y reinicia. */
export function actualizacionInstalar(): Promise<void> {
  return call<void>('update_install');
}
