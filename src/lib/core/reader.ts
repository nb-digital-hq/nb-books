/**
 * Qué controles del lector existen para el documento abierto.
 *
 * Esta es la mitad de la. que vive en el frontend: el runtime
 * declara lo que el formato sabe hacer y aquí se traduce a la lista de controles
 * que hay que pintar. Es una función pura y sin componentes a propósito, porque
 * el paradigma de la interfaz todavía no está decidido y esta regla no
 * depende de él: sea cual sea la forma que tome la pantalla, un cómic no tendrá
 * controles de tipografía.
 *
 * **La regla es omitir, no atenuar.** Un control deshabilitado es una promesa a
 * medias: el usuario lo ve, lo intenta y no pasa nada. Ofrecer un ajuste de
 * tipografía en gris sobre un cómic es peor que no ofrecer ninguno, porque
 * sugiere que el problema es suyo.
 *
 * Contrato: `docs/architecture/reader-contracts.md`, sección 4.
 */

import type { AnchoredAnnotation, ReaderCapabilities } from '$lib/bindings';

/**
 * Los controles que el lector puede llegar a mostrar.
 *
 * Lista cerrada: un control que no esté aquí no se pinta, y añadirlo obliga a
 * decidir de qué capacidad depende. Es lo que impide que la interfaz crezca con
 * botones cuya condición de existencia no sabe nadie.
 */
export type ControlDelLector =
  | 'tipografia'
  | 'busqueda'
  | 'seleccion'
  | 'anotaciones'
  | 'ajuste-de-pagina'
  | 'lectura-en-voz-alta'
  | 'audio';

/**
 * Controles que corresponden a un documento con estas capacidades.
 *
 * El orden es estable y es el de la barra: lo que se usa más, antes. Que sea
 * estable importa porque un control que cambia de sitio entre documentos obliga
 * a buscarlo cada vez.
 */
export function controlesDelLector(capacidades: ReaderCapabilities): ControlDelLector[] {
  const controles: ControlDelLector[] = [];

  if (capacidades.typography) controles.push('tipografia');
  if (capacidades.internalSearch) controles.push('busqueda');
  if (capacidades.textSelection) controles.push('seleccion');

  // Anotar necesita poder anclar a algo que sobreviva: sin selección de texto,
  // un resaltado no tiene a qué agarrarse. Anotar una página entera de cómic
  // llega y traerá su propia capacidad; adelantar el control ahora
  // sería ofrecer algo que no existe.
  if (capacidades.textSelection) controles.push('anotaciones');

  // Lo que no se re-pagina se pinta como imagen, y una imagen hay que encajarla
  // en la ventana. Es la condición contraria a las demás: aquí el control
  // aparece porque una capacidad **falta**.
  if (!capacidades.reflow) controles.push('ajuste-de-pagina');

  // Leer en voz alta necesita **texto que decir**, no audio sincronizado: lo
  // segundo es la narración que trae el propio libro y esto es sintetizar la que
  // no trae. Un escaneado sin OCR se queda sin control, y `motivoSinTexto` dice
  // por qué en lugar de dejar al usuario preguntándoselo.
  if (capacidades.textExtraction) controles.push('lectura-en-voz-alta');

  if (capacidades.synchronizedAudio) controles.push('audio');

  return controles;
}

/**
 * Indica si un control debe pintarse.
 *
 * Existe para que un componente no tenga que buscar dentro del array y para que
 * la pregunta se escriba siempre igual.
 */
export function seMuestra(capacidades: ReaderCapabilities, control: ControlDelLector): boolean {
  return controlesDelLector(capacidades).includes(control);
}

/**
 * Motivo por el que un documento no se puede buscar ni leer en voz alta.
 *
 * Devuelve `null` cuando sí se puede. No es lo mismo que ocultar el control: un
 * PDF escaneado sin OCR **se le dice al usuario**, porque para quien usa un
 * lector de pantalla la diferencia entre "no hay texto" y "la aplicación no
 * responde" lo es todo.
 */
export function motivoSinTexto(capacidades: ReaderCapabilities): 'sin-capa-de-texto' | null {
  return capacidades.textExtraction ? null : 'sin-capa-de-texto';
}

/**
 * Cómo hay que marcar una anotación según dónde haya quedado al abrir.
 *
 * Tres estados y no dos, porque son tres situaciones distintas para el usuario:
 *
 * * `ninguna`: está donde la dejó. No se marca nada; marcar lo normal enseña a
 *   ignorar las marcas.
 * * `reanclada`: el documento cambió y se la encontró por la cita. Está bien
 *   situada, pero saber que el documento cambió es cosa suya.
 * * `aproximada`: no había ni fragmento ni cita, así que se colocó por
 *   progresión y **puede estar en otro párrafo**. Esta es la que ADR-004 obliga
 *   a marcar visiblemente.
 */
export type MarcaDeAnclaje = 'ninguna' | 'reanclada' | 'aproximada';

/**
 * Marca que corresponde a una anotación reanclada.
 */
export function marcaDeAnclaje(anotacion: AnchoredAnnotation): MarcaDeAnclaje {
  if (anotacion.warn) return 'aproximada';
  if (anotacion.relocated) return 'reanclada';
  return 'ninguna';
}

/**
 * Si un valor de `src` es una ruta del sistema de archivos y no una dirección.
 *
 * # Por qué hace falta preguntarlo
 *
 * El marcado de un capítulo llega del runtime con las referencias ya resueltas:
 * `<img src="images/x.jpg">` no lo puede cargar el webview, así que el runtime
 * extrae el recurso a su caché y **pone la ruta del archivo extraído**. Esa ruta
 * no es una dirección: puesta tal cual en un `src`, el webview la resuelve
 * contra el origen de la aplicación y pide `http://localhost:1420/home/...`,
 * que no existe. El síntoma es un 404 por imagen y **ninguna ilustración de
 * ningún libro a la vista**.
 *
 * Quien convierte una ruta en algo que el webview sepa pedir es `rutaDeImagen`,
 * y vive en el puente porque depende del protocolo del webview, que el runtime
 * no tiene por qué conocer. Esta función decide **a qué** hay que aplicárselo.
 *
 * # Por qué se decide por la forma y no por una marca
 *
 * Porque el runtime devuelve una ruta del sistema, y cómo se escribe una ruta
 * del sistema depende de la plataforma: absoluta con barra en Unix, con letra de
 * unidad en Windows. Marcar el valor obligaría al runtime a saber que hay un
 * webview al otro lado, que es justo la dependencia que no queremos.
 *
 * Lo demás se deja como está: un ancla local (`#nota-3`), una ruta relativa que
 * el saneador no supo resolver, y cualquier cosa que ya lleve esquema —incluida
 * una que ya se convirtió— no se tocan. Así aplicar esto dos veces al mismo
 * marcado no lo estropea.
 */
export function esRutaDeArchivo(valor: string): boolean {
  const limpio = valor.trim();
  if (limpio.length === 0) return false;
  // La unidad de Windows se mira **antes** que el esquema: `C:/Users/...` es una
  // ruta, y la comprobación de esquema la daría por una dirección de protocolo
  // `c`. Un esquema de una sola letra no existe en la práctica.
  if (/^[a-zA-Z]:[\\/]/.test(limpio)) return true;
  // Con esquema ya es una dirección, y `//` es un protocolo heredado.
  if (limpio.startsWith('//') || /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(limpio)) return false;
  return limpio.startsWith('/');
}
