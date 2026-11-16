/**
 * Los modos de lectura y el kit de lectura accesible.
 *
 * Contrato: ADR-012, compromisos 5 y 6, y ui-components.md, sección 5.
 *.
 *
 * # Un modo es un perfil completo, no una paleta
 *
 * Cada modo fija fondo, texto, contraste y temperatura a la vez, y tiene nombre.
 * Es la vía principal de personalización del lector; el ajuste fino es
 * secundario y siempre está. Un modo con dos valores sueltos obligaría al
 * usuario a componer el resto a mano, que es justo la fricción disfrazada de
 * libertad que ADR-012 declara como modo de fallo de esta sección.
 *
 * # Lo emulado se llama emulación
 *
 * Un webview no controla el brillo del panel ni la temperatura de color del
 * hardware. Lo que hacemos es **componer una capa encima**, y la interfaz lo
 * nombra así. Presentarlo como control del dispositivo sería mentir sobre lo que
 * la aplicación puede hacer, y es la misma regla que el proyecto aplica a las
 * capacidades degradadas: lo que no está, se declara.
 */

/** Los modos del catálogo inicial. */
export type NombreDeModo =
  | 'dia'
  | 'sepia'
  | 'nocturno'
  | 'altoContraste'
  | 'papelMonocromo'
  | 'papelAtenuado';

/**
 * Un modo de lectura, con todo lo que fija.
 *
 * El nombre es una cadena y no solo uno de los seis del catálogo porque desde
 * Un paquete de personalización puede aportar los suyos, con el
 * identificador `<extensión>/<modo>`. Lo que no cambia es **qué** es un modo:
 * un perfil completo, con los mismos cuatro valores. Una extensión que pudiera
 * aportar medio modo obligaría a componer el resto a mano.
 */
export interface Modo {
  nombre: string;
  /**
   * El nombre que se ve, cuando lo pone su autor.
   *
   * Los seis del núcleo no lo llevan: los suyos están en el diccionario y se
   * traducen. El de una extensión es suyo y **no se traduce**, igual que el
   * título de una orden o el nombre de una vista.
   */
  etiqueta?: string;
  /** Color de fondo de la superficie de lectura. */
  fondo: string;
  /** Color del texto. */
  texto: string;
  /**
   * Brillo emulado, de 0,5 a 1.
   *
   * **Emulado**: es una capa de composición, no el brillo del panel.
   */
  brillo: number;
  /**
   * Temperatura emulada, de 0 (neutra) a 1 (muy cálida).
   *
   * Filtra el azul componiendo encima, que es lo único que un webview puede
   * hacer. No es el filtro de luz nocturna del sistema y no lo sustituye.
   */
  temperatura: number;
  /** Si el modo quita el color del contenido, no solo del fondo. */
  monocromo?: boolean;
}

/**
 * El catálogo inicial, en el orden en que se ofrece.
 *
 * Los dos últimos están inspirados en las pantallas de tipo papel y en la tinta
 * electrónica: uno lleva todo a monocromo de alto contraste y el otro atenúa el
 * color con una temperatura cálida. Los colores no son decorativos: los cinco
 * pares cumplen el contraste exigido y hay una prueba que lo **calcula**.
 */
export const MODOS: readonly Modo[] = [
  {
    nombre: 'dia',
    fondo: '#fbfbf9',
    texto: '#16191b',
    brillo: 1,
    temperatura: 0
  },
  {
    nombre: 'sepia',
    fondo: '#f3e9d8',
    texto: '#3a2f21',
    brillo: 0.97,
    temperatura: 0.35
  },
  {
    nombre: 'nocturno',
    fondo: '#12161a',
    texto: '#d8dee3',
    brillo: 0.9,
    temperatura: 0.15
  },
  {
    nombre: 'altoContraste',
    fondo: '#000000',
    texto: '#ffffff',
    brillo: 1,
    temperatura: 0
  },
  {
    nombre: 'papelMonocromo',
    fondo: '#eeeeec',
    texto: '#101010',
    brillo: 0.95,
    temperatura: 0.1,
    monocromo: true
  },
  {
    nombre: 'papelAtenuado',
    fondo: '#e9e3d6',
    texto: '#2c2822',
    brillo: 0.88,
    temperatura: 0.55
  }
];

/** El modo con el que se lee mientras el usuario no haya elegido. */
export const MODO_POR_DEFECTO: NombreDeModo = 'dia';

/** Busca un modo por su nombre; devuelve el de por defecto si no existe. */
export function modoPorNombre(nombre: string, extra: readonly Modo[] = []): Modo {
  return (
    MODOS.find((m) => m.nombre === nombre) ??
    extra.find((m) => m.nombre === nombre) ??
    // Un modo que no está se compone con el de siempre en lugar de dejar la
    // superficie sin colores. Pasa de verdad: se desinstala el paquete que lo
    // aportaba, o se vuelve a una generación anterior del almacén.
    (MODOS.find((m) => m.nombre === MODO_POR_DEFECTO) as Modo)
  );
}

/**
 * El filtro de composición que emula brillo y temperatura.
 *
 * Devuelve una cadena para `filter`, o `null` cuando el modo no altera nada:
 * un filtro que no hace nada sigue costando una capa de composición en cada
 * fotograma, y el presupuesto de fotograma es el que gobierna aquí.
 */
export function filtroDe(modo: Modo): string | null {
  const partes: string[] = [];
  if (modo.brillo !== 1) partes.push(`brightness(${modo.brillo})`);
  if (modo.temperatura > 0) partes.push(`sepia(${modo.temperatura})`);
  if (modo.monocromo) partes.push('grayscale(1)');
  return partes.length > 0 ? partes.join(' ') : null;
}

/** Ajustes del kit de lectura accesible. */
export interface KitAccesible {
  /**
   * Familia para dislexia o baja visión.
   *
   * `ninguna` no es una familia: es no tocar la del plano del libro.
   */
  familia: 'ninguna' | 'dislexia' | 'bajaVision';
  /** Lectura biónica: la primera parte de cada palabra en negrita. */
  bionica: boolean;
  /** Regla que sigue al cursor de bloque. */
  regla: boolean;
  /** Atenúa todo menos la línea en la que se está. */
  focoDeLinea: boolean;
  /** Animación de pase de página. Desaparece con movimiento reducido. */
  animarPagina: boolean;
}

export const KIT_POR_DEFECTO: KitAccesible = {
  familia: 'ninguna',
  bionica: false,
  regla: false,
  focoDeLinea: false,
  animarPagina: true
};

/**
 * Reparte una palabra en la parte que se marca y la que no.
 *
 * La proporción es la mitad redondeada hacia arriba en palabras cortas y algo
 * menos en las largas: marcar media palabra de doce letras no ayuda a fijar la
 * vista, la emborrona.
 */
export function partirBionica(palabra: string): [string, string] {
  const largo = palabra.length;
  if (largo <= 1) return [palabra, ''];
  const corte = largo <= 4 ? Math.ceil(largo / 2) : Math.ceil(largo * 0.4);
  return [palabra.slice(0, corte), palabra.slice(corte)];
}

/**
 * Si la animación de pase de página debe ocurrir.
 *
 * **Con movimiento reducido desaparece por completo**, no se acorta: es la regla
 * 7 de ADR-012, una animación que no cabe se elimina. Y su ausencia no altera la
 * posición de lectura, porque la animación nunca decide dónde se está.
 */
export function seAnima(kit: KitAccesible, movimientoReducido: boolean): boolean {
  return kit.animarPagina && !movimientoReducido;
}
