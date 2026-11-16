/**
 * Lo que la ficha dice al corregirla a mano y al quitar una obra, en los tres
 * idiomas.
 *
 * Aparte del diccionario por lo mismo que `copias.ts`: el diccionario español entra
 * en el paquete de arranque, que va al límite de su presupuesto, y estos textos solo
 * los usa la ficha, que se carga al abrirla.
 */

import type { Idioma } from '../diccionario';

const es = {
  editar: 'Editar la Ficha',
  editarAyuda:
    'Lo que escribas aquí gana sobre lo que traía el archivo y sobre lo que llegue de un catálogo.',
  guardar: 'Guardar',
  cancelar: 'Cancelar',
  tituloObligatorio: 'Una obra no puede quedarse sin título.',
  noSeGuardo: 'No se pudo guardar la ficha.',
  quitar: 'Quitar de la Biblioteca',
  quitarAviso:
    'Se quitan también sus anotaciones, su progreso y sus notas ancladas. El archivo se queda donde está.',
  quitarCopia: 'Borrar también la copia que hizo la aplicación',
  quitarConfirmar: 'Quitar',
  noSeQuito: 'No se pudo quitar el documento.',
  'campo-series_title': 'Serie',
  'campo-series_index': 'Número en la serie',
  'campo-description': 'Sinopsis'
};

export const TEXTOS_DE_LA_FICHA: Record<Idioma, typeof es> = {
  es,
  en: {
    editar: 'Edit the Details',
    editarAyuda:
      'What you write here wins over what the file carried and over anything a catalogue sends.',
    guardar: 'Save',
    cancelar: 'Cancel',
    tituloObligatorio: 'A work cannot be left without a title.',
    noSeGuardo: 'The details could not be saved.',
    quitar: 'Remove from the Library',
    quitarAviso:
      'Its annotations, progress and anchored notes are removed too. The file stays where it is.',
    quitarCopia: 'Also delete the copy the application made',
    quitarConfirmar: 'Remove',
    noSeQuito: 'The document could not be removed.',
    'campo-series_title': 'Series',
    'campo-series_index': 'Number in the series',
    'campo-description': 'Synopsis'
  },
  pt: {
    editar: 'Editar a Ficha',
    editarAyuda:
      'O que escrever aqui prevalece sobre o que o arquivo trazia e sobre o que chegar de um catálogo.',
    guardar: 'Guardar',
    cancelar: 'Cancelar',
    tituloObligatorio: 'Uma obra não pode ficar sem título.',
    noSeGuardo: 'Não foi possível guardar a ficha.',
    quitar: 'Remover da Biblioteca',
    quitarAviso:
      'As anotações, o progresso e as notas ancoradas também são removidos. O arquivo fica onde está.',
    quitarCopia: 'Apagar também a cópia feita pela aplicação',
    quitarConfirmar: 'Remover',
    noSeQuito: 'Não foi possível remover o documento.',
    'campo-series_title': 'Série',
    'campo-series_index': 'Número na série',
    'campo-description': 'Sinopse'
  }
};
