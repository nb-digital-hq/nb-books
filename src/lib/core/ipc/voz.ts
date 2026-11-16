/**
 * Los comandos de la voz que **no hacen falta para arrancar**: el estado del
 * modelo neuronal y preparar audio.
 *
 * # Por qué están aparte y no en `index.ts`
 *
 * Porque `index.ts` lo importa la pantalla principal, así que todo lo que hay
 * dentro entra en el paquete de arranque, que tiene un presupuesto de 250 KB
 * (`performance-budgets.md`, sección 4.11). Estos siete comandos no los llama
 * nadie hasta que alguien abre el panel de voz de un documento, y con ellos
 * dentro el arranque se pasaba del presupuesto.
 *
 * **La regla no cambia**: el puente se sigue invocando solo desde `$core/ipc`, que
 * es una carpeta en lugar de un archivo. Lo que cruza el puente pasa
 * por `call`, con su contrato de errores y su registro, exactamente igual.
 *
 * Contrato: `docs/architecture/ipc-contracts.md`.
 */

import type {
  AudioEstimate,
  AudioJob,
  AudioQueued,
  AudioScope,
  NeuralVoice,
  PreparedAudio
} from '$lib/bindings';

import { call } from './index';

/**
 * La voz neuronal local: qué modelo es, si está instalado y si esta máquina lo
 * mueve.
 *
 * Con `medir`, carga el modelo y sintetiza una frase corta para saber el factor de
 * tiempo real: tarda unos segundos y ocupa 600 MB hasta que se suelta. Es lo que
 * pide Ajustes. Sin él no carga nada y dice lo que ya se sabe, que es lo que pide el
 * narrador al arrancar: medir en cada arranque era cargar el modelo sin que nadie
 * fuera a escuchar.
 */
export function neuralVoice(medir = false): Promise<NeuralVoice> {
  return call<NeuralVoice>('neural_voice', { measure: medir });
}

/**
 * Lo que costaría preparar el audio de un capítulo o de un libro.
 *
 * Se pide **antes** de ofrecer empezar: quien va a esperar cuarenta minutos tiene
 * que saberlo antes de empezar a esperar.
 */
export function estimateAudio(scope: AudioScope, voice: string | null): Promise<AudioEstimate> {
  return call<AudioEstimate>('estimate_audio', { scope, voice });
}

/**
 * Sintetiza una muestra corta **del propio libro**, y devuelve la ruta del audio.
 *
 * Del libro y no una frase de ejemplo: quien va a esperar merece oír antes cómo
 * suena la voz diciendo sus nombres propios y su puntuación.
 */
export function sampleAudio(scope: AudioScope, voice: string | null): Promise<string> {
  return call<string>('sample_audio', { scope, voice });
}

/** Empieza a preparar, en segundo plano. El avance llega por `AudioJobProgress`. */
export function startAudio(scope: AudioScope, voice: string | null): Promise<void> {
  return call<void>('start_audio', { scope, voice });
}

/**
 * Encola el libro entero de cada obra de una colección.
 *
 * Devuelve **cuántas entraron**, que no tiene por qué ser cuántas hay: lo que no
 * cabe en el presupuesto de lo derivado no se encola, y eso se enseña.
 */
export function startAudioCollection(collectionId: string, voice: string | null): Promise<number> {
  return call<number>('start_audio_collection', { collectionId, voice });
}

/** Los que esperan turno detrás del que se prepara ahora. */
export function audioQueue(): Promise<AudioQueued[]> {
  return call<AudioQueued[]>('audio_queue');
}

/** Corta la preparación y vacía la cola. Lo hecho se queda. */
export function cancelAudio(): Promise<void> {
  return call<void>('cancel_audio');
}

/** Lo que se está preparando ahora, si hay algo. */
export function audioJob(): Promise<AudioJob | null> {
  return call<AudioJob | null>('audio_job');
}

/** El audio preparado de un alcance, con sus marcas para seguir el texto. */
export function preparedAudio(
  scope: AudioScope,
  voice: string | null
): Promise<PreparedAudio | null> {
  return call<PreparedAudio | null>('prepared_audio', { scope, voice });
}

/**
 * Quita el audio preparado de un alcance -un capítulo o el libro entero-. Lo demás
 * de ese libro se queda.
 */
export function forgetAudio(scope: AudioScope): Promise<void> {
  return call<void>('forget_audio', { scope });
}
