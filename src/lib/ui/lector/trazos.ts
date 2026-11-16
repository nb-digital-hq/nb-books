/**
 * El trazo libre: tinta a mano alzada sobre un bloque del texto.
 *
 * Contrato: `docs/design/reading-experience/README.md`. * la superficie de lectura, la otra mitad.
 *
 * # Por qué las coordenadas van normalizadas y no en píxeles
 *
 * Porque el texto se recompone. Quien lee cambia el cuerpo de letra, el ancho de
 * la columna o el modo de lectura, y el bloque que ocupaba 600 píxeles de ancho
 * pasa a ocupar 380: un trazo guardado en píxeles se quedaría donde estaba y el
 * texto se habría ido. Guardado de 0 a 1 sobre la caja del bloque, el trazo se
 * estira con él y sigue rodeando las mismas palabras.
 *
 * No es perfecto y conviene decirlo: si el bloque cambia de proporción —más
 * estrecho y más alto— el trazo se deforma. La alternativa sería anclarlo a
 * caracteres, que es lo que hace un resaltado, pero un trazo no rodea caracteres:
 * rodea una zona, y a veces el margen, que no tiene ninguno. Deformarse con el
 * bloque es lo más parecido a lo que hace la tinta en un papel que se dobla.
 *
 * # Por qué un módulo aparte y no dentro del componente
 *
 * Porque esto se puede comprobar sin navegador —son números— y el componente no.
 * Es la misma separación de `proyeccion.ts`.
 */

/** Un punto dentro de la caja de un bloque, de 0 a 1 en los dos ejes. */
export type Punto = readonly [number, number];

/** Un trazo continuo: lo que se dibuja entre apoyar y levantar. */
export interface Trazo {
  readonly puntos: readonly Punto[];
}

/** Lo que se guarda en `payload` de la anotación. */
export interface Tinta {
  readonly version: 1;
  readonly trazos: readonly Trazo[];
}

/**
 * Un trazo ya anclado, listo para pintar.
 *
 * Se ancla como un marcador —por el bloque que lo contiene— y no como un
 * resaltado, porque la tinta no rodea caracteres: rodea una zona.
 */
export interface MarcaDeTinta {
  readonly id: string;
  /** Desplazamiento del bloque dentro del texto plano del capítulo. */
  readonly inicio: number;
  readonly tinta: Tinta;
}

/** Caja de un bloque en coordenadas de la ventana. */
export interface Caja {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

/**
 * Lleva un punto de la ventana a la caja del bloque, de 0 a 1.
 *
 * Se acota a ese rango a propósito: un trazo que empieza dentro y se sale por el
 * margen se guarda pegado al borde en lugar de con coordenadas negativas, que al
 * repintarse aparecerían fuera del bloque y encima del texto de al lado.
 */
export function normalizar(x: number, y: number, caja: Caja): Punto {
  const ancho = caja.width > 0 ? caja.width : 1;
  const alto = caja.height > 0 ? caja.height : 1;
  return [acotar((x - caja.left) / ancho), acotar((y - caja.top) / alto)];
}

function acotar(valor: number): number {
  if (!Number.isFinite(valor)) return 0;
  return Math.min(1, Math.max(0, valor));
}

/**
 * Quita los puntos que no aportan forma.
 *
 * Un puntero emite decenas de eventos por segundo y muchos caen casi encima del
 * anterior: guardarlos todos multiplica el tamaño de la anotación sin cambiar lo
 * que se ve. Se conserva el primero, el último y los que se separen del último
 * conservado más que `minimo`.
 */
export function simplificar(puntos: readonly Punto[], minimo = 0.004): readonly Punto[] {
  if (puntos.length <= 2) return puntos;
  const salida: Punto[] = [puntos[0] as Punto];
  for (const punto of puntos.slice(1, -1)) {
    const ultimo = salida[salida.length - 1] as Punto;
    const dx = punto[0] - ultimo[0];
    const dy = punto[1] - ultimo[1];
    if (Math.hypot(dx, dy) >= minimo) salida.push(punto);
  }
  salida.push(puntos[puntos.length - 1] as Punto);
  return salida;
}

/**
 * El atributo `d` de un `<path>` para un trazo.
 *
 * Se dibuja con rectas entre puntos y no con curvas: sobre un trazo ya
 * simplificado la diferencia no se ve, y una curva exige puntos de control que
 * habría que guardar o recalcular. Las coordenadas salen tal cual porque el SVG
 * se pinta con `viewBox="0 0 1 1"`.
 */
export function aRuta(trazo: Trazo): string {
  const primero = trazo.puntos[0];
  if (!primero) return '';
  const resto = trazo.puntos.slice(1);
  const inicio = `M${redondear(primero[0])} ${redondear(primero[1])}`;
  if (resto.length === 0) {
    // Un punto suelto no se ve como línea: se cierra sobre sí mismo para que el
    // extremo redondeado del trazo lo pinte como un punto de tinta.
    return `${inicio}l0 0`;
  }
  return inicio + resto.map((p) => `L${redondear(p[0])} ${redondear(p[1])}`).join('');
}

/** Cuatro decimales: sobre una caja de mil píxeles eso es un décimo de píxel. */
function redondear(valor: number): number {
  return Math.round(valor * 10_000) / 10_000;
}

/**
 * Interpreta lo que venía guardado.
 *
 * Devuelve `null` ante cualquier cosa que no sea tinta reconocible, en lugar de
 * intentar arreglarla: una anotación con la geometría corrupta se declara y no se
 * pinta, que es lo mismo que el reanclaje hace con una marca cuyo tramo no cuadra con
 * su cita. Pintar una aproximación pondría tinta donde nadie la puso.
 */
/** Un punto reconocible, o nada. Cualquier otra cosa invalida el trazo entero. */
function puntoDe(crudo: unknown): Punto | null {
  if (!Array.isArray(crudo) || crudo.length !== 2) return null;
  const [x, y] = crudo as [unknown, unknown];
  if (typeof x !== 'number' || typeof y !== 'number') return null;
  if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
  return [acotar(x), acotar(y)];
}

/** Un trazo reconocible, o nada. */
function trazoDe(crudo: unknown): Trazo | null {
  if (typeof crudo !== 'object' || crudo === null) return null;
  const puntos = (crudo as { puntos?: unknown }).puntos;
  if (!Array.isArray(puntos)) return null;

  const limpios: Punto[] = [];
  for (const bruto of puntos) {
    const punto = puntoDe(bruto);
    if (!punto) return null;
    limpios.push(punto);
  }
  return limpios.length > 0 ? { puntos: limpios } : null;
}

export function desdePayload(crudo: unknown): Tinta | null {
  if (typeof crudo !== 'object' || crudo === null) return null;
  const objeto = crudo as { version?: unknown; trazos?: unknown };
  if (objeto.version !== 1 || !Array.isArray(objeto.trazos)) return null;

  const trazos: Trazo[] = [];
  for (const bruto of objeto.trazos) {
    const trazo = trazoDe(bruto);
    if (!trazo) return null;
    trazos.push(trazo);
  }
  return trazos.length > 0 ? { version: 1, trazos } : null;
}

/** Lo que se manda al runtime, ya simplificado. */
export function aPayload(trazos: readonly Trazo[]): Tinta | null {
  const utiles = trazos
    .map((t) => ({ puntos: simplificar(t.puntos) }))
    .filter((t) => t.puntos.length > 0);
  return utiles.length > 0 ? { version: 1, trazos: utiles } : null;
}
