/**
 * La preparación de la búsqueda por significado, compartida por quien la enseña
 *.
 *
 * Vive aquí y no en la carcasa porque solo la usan piezas que se cargan a demanda
 * —la búsqueda, las colecciones y el lector—, y la carcasa está en el paquete de
 * arranque, que tiene un presupuesto de 250 KB. Un solo estado y una sola
 * suscripción a los eventos, por mucho que la miren varias piezas a la vez.
 */

import { onDomainEvent, readerActivity, vectorJob } from '$core/ipc';
import type { VectorJob } from '$lib/bindings';

/** El trabajo en curso, tal como lo cuenta el runtime. */
export const preparacion = $state<{ trabajo: VectorJob | null; terminadas: number }>({
  trabajo: null,
  terminadas: 0
});

let siguiendo = false;

/** Empieza a seguir el trabajo. Llamarla más de una vez no suscribe dos veces. */
export function seguirPreparacion(): void {
  if (siguiendo) return;
  siguiendo = true;
  void vectorJob()
    .then((trabajo) => (preparacion.trabajo = trabajo))
    .catch(() => {
      // Sin respuesta no hay trabajo que enseñar; no es motivo para fallar.
    });
  void onDomainEvent('VectorJobProgress', (e) => {
    preparacion.trabajo = {
      running: true,
      paused: e.pauseReason === 'USER_PAUSED',
      pauseReason: e.pauseReason,
      editionId: e.editionId,
      done: e.done,
      total: e.total,
      queued: e.queued
    };
  });
  void onDomainEvent('VectorJobFinished', () => {
    preparacion.terminadas += 1;
    void vectorJob().then((trabajo) => (preparacion.trabajo = trabajo));
  });
}

/**
 * Cada cuánto, como mucho, avisa el lector de que está pintando.
 *
 * Medio segundo: el runtime espera a que el lector lleve un segundo y medio quieto,
 * así que un aviso cada medio segundo mientras se desplaza lo mantiene apartado sin
 * cruzar el puente en cada fotograma.
 */
const CADA = 500;
let ultimoAviso = 0;

/**
 * El lector se mueve. Solo cruza el puente si hay una preparación en marcha: sin
 * ella no hay nada que apartar.
 */
export function avisarQueElLectorPinta(): void {
  if (!preparacion.trabajo?.running) return;
  const ahora = performance.now();
  if (ahora - ultimoAviso < CADA) return;
  ultimoAviso = ahora;
  void readerActivity().catch(() => {
    // Un aviso perdido solo hace que el trabajo no se aparte esta vez.
  });
}
