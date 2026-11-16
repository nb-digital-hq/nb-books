import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// El bus de Tauri no existe fuera del webview: se simula con oyentes guardados
// por nombre, para poder publicar desde la prueba como publicaría el runtime.
const oyentes = vi.hoisted(() => new Map<string, Array<(e: { payload: unknown }) => void>>());
const listen = vi.hoisted(() =>
  vi.fn((evento: string, oyente: (e: { payload: unknown }) => void) => {
    oyentes.set(evento, [...(oyentes.get(evento) ?? []), oyente]);
    return Promise.resolve(() => {
      oyentes.set(
        evento,
        (oyentes.get(evento) ?? []).filter((o) => o !== oyente)
      );
    });
  })
);
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

const { ANUNCIOS, acumular, escucharCambios, sinCambios } = await import('./vivo');

import type { Cambios } from './vivo';

function publicar(payload: { type: string } & Record<string, unknown>) {
  for (const oyente of oyentes.get(payload.type) ?? []) oyente({ payload });
}

describe('qué anuncio obliga a releer qué', () => {
  it('un metadato corregido es de las obras, y dice de cuál', () => {
    const cambios = sinCambios();
    acumular(cambios, { type: 'MetadataUpdated', editionId: 'e1' });
    expect([...cambios.temas]).toEqual(['obras']);
    expect([...cambios.ediciones]).toEqual(['e1']);
  });

  it('el progreso lleva su cifra, y gana la última de cada edición', () => {
    const cambios = sinCambios();
    acumular(cambios, { type: 'ProgressSaved', editionId: 'e1', progression: 0.2 });
    acumular(cambios, { type: 'ProgressSaved', editionId: 'e1', progression: 0.4 });
    acumular(cambios, { type: 'ProgressSaved', editionId: 'e2', progression: null });
    expect([...cambios.temas]).toEqual(['progreso']);
    expect(cambios.progresos.get('e1')).toBe(0.4);
    expect(cambios.progresos.get('e2')).toBeNull();
  });

  it('una colección cambiada y una extensión instalada son temas distintos', () => {
    const cambios = sinCambios();
    acumular(cambios, { type: 'CollectionsChanged', collectionId: null });
    acumular(cambios, { type: 'ExtensionsChanged' });
    expect([...cambios.temas].sort()).toEqual(['colecciones', 'extensiones']);
  });

  it('importar y terminar una importación cambian las obras', () => {
    for (const anuncio of [
      { type: 'DocumentImported', editionId: 'e9', fileId: 'f9', mediaType: 'x' },
      { type: 'DocumentDeleted', editionId: 'e9' },
      {
        type: 'BulkImportFinished',
        processed: 1,
        imported: 1,
        skipped: 0,
        rejected: 0,
        duplicates: 0,
        cancelled: false
      },
      { type: 'CoversRecovered', count: 3 }
    ] as const) {
      const cambios = sinCambios();
      acumular(cambios, anuncio);
      expect([...cambios.temas], anuncio.type).toEqual(['obras']);
    }
  });
});

describe('escuchar los anuncios', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    oyentes.clear();
  });
  afterEach(() => vi.useRealTimers());

  it('se suscribe a todos los anuncios que dicen que algo cambió', async () => {
    await escucharCambios(() => undefined);
    expect([...oyentes.keys()].sort()).toEqual([...ANUNCIOS].sort());
  });

  it('una ráfaga es un solo aviso con todo lo que traía', async () => {
    const avisos: Cambios[] = [];
    await escucharCambios((c) => avisos.push(c));

    for (let i = 0; i < 500; i += 1) {
      publicar({ type: 'DocumentImported', editionId: `e${i}`, fileId: `f${i}`, mediaType: 'x' });
    }
    publicar({ type: 'CollectionsChanged', collectionId: 'c1' });
    expect(avisos).toHaveLength(0);

    await vi.advanceTimersByTimeAsync(200);
    expect(avisos).toHaveLength(1);
    expect(avisos[0]?.ediciones.size).toBe(500);
    expect([...(avisos[0]?.temas ?? [])].sort()).toEqual(['colecciones', 'obras']);
  });

  it('una ráfaga que no para avisa igual cada poco, en lugar de callar hasta el final', async () => {
    const avisos: Cambios[] = [];
    await escucharCambios((c) => avisos.push(c), { agrupar: 150, comoMucho: 1000 });

    // Un anuncio cada cien milisegundos durante tres segundos: nunca hay 150 de
    // silencio, que es lo que pasa en una importación de horas.
    for (let t = 0; t < 3000; t += 100) {
      publicar({ type: 'DocumentImported', editionId: `e${t}`, fileId: 'f', mediaType: 'x' });
      await vi.advanceTimersByTimeAsync(100);
    }
    expect(avisos.length).toBeGreaterThanOrEqual(2);
  });

  it('darse de baja deja de avisar, también de lo que estaba por llegar', async () => {
    const avisos: Cambios[] = [];
    const baja = await escucharCambios((c) => avisos.push(c));
    publicar({ type: 'ExtensionsChanged' });
    baja();
    await vi.advanceTimersByTimeAsync(500);
    publicar({ type: 'ExtensionsChanged' });
    await vi.advanceTimersByTimeAsync(500);
    expect(avisos).toHaveLength(0);
  });
});
