/**
 * La voz neuronal local, vista desde la interfaz.
 *
 * Lo que se comprueba aquí no es que suene —eso lo mide `voz_neuronal.rs` sobre
 * el modelo real— sino las tres cosas que la interfaz puede hacer mal:
 *
 * 1. **Que no se ofrezca cuando la máquina no la mueve.** El runtime lo mide y lo
 *    dice; si esto lo ignorara, la narración se cortaría cada dos párrafos.
 * 2. **Que la voz elegida viaje.** El modelo tiene tres, y pedir la síntesis sin
 *    decir cuál devolvería siempre la primera sin que nadie lo notara, porque
 *    también suena bien.
 * 3. **Que adelantar el tramo siguiente no toque lo que suena.** Es lo que evita
 *    el silencio entre frases, y el adelanto tarda lo mismo que la frase.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Locator } from '$lib/bindings';

const vista = vi.hoisted(() => vi.fn());
const sintetizar = vi.hoisted(() => vi.fn());
vi.mock('$core/ipc', () => ({
  synthesizeNeuralSpeech: sintetizar,
  rutaDeImagen: (ruta: string) => `asset://localhost/${encodeURIComponent(ruta)}`
}));
// El estado del modelo vive en el módulo diferido del puente: solo
// lo carga quien elige esta voz, para no ocupar el paquete de arranque.
vi.mock('$core/ipc/voz', () => ({ neuralVoice: vista }));

const { VozNeuronalLocal } = await import('./neuronal');

const LOCATOR: Locator = {
  version: 1,
  documentId: 'doc',
  anchors: { cfi: '/6/2!/4/2/1:0' },
  progress: 0
} as unknown as Locator;

const VOCES = [
  { id: 'ef_dora', name: 'Dora', language: 'es' },
  { id: 'em_alex', name: 'Alex', language: 'es' }
];

function respuesta(extra: Record<string, unknown> = {}) {
  return {
    id: 'kokoro-82m',
    name: 'Kokoro 82M',
    author: 'hexgrad',
    licence: 'Apache-2.0',
    host: 'huggingface.co',
    downloadBytes: 327_212_226,
    memoryBytes: 629_145_600,
    installed: true,
    ready: true,
    reason: null,
    factor: 0.4,
    voices: VOCES,
    ...extra
  };
}

let fuentes: string[] = [];

class AudioDoble {
  onended: (() => void) | null = null;
  onerror: (() => void) | null = null;
  constructor(public src: string) {
    fuentes.push(src);
  }
  play() {
    return Promise.resolve();
  }
  pause() {}
  removeAttribute() {}
  load() {}
}

beforeEach(() => {
  fuentes = [];
  vista.mockReset();
  vista.mockResolvedValue(respuesta());
  sintetizar.mockReset();
  sintetizar.mockResolvedValue('/home/quien/.cache/nb-books/voz-neuronal/sha256-abc.wav');

  globalThis.Audio = AudioDoble as unknown as typeof Audio;
  globalThis.fetch = vi.fn(async () => ({ blob: async () => new Blob(['audio']) })) as never;
  globalThis.URL.createObjectURL = vi.fn(() => 'blob:nb-books/uno');
  globalThis.URL.revokeObjectURL = vi.fn();
});

const TRAMO = { bloque: 0, locator: LOCATOR, texto: 'En un lugar de la Mancha.' };
const escuchaMuda = { alAvanzar: () => {}, alTerminarTramo: () => {}, alFallar: () => {} };

describe('la voz neuronal local', () => {
  it('no sale a la red, y lo declara', async () => {
    const voz = new VozNeuronalLocal();
    const declaracion = await voz.preparar();

    expect(declaracion.necesitaRed).toBe(false);
    expect(declaracion.disponible).toBe(true);
    expect(voz.voces()).toEqual([
      { id: 'ef_dora', nombre: 'Dora', idioma: 'es' },
      { id: 'em_alex', nombre: 'Alex', idioma: 'es' }
    ]);
  });

  // Antes esta prueba exigía lo contrario: que una máquina lenta
  // dejara la voz sin ofrecer. Ese día se fijó que la aplicación recomienda y no
  // obliga, y esto es la regresión de ese cambio.
  it('en una máquina lenta se ofrece igual, avisando de que se cortará', async () => {
    vista.mockResolvedValue(respuesta({ ready: false, reason: 'DEMASIADO_LENTA', factor: 1.7 }));
    const voz = new VozNeuronalLocal();
    const declaracion = await voz.preparar();

    expect(declaracion.disponible).toBe(true);
    expect(declaracion.advertencia).toBe('lenta');
    expect(declaracion.factor).toBe(1.7);
    expect(declaracion.motivo).toBeUndefined();
  });

  it('sin modelo instalado sí se declara no disponible, porque no hay nada que elegir', async () => {
    vista.mockResolvedValue(respuesta({ ready: false, reason: 'NO_INSTALADA', installed: false }));
    const voz = new VozNeuronalLocal();

    expect((await voz.preparar()).disponible).toBe(false);
  });

  it('no se declara disponible si el runtime no contesta', async () => {
    vista.mockRejectedValue(new Error('el puente no responde'));
    const voz = new VozNeuronalLocal();

    expect((await voz.preparar()).disponible).toBe(false);
    expect(voz.voces()).toEqual([]);
  });

  it('manda la voz elegida, y no la primera del catálogo', async () => {
    const voz = new VozNeuronalLocal();
    await voz.preparar();
    voz.hablar(TRAMO as never, { voz: 'em_alex', velocidad: 1.25 } as never, escuchaMuda as never);
    await vi.waitFor(() => expect(fuentes).toHaveLength(1));

    expect(sintetizar).toHaveBeenCalledWith('En un lugar de la Mancha.', 'em_alex', 1.25);
  });

  it('adelanta el tramo siguiente sin tocar lo que suena', async () => {
    const voz = new VozNeuronalLocal();
    await voz.preparar();
    voz.hablar(TRAMO as never, { voz: null, velocidad: 1 } as never, escuchaMuda as never);
    await vi.waitFor(() => expect(fuentes).toHaveLength(1));

    voz.adelantar(
      { ...TRAMO, bloque: 1, texto: 'Macondo era entonces una aldea.' } as never,
      { voz: null, velocidad: 1 } as never
    );

    expect(sintetizar).toHaveBeenCalledTimes(2);
    expect(fuentes).toHaveLength(1);
  });
});
