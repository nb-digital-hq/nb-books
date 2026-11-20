/**
 * Las tres clases de ancho de la carcasa.
 *
 * Contrato: `docs/architecture/ui-components.md`, sección 2.
 *
 * Este archivo es la **única fuente** de los puntos de corte. Las hojas de
 * estilo repiten los números porque CSS no puede leer una constante de
 * TypeScript dentro de una consulta de medios; `scripts/check-ui.mjs` comprueba
 * que no hayan divergido. Es el mismo problema que resuelve `check:bindings`
 * entre Rust y el puente, y se resuelve igual: se permite la repetición y se
 * prohíbe la deriva.
 */

/** Clase de ancho de la carcasa, de menor a mayor. */
export type ClaseDeAncho = 'compacto' | 'medio' | 'amplio';

/**
 * Ancho mínimo, en píxeles CSS, a partir del cual empieza cada clase.
 *
 * Los valores no son redondos por gusto: 720 es donde deja de caber una
 * columna de lectura cómoda junto a un panel, y 1080 donde caben dos paneles
 * sin estrangular el contenido.
 */
export const ANCHO_MINIMO: Readonly<Record<ClaseDeAncho, number>> = {
  compacto: 0,
  medio: 720,
  amplio: 1080
};

/** Clasifica un ancho. Un ancho negativo o no finito se trata como compacto. */
export function clasificarAncho(px: number): ClaseDeAncho {
  if (!Number.isFinite(px)) return 'compacto';
  if (px >= ANCHO_MINIMO.amplio) return 'amplio';
  if (px >= ANCHO_MINIMO.medio) return 'medio';
  return 'compacto';
}

/**
 * Cuántos paneles pueden estar **acoplados** a la vez.
 *
 * En compacto la respuesta es cero, y esa es la regla que da nombre al criterio
 * de aceptación: un panel que no sabe colapsar no entra. Cero
 * acoplados no significa cero paneles, significa que se muestran superpuestos y
 * se descartan.
 */
export function panelesAcoplados(clase: ClaseDeAncho): number {
  switch (clase) {
    case 'amplio':
      return 2;
    case 'medio':
      return 1;
    case 'compacto':
      return 0;
  }
}

/** En compacto un panel abierto se superpone al contenido en lugar de acoplarse. */
export function elPanelSeSuperpone(clase: ClaseDeAncho): boolean {
  return panelesAcoplados(clase) === 0;
}

/**
 * En compacto la navegación primaria pasa de columna lateral a barra inferior.
 *
 * Va abajo y no arriba porque en una ventana estrecha el borde inferior es el
 * que queda al alcance sin recolocar la mano, y porque la barra superior ya
 * ocupa el borde de arriba.
 */
export function laNavegacionVaAbajo(clase: ClaseDeAncho): boolean {
  return clase === 'compacto';
}
