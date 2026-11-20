import { describe, expect, it } from 'vitest';

import { queda } from './ritmo';

const ritmo = (porMinuto: number) => ({
  progressionPerMinute: porMinuto,
  minutes: 50,
  sessions: 2
});

describe('cuánto falta de un libro', () => {
  it('sin ritmo no se inventa nada', () => {
    expect(queda(null, 0.3)).toBeNull();
    expect(queda(ritmo(0.0025), null)).toBeNull();
  });

  it('con el libro terminado no queda nada que decir', () => {
    expect(queda(ritmo(0.0025), 0.995)).toBeNull();
  });

  it('en minutos, de cinco en cinco y nunca menos de cinco', () => {
    // 0,1 por leer a 0,0025 por minuto: cuarenta minutos.
    expect(queda(ritmo(0.0025), 0.9)).toEqual({ unidad: 'minutos', cuanto: 40 });
    expect(queda(ritmo(0.01), 0.985)).toEqual({ unidad: 'minutos', cuanto: 5 });
  });

  it('en horas, de media en media, en cuanto pasa de una', () => {
    // La mitad a 0,0025 por minuto: doscientos minutos, tres horas y media.
    expect(queda(ritmo(0.0025), 0.5)).toEqual({ unidad: 'horas', cuanto: 3.5 });
  });
});
