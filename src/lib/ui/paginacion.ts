/**
 * Paginación de la biblioteca.
 *
 * Contrato: `docs/architecture/performance-budgets.md`. * la paginación, que sustituye la virtualización.
 *
 * # Por qué páginas y no una ventana que sigue al desplazamiento
 *
 * La virtualización cumplía el presupuesto de nodos y aun así era la peor de
 * las dos opciones, por tres motivos que solo se ven con una biblioteca de
 * verdad:
 *
 * 1. **El tamaño del bloque dependía de la ventana, y la ventana del
 *    desplazamiento.** Así que desplazarse cambiaba cuántos elementos se pedían,
 *    y eso cambiaba a qué múltiplo se alineaba la petición: bajar tres filas
 *    podía mover el origen del bloque y obligar a traer otro. La lista se
 *    rehacía sola mientras se bajaba.
 * 2. **Con 152.080 obras no se puede llegar a ninguna parte.** Un desplazamiento
 *    continuo sirve para mirar lo que hay cerca; para llegar a la M hace falta
 *    saltar, y saltar necesita que existan sitios a los que saltar.
 * 3. **El presupuesto de nodos era una consecuencia de la aritmética.** Se
 *    cumplía porque una función lo garantizaba para todo par (total,
 *    desplazamiento), y eso hay que demostrarlo. Con páginas de tamaño acotado
 *    se cumple **por construcción**: lo que se pinta es lo que se pidió, y lo
 *    que se pide nunca pasa del mayor tamaño ofrecido.
 *
 * Lo que no cambia es de dónde sale el orden: **de SQL**. Filtrar y ordenar
 * siguen siendo del runtime, sobre el catálogo entero, y la página es una
 * ventana sobre ese orden ya hecho. Ordenar la página en la interfaz habría
 * ordenado veinticinco elementos de ciento cincuenta mil, que es lo mismo que no
 * ordenar.
 */

/**
 * Cuántos nodos del documento ocupa un elemento de la biblioteca.
 *
 * Se cuentan a mano contra el marcado de `LibraryView`, y por eso el componente
 * declara en su cabecera cuántos usa: el día que alguien añada un elemento a la
 * tarjeta, este número deja de corresponder y el presupuesto se calcula sobre
 * una cuenta que ya no es cierta.
 */
export const NODOS_POR_ELEMENTO = 9;

/**
 * Presupuesto de nodos del documento, de `performance-budgets.md`.
 *
 * Se repite aquí porque la prueba tiene que poder comprobarlo, y se comprueba
 * contra el documento: si el presupuesto cambia allí y no aquí, la prueba lo
 * dice.
 */
export const PRESUPUESTO_DE_NODOS = 1500;

/**
 * Tamaños de página que se ofrecen.
 *
 * La lista es cerrada, y esa es la mitad de la garantía: el presupuesto de nodos
 * se cumple porque **ningún tamaño ofrecido puede pasarlo**, y eso solo se puede
 * afirmar de una lista que se conoce entera. Un campo donde escribir un número
 * convertiría el presupuesto en una recomendación.
 *
 * Ciento sesenta y cinco es el mayor que cabe: 165 x 9 = 1485.
 */
export const TAMANOS_DE_PAGINA: readonly number[] = [10, 25, 50, 100];

/** Tamaño con el que se empieza. */
export const TAMANO_POR_DEFECTO = 50;

/** Qué hay que pedirle al runtime para pintar una página. */
export interface Peticion {
  readonly offset: number;
  readonly limit: number;
}

/**
 * Cuántas páginas hay.
 *
 * Cero elementos son **una** página, no cero: la vista existe igual y tiene que
 * poder decir "1 de 1" en lugar de "1 de 0", que se lee como un error.
 */
export function paginas(total: number, porPagina: number): number {
  const tamano = Math.max(1, Math.floor(porPagina));
  return Math.max(1, Math.ceil(Math.max(0, total) / tamano));
}

/**
 * Acota un número de página al rango que existe.
 *
 * Hace falta de verdad: cambiar de filtro con la página 400 abierta deja un
 * número que ya no tiene detrás ningún elemento, y pedirlo devolvería una página
 * vacía en mitad de una biblioteca llena.
 */
export function acotarPagina(pagina: number, total: number, porPagina: number): number {
  const ultima = paginas(total, porPagina);
  if (!Number.isFinite(pagina)) return 1;
  return Math.min(Math.max(1, Math.floor(pagina)), ultima);
}

/** Qué trozo del orden global es una página. */
export function peticionDe(pagina: number, porPagina: number): Peticion {
  const tamano = Math.max(1, Math.floor(porPagina));
  const numero = Math.max(1, Math.floor(pagina));
  return { offset: (numero - 1) * tamano, limit: tamano };
}

/** Cuántos nodos del documento ocuparía una página llena de este tamaño. */
export function nodosDe(porPagina: number): number {
  return Math.max(0, Math.floor(porPagina)) * NODOS_POR_ELEMENTO;
}

/**
 * Los números para un reparto concreto de extremos y vecinas, con sus huecos.
 *
 * Aparte de `numerosVisibles` porque es la parte que se repite al buscar el
 * reparto que mejor llena el sitio.
 */
function componerNumeros(
  centro: number,
  ultima: number,
  vecinas: number,
  extremos: number
): (number | null)[] {
  const necesarias = new Set<number>();
  for (let i = 1; i <= Math.min(extremos, ultima); i += 1) {
    necesarias.add(i);
    necesarias.add(ultima - i + 1);
  }
  for (let i = Math.max(1, centro - vecinas); i <= Math.min(ultima, centro + vecinas); i += 1) {
    necesarias.add(i);
  }

  const salida: (number | null)[] = [];
  let anterior = 0;
  for (const numero of [...necesarias].sort((a, b) => a - b)) {
    // Un solo hueco entre dos números seguidos no es un salto: es la página que
    // falta, y pintar la elipsis en su sitio ocupa lo mismo que pintarla.
    if (numero - anterior === 2) salida.push(anterior + 1);
    else if (numero - anterior > 2) salida.push(null);
    salida.push(numero);
    anterior = numero;
  }
  return salida;
}

/**
 * Los números de página que se pintan, con huecos donde se salta.
 *
 * Con tres mil páginas no se pueden pintar tres mil botones. Se pintan las
 * primeras, las últimas y las vecinas de la actual; donde falta algo va `null`,
 * que la vista dibuja como una elipsis **que no es un botón**: un hueco que se
 * puede pulsar y no lleva a un sitio concreto es peor que no pintarlo.
 *
 * `casillas` es cuántos huecos caben, contando las elipsis. Antes
 * Se pintaban siempre siete aunque el pie tuviera sitio para veinte, y el equipo lo
 * describió como "un espacio grandísimo" al lado de `1 … 4 5 6 … 1521`. Ahora el
 * pie dice cuántas caben y se llenan: con once, desde la primera página sale
 * `1 2 3 4 5 … 1517 1518 1519 1520 1521`.
 *
 * El reparto no cambia de forma al moverse: la primera y la última siempre están,
 * la actual siempre tiene sus vecinas, y lo que sobra cerca de un extremo se
 * llena en lugar de quedarse vacío.
 */
export function numerosVisibles(
  actual: number,
  totalDePaginas: number,
  casillas = 7
): readonly (number | null)[] {
  const ultima = Math.max(1, Math.floor(totalDePaginas));
  const centro = Math.min(Math.max(1, Math.floor(actual)), ultima);
  const limite = Math.max(5, Math.floor(casillas));

  const componer = (vecinas: number, extremos: number) =>
    componerNumeros(centro, ultima, vecinas, extremos);

  // Con todas cabiendo no hay nada que decidir.
  if (ultima <= limite) return componer(ultima, ultima);

  /*
   * El reparto de partida, para la actual en mitad de la biblioteca: la actual y
   * dos elipsis ocupan tres casillas, y el resto se reparte entre los extremos y
   * las vecinas. Con siete sale `1 … 4 5 6 … 1521`, que era lo de siempre.
   */
  const resto = limite - 3;
  let extremos = Math.max(1, Math.floor(resto / 4));
  let vecinas = Math.max(0, Math.floor((resto - 2 * extremos) / 2));
  let mejor = componer(vecinas, extremos);

  /*
   * Cerca de un extremo, la actual y sus vecinas se funden con él y sobran
   * casillas. Se llenan: primero creciendo los extremos, que es lo que da
   * `1 2 3 4 5 … 1517 1518 1519 1520 1521` desde la primera página, y después
   * las vecinas.
   */
  for (const crecer of ['extremos', 'vecinas'] as const) {
    for (;;) {
      const candidata =
        crecer === 'extremos' ? componer(vecinas, extremos + 1) : componer(vecinas + 1, extremos);
      if (candidata.length > limite || candidata.length <= mejor.length) break;
      if (crecer === 'extremos') extremos += 1;
      else vecinas += 1;
      mejor = candidata;
    }
  }
  return mejor;
}

/**
 * Lo que queda escrito en el campo de "ir a la página", ya saneado.
 *
 * Solo cifras, y nunca un número fuera de la biblioteca: lo que pasa del total
 * se queda en el total, y el cero no se admite porque no hay página cero. Se
 * sanea **mientras se escribe** y no al pulsar Intro, como se pidió el
 * 2026-09-12: así el campo no puede contener nunca algo que no lleve a ninguna
 * parte, y no hace falta ningún mensaje de error que explicar.
 *
 * Devuelve el texto que debe quedar en el campo: vacío si no hay nada útil.
 */
export function paginaEscrita(escrito: string, totalDePaginas: number): string {
  const cifras = escrito.replace(/\D/g, '').replace(/^0+/, '');
  if (cifras.length === 0) return '';
  const ultima = Math.max(1, Math.floor(totalDePaginas));
  // Se compara con cadenas largas sin pasar por `Number`, que perdería precisión
  // con veinte cifras pegadas y daría un número que no es el escrito.
  if (cifras.length > String(ultima).length) return String(ultima);
  return String(Math.min(Number(cifras), ultima));
}
