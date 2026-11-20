/**
 * Los textos de preparar audio, en los tres idiomas.
 *
 * # Por qué no están en el diccionario de cada idioma
 *
 * Porque el diccionario español entra en el paquete de arranque, que tiene un
 * presupuesto de 250 KB (`performance-budgets.md`, sección 4.11), y estos textos
 * solo los usa un panel del lector, que se carga al abrir un documento. Con ellos
 * dentro el arranque pasaba a 251,7 KB. Es la misma regla que partió el paquete
 * por destino, aplicada a los textos, y la misma que ya siguen los de la IA.
 *
 * El español manda: los otros dos tienen que tener sus mismas claves, y el tipo lo
 * exige.
 */

import type { Idioma } from '../diccionario';

const es = {
  prepararTitulo: 'Preparar Este Capítulo',
  prepararAyuda:
    'Si tu equipo no da para narrar mientras lees, se prepara antes: se paga el tiempo una vez y después suena seguido, como un audiolibro.',
  prepararDurara: 'Durará',
  prepararOcupara: 'Ocupará',
  prepararTardara: 'Tardará en prepararse',
  prepararSinMedir: 'Aún no se ha medido en este equipo',
  prepararMuestra: 'Escuchar una Muestra',
  prepararEmpezar: 'Preparar',
  prepararCancelar: 'Cancelar',
  prepararDe: 'de',
  prepararFalta: 'faltan',
  prepararListo: 'Preparado',
  prepararAMedias: 'Preparado a medias: lo hecho se puede escuchar',
  prepararOlvidar: 'Quitar lo Preparado',
  prepararTerminado: 'Preparado: ya se puede escuchar.',
  prepararCancelado: 'Cancelado. Lo que llegó a prepararse se puede escuchar.',
  prepararSeDetuvo: 'La preparación se detuvo:',
  prepararLibroListo: 'El libro entero está preparado',
  prepararPausar: 'Pausar',
  prepararSeguirEscuchando: 'Seguir Escuchando',
  prepararDesdeAqui: 'Escuchar Desde Donde Voy',
  prepararSeguirTexto: 'Seguir el texto mientras suena',
  prepararFragmento: 'Suena el fragmento',
  prepararNoSuena: 'El audio está preparado y no se deja reproducir. Vuelve a prepararlo.',
  prepararQuitarLibro: 'Quitar el Libro Preparado',
  preparadoConVoz: 'Preparado con voz {voz} (modelo {modelo})',
  preparadoVozDiferente: 'La voz o el modelo elegidos han cambiado',
  prepararRegenerar: 'Regenerar con la Voz Actual',
  audiolibrosTitulo: 'Preparar un Audiolibro',
  audiolibrosAyuda:
    'El libro entero en audio, con esta voz. Escucha primero una muestra del propio libro y decide; lo que se prepare queda en la biblioteca, bajo el filtro de audiolibros.',
  audiolibrosElige: 'Elige un libro en la biblioteca y vuelve aquí.',
  audiolibrosColeccion: 'Encolar la Colección',
  audiolibrosEnCola: 'Esperando turno',
  audiolibrosEncoladas: 'obras encoladas',
  audiolibrosNoCabe:
    'No cabe en el sitio reservado para lo que la aplicación deriva de tu biblioteca. Libera sitio, o sube el tope en Ajustes.',
  audiolibrosQueda: 'Queda',
  audiolibrosVerPreparados: 'Ver los Audiolibros',
  audiolibrosTerminado: 'Preparado. Ábrelo y escúchalo desde el panel de voz del lector.',
  audiolibrosDondeSeEscucha:
    'Se escucha desde el panel de voz del lector, siguiendo el texto y empezando por el capítulo que tengas abierto.',
  // La narración que trae el propio libro.
  narracionTitulo: 'La Narración del Libro',
  narracionAyuda:
    'Este libro trae su propia narración, leída por una persona. Se recomienda antes que una voz sintética: el texto se sigue igual y se resalta lo que suena.',
  narracionDesdeAqui: 'Escuchar Desde Donde Voy',
  narracionPausar: 'Pausar',
  narracionSeguir: 'Seguir Escuchando',
  narracionSinCapitulo:
    'Este capítulo no trae narración. Suena en los que la traen: ve a uno de ellos y vuelve a pedirla.',
  narracionNoSuena:
    'El audio de este libro no se puede reproducir en este equipo. Suele pasar con el formato AAC, cuyo decodificador no se puede distribuir con la aplicación.',
  narracionTerminada: 'Se acabó la narración del libro.'
};

export const TEXTOS_DE_AUDIO: Record<Idioma, typeof es> = {
  es,
  en: {
    prepararTitulo: 'Prepare This Chapter',
    prepararAyuda:
      'If your computer cannot narrate while you read, it is prepared beforehand: you pay the time once and then it plays straight through, like an audiobook.',
    prepararDurara: 'Will last',
    prepararOcupara: 'Will take up',
    prepararTardara: 'Will take to prepare',
    prepararSinMedir: 'Not measured on this computer yet',
    prepararMuestra: 'Listen to a Sample',
    prepararEmpezar: 'Prepare',
    prepararCancelar: 'Cancel',
    prepararDe: 'of',
    prepararFalta: 'left:',
    prepararListo: 'Prepared',
    prepararAMedias: 'Half prepared: what is done can be listened to',
    prepararOlvidar: 'Remove What Is Prepared',
    prepararTerminado: 'Prepared: ready to listen.',
    prepararCancelado: 'Cancelled. Whatever got prepared can be listened to.',
    prepararSeDetuvo: 'Preparation stopped:',
    prepararLibroListo: 'The whole book is prepared',
    prepararPausar: 'Pause',
    prepararSeguirEscuchando: 'Keep Listening',
    prepararDesdeAqui: 'Listen From Where I Am',
    prepararSeguirTexto: 'Follow the text while it plays',
    prepararFragmento: 'Playing fragment',
    prepararNoSuena: 'The audio is prepared but will not play. Prepare it again.',
    prepararQuitarLibro: 'Remove the Prepared Book',
    preparadoConVoz: 'Prepared with voice {voz} (model {modelo})',
    preparadoVozDiferente: 'The selected voice or model has changed',
    prepararRegenerar: 'Regenerate with Current Voice',
    audiolibrosTitulo: 'Prepare an Audiobook',
    audiolibrosAyuda:
      'The whole book as audio, in this voice. Listen to a sample of the book itself first and decide; whatever gets prepared stays in your library, under the audiobook filter.',
    audiolibrosElige: 'Pick a book in the library and come back here.',
    audiolibrosColeccion: 'Queue the Collection',
    audiolibrosEnCola: 'Waiting',
    audiolibrosEncoladas: 'books queued',
    audiolibrosNoCabe:
      'It does not fit in the space kept for what the app derives from your library. Free up space, or raise the limit in Settings.',
    audiolibrosQueda: 'Left',
    audiolibrosVerPreparados: 'See the Audiobooks',
    audiolibrosTerminado: "Prepared. Open it and listen from the reader's voice panel.",
    audiolibrosDondeSeEscucha:
      "You listen from the reader's voice panel, following the text and starting at the chapter you have open.",
    narracionTitulo: "The Book's Narration",
    narracionAyuda:
      'This book comes with its own narration, read by a person. It is recommended over a synthetic voice: the text is followed the same way and what plays is highlighted.',
    narracionDesdeAqui: 'Listen From Where I Am',
    narracionPausar: 'Pause',
    narracionSeguir: 'Keep Listening',
    narracionSinCapitulo:
      'This chapter has no narration. It plays in the chapters that have it: go to one of them and ask again.',
    narracionNoSuena:
      "This book's audio cannot be played on this computer. It usually happens with AAC, whose decoder cannot be shipped with the app.",
    narracionTerminada: "The book's narration has ended."
  },
  pt: {
    prepararTitulo: 'Preparar Este Capítulo',
    prepararAyuda:
      'Se o teu equipamento não dá para narrar enquanto lês, prepara-se antes: paga-se o tempo uma vez e depois toca seguido, como um audiolivro.',
    prepararDurara: 'Vai durar',
    prepararOcupara: 'Vai ocupar',
    prepararTardara: 'Vai demorar a preparar',
    prepararSinMedir: 'Ainda não foi medido neste equipamento',
    prepararMuestra: 'Ouvir Uma Amostra',
    prepararEmpezar: 'Preparar',
    prepararCancelar: 'Cancelar',
    prepararDe: 'de',
    prepararFalta: 'faltam',
    prepararListo: 'Preparado',
    prepararAMedias: 'Preparado a meio: o que está feito pode ser ouvido',
    prepararOlvidar: 'Remover o Preparado',
    prepararTerminado: 'Preparado: já se pode ouvir.',
    prepararCancelado: 'Cancelado. O que chegou a ser preparado pode ser ouvido.',
    prepararSeDetuvo: 'A preparação parou:',
    prepararLibroListo: 'O livro inteiro está preparado',
    prepararPausar: 'Pausar',
    prepararSeguirEscuchando: 'Continuar a Ouvir',
    prepararDesdeAqui: 'Ouvir a Partir de Onde Estou',
    prepararSeguirTexto: 'Seguir o texto enquanto toca',
    prepararFragmento: 'Toca o fragmento',
    prepararNoSuena: 'O áudio está preparado e não toca. Prepara-o outra vez.',
    prepararQuitarLibro: 'Remover o Livro Preparado',
    preparadoConVoz: 'Preparado com a voz {voz} (modelo {modelo})',
    preparadoVozDiferente: 'A voz ou o modelo escolhidos mudaram',
    prepararRegenerar: 'Regenerar com a Voz Atual',
    audiolibrosTitulo: 'Preparar um Audiolivro',
    audiolibrosAyuda:
      'O livro inteiro em áudio, com esta voz. Ouve primeiro uma amostra do próprio livro e decide; o que for preparado fica na biblioteca, sob o filtro de audiolivros.',
    audiolibrosElige: 'Escolhe um livro na biblioteca e volta aqui.',
    audiolibrosColeccion: 'Pôr a Coleção na Fila',
    audiolibrosEnCola: 'À espera',
    audiolibrosEncoladas: 'obras na fila',
    audiolibrosNoCabe:
      'Não cabe no espaço reservado ao que a aplicação deriva da sua biblioteca. Liberte espaço, ou suba o limite nas Definições.',
    audiolibrosQueda: 'Falta',
    audiolibrosVerPreparados: 'Ver os Audiolivros',
    audiolibrosTerminado: 'Preparado. Abre-o e ouve-o no painel de voz do leitor.',
    audiolibrosDondeSeEscucha:
      'Ouve-se no painel de voz do leitor, seguindo o texto e começando pelo capítulo que tiveres aberto.',
    narracionTitulo: 'A Narração do Livro',
    narracionAyuda:
      'Este livro traz a sua própria narração, lida por uma pessoa. Recomenda-se antes de uma voz sintética: o texto segue-se igual e realça-se o que soa.',
    narracionDesdeAqui: 'Ouvir a Partir de Onde Vou',
    narracionPausar: 'Pausar',
    narracionSeguir: 'Continuar a Ouvir',
    narracionSinCapitulo:
      'Este capítulo não traz narração. Soa nos que a trazem: vai a um deles e volta a pedi-la.',
    narracionNoSuena:
      'O áudio deste livro não pode ser reproduzido neste equipamento. Costuma acontecer com o formato AAC, cujo descodificador não pode ser distribuído com a aplicação.',
    narracionTerminada: 'Acabou a narração do livro.'
  }
};
