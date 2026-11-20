/**
 * La narración que trae el propio libro, por capítulos.
 *
 * Suena con el mismo reproductor que lo preparado de la voz: al acabar un capítulo
 * se pide el siguiente que traiga narración y se sigue, como pasa una persona la
 * página sin dejar de leer.
 *
 * # Por qué el estado es del módulo y no del panel
 *
 * Porque el panel de voz es momentáneo y la narración no: sigue sonando con el
 * panel cerrado, y al volver a abrirlo tiene que saber qué suena y si se acabó.
 * Vive aquí y no en la página porque la página va en el paquete de arranque, que
 * tiene un presupuesto de 250 KB (`performance-budgets.md`, sección 4.11), y esto
 * solo lo usa quien abre un libro narrado.
 *
 * Contrato: `docs/architecture/reader-contracts.md`, sección 5.5.
 */

import { synchronizedAudio } from '$core/ipc/lector';
import type { Locator, NavigationEntry } from '$lib/bindings';
import type { ReproductorDePreparado } from '$lib/voz/reproductor.svelte';

import { indicePlano } from './posicion';

/** Lo que el panel enseña de la narración. */
export const narracion = $state<{
  /** El documento al que se refiere lo de abajo. */
  fileId: string | null;
  /** El capítulo que suena, si lo que está cargado es la narración del libro. */
  href: string | null;
  sinCapitulo: boolean;
  terminada: boolean;
}>({ fileId: null, href: null, sinCapitulo: false, terminada: false });

/** Lo que hace falta para sonar y para mover el lector. */
export interface Contexto {
  reproductor: ReproductorDePreparado;
  fileId: string;
  entradas: readonly NavigationEntry[];
  /** Lleva el lector a un sitio. */
  alIr: (locator: Locator) => void;
  /** Se llama antes de sonar, para callar la voz en vivo: dos voces no se entienden. */
  alEmpezar: () => void;
}

/** Olvida lo de otro documento: la narración es de un libro. */
export function delDocumento(fileId: string): void {
  if (narracion.fileId === fileId) return;
  narracion.fileId = fileId;
  narracion.href = null;
  narracion.sinCapitulo = false;
  narracion.terminada = false;
}

/** Suena desde donde se va leyendo, si el capítulo trae narración. */
export async function escuchar(contexto: Contexto, desde: Locator): Promise<void> {
  delDocumento(contexto.fileId);
  narracion.sinCapitulo = false;
  narracion.terminada = false;
  const pista = await synchronizedAudio(contexto.fileId, desde.href).catch(() => null);
  if (!pista) {
    narracion.sinCapitulo = true;
    return;
  }
  contexto.alEmpezar();
  await contexto.reproductor.cargar(pista);
  contexto.reproductor.alTerminar = () => void seguir(contexto, pista.href);
  narracion.href = pista.href;
  contexto.reproductor.desde(desde);
}

/** El capítulo siguiente que trae narración, cargado y sonando desde su principio. */
export async function seguir(contexto: Contexto, acabado: string): Promise<void> {
  const plano = indicePlano(contexto.entradas);
  const desde = plano.findIndex((e) => e.locator.href === acabado);
  const vistos = new Set([acabado]);
  for (const entrada of plano.slice(desde + 1)) {
    if (vistos.has(entrada.locator.href)) continue;
    vistos.add(entrada.locator.href);
    const pista = await synchronizedAudio(contexto.fileId, entrada.locator.href).catch(() => null);
    if (!pista) continue;
    contexto.alIr(entrada.locator);
    await contexto.reproductor.cargar(pista);
    contexto.reproductor.alTerminar = () => void seguir(contexto, pista.href);
    narracion.href = pista.href;
    contexto.reproductor.desde(entrada.locator);
    return;
  }
  narracion.href = null;
  narracion.terminada = true;
}
