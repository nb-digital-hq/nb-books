/**
 * Los dos planos de personalización, en el lado que compone el libro.
 *
 * El otro lado está en `src-tauri/tests/preferencias.rs`, que comprueba que
 * tocar uno no escribe en el otro. Aquí se comprueba lo que falta para que la
 * separación sea real: que esta capa **no lee** el plano de la aplicación,
 * aunque le llegue entero.
 */

import { describe, expect, it } from 'vitest';

import type { Setting } from '$lib/bindings';

import {
  CLAVES,
  COMPOSICION_POR_DEFECTO,
  composicionDe,
  cuerpoSiguiente,
  LIMITES,
  valorDe
} from './libro';

const APLICACION: Setting[] = [
  { key: 'theme', value: 'dark', affectsFirstPaint: true, plane: 'application' },
  { key: 'font_scale', value: 2, affectsFirstPaint: true, plane: 'application' },
  { key: 'high_contrast', value: true, affectsFirstPaint: true, plane: 'application' }
];

const LIBRO: Setting[] = [
  { key: 'book_font_family', value: 'serif', affectsFirstPaint: false, plane: 'book' },
  { key: 'book_font_size', value: 24, affectsFirstPaint: false, plane: 'book' },
  { key: 'book_line_height', value: 1.9, affectsFirstPaint: false, plane: 'book' },
  { key: 'book_margin', value: 64, affectsFirstPaint: false, plane: 'book' },
  { key: 'book_line_width', value: 80, affectsFirstPaint: false, plane: 'book' },
  { key: 'book_justify', value: true, affectsFirstPaint: false, plane: 'book' },
  { key: 'book_pagination', value: 'paged', affectsFirstPaint: false, plane: 'book' },
  { key: 'book_reading_mode', value: 'nocturno', affectsFirstPaint: false, plane: 'book' },
  { key: 'book_accessible_font', value: 'dislexia', affectsFirstPaint: false, plane: 'book' },
  { key: 'book_bionic', value: true, affectsFirstPaint: false, plane: 'book' },
  { key: 'book_page_animation', value: false, affectsFirstPaint: false, plane: 'book' }
];

describe('el plano del libro', () => {
  it('sale entero de las preferencias del libro', () => {
    expect(composicionDe([...APLICACION, ...LIBRO])).toEqual({
      familia: 'serif',
      cuerpo: 24,
      interlineado: 1.9,
      margen: 64,
      ancho: 80,
      alineacion: 'justificado',
      paginado: true,
      modo: 'nocturno',
      kit: {
        familia: 'dislexia',
        bionica: true,
        regla: false,
        focoDeLinea: false,
        animarPagina: false
      }
    });
  });

  it('no lo altera ninguna preferencia de la aplicación', () => {
    // Aunque llegue el plano de la aplicación entero, y con valores extremos.
    expect(composicionDe(APLICACION)).toEqual(COMPOSICION_POR_DEFECTO);
  });

  it('una preferencia del libro mal etiquetada no se lee', () => {
    // El plano lo declara el runtime. Una clave del libro que llegara marcada
    // como de la aplicación es un error de contrato, y aquí se ignora en lugar
    // de aplicarse a medias.
    const impostora: Setting[] = [
      { key: 'book_font_size', value: 40, affectsFirstPaint: false, plane: 'application' }
    ];
    expect(composicionDe(impostora).cuerpo).toBe(COMPOSICION_POR_DEFECTO.cuerpo);
  });

  it('un valor fuera de rango se acota en lugar de dejar el texto ilegible', () => {
    const extremos: Setting[] = [
      { key: 'book_font_size', value: 900, affectsFirstPaint: false, plane: 'book' },
      { key: 'book_line_height', value: 0, affectsFirstPaint: false, plane: 'book' },
      { key: 'book_line_width', value: -5, affectsFirstPaint: false, plane: 'book' }
    ];
    const composicion = composicionDe(extremos);

    expect(composicion.cuerpo).toBe(LIMITES.cuerpo.max);
    expect(composicion.interlineado).toBe(LIMITES.interlineado.min);
    expect(composicion.ancho).toBe(LIMITES.ancho.min);
  });

  it('un valor con el tipo equivocado cae al de por defecto', () => {
    const corrupta: Setting[] = [
      { key: 'book_font_size', value: 'grande', affectsFirstPaint: false, plane: 'book' },
      { key: 'book_alignment', value: 1, affectsFirstPaint: false, plane: 'book' }
    ];
    const composicion = composicionDe(corrupta);

    expect(composicion.cuerpo).toBe(COMPOSICION_POR_DEFECTO.cuerpo);
    expect(composicion.alineacion).toBe(COMPOSICION_POR_DEFECTO.alineacion);
  });

  it('una alineación que nadie definió no llega al atributo del marcado', () => {
    // Va a parar a un `data-alineacion`, y de ahí a un selector. Dejar pasar
    // cualquier cadena sería dejar que el texto se quedara sin ninguna regla.
    const inventada: Setting[] = [
      { key: 'book_alignment', value: 'diagonal', affectsFirstPaint: false, plane: 'book' }
    ];
    expect(composicionDe(inventada).alineacion).toBe(COMPOSICION_POR_DEFECTO.alineacion);
  });

  it('quien tenía el texto justificado de antes lo sigue teniendo', () => {
    // `book_justify` es la preferencia anterior a que hubiera alineación, y es
    // la que lleva una biblioteca exportada antes. Si no se leyera, abrir esa
    // biblioteca cambiaría en silencio cómo se ve el texto.
    const antigua: Setting[] = [
      { key: 'book_justify', value: true, affectsFirstPaint: false, plane: 'book' }
    ];
    expect(composicionDe(antigua).alineacion).toBe('justificado');
  });

  it('la alineación escrita manda sobre la preferencia antigua', () => {
    // Al revés, quien tuviera justificado no habría podido quitarlo nunca desde
    // el control nuevo: es el defecto que tenía tener las dos vivas a la vez.
    const ambas: Setting[] = [
      { key: 'book_justify', value: true, affectsFirstPaint: false, plane: 'book' },
      { key: 'book_alignment', value: 'izquierda', affectsFirstPaint: false, plane: 'book' }
    ];
    expect(composicionDe(ambas).alineacion).toBe('izquierda');
  });

  it('cada valor de la composición tiene su preferencia y ninguna sobra', () => {
    expect(Object.keys(CLAVES).sort()).toEqual(Object.keys(COMPOSICION_POR_DEFECTO).sort());
  });

  it('un modo desconocido cae al de por defecto en lugar de dejar el lector sin colores', () => {
    const raro: Setting[] = [
      { key: 'book_reading_mode', value: 'inventado', affectsFirstPaint: false, plane: 'book' }
    ];
    expect(composicionDe(raro).modo).toBe(COMPOSICION_POR_DEFECTO.modo);
  });

  it('la paginación se guarda con las palabras del runtime y no con un booleano', () => {
    expect(valorDe('paginado', { ...COMPOSICION_POR_DEFECTO, paginado: true })).toBe('paged');
    expect(valorDe('paginado', COMPOSICION_POR_DEFECTO)).toBe('scroll');
    expect(valorDe('cuerpo', { ...COMPOSICION_POR_DEFECTO, cuerpo: 20 })).toBe(20);
  });
});

describe('el mando de Leer en el PC cambia el tamaño de letra', () => {
  it('de dos en dos y sin salirse de los límites', () => {
    expect(cuerpoSiguiente(18, 1)).toBe(20);
    expect(cuerpoSiguiente(18, -1)).toBe(16);
    expect(cuerpoSiguiente(LIMITES.cuerpo.max, 1)).toBe(LIMITES.cuerpo.max);
    expect(cuerpoSiguiente(LIMITES.cuerpo.min, -1)).toBe(LIMITES.cuerpo.min);
  });
});
