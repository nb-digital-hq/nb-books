/**
 * **El criterio de aceptación, segunda mitad:** la carcasa se retira
 * **entera** en modo inmersivo.
 *
 * Se comprueba sobre el marcado que sale de verdad, no sobre un booleano del
 * estado: el defecto que esto vigila es olvidarse de una zona, y un booleano
 * bien puesto con una zona que no lo mira sigue dejando la barra superior encima
 * del texto.
 *
 * Se renderiza con `svelte/server`, que produce el HTML sin necesitar navegador.
 * Lo que no cubre son los efectos y los eventos, que no corren en ese modo: el
 * teclado se prueba en `teclado.test.ts` y los eventos reales del webview son
 * la superficie de lectura.
 */

import { createRawSnippet } from 'svelte';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';

import type { ReaderCapabilities, SessionInfo } from '$lib/bindings';

import AppShell from '../AppShell.svelte';
import ReaderView from './ReaderView.svelte';

const trozo = (texto: string) => createRawSnippet(() => ({ render: () => `<p>${texto}</p>` }));

const EPUB: ReaderCapabilities = {
  textSelection: true,
  internalSearch: true,
  reflow: true,
  typography: true,
  synchronizedAudio: false,
  textExtraction: true
};

const COMIC: ReaderCapabilities = {
  textSelection: false,
  internalSearch: false,
  reflow: false,
  typography: false,
  synchronizedAudio: false,
  textExtraction: false
};

function sesion(capabilities: ReaderCapabilities): SessionInfo {
  return {
    fileId: 'f1',
    editionId: 'e1',
    title: 'Una obra',
    mediaType: capabilities.reflow ? 'application/epub+zip' : 'application/vnd.comicbook+zip',
    capabilities,
    navigation: {
      entries: [
        {
          title: 'Capítulo 1',
          locator: {
            version: 1,
            href: 'OEBPS/c1.xhtml',
            mediaType: 'application/xhtml+xml',
            title: 'Capítulo 1',
            locations: { fragments: ['#char=0'], progression: 0, totalProgression: 0 },
            text: { highlight: 'Muchos años después' }
          },
          children: []
        }
      ]
    },
    locator: {
      version: 1,
      href: 'OEBPS/c1.xhtml',
      mediaType: 'application/xhtml+xml',
      title: 'Capítulo 1',
      locations: { fragments: ['#char=0'], progression: 0, totalProgression: 0 },
      text: { highlight: 'Muchos años después' }
    },
    approximate: false,
    language: 'es',
    fresh: true
  };
}

function carcasa(inmersivo: boolean): string {
  return render(AppShell, {
    props: {
      inmersivo,
      banner: trozo('barra'),
      navegacion: trozo('destinos'),
      principal: trozo('documento'),
      paneles: trozo('panel')
    }
  }).body;
}

describe('la carcasa se retira entera al leer', () => {
  it('fuera de la inmersión están las cuatro zonas', () => {
    const html = carcasa(false);

    expect(html).toContain('<header');
    expect(html).toContain('<nav');
    expect(html).toContain('<main');
    expect(html).toContain('panel');
  });

  it('en inmersión no queda ni la barra, ni la navegación, ni los paneles', () => {
    const html = carcasa(true);

    expect(html).not.toContain('<header');
    expect(html).not.toContain('<nav');
    expect(html).not.toContain('barra');
    expect(html).not.toContain('destinos');
    expect(html).not.toContain('panel');
  });

  it('el contenido y la vía de escape se quedan', () => {
    const html = carcasa(true);

    // El texto es lo único que queda, y el enlace de salto sigue llevando a él:
    // retirar la carcasa no puede dejar a nadie sin forma de llegar al
    // contenido.
    expect(html).toContain('<main');
    expect(html).toContain('documento');
    expect(html).toContain('href="#nb-principal"');
  });

  it('la inmersión se anuncia, porque quedarse sin carcasa es información', () => {
    expect(carcasa(true)).toContain('role="status"');
    // Y el anuncio dice por dónde se vuelve.
    expect(carcasa(true)).toContain('Escape');
  });
});

describe('los controles del lector salen de las capacidades', () => {
  it('un EPUB ofrece tipografía y búsqueda', () => {
    const html = render(ReaderView, { props: { sesion: sesion(EPUB) } }).body;

    expect(html).toContain('Ajustes del Libro');
    expect(html).toContain('Buscar en el Documento');
  });

  it('sin nada elegido no se ofrece anotar, porque pulsarlo no haría nada', () => {
    // Es lo que hacía: la barra pintaba anotar, resaltar,
    // consultar y extender con el mismo aspecto que los demás, y los cuatro
    // eran inertes hasta que hubiera una selección. Un control que no responde
    // al pulsarlo enseña que la aplicación no responde.
    const html = render(ReaderView, { props: { sesion: sesion(EPUB) } }).body;

    expect(html).not.toContain('Escribir una Nota');
    expect(html).not.toContain('Resaltar');
    expect(html).not.toContain('Consultar la Selección');
  });

  it('la barra va agrupada por familias, no en una sola hilera', () => {
    const html = render(ReaderView, { props: { sesion: sesion(EPUB) } }).body;

    // Veintidós iconos seguidos, del mismo tamaño y sin separación, no se leen:
    // se recorren enteros cada vez.
    expect(html).toContain('data-grupo="navegacion"');
    expect(html).toContain('data-grupo="paneles"');
    expect(html).toContain('data-grupo="presentacion"');
    // La salida ya no tiene botón propio: la cruz de la carcasa cierra el
    // documento, y dos cruces que parecían lo mismo confundían.
    expect(html).not.toContain('data-grupo="salida"');
  });

  it('la barra cabe en una fila a 1100 px: moverse por párrafos es solo tecla', () => {
    const html = render(ReaderView, { props: { sesion: sesion(EPUB) } }).body;
    expect(html).not.toContain('Párrafo Anterior');
    expect(html).not.toContain('Volver (Escape)');
  });

  it('guardar la nota no ocupa un botón permanente en la barra', () => {
    // Su tecla existe y hace falta —Ctrl+Intro mientras se escribe—, pero el
    // botón solo tendría sentido con el campo abierto, y ese campo trae el suyo.
    const html = render(ReaderView, { props: { sesion: sesion(EPUB) } }).body;

    expect(html).not.toContain('Guardar la Nota');
  });

  it('dice por dónde va la lectura, que es lo que no decía en ninguna parte', () => {
    const html = render(ReaderView, { props: { sesion: sesion(EPUB) } }).body;

    expect(html).toContain('role="progressbar"');
    expect(html).toContain('aria-label="Progreso de la lectura"');
  });

  it('un cómic no los ofrece, ni siquiera atenuados', () => {
    const html = render(ReaderView, { props: { sesion: sesion(COMIC) } }).body;

    expect(html).not.toContain('Ajustes del Libro');
    expect(html).not.toContain('Buscar en el Documento');
    expect(html).not.toContain('Escribir una Nota');
    // Lo que no depende del texto sigue estando.
    expect(html).toContain('Poner un Marcador');
    expect(html).toContain('Lectura Inmersiva');
    // Y ningún control queda deshabilitado: se omite, no se atenúa.
    expect(html).not.toContain('disabled');
  });

  it('cada control lleva su atajo en el nombre accesible', () => {
    const html = render(ReaderView, { props: { sesion: sesion(EPUB) } }).body;

    // Los atajos son visibles, que es lo que permite aprenderlos sin manual.
    expect(html).toContain('Poner un Marcador (B)');
    expect(html).toContain('¿Dónde estoy? (P)');
    expect(html).toContain('Buscar en el Documento (Ctrl+F)');
  });

  it('cuenta con una región viva para el paradero accesible', () => {
    const html = render(ReaderView, { props: { sesion: sesion(EPUB) } }).body;

    expect(html).toContain('nb-anuncio');
    expect(html).toContain('role="status"');
    expect(html).toContain('aria-live="polite"');
  });

  it('el recordatorio de teclas nombra la salida y las acciones, no una lista entera', () => {
    // En inmersión no queda ningún control a la vista: hay que decir por dónde
    // se vuelve. Cinco teclas y no dieciséis, porque un recordatorio que hay que
    // leer entero no se lee.
    const html = render(ReaderView, { props: { sesion: sesion(EPUB) } }).body;

    // Fuera de la inmersión no está: se muestra al entrar y se retira solo.
    expect(html).not.toContain('nb-recordatorio');
  });

  it('una posición aproximada se avisa en lugar de disimularse', () => {
    const aproximada = { ...sesion(EPUB), approximate: true };
    const html = render(ReaderView, { props: { sesion: aproximada } }).body;

    expect(html).toContain('aproximad');
    expect(html).toContain('role="status"');
  });
});
