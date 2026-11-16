import { describe, expect, it } from 'vitest';

import {
  ANCHO_MINIMO,
  type ClaseDeAncho,
  clasificarAncho,
  elPanelSeSuperpone,
  laNavegacionVaAbajo,
  panelesAcoplados
} from './anchos';

const CLASES: ClaseDeAncho[] = ['compacto', 'medio', 'amplio'];

describe('clases de ancho de la carcasa', () => {
  it('el punto de corte pertenece a la clase que empieza, no a la que termina', () => {
    // Un punto de corte ambiguo produce el defecto clásico: la interfaz
    // parpadea entre dos disposiciones al arrastrar el borde de la ventana.
    expect(clasificarAncho(ANCHO_MINIMO.medio - 1)).toBe('compacto');
    expect(clasificarAncho(ANCHO_MINIMO.medio)).toBe('medio');
    expect(clasificarAncho(ANCHO_MINIMO.amplio - 1)).toBe('medio');
    expect(clasificarAncho(ANCHO_MINIMO.amplio)).toBe('amplio');
  });

  it('las tres clases cubren todo ancho posible y no se solapan', () => {
    const vistas = new Set<ClaseDeAncho>();
    for (let px = 0; px <= 2600; px += 1) vistas.add(clasificarAncho(px));
    expect([...vistas].sort()).toEqual([...CLASES].sort());
  });

  it('un ancho imposible se trata como compacto en lugar de romper la disposición', () => {
    // Puede llegar de un observador de tamaño sobre un elemento aún sin medir.
    // Se responde compacto y no amplio porque compacto es la única disposición
    // que se ve bien a cualquier tamaño: equivocarse hacia ahí no rompe nada.
    expect(clasificarAncho(Number.NaN)).toBe('compacto');
    expect(clasificarAncho(Number.POSITIVE_INFINITY)).toBe('compacto');
    expect(clasificarAncho(-1)).toBe('compacto');
    expect(clasificarAncho(0)).toBe('compacto');
  });

  it('ningún panel se acopla en la clase compacta', () => {
    // Es el criterio de aceptación: un panel que no sabe colapsar no
    // entra. Cero acoplados no es cero paneles, es cero paneles con hueco propio.
    expect(panelesAcoplados('compacto')).toBe(0);
    expect(elPanelSeSuperpone('compacto')).toBe(true);
  });

  it('la cantidad de paneles acoplados nunca decrece al ensanchar', () => {
    const cantidades = CLASES.map(panelesAcoplados);
    const ordenadas = [...cantidades].sort((a, b) => a - b);
    expect(cantidades).toEqual(ordenadas);
    expect(cantidades).toEqual([0, 1, 2]);
  });

  it('la navegación solo baja al borde inferior en la clase compacta', () => {
    expect(laNavegacionVaAbajo('compacto')).toBe(true);
    expect(laNavegacionVaAbajo('medio')).toBe(false);
    expect(laNavegacionVaAbajo('amplio')).toBe(false);
  });

  it('superponerse y acoplarse son excluyentes', () => {
    for (const clase of CLASES) {
      expect(elPanelSeSuperpone(clase)).toBe(panelesAcoplados(clase) === 0);
    }
  });
});
