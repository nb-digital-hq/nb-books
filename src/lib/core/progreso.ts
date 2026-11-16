/**
 * Cuánto se ha leído, en un número entero de 0 a 100. **Una sola regla para toda
 * la aplicación**.
 *
 * Antes cada sitio redondeaba a su manera: la barra lateral, Inicio,
 * la ficha y el lector podían decir 1 % y 2 % del mismo libro a la vez. Se trunca,
 * y no se redondea, porque decir que se leyó más de lo leído es lo único que no
 * puede pasar: un libro al 99,6 % no está terminado. El 100 % es solo lo
 * terminado de verdad.
 */
export function porcentajeLeido(progresion: number | null | undefined): number | null {
  if (typeof progresion !== 'number' || !Number.isFinite(progresion)) return null;
  if (progresion >= 0.9999) return 100;
  return Math.min(99, Math.max(0, Math.floor(progresion * 100)));
}
