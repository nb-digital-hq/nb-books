/**
 * Criterio de aceptación de la., la mitad que vive aquí.
 *
 * "Abrir un cómic no muestra controles de tipografía; abrir un EPUB sí."
 */

import { describe, expect, it } from 'vitest';

import type { AnchoredAnnotation, ReaderCapabilities } from '$lib/bindings';

import {
  controlesDelLector,
  esRutaDeArchivo,
  marcaDeAnclaje,
  motivoSinTexto,
  seMuestra
} from './reader';

/** Lo que declara `reader/epub.rs`. */
const EPUB: ReaderCapabilities = {
  textSelection: true,
  internalSearch: true,
  reflow: true,
  typography: true,
  synchronizedAudio: false,
  textExtraction: true
};

/** Lo que declara `reader/comic.rs`: todo falso, y eso es información. */
const COMIC: ReaderCapabilities = {
  textSelection: false,
  internalSearch: false,
  reflow: false,
  typography: false,
  synchronizedAudio: false,
  textExtraction: false
};

/** Lo que declara `reader/pdf.rs` para un escaneo sin OCR. */
const PDF_ESCANEADO: ReaderCapabilities = {
  textSelection: false,
  internalSearch: false,
  reflow: false,
  typography: false,
  synchronizedAudio: false,
  textExtraction: false
};

/** Lo que declara `reader/pdf.rs` para un PDF con capa de texto. */
const PDF_CON_TEXTO: ReaderCapabilities = {
  textSelection: true,
  internalSearch: true,
  reflow: false,
  typography: false,
  synchronizedAudio: false,
  textExtraction: true
};

describe('controles del lector', () => {
  it('abrir un cómic no muestra controles de tipografía y abrir un EPUB sí', () => {
    expect(seMuestra(EPUB, 'tipografia')).toBe(true);
    expect(seMuestra(COMIC, 'tipografia')).toBe(false);
  });

  it('lo no soportado se omite, no se muestra deshabilitado', () => {
    // La lista es lo que se pinta. Un control ausente de aquí no existe en la
    // pantalla: no hay ningún estado intermedio en el que aparezca en gris.
    expect(controlesDelLector(COMIC)).not.toContain('busqueda');
    expect(controlesDelLector(COMIC)).not.toContain('seleccion');
    expect(controlesDelLector(COMIC)).not.toContain('anotaciones');
  });

  it('un cómic conserva el único control que sí le corresponde', () => {
    // Lo que no se re-pagina se pinta como imagen, y una imagen hay que
    // encajarla. Es la única condición que aparece por ausencia.
    expect(controlesDelLector(COMIC)).toEqual(['ajuste-de-pagina']);
  });

  it('un EPUB no ofrece ajuste de página porque se re-pagina solo', () => {
    expect(controlesDelLector(EPUB)).not.toContain('ajuste-de-pagina');
    expect(controlesDelLector(EPUB)).toEqual([
      'tipografia',
      'busqueda',
      'seleccion',
      'anotaciones',
      // leer en voz alta depende de que haya texto que decir, no de que
      // el libro traiga su propia narración.
      'lectura-en-voz-alta'
    ]);
  });

  it('el orden de los controles no depende del documento', () => {
    const conAudio: ReaderCapabilities = { ...EPUB, synchronizedAudio: true };
    const controles = controlesDelLector(conAudio);

    // Un control que cambia de sitio entre documentos obliga a buscarlo cada
    // vez, y para quien navega con teclado cambia el número de tabulaciones.
    expect(controles.indexOf('tipografia')).toBeLessThan(controles.indexOf('busqueda'));
    expect(controles.indexOf('busqueda')).toBeLessThan(controles.indexOf('audio'));
  });

  it('un documento sin capa de texto lo declara en lugar de callarlo', () => {
    // Ocultar el control de búsqueda no basta: para quien usa un lector de
    // pantalla, la diferencia entre "no hay texto" y "no responde" lo es todo.
    expect(motivoSinTexto(PDF_ESCANEADO)).toBe('sin-capa-de-texto');
    expect(motivoSinTexto(EPUB)).toBeNull();
  });

  it('ningún control aparece dos veces', () => {
    const controles = controlesDelLector(EPUB);
    expect(new Set(controles).size).toBe(controles.length);
  });
});

describe('leer en voz alta depende de que haya texto que decir', () => {
  it('un EPUB y un PDF con capa de texto lo ofrecen', () => {
    expect(seMuestra(EPUB, 'lectura-en-voz-alta')).toBe(true);
    expect(seMuestra(PDF_CON_TEXTO, 'lectura-en-voz-alta')).toBe(true);
  });

  it('un escaneado sin OCR no lo ofrece, y el motivo se declara', () => {
    // Las dos mitades del criterio a la vez: el control no se pinta,
    // y no se pinta **diciendo por qué**. Ocultarlo sin más dejaría a quien
    // depende de la voz sin saber si el fallo es del documento o suyo.
    expect(seMuestra(PDF_ESCANEADO, 'lectura-en-voz-alta')).toBe(false);
    expect(motivoSinTexto(PDF_ESCANEADO)).toBe('sin-capa-de-texto');
  });

  it('no depende del audio sincronizado, que es otra cosa', () => {
    // El audio sincronizado es la narración que trae el propio libro. Esto es
    // sintetizar la que no trae, y confundirlas dejaría sin voz a casi todos los
    // documentos, que es justo lo contrario de un suelo de accesibilidad.
    expect(EPUB.synchronizedAudio).toBe(false);
    expect(seMuestra(EPUB, 'lectura-en-voz-alta')).toBe(true);
    expect(seMuestra(EPUB, 'audio')).toBe(false);
  });

  it('un cómic no lo ofrece: no hay texto', () => {
    expect(seMuestra(COMIC, 'lectura-en-voz-alta')).toBe(false);
  });
});

describe('marcado de anotaciones reancladas', () => {
  /** Lo que devuelve el runtime, con lo que no mira esta función omitido. */
  function anclada(relocated: boolean, warn: boolean): AnchoredAnnotation {
    return { relocated, warn } as AnchoredAnnotation;
  }

  it('una anotación que sigue donde estaba no se marca', () => {
    // Marcar lo normal enseña a ignorar las marcas, y entonces la que sí
    // importa pasa desapercibida.
    expect(marcaDeAnclaje(anclada(false, false))).toBe('ninguna');
  });

  it('una reanclada por cita se marca sin alarmar', () => {
    // Está bien situada; lo que cambió es el documento, y eso es cosa suya.
    expect(marcaDeAnclaje(anclada(true, false))).toBe('reanclada');
  });

  it('una colocada por progresión se marca como aproximada', () => {
    // Es la que ADR-004 obliga a marcar visiblemente: puede estar en otro
    // párrafo.
    expect(marcaDeAnclaje(anclada(true, true))).toBe('aproximada');
  });
});

describe('qué src del libro hay que convertir en dirección', () => {
  it('una ruta absoluta de Unix es un archivo', () => {
    // Es lo que devuelve el runtime tras extraer el recurso a su caché.
    expect(esRutaDeArchivo('/home/alguien/.cache/nb-books/recursos/abc.jpg')).toBe(true);
  });

  it('una ruta de Windows con unidad también', () => {
    expect(esRutaDeArchivo('C:\\Users\\alguien\\AppData\\Local\\NB Books\\Cache\\a.png')).toBe(
      true
    );
    expect(esRutaDeArchivo('C:/Users/alguien/a.png')).toBe(true);
  });

  it('lo que ya lleva esquema se deja en paz, y por eso convertir dos veces no rompe', () => {
    // `asset:` es lo que devuelve la conversión: aplicarla otra vez produciría
    // una dirección con la anterior dentro.
    expect(esRutaDeArchivo('asset://localhost/%2Fhome%2Falguien%2Fa.jpg')).toBe(false);
    expect(esRutaDeArchivo('http://asset.localhost/a.jpg')).toBe(false);
    expect(esRutaDeArchivo('data:image/png;base64,AAAA')).toBe(false);
  });

  it('un ancla local no es un archivo', () => {
    expect(esRutaDeArchivo('#nota-3')).toBe(false);
  });

  it('una relativa que nadie resolvió se deja como está, no se inventa una raíz', () => {
    // Convertirla produciría una ruta del sistema que no existe, y el fallo
    // pasaría de "no se ve la imagen" a "se pide un archivo de otro sitio".
    expect(esRutaDeArchivo('images/portada.jpg')).toBe(false);
    expect(esRutaDeArchivo('../Images/x.png')).toBe(false);
  });

  it('un protocolo heredado no es un archivo', () => {
    expect(esRutaDeArchivo('//ejemplo.org/x.jpg')).toBe(false);
  });

  it('vacío o en blanco, nada que convertir', () => {
    expect(esRutaDeArchivo('')).toBe(false);
    expect(esRutaDeArchivo('   ')).toBe(false);
  });
});
