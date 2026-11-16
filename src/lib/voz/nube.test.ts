/**
 * **Por qué existe esta prueba, con nombre y apellidos.**
 *
 * Porque la voz de un servicio se sintetizaba bien, el archivo se escribía bien,
 * y no sonaba nada. El motor real en Linux es WebKitGTK, y **no reproduce
 * medios servidos por un esquema propio** como el de los recursos de la
 * aplicación: las imágenes sí, porque las decodifica el propio motor, pero el
 * audio se lo entrega a GStreamer, que no sabe hablar ese esquema y responde
 * `MediaError` 4, fuente no soportada. Medido en WebKitGTK: por la dirección
 * directa `play()` se rechaza con `NotSupportedError`, y como blob suena entero.
 *
 * El síntoma era el peor posible: no fallaba nada visible. La narración pedía
 * el primer tramo, lo sintetizaba, no sonaba, y como `onended` no llegaba nunca,
 * se quedaba ahí parada sin decir por qué.
 *
 * La suite de interfaz corre en Chromium, donde la dirección directa funciona,
 * así que **no puede cazar esta regresión**. Esto sí: comprueba la propiedad que
 * arregla el defecto, que es que lo que se le enchufa al audio no es la
 * dirección del archivo.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Locator } from '$lib/bindings';

const sintetizar = vi.hoisted(() => vi.fn());
vi.mock('$core/ipc', () => ({
  synthesizeSpeech: sintetizar,
  rutaDeImagen: (ruta: string) => `asset://localhost/${encodeURIComponent(ruta)}`
}));

const { VozEnLaNube } = await import('./nube');

const LOCATOR: Locator = {
  version: 1,
  documentId: 'doc',
  anchors: { cfi: '/6/2!/4/2/1:0' },
  progress: 0
} as unknown as Locator;

/** Lo que le llega al elemento de audio, que es lo que se está comprobando. */
let fuentes: string[] = [];
let revocadas: string[] = [];
let reproducciones: Array<() => void> = [];

class AudioDoble {
  onended: (() => void) | null = null;
  onerror: (() => void) | null = null;
  constructor(public src: string) {
    fuentes.push(src);
  }
  play() {
    reproducciones.push(() => this.onended?.());
    return Promise.resolve();
  }
  pause() {}
  removeAttribute() {}
  load() {}
}

beforeEach(() => {
  fuentes = [];
  revocadas = [];
  reproducciones = [];
  sintetizar.mockReset();
  sintetizar.mockResolvedValue('/home/quien/.cache/nb-books/voz/sha256-abc.mp3');

  globalThis.Audio = AudioDoble as unknown as typeof Audio;
  globalThis.fetch = vi.fn(async () => ({ blob: async () => new Blob(['audio']) })) as never;
  globalThis.URL.createObjectURL = vi.fn(() => 'blob:nb-books/uno');
  globalThis.URL.revokeObjectURL = vi.fn((url: string) => {
    revocadas.push(url);
  });
});

const TRAMO = { bloque: 0, locator: LOCATOR, texto: 'En un lugar de la Mancha.' };
const OPCIONES = { velocidad: 1, voz: null };
const escuchaMuda = { alAvanzar: () => {}, alTerminarTramo: () => {}, alFallar: () => {} };

describe('la voz de un servicio, en el motor que la aplicación usa de verdad', () => {
  it('no le enchufa al audio la dirección del archivo, que en WebKitGTK no suena', async () => {
    const voz = new VozEnLaNube();
    voz.hablar(TRAMO as never, OPCIONES as never, escuchaMuda as never);
    await vi.waitFor(() => expect(fuentes).toHaveLength(1));

    expect(fuentes[0]).toBe('blob:nb-books/uno');
    expect(fuentes[0]).not.toContain('asset://');
    expect(globalThis.fetch).toHaveBeenCalledWith(
      'asset://localhost/%2Fhome%2Fquien%2F.cache%2Fnb-books%2Fvoz%2Fsha256-abc.mp3'
    );
  });

  it('devuelve la memoria del audio al terminar el tramo', async () => {
    const voz = new VozEnLaNube();
    let termino = false;
    voz.hablar(
      TRAMO as never,
      OPCIONES as never,
      {
        ...escuchaMuda,
        alTerminarTramo: () => {
          termino = true;
        }
      } as never
    );
    await vi.waitFor(() => expect(reproducciones).toHaveLength(1));

    reproducciones[0]?.();

    expect(termino).toBe(true);
    // Un blob vive hasta que alguien lo suelta, y aquí no se cierra ninguna
    // pestaña: sin esto, cada frase de cada libro se quedaría en memoria.
    expect(revocadas).toEqual(['blob:nb-books/uno']);
  });

  /**
   * **Lo que esta prueba no cubre, dicho como tal.** Comprueba que el proveedor
   * sabe adelantar y que la máquina emite el tramo siguiente
   * (`narracion.test.ts`), pero **no que alguien los una**: quitar la llamada a
   * `adelantar` de `narrador.svelte.ts` deja las 48 pruebas en verde. Se
   * comprobó rompiéndolo. Es el hueco de siempre —capacidad construida y sin
   * conectar—, y `check-puente.mjs` no lo ve porque solo cruza comandos del
   * puente, no llamadas entre módulos de la interfaz. Aquí se deja escrito en
   * lugar de aparentar una cobertura que no hay.
   */
  it('va pidiendo el tramo siguiente mientras suena el actual', async () => {
    const voz = new VozEnLaNube();
    voz.hablar(TRAMO as never, OPCIONES as never, escuchaMuda as never);
    await vi.waitFor(() => expect(fuentes).toHaveLength(1));

    voz.adelantar(
      { ...TRAMO, bloque: 1, texto: 'Macondo era entonces una aldea.' } as never,
      OPCIONES as never
    );

    // Medido contra un servidor real, generar una frase tarda casi lo que dura:
    // sin adelantarla, entre frase y frase queda un silencio igual de largo.
    expect(sintetizar).toHaveBeenCalledTimes(2);
    expect(sintetizar).toHaveBeenLastCalledWith('Macondo era entonces una aldea.', 1);
    // Y no toca lo que está sonando: no crea audio ni cambia la generación.
    expect(fuentes).toHaveLength(1);
  });

  it('si el adelanto falla, no se entera quien escucha', async () => {
    const voz = new VozEnLaNube();
    let fallo = false;
    voz.hablar(
      TRAMO as never,
      OPCIONES as never,
      {
        ...escuchaMuda,
        alFallar: () => {
          fallo = true;
        }
      } as never
    );
    await vi.waitFor(() => expect(fuentes).toHaveLength(1));

    sintetizar.mockRejectedValueOnce(new Error('el servidor no responde'));
    voz.adelantar(TRAMO as never, OPCIONES as never);
    await vi.waitFor(() => expect(sintetizar).toHaveBeenCalledTimes(2));

    expect(fallo).toBe(false);
  });

  it('parar suelta también el audio que estaba sonando', async () => {
    const voz = new VozEnLaNube();
    voz.hablar(TRAMO as never, OPCIONES as never, escuchaMuda as never);
    await vi.waitFor(() => expect(fuentes).toHaveLength(1));

    voz.parar();

    expect(revocadas).toEqual(['blob:nb-books/uno']);
  });
});
