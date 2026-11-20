import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Setting, SettingValue } from '$lib/bindings';

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

/** Almacenamiento del webview simulado, con el mismo contrato que el real. */
class AlmacenSimulado implements Storage {
  private datos = new Map<string, string>();
  fallarAlEscribir = false;

  get length(): number {
    return this.datos.size;
  }
  clear(): void {
    this.datos.clear();
  }
  getItem(key: string): string | null {
    return this.datos.get(key) ?? null;
  }
  key(index: number): string | null {
    return [...this.datos.keys()][index] ?? null;
  }
  removeItem(key: string): void {
    this.datos.delete(key);
  }
  setItem(key: string, value: string): void {
    if (this.fallarAlEscribir) throw new Error('cuota agotada');
    this.datos.set(key, value);
  }
}

let almacen: AlmacenSimulado;

beforeEach(() => {
  almacen = new AlmacenSimulado();
  vi.stubGlobal('localStorage', almacen);
  invoke.mockReset();
});

const {
  CLAVES_PRIMER_PINTADO,
  PRIMER_PINTADO_POR_DEFECTO,
  cargarPreferencias,
  escribirCacheDeArranque,
  guardarPreferencia,
  leerCacheDeArranque,
  olvidarCacheDeArranque
} = await import('./settings');

/** Instantánea como la que devuelve el runtime. */
function instantanea(sobrescribir: Record<string, SettingValue> = {}): Setting[] {
  const base: Setting[] = [
    { key: 'theme', value: 'dark', affectsFirstPaint: true, plane: 'application' },
    { key: 'high_contrast', value: true, affectsFirstPaint: true, plane: 'application' },
    { key: 'reduced_motion', value: true, affectsFirstPaint: true, plane: 'application' },
    { key: 'font_scale', value: 1.5, affectsFirstPaint: true, plane: 'application' },
    { key: 'language', value: 'pt', affectsFirstPaint: true, plane: 'application' },
    { key: 'reading_speed', value: 220, affectsFirstPaint: false, plane: 'application' },
    { key: 'ai_provider', value: 'anthropic', affectsFirstPaint: false, plane: 'application' },
    { key: 'ai_memory_limit_mb', value: 4096, affectsFirstPaint: false, plane: 'application' },
    // El plano del libro no entra en la caché de arranque: el libro se abre
    // después del primer pintado.
    { key: 'book_font_size', value: 18, affectsFirstPaint: false, plane: 'book' },
    { key: 'book_line_height', value: 1.6, affectsFirstPaint: false, plane: 'book' }
  ];
  return base.map((s) => {
    const nuevo = sobrescribir[s.key];
    return nuevo === undefined ? s : { ...s, value: nuevo };
  });
}

describe('caché de arranque de las preferencias', () => {
  it('sin caché pinta con los valores por defecto en lugar de fallar', () => {
    expect(leerCacheDeArranque()).toEqual(PRIMER_PINTADO_POR_DEFECTO);
  });

  it('una caché corrupta no rompe el arranque: se pinta por defecto', () => {
    almacen.setItem('nb-books.first-paint.v1', '{ esto no es json');
    expect(leerCacheDeArranque()).toEqual(PRIMER_PINTADO_POR_DEFECTO);

    almacen.setItem('nb-books.first-paint.v1', '["ni esto"]');
    expect(leerCacheDeArranque()).toEqual(PRIMER_PINTADO_POR_DEFECTO);
  });

  it('un valor de la caché con otro tipo se ignora, no se pinta con él', () => {
    almacen.setItem(
      'nb-books.first-paint.v1',
      JSON.stringify({ font_scale: 'enorme', theme: 'dark' })
    );

    const leido = leerCacheDeArranque();

    expect(leido.font_scale).toBe(PRIMER_PINTADO_POR_DEFECTO.font_scale);
    expect(leido.theme).toBe('dark');
  });

  it('la caché guarda solo los cinco valores del primer pintado', () => {
    escribirCacheDeArranque(instantanea());

    const guardado = JSON.parse(almacen.getItem('nb-books.first-paint.v1') ?? '{}');

    expect(Object.keys(guardado).sort()).toEqual([...CLAVES_PRIMER_PINTADO].sort());
    expect(guardado).not.toHaveProperty('ai_provider');
    expect(guardado).not.toHaveProperty('reading_speed');
  });

  it('sin almacenamiento disponible sigue funcionando con los valores por defecto', () => {
    vi.stubGlobal('localStorage', undefined);

    expect(() => escribirCacheDeArranque(instantanea())).not.toThrow();
    expect(leerCacheDeArranque()).toEqual(PRIMER_PINTADO_POR_DEFECTO);
  });

  it('si el almacenamiento rechaza la escritura, la aplicación sigue', () => {
    almacen.fallarAlEscribir = true;

    expect(() => escribirCacheDeArranque(instantanea())).not.toThrow();
  });
});

describe('coherencia con la fuente de verdad', () => {
  /**
   * **Criterio de aceptación.** Borrar los datos del webview no pierde
   * ninguna preferencia: la caché es una copia de arranque, no la fuente.
   */
  it('borrar la caché del webview no pierde ninguna preferencia', async () => {
    invoke.mockResolvedValue(instantanea());
    await cargarPreferencias();
    expect(almacen.getItem('nb-books.first-paint.v1')).not.toBeNull();

    // El usuario limpia los datos del navegador.
    olvidarCacheDeArranque();
    almacen.clear();
    expect(leerCacheDeArranque()).toEqual(PRIMER_PINTADO_POR_DEFECTO);

    // Y al volver a arrancar, todo sigue ahí, porque estaba en SQLite.
    const recuperadas = await cargarPreferencias();

    expect(recuperadas).toEqual(instantanea());
    expect(leerCacheDeArranque()).toEqual({
      theme: 'dark',
      high_contrast: true,
      reduced_motion: true,
      font_scale: 1.5,
      language: 'pt'
    });
  });

  it('ante discrepancia gana SQLite y la caché se reescribe', async () => {
    almacen.setItem('nb-books.first-paint.v1', JSON.stringify({ theme: 'light', font_scale: 3 }));
    expect(leerCacheDeArranque().theme).toBe('light');

    invoke.mockResolvedValue(instantanea());
    await cargarPreferencias();

    expect(leerCacheDeArranque().theme).toBe('dark');
    expect(leerCacheDeArranque().font_scale).toBe(1.5);
  });

  it('escribir va primero al runtime y solo después a la caché', async () => {
    const orden: string[] = [];
    invoke.mockImplementationOnce(async () => {
      orden.push('runtime');
      return {
        key: 'theme',
        value: 'dark',
        affectsFirstPaint: true,
        plane: 'application'
      } satisfies Setting;
    });

    await guardarPreferencia('theme', 'dark', instantanea());
    orden.push('cache');

    expect(orden).toEqual(['runtime', 'cache']);
    expect(invoke).toHaveBeenCalledWith('update_setting', { key: 'theme', value: 'dark' });
    expect(leerCacheDeArranque().theme).toBe('dark');
  });

  it('si el runtime rechaza la escritura, la caché no miente', async () => {
    escribirCacheDeArranque(instantanea({ theme: 'dark' }));
    invoke.mockRejectedValueOnce({ code: 'INTERNAL' });

    await expect(guardarPreferencia('theme', 'imposible', instantanea())).rejects.toMatchObject({
      code: 'INTERNAL'
    });

    expect(leerCacheDeArranque().theme).toBe('dark');
  });

  it('la caché guarda el valor que responde el runtime, no el que se pidió', async () => {
    invoke.mockResolvedValueOnce({
      key: 'font_scale',
      value: 2,
      affectsFirstPaint: true,
      plane: 'application'
    } satisfies Setting);

    await guardarPreferencia('font_scale', 99, instantanea());

    expect(leerCacheDeArranque().font_scale).toBe(2);
  });

  it('una preferencia que no es del primer pintado no entra en la caché', async () => {
    invoke.mockResolvedValueOnce({
      key: 'ai_provider',
      value: 'anthropic',
      affectsFirstPaint: false,
      plane: 'application'
    } satisfies Setting);

    await guardarPreferencia('ai_provider', 'anthropic', instantanea());

    const guardado = almacen.getItem('nb-books.first-paint.v1');
    expect(guardado).toBeNull();
  });
});
