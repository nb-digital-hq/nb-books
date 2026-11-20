/**
 * Las reglas de ADR-013 que gobiernan la navegación primaria.
 *
 * La de "el Lector no se cierra al navegar" es de las que se rompen sin que
 * nadie lo note al escribir el código, y de las que el usuario nota
 * inmediatamente: consultar algo en la biblioteca a mitad de un capítulo y
 * volver al principio del libro.
 */

import { describe, expect, it } from 'vitest';

import { ICONOS } from '../iconos';
import {
  type ContextoDeNavegacion,
  DESTINOS,
  destinosDelPie,
  destinosPrincipales,
  destinosVisibles,
  destinoTrasCerrar,
  navegarCierraElDocumento,
  SIN_CONTEXTO
} from './destinos';

const TODO: ContextoDeNavegacion = {
  hayDocumentoAbierto: true,
  hayCatalogos: true
};

describe('los destinos contextuales', () => {
  it('una instalación nueva solo muestra los que existen siempre', () => {
    expect(destinosVisibles(SIN_CONTEXTO).map((d) => d.destino)).toEqual([
      'inicio',
      'biblioteca',
      'estudio',
      'ajustes'
    ]);
  });

  it('el Lector aparece en cuanto hay un documento abierto', () => {
    const con = destinosVisibles({ ...SIN_CONTEXTO, hayDocumentoAbierto: true });
    expect(con.map((d) => d.destino)).toContain('lector');
    // Y en su sitio, no al final: el orden de la barra no cambia porque aparezca
    // un destino, o habría que volver a buscarlos todos.
    expect(con.findIndex((d) => d.destino === 'lector')).toBe(2);
  });

  it('Descubrir espera a que haya un catálogo', () => {
    expect(destinosVisibles(SIN_CONTEXTO).map((d) => d.destino)).not.toContain('descubrir');
    expect(destinosVisibles(TODO).map((d) => d.destino)).toContain('descubrir');
  });

  it('no se muestra atenuado: o está o no está', () => {
    // La regla 5 de ADR-013 prohíbe el elemento permanentemente en gris, así
    // que la lista no tiene forma de expresar "presente pero deshabilitado".
    for (const entrada of DESTINOS) {
      expect(Object.keys(entrada)).not.toContain('deshabilitado');
    }
  });

  it('cada destino tiene un icono del catálogo, porque no hay emojis', () => {
    for (const entrada of DESTINOS) {
      expect(entrada.icono in ICONOS, `${entrada.destino} sin icono`).toBe(true);
    }
  });

  it('Ajustes va al pie y no estorba al recorrido principal', () => {
    expect(destinosDelPie(TODO).map((d) => d.destino)).toEqual(['ajustes']);
    expect(destinosPrincipales(TODO).map((d) => d.destino)).not.toContain('ajustes');
  });
});

describe('el Lector no se cierra al navegar', () => {
  it('ningún destino termina la sesión de lectura', () => {
    for (const entrada of DESTINOS) {
      expect(navegarCierraElDocumento(entrada.destino)).toBe(false);
    }
  });

  it('cerrar el documento desde el Lector devuelve a Inicio', () => {
    expect(destinoTrasCerrar('lector')).toBe('inicio');
  });

  it('cerrar el documento desde otro sitio no mueve a nadie', () => {
    expect(destinoTrasCerrar('biblioteca')).toBe('biblioteca');
  });
});
