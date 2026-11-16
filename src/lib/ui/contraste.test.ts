import { describe, expect, it } from 'vitest';

import { contrasteHex, interpretarHex, luminanciaRelativa, relacionDeContraste } from './contraste';

describe('relación de contraste WCAG', () => {
  it('el blanco sobre negro da el máximo de 21 y un color contra sí mismo da 1', () => {
    // Los dos extremos de la escala. Si estos se mueven, la fórmula está mal y
    // todas las demás cifras que produce este módulo son inservibles.
    expect(contrasteHex('#ffffff', '#000000')).toBeCloseTo(21, 2);
    expect(contrasteHex('#000000', '#ffffff')).toBeCloseTo(21, 2);
    expect(contrasteHex('#1a5545', '#1a5545')).toBeCloseTo(1, 6);
  });

  it('coincide con los valores publicados por WCAG para colores conocidos', () => {
    // #767676 sobre blanco es el gris más claro que alcanza 4,5:1, y es el
    // ejemplo canónico de la propia norma.
    expect(contrasteHex('#767676', '#ffffff')).toBeGreaterThanOrEqual(4.5);
    expect(contrasteHex('#777777', '#ffffff')).toBeLessThan(4.5);
  });

  it('el orden de los colores no altera el resultado', () => {
    expect(contrasteHex('#1a5545', '#f5f7f4')).toBeCloseTo(contrasteHex('#f5f7f4', '#1a5545'), 10);
  });

  it('la luminancia respeta el peso de cada canal', () => {
    // El verde pesa mucho más que el azul: es lo que hace que un azul oscuro
    // pase el contraste donde un verde del mismo brillo aparente no lo hace.
    const verde = luminanciaRelativa({ r: 0, g: 255, b: 0 });
    const rojo = luminanciaRelativa({ r: 255, g: 0, b: 0 });
    const azul = luminanciaRelativa({ r: 0, g: 0, b: 255 });
    expect(verde).toBeGreaterThan(rojo);
    expect(rojo).toBeGreaterThan(azul);
    expect(verde + rojo + azul).toBeCloseTo(1, 6);
  });

  it('acepta la forma de tres dígitos como equivalente a la de seis', () => {
    expect(interpretarHex('#fff')).toEqual(interpretarHex('#ffffff'));
    expect(interpretarHex('#1a3')).toEqual(interpretarHex('#11aa33'));
  });

  it('distingue un color inválido de un color malo en lugar de lanzar', () => {
    // Quien recorre una hoja de estilos necesita saltarse lo que no es un color,
    // no detenerse: `var(--otro)` y `transparent` aparecen constantemente.
    expect(interpretarHex('var(--nb-text)')).toBeNull();
    expect(interpretarHex('transparent')).toBeNull();
    expect(interpretarHex('#12345')).toBeNull();
    expect(() => contrasteHex('#zzz', '#fff')).toThrow();
  });

  it('el contraste crece al separar las luminancias', () => {
    const cerca = relacionDeContraste({ r: 100, g: 100, b: 100 }, { r: 120, g: 120, b: 120 });
    const lejos = relacionDeContraste({ r: 20, g: 20, b: 20 }, { r: 220, g: 220, b: 220 });
    expect(lejos).toBeGreaterThan(cerca);
  });
});
