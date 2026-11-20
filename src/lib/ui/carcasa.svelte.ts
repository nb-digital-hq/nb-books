/**
 * Estado compartido de la carcasa: la clase de ancho vigente.
 *
 * Vive en un contexto y no en un módulo global porque puede haber más de una
 * carcasa montada a la vez en las pruebas, y porque un estado global de
 * disposición es exactamente lo que hace que un componente deje de poder
 * probarse aislado.
 */

import { getContext, setContext } from 'svelte';

import { type ClaseDeAncho, clasificarAncho } from './anchos';

const CLAVE = Symbol('nb.carcasa');

export interface ContextoDeCarcasa {
  /** Clase de la carcasa entera. Decide dónde va la navegación. */
  readonly clase: ClaseDeAncho;
  /**
   * Clase de la **columna de contenido**, sin la barra lateral.
   *
   * Es la que decide si un panel se acopla o se superpone, y no la de la
   * carcasa. El panel no compite por el espacio con la barra lateral: compite
   * con el contenido. Medirlo sobre la ventana entera dejaba, en una ventana de
   * 720, doscientos ocho píxeles de contenido junto a un panel de doscientos
   * ochenta y ocho: el contenido se cortaba y aparecía una barra de
   * desplazamiento horizontal dentro de la vista.
   */
  readonly claseDelContenido: ClaseDeAncho;
}

export function proveerCarcasa(contexto: ContextoDeCarcasa): void {
  setContext(CLAVE, contexto);
}

/**
 * Devuelve la clase de ancho vigente.
 *
 * Fuera de una carcasa devuelve `amplio`: un componente montado suelto, en una
 * prueba o en un banco de componentes, debe verse entero y no en su forma
 * colapsada.
 */
export function usarCarcasa(): ContextoDeCarcasa {
  return (
    getContext<ContextoDeCarcasa | undefined>(CLAVE) ?? {
      clase: 'amplio',
      claseDelContenido: 'amplio'
    }
  );
}

/** Ancho inicial, antes de que el observador de tamaño haya medido nada. */
export function claseInicial(): ClaseDeAncho {
  const ancho = globalThis.innerWidth;
  return typeof ancho === 'number' ? clasificarAncho(ancho) : 'amplio';
}
