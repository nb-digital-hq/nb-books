/**
 * **La regla 3 del contrato de carga**, comprobada con las respuestas
 * desordenadas a propósito.
 *
 * El caso que motiva la regla es teclear en un filtro: una petición por
 * pulsación, y la tercera contestando antes que la segunda. Sin guarda, la
 * segunda pinta encima y deja en pantalla el resultado de una consulta que ya
 * nadie escribió. Aquí se reproduce con promesas que se resuelven a mano, que
 * es la única forma de que el orden sea el del defecto y no el del azar.
 */

import { describe, expect, it, vi } from 'vitest';

import { guardaDeRespuesta } from './carga';

/** Una promesa que se resuelve cuando esta prueba lo diga. */
function aplazada<T>() {
  let resolver!: (valor: T) => void;
  let rechazar!: (motivo: unknown) => void;
  const promesa = new Promise<T>((si, no) => {
    resolver = si;
    rechazar = no;
  });
  return { promesa, resolver, rechazar };
}

describe('solo se pinta la última respuesta pedida', () => {
  it('la respuesta que llega tarde se descarta aunque traiga datos', async () => {
    const guarda = guardaDeRespuesta();
    const primera = aplazada<string>();
    const segunda = aplazada<string>();

    const unaEspera = guarda(() => primera.promesa);
    const otraEspera = guarda(() => segunda.promesa);

    // La segunda contesta antes, y la primera después: el orden del defecto.
    segunda.resolver('lo último que se escribió');
    primera.resolver('lo que se borró hace dos pulsaciones');

    expect(await otraEspera).toEqual({ vigente: true, valor: 'lo último que se escribió' });
    expect(await unaEspera).toEqual({ vigente: false });
  });

  it('la última cuenta aunque conteste la última', async () => {
    const guarda = guardaDeRespuesta();
    const primera = aplazada<string>();
    const segunda = aplazada<string>();

    const unaEspera = guarda(() => primera.promesa);
    const otraEspera = guarda(() => segunda.promesa);

    primera.resolver('vieja');
    segunda.resolver('nueva');

    expect(await unaEspera).toEqual({ vigente: false });
    expect(await otraEspera).toEqual({ vigente: true, valor: 'nueva' });
  });

  it('el fallo de una petición que ya nadie espera no se propaga', async () => {
    // Quien escribió otra cosa no tiene por qué ver el error de lo que borró:
    // sería un mensaje sobre una consulta que ya no está en pantalla.
    const guarda = guardaDeRespuesta();
    const primera = aplazada<string>();
    const segunda = aplazada<string>();

    const unaEspera = guarda(() => primera.promesa);
    const otraEspera = guarda(() => segunda.promesa);

    segunda.resolver('la que vale');
    primera.rechazar(new Error('NETWORK_FAILURE'));

    await expect(unaEspera).resolves.toEqual({ vigente: false });
    await expect(otraEspera).resolves.toEqual({ vigente: true, valor: 'la que vale' });
  });

  it('el fallo de la petición vigente sí se propaga', async () => {
    // Lo contrario sería tragarse el error de lo que la persona está mirando.
    const guarda = guardaDeRespuesta();

    await expect(guarda(() => Promise.reject(new Error('STORAGE_FAILURE')))).rejects.toThrow(
      'STORAGE_FAILURE'
    );
  });

  it('dos filtros distintos no se descartan el uno al otro', async () => {
    // Una guarda por cosa que se pide. Compartirla haría que escribir en el
    // buscador de estanterías tirara la respuesta del buscador de la biblioteca.
    const deEstanterias = guardaDeRespuesta();
    const deBiblioteca = guardaDeRespuesta();

    const una = deEstanterias(() => Promise.resolve('estanterías'));
    const otra = deBiblioteca(() => Promise.resolve('biblioteca'));

    expect(await una).toEqual({ vigente: true, valor: 'estanterías' });
    expect(await otra).toEqual({ vigente: true, valor: 'biblioteca' });
  });

  it('no llama al trabajo más veces de las que se le pide', async () => {
    const trabajo = vi.fn(() => Promise.resolve(1));
    const guarda = guardaDeRespuesta();

    await guarda(trabajo);
    await guarda(trabajo);

    expect(trabajo).toHaveBeenCalledTimes(2);
  });
});
