/**
 * La narración, como máquina pura.
 *
 * # Por qué otra vez una máquina pura
 *
 * Por lo mismo que el lector y que el panel de anotaciones: es lo que permite
 * comprobar el comportamiento sin navegador. Y aquí importa más que en ningún
 * otro sitio, porque el navegador que hay debajo **es el que no se puede
 * conducir**: la síntesis del sistema no existe en el Chromium de las pruebas de
 * interfaz igual que en el webview del binario, así que si el avance viviera
 * dentro del componente no habría forma de comprobarlo en ninguna parte.
 *
 * # Qué narra: los bloques, y por eso vale para los dos formatos
 *
 * El criterio dice "sobre EPUB y sobre PDF con capa de texto". No hace
 * falta ninguna rama para eso: la superficie de lectura ya entrega **bloques**
 * —párrafos del marcado en un EPUB, fragmentos de la capa de texto en un PDF—, y
 * narrar es recorrerlos. Un PDF escaneado sin capa de texto no tiene bloques, y
 * ahí no hay nada que narrar: se declara, que es la otra mitad del criterio.
 *
 * Contrato: `docs/architecture/voice-services.md`.
 */

import type { Locator } from '$lib/bindings';

import type { MarcaDeVoz, Tramo } from './contrato';

/** En qué está la narración. */
export type EstadoDeVoz = 'parada' | 'hablando' | 'pausada';

/** Estado completo de la narración. */
export interface EstadoDeNarracion {
  estado: EstadoDeVoz;
  /** Bloque que se está diciendo. `-1` cuando no se dice ninguno. */
  bloque: number;
  /** Por dónde va dentro del bloque, o `null` si todavía no ha llegado marca. */
  marca: MarcaDeVoz | null;
  /** Voz elegida, o `null` para la que el proveedor tenga por defecto. */
  voz: string | null;
  velocidad: number;
}

/** Lo que el componente tiene que pedirle al proveedor. */
export type EfectoDeVoz =
  | { tipo: 'hablar'; tramo: Tramo; siguiente: Tramo | null }
  | { tipo: 'pausar' }
  | { tipo: 'reanudar' }
  | { tipo: 'parar' };

export interface TransicionDeVoz {
  estado: EstadoDeNarracion;
  efecto?: EfectoDeVoz;
}

/**
 * Velocidades que se ofrecen.
 *
 * Discretas y no un deslizador continuo: quien usa la voz para leer ajusta una
 * vez y no vuelve, y un deslizador obliga a afinar con el ratón un valor que da
 * igual que sea 1,37 o 1,4. Los extremos son los que las plataformas garantizan.
 *
 * La mitad se añade. Uno es la velocidad **que el usuario tenga
 * configurada en su sistema**, no una nuestra, y la de `espeak-ng` recién
 * instalado es rápida: quien la escucha por primera vez la describe como que no
 * vocaliza. Sin un escalón por debajo de 0,75 no había forma de bajar de ahí
 * salvo reconfigurando `speech-dispatcher`, que es exactamente lo que esta
 * aplicación no puede pedirle a nadie.
 */
export const VELOCIDADES = [0.5, 0.75, 1, 1.25, 1.5, 2] as const;

export const VELOCIDAD_POR_DEFECTO = 1;

/** Estado de una narración que no ha empezado. */
export function narracionInicial(): EstadoDeNarracion {
  return {
    estado: 'parada',
    bloque: -1,
    marca: null,
    voz: null,
    velocidad: VELOCIDAD_POR_DEFECTO
  };
}

/** El texto de un bloque, tal y como se manda a decir. */
export interface BloqueNarrable {
  texto: string;
}

function tramoDe(
  bloques: readonly BloqueNarrable[],
  indice: number,
  locator: Locator
): Tramo | null {
  const bloque = bloques[indice];
  if (!bloque) return null;
  const texto = bloque.texto.trim();
  // Un bloque en blanco no se dice: la voz se quedaría muda unos segundos y
  // parecería que se ha colgado.
  if (texto.length === 0) return null;
  return { bloque: indice, texto, locator };
}

/**
 * Busca el siguiente bloque con algo que decir.
 *
 * Salta los vacíos en lugar de pararse en ellos, que es lo que hacía que la
 * narración se detuviera en mitad de un capítulo con una línea en blanco.
 */
function siguienteConTexto(
  bloques: readonly BloqueNarrable[],
  desde: number,
  locator: Locator
): Tramo | null {
  for (let i = Math.max(0, desde); i < bloques.length; i += 1) {
    const tramo = tramoDe(bloques, i, locator);
    if (tramo) return tramo;
  }
  return null;
}

/**
 * Empieza a narrar desde un bloque.
 *
 * Sin bloques con texto no devuelve efecto y se queda parada: es un documento
 * que no se puede narrar, y quien lo pinta ya lo dice con `motivoSinTexto`.
 */
export function empezar(
  estado: EstadoDeNarracion,
  bloques: readonly BloqueNarrable[],
  desde: number,
  locator: Locator
): TransicionDeVoz {
  const tramo = siguienteConTexto(bloques, desde, locator);
  if (!tramo) return { estado: { ...estado, estado: 'parada', bloque: -1, marca: null } };

  return {
    estado: { ...estado, estado: 'hablando', bloque: tramo.bloque, marca: null },
    // El siguiente viaja con el efecto para que quien lo aplique pueda irlo
    // preparando: la máquina sigue sin saber nada de redes ni de audio.
    efecto: {
      tipo: 'hablar',
      tramo,
      siguiente: siguienteConTexto(bloques, tramo.bloque + 1, locator)
    }
  };
}

/** Alterna entre hablar y pausar, que es lo que hace la tecla y el botón. */
export function alternar(
  estado: EstadoDeNarracion,
  bloques: readonly BloqueNarrable[],
  desde: number,
  locator: Locator
): TransicionDeVoz {
  if (estado.estado === 'hablando') {
    return { estado: { ...estado, estado: 'pausada' }, efecto: { tipo: 'pausar' } };
  }
  if (estado.estado === 'pausada') {
    return { estado: { ...estado, estado: 'hablando' }, efecto: { tipo: 'reanudar' } };
  }
  return empezar(estado, bloques, desde, locator);
}

/** Corta la narración y suelta el sitio. */
export function parar(estado: EstadoDeNarracion): TransicionDeVoz {
  return {
    estado: { ...estado, estado: 'parada', bloque: -1, marca: null },
    efecto: { tipo: 'parar' }
  };
}

/** Apunta por dónde va la voz. No cambia nada más. */
export function avanzar(estado: EstadoDeNarracion, marca: MarcaDeVoz): EstadoDeNarracion {
  // Una marca de un bloque que ya no es el que se dice llega tarde: el proveedor
  // la emitió antes de que la cancelación surtiera efecto, y aplicarla movería el
  // resaltado hacia atrás.
  if (estado.estado === 'parada' || marca.bloque !== estado.bloque) return estado;
  return { ...estado, marca };
}

/**
 * El tramo terminó: se pasa al siguiente.
 *
 * Cuando no queda ninguno, la narración se para sola. No da la vuelta al
 * documento: quien escucha un libro entero no quiere volver a empezar por el
 * principio sin haberlo pedido.
 */
export function terminarTramo(
  estado: EstadoDeNarracion,
  bloque: number,
  bloques: readonly BloqueNarrable[],
  locator: Locator
): TransicionDeVoz {
  if (estado.estado !== 'hablando' || bloque !== estado.bloque) return { estado };
  return empezar({ ...estado, marca: null }, bloques, bloque + 1, locator);
}

/**
 * Cambia la velocidad o la voz.
 *
 * Si estaba hablando, vuelve a empezar el bloque en curso: ninguna plataforma
 * permite cambiar la velocidad de un enunciado ya encolado, y seguir con la
 * anterior haría que el ajuste pareciera roto hasta el párrafo siguiente.
 * Reiniciar el bloque es la peor opción salvo todas las demás.
 */
export function ajustar(
  estado: EstadoDeNarracion,
  cambio: Partial<Pick<EstadoDeNarracion, 'voz' | 'velocidad'>>,
  bloques: readonly BloqueNarrable[],
  locator: Locator
): TransicionDeVoz {
  const siguiente = { ...estado, ...cambio };
  if (estado.estado !== 'hablando' || estado.bloque < 0) return { estado: siguiente };
  return empezar({ ...siguiente, marca: null }, bloques, estado.bloque, locator);
}

/**
 * Si un **tramo** está siendo narrado ahora mismo.
 *
 * Tramo y no bloque: la voz avanza por frases y el resaltado va por
 * párrafos, que es la unidad que se ve. La traducción de una cosa a la otra la
 * hace el narrador, que es quien sabe de qué párrafo salió cada frase; aquí la
 * pregunta sigue siendo la misma y sobre el mismo índice.
 *
 * Es el seguimiento visual del texto, y por eso la marca lleva el `Locator`: sin
 * él sería un número, que no significa lo mismo al pasar de página.
 */
export function seNarra(estado: EstadoDeNarracion, bloque: number): boolean {
  return estado.estado !== 'parada' && estado.bloque === bloque;
}
