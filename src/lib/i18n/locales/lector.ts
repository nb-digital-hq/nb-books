/**
 * Textos del lector que solo hacen falta con un documento abierto, en los tres
 * idiomas.
 *
 * # Por qué no están en el diccionario de cada idioma
 *
 * Por lo mismo que los de preparar audio (`audio.ts`): el diccionario español
 * entra en el paquete de arranque, que tiene un presupuesto de 250 KB
 * (`performance-budgets.md`, sección 4.11), y estos solo los usa la vista del
 * lector, que se carga al abrir el primer documento. Con ellos dentro, y con lo
 * demás, el arranque pasaba a 250,6 KB.
 *
 * El español manda: los otros dos tienen que tener sus mismas claves, y el tipo lo
 * exige.
 */

import type { Idioma } from '../diccionario';

const es = {
  // Lo que el capítulo trae y no se muestra. `{lista}` lo compone la
  // vista con las clases que haya, unidas como se unen en cada idioma.
  omitido: 'Este capítulo trae {lista} que el lector no muestra.',
  omitidoAudio: 'audio',
  omitidoVideo: 'vídeo',
  omitidoEmbedded: 'contenido incrustado',
  omitidoForm: 'un formulario',
  // El mapa del libro.
  mapaTitulo: 'Mapa del Libro',
  mapaCapitulos: 'Capítulos',
  mapaMarcas: 'Anotaciones',
  mapaLeido: 'leído',
  mapaAqui: 'Estás aquí',
  mapaReunidos: 'y {n} más',
  mapaSinMarcas: 'Todavía no hay anotaciones en este libro.',
  mapaAyuda: 'Las flechas recorren cada fila; Enter lleva a ese sitio.',
  mapaHighlight: 'resaltado',
  mapaHighlights: 'resaltados',
  mapaNote: 'nota',
  mapaNotes: 'notas',
  mapaBookmark: 'marcador',
  mapaBookmarks: 'marcadores',
  mapaUnderline: 'subrayado',
  mapaUnderlines: 'subrayados',
  mapaDrawing: 'trazo',
  mapaDrawings: 'trazos',
  // Los nombres del libro.
  nombresTitulo: 'Nombres del Libro',
  nombresAyuda:
    'Salen del propio texto, sin IA: los nombres que se repiten, con dónde aparecen. No distingue quién es una persona y qué es un lugar.',
  nombresCargando: 'Buscando los nombres del libro',
  nombresVacio: 'No aparece ningún nombre que se repita.',
  nombresSinIndice:
    'El libro aún no está indexado: los sitios salen del texto, sin la tolerancia a tildes de la búsqueda.',
  nombresSinTexto: 'Este documento no tiene texto del que sacar nombres.',
  nombresMenciones: 'menciones',
  nombresSitio: 'Ir a este sitio',
  // Ritmo de lectura. Sin presión: lo que queda, a tu ritmo, y nada más.
  ritmoMinutos: 'unos {n} min más, a tu ritmo',
  ritmoHoras: 'unas {n} h más, a tu ritmo',
  ritmoAyuda: 'Sale de las veces que lo has leído, sin cronómetro.',
  // Las anotaciones en un archivo que se lee sin NB Books.
  anotacionesMarkdown: 'Como Markdown',
  anotacionesOtrosFormatos: 'Otros Formatos',
  anotacionesTexto: 'Como Texto Plano'
};

export const TEXTOS_DEL_LECTOR: Record<Idioma, typeof es> = {
  es,
  en: {
    omitido: 'This chapter includes {lista} that the reader does not show.',
    omitidoAudio: 'audio',
    omitidoVideo: 'video',
    omitidoEmbedded: 'embedded content',
    omitidoForm: 'a form',
    mapaTitulo: 'Book Map',
    mapaCapitulos: 'Chapters',
    mapaMarcas: 'Annotations',
    mapaLeido: 'read',
    mapaAqui: 'You are here',
    mapaReunidos: 'and {n} more',
    mapaSinMarcas: 'There are no annotations in this book yet.',
    mapaAyuda: 'The arrow keys move along each row; Enter takes you there.',
    mapaHighlight: 'highlight',
    mapaHighlights: 'highlights',
    mapaNote: 'note',
    mapaNotes: 'notes',
    mapaBookmark: 'bookmark',
    mapaBookmarks: 'bookmarks',
    mapaUnderline: 'underline',
    mapaUnderlines: 'underlines',
    mapaDrawing: 'drawing',
    mapaDrawings: 'drawings',
    nombresTitulo: 'Names in the Book',
    nombresAyuda:
      'They come from the text itself, without AI: the names that repeat, with where they appear. It does not tell a person from a place.',
    nombresCargando: 'Looking for the names in the book',
    nombresVacio: 'No name repeats in this book.',
    nombresSinIndice:
      'The book is not indexed yet: places come from the text, without the accent tolerance of search.',
    nombresSinTexto: 'This document has no text to take names from.',
    nombresMenciones: 'mentions',
    nombresSitio: 'Go to this place',
    ritmoMinutos: 'about {n} more min at your pace',
    ritmoHoras: 'about {n} more h at your pace',
    ritmoAyuda: 'It comes from the times you have read it, without a stopwatch.',
    anotacionesMarkdown: 'As Markdown',
    anotacionesOtrosFormatos: 'Other Formats',
    anotacionesTexto: 'As Plain Text'
  },
  pt: {
    omitido: 'Este capítulo traz {lista} que o leitor não mostra.',
    omitidoAudio: 'áudio',
    omitidoVideo: 'vídeo',
    omitidoEmbedded: 'conteúdo incorporado',
    omitidoForm: 'um formulário',
    mapaTitulo: 'Mapa do Livro',
    mapaCapitulos: 'Capítulos',
    mapaMarcas: 'Anotações',
    mapaLeido: 'lido',
    mapaAqui: 'Estás aqui',
    mapaReunidos: 'e mais {n}',
    mapaSinMarcas: 'Ainda não há anotações neste livro.',
    mapaAyuda: 'As setas percorrem cada fila; Enter leva a esse sítio.',
    mapaHighlight: 'destaque',
    mapaHighlights: 'destaques',
    mapaNote: 'nota',
    mapaNotes: 'notas',
    mapaBookmark: 'marcador',
    mapaBookmarks: 'marcadores',
    mapaUnderline: 'sublinhado',
    mapaUnderlines: 'sublinhados',
    mapaDrawing: 'traço',
    mapaDrawings: 'traços',
    nombresTitulo: 'Nomes do Livro',
    nombresAyuda:
      'Saem do próprio texto, sem IA: os nomes que se repetem, com onde aparecem. Não distingue quem é uma pessoa e o que é um lugar.',
    nombresCargando: 'A procurar os nomes do livro',
    nombresVacio: 'Não aparece nenhum nome que se repita.',
    nombresSinIndice:
      'O livro ainda não está indexado: os sítios saem do texto, sem a tolerância a acentos da pesquisa.',
    nombresSinTexto: 'Este documento não tem texto de onde tirar nomes.',
    nombresMenciones: 'menções',
    nombresSitio: 'Ir para este sítio',
    ritmoMinutos: 'mais uns {n} min, ao teu ritmo',
    ritmoHoras: 'mais umas {n} h, ao teu ritmo',
    ritmoAyuda: 'Sai das vezes que o leste, sem cronómetro.',
    anotacionesMarkdown: 'Como Markdown',
    anotacionesOtrosFormatos: 'Outros Formatos',
    anotacionesTexto: 'Como Texto Simples'
  }
};
