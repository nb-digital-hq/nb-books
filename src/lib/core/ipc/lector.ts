/**
 * Los comandos que **no hacen falta para arrancar**: la narración del
 * libro, los nombres del libro y el vocabulario consultado
 *.
 *
 * # Por qué están aparte y no en `index.ts`
 *
 * Por lo mismo que `voz.ts`: `index.ts` lo importa la pantalla principal, así que
 * todo lo que hay dentro entra en el paquete de arranque, que tiene un presupuesto
 * de 250 KB (`performance-budgets.md`, sección 4.11). Estos solo los llaman piezas
 * que se cargan a demanda -el panel de voz, el índice, Estudio-, y con ellos dentro
 * el arranque llegaba a 249,8 KB.
 *
 * **La regla no cambia**: el puente se invoca solo desde `$core/ipc`, y lo que lo
 * cruza pasa por `call`, con su contrato de errores.
 *
 * Contrato: `docs/architecture/ipc-contracts.md`.
 */

import { save } from '@tauri-apps/plugin-dialog';

import type {
  EntityIndex,
  EntityPlaces,
  ReadingPace,
  SynchronizedAudio,
  VocabularyCards
} from '$lib/bindings';

import { call } from './index';

/**
 * La narración que trae el propio libro para un capítulo, o nula.
 *
 * El audio viene ya extraído a la caché: se reproduce con el mismo reproductor
 * que lo preparado, que lo pide por el protocolo de assets.
 */
export function synchronizedAudio(fileId: string, href: string): Promise<SynchronizedAudio | null> {
  return call<SynchronizedAudio | null>('synchronized_audio', { fileId, href });
}

/**
 * Los nombres del libro abierto, con dónde aparece cada uno.
 *
 * Local y sin IA. La primera vez recorre el libro entero; las siguientes, mientras
 * siga abierto, contestan lo mismo sin recorrerlo.
 */
export function bookEntities(fileId: string): Promise<EntityIndex> {
  return call<EntityIndex>('book_entities', { fileId });
}

/**
 * Dónde aparece un nombre del libro abierto, según el índice de búsqueda si está
 *. Se pide al desplegar el nombre y no con la lista: preguntarlo para
 * todos a la vez costaba segundos en los libros más largos de la biblioteca real.
 */
export function entityPlaces(fileId: string, name: string): Promise<EntityPlaces> {
  return call<EntityPlaces>('entity_places', { fileId, name });
}

/**
 * Guarda el vocabulario consultado en CSV donde el usuario diga. Devuelve
 * cuántas palabras salieron, o `null` si se cancela, que no es un error.
 */
export async function guardarVocabulario(
  nombrePorDefecto: string,
  titulo: string
): Promise<number | null> {
  const destino = await save({ defaultPath: nombrePorDefecto, title: titulo });
  if (destino === null) return null;
  return call<number>('save_vocabulary', { path: destino });
}

/**
 * Hace tarjetas de estudio con el vocabulario, con la definición del diccionario de
 * aquí dentro y sin salir a la red.
 */
export function vocabularyToCards(): Promise<VocabularyCards> {
  return call<VocabularyCards>('vocabulary_to_cards');
}

/**
 * A qué ritmo se lee una edición, o nulo si no hay bastante lectura para decirlo
 *. Lo que falta lo calcula quien sabe por dónde se va.
 */
export function readingPace(editionId: string): Promise<ReadingPace | null> {
  return call<ReadingPace | null>('reading_pace', { editionId });
}
