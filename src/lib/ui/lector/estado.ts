/**
 * El estado del lector, como máquina pura.
 *
 * # Por qué el estado vive fuera del componente
 *
 * Dos criterios de aceptación lo exigen:
 *
 * 1. **"La carcasa se retira entera en modo inmersivo y vuelve sin perder la
 *    posición."** Si la posición viviera dentro del componente de la superficie,
 *    montarlo y desmontarlo la perdería, y entrar en inmersión es exactamente
 *    eso. Aquí el modo es un campo del estado y la posición es otro: no se
 *    tocan.
 * 2. **"El flujo completo operable solo con teclado."** Con las transiciones
 *    aquí, el flujo entero se prueba como una lista de pulsaciones. Dentro del
 *    componente habría que conducir un navegador para verlo.
 *
 * Es el mismo motivo por el que la aritmética de la ventana virtualizada de
 * la lógica no vive dentro de `LibraryView`.
 *
 * # Los efectos son datos
 *
 * Aplicar un comando no llama al puente: devuelve **qué habría que hacer**. El
 * componente lo ejecuta. Así la máquina no necesita runtime para probarse y no
 * puede quedarse a medias entre dos estados por culpa de una llamada que falló.
 */

import type { Locator } from '$lib/bindings';

import type { Comando, PanelDelLector } from './comandos';

/**
 * Una unidad anclable de la superficie de lectura.
 *
 * Es un párrafo del marcado en un EPUB y un fragmento de la capa de texto en un
 * PDF. El cursor se mueve por bloques y no por caracteres porque **no hay
 * navegación por cursor de texto garantizada en un webview**: WebKitGTK,
 * WebView2 y WKWebView no coinciden, y una selección que depende de eso
 * funcionaría en una plataforma y no en otra. Por bloques funciona en las tres,
 * y además es la misma regla para los dos formatos.
 */
export interface Bloque {
  /** Texto del bloque, tal y como se ve. Es lo que se manda como cita. */
  texto: string;
}

/** Estado completo del lector. */
export interface EstadoDelLector {
  bloques: Bloque[];
  /** Bloque enfocado. `-1` cuando todavía no se ha entrado al texto. */
  cursor: number;
  /**
   * Extremo fijo de la selección, o `null` si no hay selección extendida.
   *
   * Con `ancla` nula y cursor en un bloque, la selección es ese bloque: estar
   * sobre un párrafo ya es tenerlo seleccionado, y así anotar cuesta una tecla
   * en lugar de dos.
   */
  ancla: number | null;
  /** La carcasa se ha retirado y solo queda el texto. */
  inmersivo: boolean;
  /** La ventana está a pantalla completa del sistema. */
  pantallaCompleta: boolean;
  panel: PanelDelLector | null;
  /** Nota en curso. `null` cuando no se está escribiendo ninguna. */
  redaccion: { cuerpo: string } | null;
  /**
   * El texto exacto que se seleccionó con el ratón, si fue así.
   *
   * La selección por bloques sigue siendo la del teclado; esta convive con ella
   * para poder anotar una frase o una palabra, que es lo que hace cualquier
   * lector. Va con `cursor` y `ancla` puestos sobre los bloques que toca, así
   * que todo lo que mira la selección -la barra, las teclas- la ve igual; y
   * moverse con el teclado la suelta.
   */
  libre?: string | null;
  /** Ampliación de una página que se pinta como imagen: 1 es al ancho. */
  zoom?: number;
  /** Dónde está leyendo. Nunca la toca un cambio de modo. */
  locator: Locator;
  /**
   * El modo de dibujo está encendido.
   *
   * Es un modo y no un gesto suelto porque dibujar y seleccionar texto compiten
   * por el mismo movimiento —apoyar y arrastrar—, así que no pueden convivir. Con
   * un modo explícito quien lee sabe en cuál está; sin él, arrastrar haría una
   * cosa u otra según un estado invisible, que es justo la interfaz "secreta"
   * que este proyecto persigue.
   */
  dibujando: boolean;
}

/** Lo que el componente tiene que ejecutar tras aplicar un comando. */
export type Efecto =
  | { tipo: 'pagina'; direccion: 1 | -1 }
  | { tipo: 'capitulo'; direccion: 1 | -1 }
  | { tipo: 'avance' }
  | { tipo: 'consultar'; seleccion: string }
  | {
      tipo: 'anotar';
      clase: 'note' | 'highlight' | 'underline';
      cita: string;
      cuerpo: string | null;
    }
  | { tipo: 'marcador' }
  | { tipo: 'pantalla-completa'; activa: boolean }
  | { tipo: 'donde-estoy' };

/**
 * Marca la selección con una clase, o no hace nada si no hay selección.
 *
 * Sin selección no se marca: marcar sin decir qué se marca produciría una
 * anotación anclada a lo que hubiera debajo del cursor, que no es lo que nadie
 * pidió. Es la misma regla que ya cumplía anotar.
 */
function marcarSeleccion(estado: EstadoDelLector, clase: 'highlight' | 'underline'): Transicion {
  const cita = citaSeleccionada(estado);
  // Marcado lo libre, se suelta: la siguiente tecla vuelve a los bloques.
  return cita
    ? { estado: { ...estado, libre: null }, efecto: { tipo: 'anotar', clase, cita, cuerpo: null } }
    : { estado };
}

/** Resultado de aplicar un comando: el estado nuevo y lo que queda por hacer. */
export interface Transicion {
  estado: EstadoDelLector;
  efecto?: Efecto;
}

/** Estado inicial de un documento recién abierto. */
export function estadoInicial(locator: Locator, bloques: Bloque[] = []): EstadoDelLector {
  return {
    bloques,
    cursor: -1,
    ancla: null,
    inmersivo: false,
    pantallaCompleta: false,
    panel: null,
    redaccion: null,
    locator,
    dibujando: false
  };
}

/** Rango de bloques seleccionado, de menor a mayor, o `null` si no hay ninguno. */
export function seleccion(estado: EstadoDelLector): { desde: number; hasta: number } | null {
  if (estado.cursor < 0 || estado.bloques.length === 0) return null;
  const otro = estado.ancla ?? estado.cursor;
  return {
    desde: Math.min(estado.cursor, otro),
    hasta: Math.max(estado.cursor, otro)
  };
}

/**
 * Texto de lo seleccionado, que es lo que viaja como cita.
 *
 * Los bloques se unen con **un espacio** porque así es como quedan separados en
 * el texto del documento, que el runtime tiene normalizado: el espacio en blanco
 * del marcado no es contenido. El runtime normaliza además la cita que recibe,
 * de modo que un párrafo escrito en varias líneas en el XHTML ancla igual; sin
 * eso no se podría anotar casi ningún EPUB real, que vienen indentados.
 */
export function citaSeleccionada(estado: EstadoDelLector): string {
  const rango = seleccion(estado);
  if (!rango) return '';
  if (estado.libre && estado.libre.trim().length > 0) return estado.libre.trim();
  return estado.bloques
    .slice(rango.desde, rango.hasta + 1)
    .map((b) => b.texto.trim())
    .filter((t) => t.length > 0)
    .join(' ')
    .trim();
}

function acotar(indice: number, total: number): number {
  if (total === 0) return -1;
  return Math.max(0, Math.min(total - 1, indice));
}

/** Mueve el cursor de bloque, soltando la selección extendida. */
function mover(estado: EstadoDelLector, paso: 1 | -1): Transicion {
  const desde = estado.cursor < 0 ? (paso > 0 ? -1 : 0) : estado.cursor;
  return {
    estado: {
      ...estado,
      cursor: acotar(desde + paso, estado.bloques.length),
      // Moverse sin mayúsculas suelta la selección: si no, cada movimiento la
      // arrastraría y el usuario acabaría con medio capítulo seleccionado sin
      // haberlo pedido.
      ancla: null,
      libre: null
    }
  };
}

/** Extiende la selección desde donde estuviera el ancla. */
function extender(estado: EstadoDelLector, paso: 1 | -1): Transicion {
  if (estado.bloques.length === 0) return { estado };
  const cursor = estado.cursor < 0 ? 0 : estado.cursor;
  return {
    estado: {
      ...estado,
      ancla: estado.ancla ?? cursor,
      libre: null,
      cursor: acotar(cursor + paso, estado.bloques.length)
    }
  };
}

/** Guarda la nota en curso, o la descarta si quedó vacía. */
function confirmar(estado: EstadoDelLector): Transicion {
  if (!estado.redaccion) return { estado };

  const cita = citaSeleccionada(estado);
  const cuerpo = estado.redaccion.cuerpo.trim();
  // Una nota vacía no es una nota. Se cancela en lugar de guardar algo que el
  // usuario tendría que borrar después.
  if (!cita || cuerpo.length === 0) return { estado: { ...estado, redaccion: null } };

  return {
    estado: { ...estado, redaccion: null },
    efecto: { tipo: 'anotar', clase: 'note', cita, cuerpo }
  };
}

/**
 * Sale de un escalón, del más interior al más exterior.
 *
 * Uno por pulsación: saltarse escalones haría que Escape a veces sacara al
 * usuario de la lectura sin que lo hubiera pedido.
 */
function salir(estado: EstadoDelLector): Transicion {
  if (estado.redaccion) return { estado: { ...estado, redaccion: null } };
  if (estado.panel) return { estado: { ...estado, panel: null } };
  // Antes que la selección: si se está dibujando, Escape apaga el lápiz, que es
  // lo que quien lo pulsa espera. Dejarlo encendido haría que el siguiente
  // arrastre dibujara sin querer.
  if (estado.dibujando) return { estado: { ...estado, dibujando: false } };
  if (estado.ancla !== null || estado.libre) {
    return { estado: { ...estado, ancla: null, libre: null } };
  }
  if (estado.pantallaCompleta) {
    return {
      estado: { ...estado, pantallaCompleta: false, inmersivo: false },
      efecto: { tipo: 'pantalla-completa', activa: false }
    };
  }
  if (estado.inmersivo) return { estado: { ...estado, inmersivo: false } };
  return { estado };
}

/**
 * Aplica un comando al estado.
 *
 * Nunca falla: un comando que no aplica devuelve el mismo estado. Devolver un
 * error obligaría a cada componente a decidir qué hacer con "has pulsado una
 * tecla que ahora no sirve", y la respuesta correcta siempre es no hacer nada.
 */
export function aplicar(estado: EstadoDelLector, comando: Comando): Transicion {
  switch (comando.id) {
    case 'bloque-siguiente':
      return mover(estado, 1);
    case 'bloque-anterior':
      return mover(estado, -1);
    case 'extender-siguiente':
      return extender(estado, 1);
    case 'extender-anterior':
      return extender(estado, -1);

    case 'pagina-siguiente':
      return { estado, efecto: { tipo: 'pagina', direccion: 1 } };
    case 'pagina-anterior':
      return { estado, efecto: { tipo: 'pagina', direccion: -1 } };
    case 'capitulo-siguiente':
      return { estado, efecto: { tipo: 'capitulo', direccion: 1 } };
    case 'capitulo-anterior':
      return { estado, efecto: { tipo: 'capitulo', direccion: -1 } };
    // Encender o apagar el avance no cambia nada de lo que esta máquina guarda:
    // el ritmo vive en la vista, que es la que sabe desplazar.
    case 'avance-automatico':
      return { estado, efecto: { tipo: 'avance' } };

    case 'anotar':
      // Escribir la nota es un paso más, y por eso el estado guarda la
      // redacción en lugar de crear la anotación aquí.
      return citaSeleccionada(estado)
        ? { estado: { ...estado, redaccion: { cuerpo: '' } } }
        : { estado };

    case 'confirmar':
      return confirmar(estado);

    /*
     * Resaltar y subrayar son hermanos: misma mecánica, distinta clase.
     *
     * Subrayar es clase propia y no una variante, decidido: el gesto
     * es otro —"esta palabra la quiero buscar o traducir"— y las dos tienen que
     * poder convivir sobre las mismas palabras. Por eso son dos comandos y no uno
     * con modificador: un modificador obligaría a saber cuál está puesto antes de
     * marcar, y marcar tiene que ser inmediato.
     */
    /*
     * El lápiz se enciende y se apaga; no dibuja por sí solo.
     *
     * Encenderlo suelta la selección: con texto seleccionado y el lápiz activo,
     * el primer arrastre borraría la selección de todos modos, y verla marcada
     * mientras ya no se puede extender dice algo que no es cierto.
     */
    case 'trazo':
      return {
        estado: { ...estado, dibujando: !estado.dibujando, ancla: null }
      };

    case 'resaltar':
      return marcarSeleccion(estado, 'highlight');

    case 'subrayar':
      return marcarSeleccion(estado, 'underline');

    case 'marcador':
      return { estado, efecto: { tipo: 'marcador' } };

    case 'donde-estoy':
      return { estado, efecto: { tipo: 'donde-estoy' } };

    /*
     * Consultar abre el panel **y pide la consulta**, que es lo que lo separa
     * de los demás paneles: los otros se abren sobre algo que ya está cargado.
     *
     * Cerrar no consulta: sería preguntar otra vez lo mismo al salir. Y una
     * selección vacía no abre nada, por la misma regla que anotar.
     *
     * Lo que ninguna de las dos ramas hace es tocar `locator`, `cursor` ni
     * `ancla`. Ese es el criterio de aceptación —consultar no mueve
     * la posición de lectura ni un locator— convertido en una propiedad de esta
     * máquina, que se puede comprobar sin navegador.
     */
    case 'consultar': {
      if (estado.panel === 'consulta') return { estado: { ...estado, panel: null } };
      const seleccion = citaSeleccionada(estado);
      if (!seleccion) return { estado };
      return { estado: { ...estado, panel: 'consulta' }, efecto: { tipo: 'consultar', seleccion } };
    }

    case 'indice':
    case 'anotaciones':
    case 'voz':
    case 'ajustes-del-libro':
    case 'modo-de-lectura':
    case 'fondo-sonoro':
    case 'buscar':
    case 'estudiar': {
      const destino = comando.panel ?? null;
      // Abrir un panel **no toca el locator ni el cursor**, y por eso está en la
      // misma rama que los demás. Es el criterio de aceptación
      // —consultar no mueve la posición de lectura ni un locator— y aquí es una
      // propiedad de la máquina, no una promesa del componente.
      return { estado: { ...estado, panel: estado.panel === destino ? null : destino } };
    }

    case 'inmersion': {
      const inmersivo = !estado.inmersivo;
      // Entrar en inmersión cierra los paneles: la carcasa se va **entera**, y
      // un panel abierto es carcasa.
      return { estado: { ...estado, inmersivo, panel: inmersivo ? null : estado.panel } };
    }

    case 'acercar':
      return {
        estado: {
          ...estado,
          zoom: Math.min(4, Math.round(((estado.zoom ?? 1) + 0.25) * 100) / 100)
        }
      };
    case 'alejar':
      return {
        estado: {
          ...estado,
          zoom: Math.max(1, Math.round(((estado.zoom ?? 1) - 0.25) * 100) / 100)
        }
      };

    case 'pantalla-completa': {
      const activa = !estado.pantallaCompleta;
      return {
        estado: { ...estado, pantallaCompleta: activa, inmersivo: activa || estado.inmersivo },
        efecto: { tipo: 'pantalla-completa', activa }
      };
    }

    // La división la gobierna la aplicación y no esta máquina: cada mitad es un
    // lector entero con su estado, y meterla aquí obligaría a que un lector
    // supiera de la existencia del otro. La vista se lo cuenta a la carcasa por
    // `alDividir`; lo que esta máquina hace es no tocar nada.
    case 'dividir':
    case 'cambiar-mitad':
    case 'estrechar-mitad':
    case 'ensanchar-mitad':
      return { estado };

    case 'salir':
      return salir(estado);
  }
}

/** Escribe en la nota en curso. Fuera de una redacción no hace nada. */
export function redactar(estado: EstadoDelLector, cuerpo: string): EstadoDelLector {
  if (!estado.redaccion) return estado;
  return { ...estado, redaccion: { cuerpo } };
}

/**
 * Qué zonas de la carcasa se pintan.
 *
 * Un solo predicado para las tres, y no tres condiciones repartidas por el
 * marcado: con tres, olvidar una deja la barra superior encima del texto en modo
 * inmersivo y nadie lo nota hasta verlo. Con uno, la prueba lo comprueba sobre
 * el marcado que sale de verdad.
 */
export function zonasVisibles(estado: EstadoDelLector): {
  banner: boolean;
  navegacion: boolean;
  paneles: boolean;
} {
  const visible = !estado.inmersivo;
  return { banner: visible, navegacion: visible, paneles: visible && estado.panel !== null };
}

/**
 * Selecciona con el ratón: los bloques que toca la selección del
 * navegador, y su texto exacto como cita.
 */
export function seleccionarLibre(
  estado: EstadoDelLector,
  desde: number,
  hasta: number,
  texto: string
): EstadoDelLector {
  const limpio = texto.replace(/\s+/g, ' ').trim();
  if (limpio.length === 0 || desde < 0 || hasta < 0) return { ...estado, libre: null };
  return {
    ...estado,
    cursor: Math.max(desde, hasta),
    ancla: Math.min(desde, hasta),
    libre: limpio
  };
}
