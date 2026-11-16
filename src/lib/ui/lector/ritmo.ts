/**
 * Cuánto falta de un libro, en tiempo y no en porcentaje.
 *
 * Contrato: `docs/design/reading-experience/README.md`, sección 4. El ritmo lo
 * da el runtime con las sesiones que ya guarda (`ReadingPace`); aquí se cruza con
 * dónde se está **ahora** y se redondea como se dice en voz alta. Sin cronómetro, y
 * sin cifras que empujen: "unos 40 minutos más", nunca "te faltan 38 min 12 s".
 *
 * Pura para poder probar los redondeos sin montar el lector.
 */

import type { ReadingPace } from '$lib/bindings';

/** Lo que queda, redondeado para decirlo: minutos de cinco en cinco, u horas de media en media. */
export type Queda = { unidad: 'minutos'; cuanto: number } | { unidad: 'horas'; cuanto: number };

/**
 * Lo que queda por leer a ese ritmo desde esa progresión, o nulo si no se puede
 * decir: sin ritmo, sin posición, o con el libro ya terminado.
 */
export function queda(ritmo: ReadingPace | null, progresion: number | null): Queda | null {
  if (!ritmo || progresion === null || !(ritmo.progressionPerMinute > 0)) return null;
  if (progresion >= 0.99) return null;
  const minutos = (1 - Math.max(0, progresion)) / ritmo.progressionPerMinute;
  if (!Number.isFinite(minutos)) return null;
  if (minutos < 57.5)
    return { unidad: 'minutos', cuanto: Math.max(5, Math.round(minutos / 5) * 5) };
  return { unidad: 'horas', cuanto: Math.round((minutos / 60) * 2) / 2 };
}
