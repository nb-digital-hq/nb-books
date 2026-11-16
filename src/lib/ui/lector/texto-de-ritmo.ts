/**
 * La frase de lo que queda de un libro, en el idioma de la interfaz.
 *
 * Aparte de `ritmo.ts` para que aquella se pruebe sin diccionarios; esta solo
 * compone. Los textos son diferidos: los usan el lector y la ficha, que se cargan a
 * demanda.
 */

import { idioma } from '$lib/i18n';
import { TEXTOS_DEL_LECTOR } from '$lib/i18n/locales/lector';

import type { Queda } from './ritmo';

export function textoDeQueda(queda: Queda | null): string | null {
  if (!queda) return null;
  const t = TEXTOS_DEL_LECTOR[idioma()];
  const cifra = new Intl.NumberFormat(idioma(), { maximumFractionDigits: 1 }).format(queda.cuanto);
  return (queda.unidad === 'minutos' ? t.ritmoMinutos : t.ritmoHoras).replace('{n}', cifra);
}
