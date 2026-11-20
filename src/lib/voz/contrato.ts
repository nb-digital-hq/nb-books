/**
 * El contrato de proveedor de voz.
 *
 * # Por qué un contrato y no una llamada a `speechSynthesis`
 *
 * Porque la voz del sistema no es la única. La voz neuronal trae un
 * motor neuronal local, y sin este contrato añadirlo obligaría a reescribir la
 * canalización de lectura y el seguimiento visual del texto. Es el mismo patrón
 * que el contrato de formato de `reader-contracts.md` y que el de consulta sobre
 * la selección: un sitio donde se declara qué sabe hacer cada proveedor, y una
 * interfaz que no sabe a quién le está preguntando.
 *
 * # Por qué el contrato vive en el frontend y no en Rust
 *
 * Porque el primer proveedor **solo existe en el webview**: `speechSynthesis` es
 * una API del navegador y Rust no puede llamarla. El proveedor neuronal de la
 * el producto sí vivirá en Rust, y entrará por aquí como un proveedor más cuya
 * síntesis cruza el puente; el audio se reproduce donde se reproduce todo, que
 * es la ventana.
 *
 * # Lo que el contrato transporta, y por qué importa
 *
 * **Marcas ancladas al Locator, no solo audio.** Es la parte del contrato que la
 * ficha subraya, y el motivo es que el seguimiento visual del texto no
 * se puede añadir después sobre un contrato que solo lleve sonido: haría falta
 * saber, en cada instante, qué parte del documento se está diciendo. Un
 * proveedor que no sepa alinear devuelve marcas por bloque, que es lo mínimo
 * para resaltar por dónde va; uno que sepa más devuelve marcas por palabra.
 *
 * Contrato: `docs/architecture/voice-services.md`., y
 * `docs/architecture/accessibility.md`, sección 5.
 */

import type { Locator } from '$lib/bindings';

/** Una voz que un proveedor ofrece. */
export interface VozDisponible {
  /** Identificador estable dentro del proveedor. */
  id: string;
  /** Nombre tal y como lo enseña el sistema. No se traduce: es un nombre propio. */
  nombre: string;
  /** Etiqueta BCP-47. Sirve para elegir la voz del idioma del documento. */
  idioma: string;
  /** Si el sistema la considera la voz por defecto de su idioma. */
  porDefecto?: boolean;
}

/**
 * Un tramo de texto que se manda a decir.
 *
 * Lleva su `locator` porque una marca sin posición no sirve para seguir el
 * texto: es lo que convierte "está diciendo el carácter 412" en "está en este
 * párrafo de este capítulo".
 */
export interface Tramo {
  /** Índice del bloque dentro de la superficie de lectura. */
  bloque: number;
  texto: string;
  /** Dónde está el bloque en el documento. Viaja con cada marca que genere. */
  locator: Locator;
}

/**
 * Dónde va la voz ahora mismo.
 *
 * `desde` y `hasta` son desplazamientos en caracteres **dentro del texto del
 * tramo**, no del documento: el documento no tiene un índice de caracteres
 * estable entre formatos, y ADR-004 prohíbe anclar por número. Para saber dónde
 * cae en el documento está `locator`, que es lo que se guarda si hace falta.
 */
export interface MarcaDeVoz {
  bloque: number;
  locator: Locator;
  desde: number;
  hasta: number;
}

/** Lo que un proveedor declara de sí mismo, antes de que nadie le pida nada. */
export interface DeclaracionDeVoz {
  /** Identificador estable. `sistema` es el de la síntesis del sistema. */
  id: string;
  /**
   * Si este proveedor **puede funcionar en esta máquina**.
   *
   * Se declara, no se deduce. Es el criterio de aceptación: la
   * ausencia de voz en una plataforma se dice, no se disimula. WebKitGTK no
   * siempre trae síntesis, y una aplicación que enseñara el botón y no dijera
   * nada al pulsarlo sería peor que una que reconoce que ahí no hay voz.
   */
  disponible: boolean;
  /**
   * Por qué no está disponible, cuando no lo está.
   *
   * Un código, no una frase: lo traduce la capa de idioma, igual que los
   * errores del runtime.
   *
   * `sin-sintesis` es "este webview no tiene la API"; `sin-sintetizador`, "el
   * sistema no tiene con qué hablar". La diferencia importa porque **la segunda
   * tiene arreglo y la primera no**: una se resuelve instalando un paquete y la
   * otra no se resuelve con ningún paquete.
   */
  motivo?:
    | 'sin-sintesis'
    | 'sin-sintetizador'
    | 'sin-voces'
    | 'sin-recursos'
    /**
     * El proveedor existe y **no está configurado**.
     *
     * Es de la voz remota, y es distinto de todos los demás: no falta
     * nada en la máquina, falta una decisión del usuario que nadie puede tomar
     * por él —a qué servicio le manda el texto de sus libros—. Por eso el texto
     * que lo acompaña lleva a Ajustes en lugar de explicar una carencia.
     */
    | 'sin-servicio';
  /**
   * Está disponible **y no se recomienda aquí**, con el motivo.
   *
   * `lenta`: la voz neuronal tarda más en decir un párrafo de lo que dura, medido en
   * esta máquina, así que en vivo se cortará. Antes eso la quitaba de
   * la narración en vivo; ese día se fijó que la aplicación recomienda y no
   * obliga, así que se ofrece y se dice lo que va a pasar.
   */
  advertencia?: 'lenta';
  /** El factor de tiempo real medido, cuando la advertencia sale de él. */
  factor?: number;
  /** Si necesita salir a la red. Lo declara él; nadie lo deduce desde fuera. */
  necesitaRed: boolean;
  /** Si sabe decir por dónde va con más finura que el bloque entero. */
  marcaPorPalabra: boolean;
}

/** Qué hace la narración cuando termina un tramo. */
export type FinDeTramo = 'seguir' | 'parar';

/** Lo que quien narra le pide al proveedor. */
export interface OpcionesDeVoz {
  /** Voz elegida, o `null` para la que el proveedor considere suya por defecto. */
  voz: string | null;
  /** Multiplicador de velocidad. 1 es la del sistema. */
  velocidad: number;
}

/** Lo que el proveedor va contando mientras habla. */
export interface EscuchaDeVoz {
  /** Por dónde va. Un proveedor sin alineación la emite una vez por tramo. */
  alAvanzar: (marca: MarcaDeVoz) => void;
  /** El tramo terminó de decirse entero. */
  alTerminarTramo: (bloque: number) => void;
  /**
   * Algo falló mientras hablaba.
   *
   * Un código, como en todo lo demás: la voz se puede cortar porque el sistema
   * la interrumpió, y eso no es lo mismo que no tenerla.
   */
  alFallar: (motivo: 'interrumpida' | 'sin-voz' | 'desconocido') => void;
}

/**
 * Un proveedor de voz.
 *
 * Lista cerrada de operaciones a propósito: cuantas menos, más fácil es que un
 * proveedor nuevo las implemente todas bien. Pausar y reanudar están separadas
 * de parar porque son cosas distintas para quien escucha: pausar conserva el
 * sitio y parar lo suelta.
 */
export interface ProveedorDeVoz {
  declarar(): DeclaracionDeVoz;
  /** Voces que ofrece. Vacío cuando no está disponible. */
  voces(): VozDisponible[];
  /** Empieza a decir un tramo. Sustituye a lo que estuviera diciendo. */
  hablar(tramo: Tramo, opciones: OpcionesDeVoz, escucha: EscuchaDeVoz): void;
  /**
   * Va preparando el tramo siguiente mientras suena el actual.
   *
   * Opcional porque solo tiene sentido donde preparar cuesta: la voz del
   * sistema empieza a hablar de inmediato y no tiene nada que adelantar. La de
   * un servicio sí: hay que pedirle el audio, y **medido contra un servidor
   * real, generar una frase tarda casi lo que dura**. Sin adelantarla, entre
   * frase y frase queda un silencio tan largo como la frase.
   *
   * No informa de nada ni puede fallar hacia fuera: si el adelanto no llega a
   * tiempo o falla, cuando toque se pedirá otra vez por la vía normal.
   */
  adelantar?(tramo: Tramo, opciones: OpcionesDeVoz): void;
  pausar(): void;
  reanudar(): void;
  /** Corta y suelta el sitio. */
  parar(): void;
}
