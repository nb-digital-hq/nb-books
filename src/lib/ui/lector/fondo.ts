/**
 * El fondo del modo de lectura ya filtrado, para el lector.
 *
 * Vive aparte de `modos.ts` porque `modos.ts` va en el paquete de arranque -lo usan
 * los Ajustes- y esto solo lo necesita el lector, que se carga al abrir un
 * documento: dentro de `modos.ts` le costaba al arranque 544 bytes que no tenía
 * (performance-budgets 4.11).
 */

import type { Modo } from './modos';

/**
 * El color de fondo del modo **ya filtrado**, calculado aquí y no por el motor.
 *
 * # Por qué existe
 *
 * Detrás de la barra del lector se pinta el fondo del modo con el mismo filtro que
 * la superficie, para que no se vea una costura entre la barra y el texto. Con el
 * filtro puesto en esa capa, **abrir cualquier panel costaba 90-110 ms** en
 * WebKitGTK contra 30 sin él: el panel se apoya encima y el motor vuelve a pasar el
 * filtro por todo el fondo del lector. Y esa capa es un color liso, así que el
 * filtro se puede calcular una vez.
 *
 * Son las matrices de Filter Effects en sRGB, en el orden de `filtroDe`. Medido
 * contra el propio motor el 2026-09-25, sobre los cinco modos: coinciden con una
 * diferencia máxima de 1/255 en un canal, por redondeo. `modos.test.ts` lo fija.
 */
export function fondoFiltrado(modo: Modo): string {
  const acotar = (x: number) => Math.min(1, Math.max(0, x));
  const canal = (i: number) => Number.parseInt(modo.fondo.slice(i, i + 2), 16) / 255;
  let r = canal(1);
  let g = canal(3);
  let b = canal(5);
  if (modo.brillo !== 1) {
    r = acotar(r * modo.brillo);
    g = acotar(g * modo.brillo);
    b = acotar(b * modo.brillo);
  }
  if (modo.temperatura > 0) {
    const a = 1 - modo.temperatura;
    [r, g, b] = [
      acotar((0.393 + 0.607 * a) * r + (0.769 - 0.769 * a) * g + (0.189 - 0.189 * a) * b),
      acotar((0.349 - 0.349 * a) * r + (0.686 + 0.314 * a) * g + (0.168 - 0.168 * a) * b),
      acotar((0.272 - 0.272 * a) * r + (0.534 - 0.534 * a) * g + (0.131 + 0.869 * a) * b)
    ];
  }
  if (modo.monocromo) {
    const y = acotar(0.2126 * r + 0.7152 * g + 0.0722 * b);
    r = y;
    g = y;
    b = y;
  }
  const hex = (x: number) =>
    Math.round(x * 255)
      .toString(16)
      .padStart(2, '0');
  return `#${hex(r)}${hex(g)}${hex(b)}`;
}
