import { describe, expect, it } from 'vitest';

import type { ExtensionReadingMode } from '$lib/bindings';

import { UMBRAL_TEXTO } from '../contraste';
import { aModo, legibles } from './modoDeExtension';

/** Uno de los tres del paquete que se publica, para que si deja de valer se entere esta prueba. */
const PERGAMINO: ExtensionReadingMode = {
  id: 'lat.nbdigitallabs.books.personalizacion.papel/pergamino',
  extension: 'lat.nbdigitallabs.books.personalizacion.papel',
  name: 'Pergamino',
  background: '#f2e8d5',
  text: '#33291b',
  brightness: 96,
  warmth: 45,
  monochrome: false
};

describe('un modo de lectura que aporta una extensión', () => {
  it('llega al lector con el porcentaje pasado a fracción', () => {
    // El manifiesto se escribe a mano y lleva enteros; la superficie compone con
    // fracciones. La conversión vive en un sitio para que no haya dos.
    const modo = aModo(PERGAMINO);
    expect(modo.brillo).toBeCloseTo(0.96);
    expect(modo.temperatura).toBeCloseTo(0.45);
    expect(modo.nombre).toBe(PERGAMINO.id);
    // El nombre que se ve es el que puso su autor, y no se traduce.
    expect(modo.etiqueta).toBe('Pergamino');
  });

  it('el paquete que se publica cumple el contraste que exige el sistema de diseño', () => {
    const { modos, descartados } = legibles([PERGAMINO]);
    expect(descartados).toEqual([]);
    expect(modos).toHaveLength(1);
  });

  it('uno que no se lee no se ofrece, y se dice cuánto medía', () => {
    // Un modo ilegible en la lista es una trampa: quien lo elige descubre el
    // problema justo cuando ya no puede leer para arreglarlo.
    const { modos, descartados } = legibles([{ ...PERGAMINO, text: '#e8ddc8' }, PERGAMINO]);

    expect(modos.map((m) => m.etiqueta)).toEqual(['Pergamino']);
    expect(descartados).toHaveLength(1);
    expect(descartados[0]?.medido).toBeLessThan(UMBRAL_TEXTO);
    expect(descartados[0]?.umbral).toBe(UMBRAL_TEXTO);
  });

  it('se mide con el umbral del texto y no con el de interfaz', () => {
    // Lo que un modo pinta es el libro: fondo y texto de la superficie de
    // lectura, que es lo que alguien mira durante horas.
    // 3,5 de contraste: pasaría el umbral de interfaz y no el de texto, que es
    // justo la diferencia que se quiere comprobar.
    const justo = { ...PERGAMINO, background: '#ffffff', text: '#8a8a8a' };
    expect(legibles([justo]).descartados).toHaveLength(1);
  });
});
