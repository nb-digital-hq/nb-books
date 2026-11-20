import { describe, expect, it, vi } from 'vitest';
import { coalescer, espaciar } from './coalescer';

describe('las relecturas repetidas se juntan', () => {
  it('diez peticiones mientras una está en marcha se quedan en dos', async () => {
    let veces = 0;
    const releer = coalescer(async () => {
      veces += 1;
      await new Promise((r) => setTimeout(r, 20));
    }, 5);
    await Promise.all(Array.from({ length: 10 }, () => releer()));
    expect(veces).toBe(2);
  });

  it('espaciar deja pasar una sola llamada por intervalo', () => {
    vi.useFakeTimers();
    const tarea = vi.fn();
    const llamar = espaciar(tarea, 1000);
    llamar();
    llamar();
    vi.advanceTimersByTime(1001);
    llamar();
    expect(tarea).toHaveBeenCalledTimes(2);
    vi.useRealTimers();
  });
});
