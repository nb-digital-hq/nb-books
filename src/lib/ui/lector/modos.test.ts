/**
 * **Criterio de aceptación:** cada modo cumple el contraste exigido,
 * comprobado automáticamente, y la animación desaparece con movimiento
 * reducido.
 *
 * El contraste no se afirma: se calcula sobre los colores del catálogo, igual
 * que `tema.test.ts` lo calcula sobre `app.css`. Un modo que se quede corto no
 * se puede publicar aunque se vea bien en la pantalla de quien lo escribió.
 */

import { describe, expect, it } from 'vitest';

import { contrasteHex, UMBRAL_TEXTO } from '../contraste';
import { fondoFiltrado } from './fondo';
import {
  filtroDe,
  KIT_POR_DEFECTO,
  MODO_POR_DEFECTO,
  MODOS,
  modoPorNombre,
  partirBionica,
  seAnima
} from './modos';

describe('los modos de lectura', () => {
  for (const modo of MODOS) {
    it(`${modo.nombre}: el texto alcanza ${UMBRAL_TEXTO}:1 sobre su fondo`, () => {
      expect(contrasteHex(modo.texto, modo.fondo)).toBeGreaterThanOrEqual(UMBRAL_TEXTO);
    });
  }

  it('cada uno es un perfil completo, no una paleta', () => {
    // ADR-012, compromiso 5: un modo fija fondo, texto, brillo y temperatura a
    // la vez. Con dos valores sueltos, el usuario tendría que componer el resto
    // a mano, que es la fricción que el compromiso existe para evitar.
    for (const modo of MODOS) {
      expect(modo.fondo).toMatch(/^#[0-9a-f]{6}$/i);
      expect(modo.texto).toMatch(/^#[0-9a-f]{6}$/i);
      expect(modo.brillo).toBeGreaterThan(0.5);
      expect(modo.brillo).toBeLessThanOrEqual(1);
      expect(modo.temperatura).toBeGreaterThanOrEqual(0);
      expect(modo.temperatura).toBeLessThanOrEqual(1);
      // El nombre visible ya no vive aquí: es del diccionario, y
      // que ninguno se quede sin él lo comprueba `i18n.test.ts` en los tres
      // idiomas, que es más de lo que esta prueba podía comprobar.
    }
  });

  it('el catálogo tiene los seis que ADR-012 nombra', () => {
    expect(MODOS.map((m) => m.nombre)).toEqual([
      'dia',
      'sepia',
      'nocturno',
      'altoContraste',
      'papelMonocromo',
      'papelAtenuado'
    ]);
  });

  it('un nombre desconocido cae al modo por defecto en lugar de dejar el lector sin colores', () => {
    expect(modoPorNombre('inventado').nombre).toBe(MODO_POR_DEFECTO);
    expect(modoPorNombre('sepia').nombre).toBe('sepia');
  });
});

describe('la emulación de brillo y temperatura', () => {
  it('el modo que no altera nada no compone ninguna capa', () => {
    // Un filtro que no hace nada sigue costando una capa por fotograma, y el
    // presupuesto de fotograma es el que manda aquí.
    expect(filtroDe(modoPorNombre('dia'))).toBeNull();
  });

  it('los modos cálidos y monocromos sí la componen', () => {
    expect(filtroDe(modoPorNombre('sepia'))).toContain('sepia(');
    expect(filtroDe(modoPorNombre('papelMonocromo'))).toContain('grayscale(1)');
    expect(filtroDe(modoPorNombre('papelAtenuado'))).toContain('brightness(');
  });
});

describe('el kit de lectura accesible', () => {
  it('no toca nada mientras no se active', () => {
    expect(KIT_POR_DEFECTO.familia).toBe('ninguna');
    expect(KIT_POR_DEFECTO.bionica).toBe(false);
    expect(KIT_POR_DEFECTO.regla).toBe(false);
    expect(KIT_POR_DEFECTO.focoDeLinea).toBe(false);
  });

  it('la lectura biónica marca menos proporción cuanto más larga es la palabra', () => {
    expect(partirBionica('de')).toEqual(['d', 'e']);
    expect(partirBionica('casa')).toEqual(['ca', 'sa']);
    expect(partirBionica('extraordinario')).toEqual(['extrao', 'rdinario']);
  });

  it('una palabra de una letra no se parte', () => {
    expect(partirBionica('y')).toEqual(['y', '']);
    expect(partirBionica('')).toEqual(['', '']);
  });

  it('la animación de página desaparece por completo con movimiento reducido', () => {
    // Regla 7 de ADR-012: una animación que no cabe se elimina, no se recorta.
    expect(seAnima({ ...KIT_POR_DEFECTO, animarPagina: true }, true)).toBe(false);
    expect(seAnima({ ...KIT_POR_DEFECTO, animarPagina: true }, false)).toBe(true);
    expect(seAnima({ ...KIT_POR_DEFECTO, animarPagina: false }, false)).toBe(false);
  });
});

/**
 * El fondo filtrado se calcula aquí para no filtrar en cada fotograma, y
 * tiene que ser el mismo color que da el filtro del motor: detrás de la barra va
 * este y detrás del texto el del filtro, y si no coincidieran se vería una costura.
 *
 * Los valores del motor se leyeron el 2026-09-25 en WebKitGTK, pintando cada fondo
 * con su filtro y leyendo el píxel. Se admite 1/255 de diferencia por el redondeo.
 */
describe('el fondo filtrado es el que da el motor', () => {
  const MOTOR: Record<string, [number, number, number]> = {
    sepia: [255, 242, 211],
    nocturno: [18, 20, 22],
    altoContraste: [0, 0, 0],
    papelMonocromo: [230, 230, 230],
    papelAtenuado: [241, 222, 188]
  };
  for (const [nombre, esperado] of Object.entries(MOTOR)) {
    it(nombre, () => {
      const color = fondoFiltrado(modoPorNombre(nombre));
      const calculado = [1, 3, 5].map((i) => Number.parseInt(color.slice(i, i + 2), 16));
      for (const [i, c] of calculado.entries()) {
        expect(Math.abs(c - (esperado[i] as number))).toBeLessThanOrEqual(1);
      }
    });
  }

  it('sin filtro, es el fondo tal cual', () => {
    expect(fondoFiltrado(modoPorNombre('dia'))).toBe(modoPorNombre('dia').fondo);
  });
});
