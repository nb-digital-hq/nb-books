import { describe, expect, it } from 'vitest';
import { porcentajeLeido } from './progreso';

describe('el porcentaje leído es uno solo en toda la aplicación', () => {
  it('trunca: nunca dice que se leyó más de lo leído', () => {
    expect(porcentajeLeido(0.0134)).toBe(1);
    expect(porcentajeLeido(0.4669)).toBe(46);
    expect(porcentajeLeido(0.996)).toBe(99);
  });
  it('el 100 % es solo lo terminado, y sin progreso no hay cifra', () => {
    expect(porcentajeLeido(1)).toBe(100);
    expect(porcentajeLeido(null)).toBeNull();
    expect(porcentajeLeido(Number.NaN)).toBeNull();
  });
});
