/**
 * El contrato de carga de una vista.
 *
 * Contrato: `docs/architecture/optimization-catalog.md`, sección
 * 1.3, reglas 3 y 4.
 *
 * # Por qué existe
 *
 * Una vista tiene **cuatro** estados -vacía, cargando, con datos y en error- y
 * los cuatro se diseñan a la vez. Cuando no es así, el que falta no es que se
 * vea mal: es que no existe, y lo que se ve en su lugar es otro estado
 * mintiendo. Una lista vacía mientras se carga dice "no hay nada" cuando lo que
 * pasa es "todavía no lo sé", y son cosas distintas para quien mira.
 *
 * Los cuatro estados **no se modelan aquí con un tipo**: cada vista los lleva
 * como los lleva `LibraryView` -un `cargando` y un `fallo`-, y
 * añadir un tipo paralelo que solo usara la mitad de las vistas sería una
 * segunda forma de decir lo mismo. Cómo se **enseña** el estado de carga lo
 * decide el indicador de carga: un indicador se mueve,
 * y el innegociable dice que nada se anima en reposo.
 *
 * # Y por qué la guarda de respuesta obsoleta va aquí y no en cada vista
 *
 * Porque **no es un problema de velocidad, es de corrección**, y se manifiesta
 * como lentitud. Al teclear en un filtro se dispara una petición por pulsación;
 * si la tercera contesta antes que la segunda, la segunda pinta encima y deja
 * en pantalla el resultado de una consulta que ya nadie escribió. Con la
 * biblioteca real, donde una consulta puede tardar segundos y otra
 * milisegundos, deja de ser improbable y pasa a ser lo normal.
 *
 * Esto no se arregla con un antirrebote: el antirrebote hace menos peticiones,
 * no ordena las que quedan. Lo que hay que hacer es **descartar la respuesta
 * que ya no corresponde a lo último que se pidió**, y eso solo lo sabe quien
 * lleva la cuenta.
 */

/** Lo que devuelve una petición pasada por la guarda. */
export type Respuesta<T> =
  | { readonly vigente: true; readonly valor: T }
  | { readonly vigente: false };

/**
 * Una guarda por cada cosa que se pide, no una por aplicación.
 *
 * Dos filtros distintos no compiten entre sí, y compartir la cuenta haría que
 * escribir en uno descartara la respuesta del otro.
 *
 * @example
 * const guarda = guardaDeRespuesta();
 * const r = await guarda(() => libraryShelves(peticion));
 * if (r.vigente) estanterias = r.valor;
 */
export function guardaDeRespuesta(): <T>(trabajo: () => Promise<T>) => Promise<Respuesta<T>> {
  let ultima = 0;

  return async <T>(trabajo: () => Promise<T>): Promise<Respuesta<T>> => {
    ultima += 1;
    const mia = ultima;
    try {
      const valor = await trabajo();
      return mia === ultima ? { vigente: true, valor } : { vigente: false };
    } catch (error) {
      // Un fallo de una petición que ya nadie espera no se enseña: quien
      // escribió otra cosa no tiene por qué ver el error de lo que borró.
      if (mia !== ultima) return { vigente: false };
      throw error;
    }
  };
}
