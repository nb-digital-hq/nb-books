/**
 * Criterio de aceptación de la., la mitad que vive aquí.
 *
 * "Un PDF escaneado aparece marcado como no buscable, y la interfaz lo explica."
 *
 * Lo segundo es lo que se comprueba aquí: que de cada motivo salga una
 * explicación distinta, y que solo una de ellas ofrezca OCR.
 */

import { describe, expect, it } from 'vitest';

import type { SearchStatus } from '$lib/bindings';

import { accionSobreBusqueda, mensajeDeBusqueda, sePuedeBuscar } from './search';

const PENDIENTE: SearchStatus = { state: 'pending' };
const BUSCABLE: SearchStatus = { state: 'searchable' };
const ESCANEO: SearchStatus = { state: 'notSearchable', reason: 'NO_TEXT_LAYER' };
const COMIC: SearchStatus = { state: 'notSearchable', reason: 'IMAGE_ONLY' };
const DESCONECTADO: SearchStatus = { state: 'notSearchable', reason: 'FILE_UNAVAILABLE' };

describe('explicación de por qué un documento no se puede buscar', () => {
  it('un escaneo sin OCR se explica y se le ofrece OCR', () => {
    expect(sePuedeBuscar(ESCANEO)).toBe(false);
    expect(mensajeDeBusqueda(ESCANEO)).toBe('busqueda.sin-capa-de-texto');
    expect(accionSobreBusqueda(ESCANEO)).toBe('ocr');
  });

  it('a un cómic no se le ofrece OCR', () => {
    // Convertir en texto unos dibujos no significa nada, y ofrecerlo haría
    // que el usuario lo intentara y no pasara nada.
    expect(mensajeDeBusqueda(COMIC)).toBe('busqueda.sin-texto-imagen');
    expect(accionSobreBusqueda(COMIC)).toBe('ninguna');
  });

  it('un volumen desconectado se explica como algo que se arregla solo', () => {
    expect(mensajeDeBusqueda(DESCONECTADO)).toBe('busqueda.archivo-no-disponible');
    expect(accionSobreBusqueda(DESCONECTADO)).toBe('esperar');
  });

  it('lo pendiente no es lo mismo que lo no buscable', () => {
    // Mientras la biblioteca se indexa, decir "no se puede buscar" sería
    // mentir. "Aún no" es verdad.
    expect(sePuedeBuscar(PENDIENTE)).toBe(false);
    expect(mensajeDeBusqueda(PENDIENTE)).toBe('busqueda.pendiente');
    expect(mensajeDeBusqueda(PENDIENTE)).not.toBe(mensajeDeBusqueda(ESCANEO));
    expect(accionSobreBusqueda(PENDIENTE)).toBe('esperar');
  });

  it('un documento buscable no necesita explicación ni acción', () => {
    expect(sePuedeBuscar(BUSCABLE)).toBe(true);
    expect(accionSobreBusqueda(BUSCABLE)).toBe('ninguna');
  });

  it('un motivo desconocido no rompe la pantalla', () => {
    // Un runtime más nuevo que el frontend es una situación normal durante
    // una actualización.
    const futuro = { state: 'notSearchable', reason: 'ALGO_QUE_NO_EXISTE' } as SearchStatus;
    expect(() => mensajeDeBusqueda(futuro)).not.toThrow();
    expect(accionSobreBusqueda(futuro)).toBe('ninguna');
  });
});
