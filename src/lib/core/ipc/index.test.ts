import { describe, expect, it, vi } from 'vitest';

// El puente de Tauri no existe fuera del webview: se simula para poder probar
// la capa de acceso de forma aislada, tal y como exige
// docs/developer/testing/unit.md
const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

const { NbError, componentStates, healthCheck, onDomainEvent } = await import('./index');

describe('capa de acceso al runtime', () => {
  it('devuelve el informe de salud tal y como lo emite el runtime', async () => {
    invoke.mockResolvedValueOnce({ version: '1.0.0', schemaVersion: 1 });

    await expect(healthCheck()).resolves.toEqual({ version: '1.0.0', schemaVersion: 1 });
    expect(invoke).toHaveBeenCalledWith('health_check', undefined);
  });

  it('devuelve el estado de los componentes', async () => {
    invoke.mockResolvedValueOnce([{ component: 'database', state: 'READY', reasonCode: null }]);

    await expect(componentStates()).resolves.toHaveLength(1);
    expect(invoke).toHaveBeenCalledWith('component_states', undefined);
  });

  it('traduce el error del runtime a NbError conservando el código canónico', async () => {
    invoke.mockRejectedValueOnce({ code: 'SCHEMA_MISMATCH' });

    await expect(healthCheck()).rejects.toBeInstanceOf(NbError);

    invoke.mockRejectedValueOnce({ code: 'SCHEMA_MISMATCH' });
    await expect(healthCheck()).rejects.toMatchObject({ code: 'SCHEMA_MISMATCH' });
  });

  it('clasifica como INTERNAL cualquier fallo que se salte el contrato', async () => {
    // Un fallo sin código canónico significa que algo no pasó por el tipo de
    // dominio de Rust. No se propaga su contenido: podría filtrar rutas del
    // usuario. Ver docs/architecture/ipc-contracts.md
    invoke.mockRejectedValueOnce('fallo al abrir /home/alguien/Documentos/libro.pdf');

    const fallo = await healthCheck().catch((e: unknown) => e);

    expect(fallo).toMatchObject({ code: 'INTERNAL' });
    expect(String(fallo)).not.toContain('/home/alguien');
  });

  it('entrega al consumidor solo la carga útil del evento', async () => {
    const recibidos: unknown[] = [];
    listen.mockImplementationOnce((_evento: string, handler: (e: { payload: unknown }) => void) => {
      handler({ payload: { type: 'ComponentStateChanged', component: 'database' } });
      return Promise.resolve(() => undefined);
    });

    await onDomainEvent('ComponentStateChanged', (event) => recibidos.push(event));

    expect(listen).toHaveBeenCalledWith('ComponentStateChanged', expect.any(Function));
    expect(recibidos).toEqual([{ type: 'ComponentStateChanged', component: 'database' }]);
  });
});
