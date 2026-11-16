/**
 * Lo que el panel de estudio está generando, **fuera del panel**.
 *
 * El panel es momentáneo -se cierra al pulsar fuera-, y mientras el modelo local
 * genera dice "puedes seguir leyendo": seguir leyendo es tocar el texto, y eso lo
 * cerraba llevándose el resultado que estaba esperando. La generación seguía en el
 * runtime -entre uno y seis minutos, medido- y su respuesta llegaba a un componente
 * que ya no existía. Encontrado al revisar la fase el 2026-09-22.
 *
 * Aquí vive lo que tiene que sobrevivir al panel: qué se genera, si está en marcha,
 * lo generado y la propuesta editable. Al volver a abrirlo, está donde se dejó.
 */

import type {
  GeneratedCard,
  GeneratedConcept,
  GeneratedQuestion,
  StudyFunction,
  StudyGeneration
} from '$lib/bindings';

export const estudioDelLector = $state({
  /** De qué libro es lo que hay aquí. Otro libro empieza de cero. */
  editionId: null as string | null,
  funcion: 'summary' as StudyFunction,
  generando: false,
  resultado: null as StudyGeneration | null,
  fallo: null as string | null,
  sinPermiso: false,
  guardado: false,
  // La propuesta editable: copias, para que editar no toque lo generado.
  tarjetas: [] as GeneratedCard[],
  preguntas: [] as GeneratedQuestion[],
  conceptos: [] as GeneratedConcept[]
});

/** Empieza de cero para otro libro, salvo que haya algo generándose. */
export function estudioPara(editionId: string): void {
  if (estudioDelLector.editionId === editionId || estudioDelLector.generando) return;
  estudioDelLector.editionId = editionId;
  estudioDelLector.resultado = null;
  estudioDelLector.fallo = null;
  estudioDelLector.sinPermiso = false;
  estudioDelLector.guardado = false;
  estudioDelLector.tarjetas = [];
  estudioDelLector.preguntas = [];
  estudioDelLector.conceptos = [];
}
