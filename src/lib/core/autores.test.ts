import { describe, expect, it } from 'vitest';
import { autoresBreves } from './autores';

describe('los autores de una tarjeta se abrevian cuando son muchos', () => {
  it('hasta tres se enseñan enteros', () => {
    expect(autoresBreves('A, B, C', 'y {n} más')).toBe('A, B, C');
  });
  it('con más, los dos primeros y cuántos quedan', () => {
    expect(autoresBreves('A, B, C, D, E', 'y {n} más')).toBe('A, B y 3 más');
  });
  it('sin autores, nada', () => {
    expect(autoresBreves(null, 'y {n} más')).toBe('');
  });
});
