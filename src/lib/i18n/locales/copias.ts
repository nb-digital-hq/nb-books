/**
 * Lo que el panel de importar dice de las copias exactas, en los tres
 * idiomas.
 *
 * Aparte del diccionario por lo mismo que `lector.ts`: el diccionario español entra
 * en el paquete de arranque, que va al límite de su presupuesto, y estos textos solo
 * los usa el panel de importar, que se carga al abrirlo.
 */

import type { Idioma } from '../diccionario';

const es = {
  copias: 'copias exactas',
  copiasAyuda:
    'Mismo contenido que algo que ya estaba, en otra ruta: no se ha añadido otra vez. Puedes borrar la copia si no la quieres.',
  copiaDe: 'copia de'
};

export const TEXTOS_DE_COPIAS: Record<Idioma, typeof es> = {
  es,
  en: {
    copias: 'exact copies',
    copiasAyuda:
      'Same content as something already there, at another path: it was not added again. You can delete the copy if you do not want it.',
    copiaDe: 'copy of'
  },
  pt: {
    copias: 'cópias exatas',
    copiasAyuda:
      'O mesmo conteúdo que algo que já lá estava, noutro caminho: não se adicionou outra vez. Podes apagar a cópia se não a quiseres.',
    copiaDe: 'cópia de'
  }
};
