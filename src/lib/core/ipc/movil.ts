/**
 * Comandos de NB Books Mobile (ADR-016): el servidor del teléfono, sus
 * teléfonos y Leer en el PC.
 *
 * Aparte de `index.ts` por lo mismo que `ficha.ts`: lo que hay en `index.ts` entra
 * en el paquete de arranque, que tiene un presupuesto de 250 KB, y esto solo lo
 * usan Ajustes, el lector y la carcasa cuando llega una orden del teléfono.
 *
 * Contrato: `docs/architecture/mobile-contract.md`.
 */

import type { MobileDevice, MobilePairing, MobileServerState } from '$lib/bindings';

import { call } from './index';

/** Si el servidor del teléfono escucha, y dónde. */
export function mobileState(): Promise<MobileServerState> {
  return call<MobileServerState>('mobile_state');
}

/** Enciende o apaga el servidor del teléfono, y lo recuerda. */
export function mobileSetEnabled(enabled: boolean): Promise<MobileServerState> {
  return call<MobileServerState>('mobile_set_enabled', { enabled });
}

/** Un QR y un código nuevos para emparejar. Enciende el servidor. */
export function mobilePair(): Promise<MobilePairing> {
  return call<MobilePairing>('mobile_pair');
}

/** Los teléfonos emparejados. */
export function mobileDevices(): Promise<MobileDevice[]> {
  return call<MobileDevice[]>('mobile_devices');
}

/** Revoca un teléfono: deja de poder conectarse en ese momento. */
export function mobileRevoke(id: string): Promise<void> {
  return call<void>('mobile_revoke', { id });
}

/** Corta Leer en el PC. */
export function mobileReleaseControl(): Promise<void> {
  return call<void>('mobile_release_control');
}

/** Dónde está el lector, para el teléfono que lo maneja. */
export function readerPublishState(reader: Record<string, unknown>): Promise<void> {
  return call<void>('reader_publish_state', { reader });
}
