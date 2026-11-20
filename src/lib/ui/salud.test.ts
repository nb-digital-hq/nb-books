/**
 * **Criterio de aceptación:** simular el fallo del motor de PDF oculta
 * lo que depende de él y deja el resto operativo, sin pantalla en blanco.
 *
 * Se comprueba sobre la función que decide, no mirando la pantalla: es lo que
 * permite ejercitar los siete estados en lugar de los dos que uno recuerda.
 */

import { describe, expect, it } from 'vitest';

import type { ComponentState, ComponentStatus } from '$lib/bindings';

import { avisos, COMPONENTES, hayAlgoQueDecir, seOfrece } from './salud';

const estado = (component: string, state: ComponentState): ComponentStatus => ({
  component,
  state,
  reasonCode: null
});

describe('qué ofrece la interfaz según la salud', () => {
  it('el fallo de un componente no se lleva por delante a los demás', () => {
    const estados = [
      estado(COMPONENTES.base, 'READY'),
      estado(COMPONENTES.lector, 'FAILED'),
      estado(COMPONENTES.busqueda, 'READY')
    ];

    expect(seOfrece(estados, COMPONENTES.lector)).toBe(false);
    // Y lo demás sigue en pie: es la regla 1 de la Constitución.
    expect(seOfrece(estados, COMPONENTES.base)).toBe(true);
    expect(seOfrece(estados, COMPONENTES.busqueda)).toBe(true);
  });

  it('un componente degradado sigue ofreciéndose, porque a medias es más que nada', () => {
    expect(seOfrece([estado(COMPONENTES.busqueda, 'DEGRADED')], COMPONENTES.busqueda)).toBe(true);
  });

  it('sin noticias se ofrece, en lugar de esconder media aplicación mientras arranca', () => {
    expect(seOfrece([], COMPONENTES.lector)).toBe(true);
    expect(seOfrece([estado(COMPONENTES.base, 'READY')], COMPONENTES.ia)).toBe(true);
  });

  it('lo que no está no se ofrece, ni instalado ni soportado ni parado', () => {
    for (const caido of ['NOT_INSTALLED', 'NOT_SUPPORTED', 'STOPPED', 'FAILED'] as const) {
      expect(seOfrece([estado(COMPONENTES.ia, caido)], COMPONENTES.ia)).toBe(false);
    }
  });
});

describe('qué se le cuenta al usuario', () => {
  it('lo que no tiene arreglo no se anuncia', () => {
    // Esta plataforma no lo tendrá nunca: recordárselo cada vez es recordarle
    // algo que no puede cambiar.
    expect(avisos([estado(COMPONENTES.ia, 'NOT_SUPPORTED')])).toEqual([]);
    expect(avisos([estado(COMPONENTES.ia, 'STOPPED')])).toEqual([]);
    expect(avisos([estado(COMPONENTES.base, 'READY')])).toEqual([]);
  });

  it('lo que sí se anuncia va de más grave a menos', () => {
    const lista = avisos([
      estado(COMPONENTES.ia, 'NOT_INSTALLED'),
      estado(COMPONENTES.busqueda, 'DEGRADED'),
      estado(COMPONENTES.lector, 'FAILED')
    ]);

    expect(lista.map((a) => a.estado)).toEqual(['FAILED', 'DEGRADED', 'NOT_INSTALLED']);
  });

  it('solo es accionable lo que el usuario puede arreglar', () => {
    const [falta] = avisos([estado(COMPONENTES.ia, 'NOT_INSTALLED')]);
    const [roto] = avisos([estado(COMPONENTES.lector, 'FAILED')]);

    expect(falta?.accionable).toBe(true);
    expect(roto?.accionable).toBe(false);
  });

  it('el indicador de la barra solo aparece cuando hay algo que decir', () => {
    expect(hayAlgoQueDecir([estado(COMPONENTES.base, 'READY')])).toBe(false);
    expect(hayAlgoQueDecir([estado(COMPONENTES.lector, 'FAILED')])).toBe(true);
  });
});
