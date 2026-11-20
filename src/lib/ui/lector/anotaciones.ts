/**
 * El panel de anotaciones, como máquina pura.
 *
 * # Por qué esto existe separado del componente
 *
 * El criterio de aceptación dice que una prueba **recorre el panel,
 * salta a una anotación, la edita y la borra sin usar el ratón ni una sola
 * vez**. Una afirmación así solo vale si se puede ejecutar, y aquí se ejecuta:
 * el recorrido entero es una lista de pulsaciones contra [`teclaDelPanel`] y
 * [`aplicarEnPanel`], las dos puras, sin navegador y sin puente.
 *
 * Es la misma forma que hizo demostrable `estado.ts`, y por el mismo
 * motivo: dentro del componente habría que conducir un navegador para verlo, y
 * eso exige conducir el binario, que sigue bloqueado por WebKitWebDriver.
 *
 * # Los efectos son datos
 *
 * Saltar, editar y borrar no llaman al puente: devuelven **qué habría que
 * hacer**. Importa más aquí que en el lector, porque editar una anotación no la
 * modifica —la da de baja y crea su sucesora, con identificador nuevo— y una
 * máquina que llamara al puente tendría que saber esperar la respuesta para
 * quedarse con el identificador bueno.
 *
 * # Por qué el cursor vive aquí y no en el foco del DOM
 *
 * El panel usa **tabindex móvil**: la lista es una sola parada de tabulación y
 * las flechas mueven dentro. Con tres acciones por fila, cuarenta anotaciones
 * serían ciento veinte paradas de tabulación, y "orden de tabulación
 * predecible" es entregable literal de la ficha. Quién está activo es entonces
 * un dato del estado, no una consecuencia de dónde quedó el foco.
 *
 * Contrato: `docs/architecture/reader-contracts.md` y
 * `docs/architecture/accessibility.md`, sección 4.
 */

import type { Locator, OutlineEntry } from '$lib/bindings';

import type { Pulsacion } from './comandos';

/**
 * En qué está el panel.
 *
 * `confirmando` existe porque borrar es destructivo y una sola tecla que
 * destruye no es operable, es peligrosa: quien navega por teclado recorre la
 * lista con las flechas y una pulsación de más no puede costarle una anotación.
 */
export type ModoDelPanel = 'lista' | 'editando' | 'confirmando';

/** Estado completo del panel. */
export interface EstadoDelPanel {
  /** Las anotaciones del documento, en orden de lectura. Las perdidas al final. */
  entradas: OutlineEntry[];
  /** Fila activa. `-1` cuando la lista está vacía. */
  cursor: number;
  modo: ModoDelPanel;
  /** Cuerpo en edición. Vacío fuera del modo `editando`. */
  borrador: string;
}

/** Lo que una tecla pide hacer. Lista cerrada. */
export type AccionDelPanel =
  | 'siguiente'
  | 'anterior'
  | 'primera'
  | 'ultima'
  | 'saltar'
  | 'editar'
  | 'guardar'
  | 'borrar'
  | 'confirmar-borrado'
  | 'cancelar';

/** Lo que el componente tiene que ejecutar contra el puente. */
export type EfectoDelPanel =
  | { tipo: 'saltar'; locator: Locator }
  | { tipo: 'editar'; id: string; cuerpo: string | null }
  | { tipo: 'borrar'; id: string };

/** Resultado de aplicar una acción: el estado nuevo y lo que queda por hacer. */
export interface TransicionDelPanel {
  estado: EstadoDelPanel;
  efecto?: EfectoDelPanel;
}

/** Estado de un panel recién abierto. */
export function panelInicial(entradas: OutlineEntry[] = []): EstadoDelPanel {
  return {
    entradas,
    cursor: entradas.length > 0 ? 0 : -1,
    modo: 'lista',
    borrador: ''
  };
}

/**
 * Sustituye la lista conservando dónde estaba el usuario.
 *
 * Se llama cada vez que el runtime devuelve el panel de nuevo: al anotar, al
 * editar y al borrar. Devolver el cursor a cero en cada recarga haría que
 * borrar la anotación número treinta te dejara en la primera, que es la clase de
 * pérdida de sitio que el producto existe para evitar.
 */
export function conEntradas(estado: EstadoDelPanel, entradas: OutlineEntry[]): EstadoDelPanel {
  return {
    entradas,
    cursor: acotar(estado.cursor, entradas.length),
    // La lista cambió bajo los pies: seguir editando o confirmando un borrado
    // sobre una fila que quizá ya no es la misma es cómo se borra lo que no se
    // quería borrar.
    modo: 'lista',
    borrador: ''
  };
}

/**
 * Pone el cursor sobre una anotación concreta, por identificador.
 *
 * Existe para el gesto de señalar una marca en la página: pulsar un resaltado
 * abre el panel **en esa anotación** y no en la primera. Sin esto, señalar
 * contestaba "está entre estas cuarenta", que es casi lo mismo que no contestar.
 *
 * Si no está en la lista no se mueve: una anotación que no aparece es una que se
 * acaba de borrar, y saltar a otra fila sería peor que quedarse.
 */
export function conCursorEn(estado: EstadoDelPanel, id: string): EstadoDelPanel {
  const cursor = estado.entradas.findIndex((entrada) => entrada.annotation.id === id);
  if (cursor < 0) return estado;
  return { ...estado, cursor, modo: 'lista', borrador: '' };
}

function acotar(indice: number, total: number): number {
  if (total === 0) return -1;
  return Math.max(0, Math.min(total - 1, indice));
}

/** La fila activa, o `null` si la lista está vacía. */
export function entradaActiva(estado: EstadoDelPanel): OutlineEntry | null {
  if (estado.cursor < 0) return null;
  return estado.entradas[estado.cursor] ?? null;
}

/**
 * Si se puede saltar a una entrada.
 *
 * Una anotación **perdida** no ofrece salto: `lost` significa que ninguna vía de
 * la cascada supo colocarla en este documento, así que el salto
 * fallaría. Un control que falla al pulsarlo es peor que no ofrecerlo, que es la
 * misma regla de omitir en lugar de atenuar.
 */
export function sePuedeSaltar(entrada: OutlineEntry): boolean {
  return !entrada.lost;
}

/**
 * Si se puede editar una entrada.
 *
 * Un marcador no: no lleva texto, y darle uno lo convertiría en una nota
 * disfrazada de marcador. Lo que se hace con un marcador que sobra es borrarlo,
 * y eso sí se ofrece.
 */
export function sePuedeEditar(entrada: OutlineEntry): boolean {
  return entrada.annotation.kind !== 'bookmark';
}

/** Lo que hace cada tecla en la lista. Tabla y no cadena de condiciones. */
const EN_LA_LISTA: Record<string, AccionDelPanel> = {
  ArrowDown: 'siguiente',
  ArrowUp: 'anterior',
  Home: 'primera',
  End: 'ultima',
  Enter: 'saltar',
  // Las dos teclas de borrado: los portátiles de Apple no traen Supr, y dejar
  // el borrado sin tecla en un teclado entero incumpliría el criterio justo
  // donde más se nota.
  Delete: 'borrar',
  Backspace: 'borrar'
};

function enLaLista(pulsacion: Pulsacion): AccionDelPanel | null {
  const directa = EN_LA_LISTA[pulsacion.key];
  if (directa) return directa;
  // Bloq Mayús puesto entrega `E`, y el panel dejaría de responder sin motivo
  // visible. Es el mismo cuidado que tiene `comandoParaTecla`.
  return pulsacion.key.toLowerCase() === 'e' ? 'editar' : null;
}

/**
 * Escribiendo, el teclado es del campo: las letras son letras.
 *
 * Pasan las dos mismas de siempre, guardar y salir, que es lo que impide
 * quedarse encerrado en un campo sin ratón con el que salir.
 */
function escribiendo(pulsacion: Pulsacion, control: boolean): AccionDelPanel | null {
  if (pulsacion.key === 'Enter' && control) return 'guardar';
  if (pulsacion.key === 'Escape') return 'cancelar';
  return null;
}

function confirmandoElBorrado(pulsacion: Pulsacion, control: boolean): AccionDelPanel | null {
  // Enter a secas confirma; el de guardar no vale aquí, porque son dos gestos
  // distintos y confundirlos borra lo que no se quería borrar.
  if (pulsacion.key === 'Enter' && !control) return 'confirmar-borrado';
  if (pulsacion.key === 'Escape') return 'cancelar';
  return null;
}

/**
 * Qué acción pide una pulsación dentro del panel, o `null` si ninguna.
 *
 * Solo devuelve algo para las teclas que el panel **consume**. Importa que la
 * lista sea corta: lo que no está aquí sigue subiendo hasta el lector, y así la
 * tecla del índice o la de pantalla completa siguen funcionando con el panel
 * abierto. Lo que sí está aquí, el componente detiene, porque las flechas y
 * Enter significan otra cosa en el texto y sin detenerlas se dispararían las
 * dos.
 */
export function teclaDelPanel(pulsacion: Pulsacion, modo: ModoDelPanel): AccionDelPanel | null {
  // Se compara con Ctrl o con Meta: en macOS la tecla de los atajos es Command,
  // y exigir Ctrl allí dejaría guardar sin atajo.
  const control = Boolean(pulsacion.ctrlKey || pulsacion.metaKey);

  if (modo === 'editando') return escribiendo(pulsacion, control);
  if (modo === 'confirmando') return confirmandoElBorrado(pulsacion, control);
  // Con Control pulsado, el atajo es del lector: buscar y cambiar de mitad
  // tienen que seguir funcionando con el panel abierto.
  return control ? null : enLaLista(pulsacion);
}

/**
 * Aplica una acción al panel.
 *
 * Nunca falla: una acción que no aplica devuelve el mismo estado. Es la misma
 * decisión que en `estado.ts`, y por el mismo motivo: la respuesta correcta a
 * "has pulsado una tecla que ahora no sirve" siempre es no hacer nada.
 */
export function aplicarEnPanel(estado: EstadoDelPanel, accion: AccionDelPanel): TransicionDelPanel {
  const total = estado.entradas.length;
  const activa = entradaActiva(estado);

  switch (accion) {
    // El cursor no da la vuelta. En una lista que está en orden de lectura,
    // pasar del final al principio te manda al otro extremo del libro sin
    // haberlo pedido y sin nada que lo anuncie.
    case 'siguiente':
      return { estado: { ...estado, cursor: acotar(estado.cursor + 1, total) } };
    case 'anterior':
      return { estado: { ...estado, cursor: acotar(estado.cursor - 1, total) } };
    case 'primera':
      return { estado: { ...estado, cursor: acotar(0, total) } };
    case 'ultima':
      return { estado: { ...estado, cursor: acotar(total - 1, total) } };

    case 'saltar':
      if (!activa || !sePuedeSaltar(activa)) return { estado };
      return { estado, efecto: { tipo: 'saltar', locator: activa.annotation.target } };

    case 'editar':
      if (!activa || !sePuedeEditar(activa)) return { estado };
      return {
        estado: { ...estado, modo: 'editando', borrador: activa.annotation.body ?? '' }
      };

    case 'guardar':
      return guardar(estado, activa);

    case 'borrar':
      if (!activa) return { estado };
      return { estado: { ...estado, modo: 'confirmando' } };

    case 'confirmar-borrado':
      if (!activa || estado.modo !== 'confirmando') return { estado };
      return {
        estado: { ...estado, modo: 'lista' },
        efecto: { tipo: 'borrar', id: activa.annotation.id }
      };

    case 'cancelar':
      return { estado: { ...estado, modo: 'lista', borrador: '' } };
  }
}

/**
 * Guarda la edición en curso.
 *
 * **Un cuerpo vacío no significa lo mismo en las dos clases**, y tratarlo igual
 * perdería datos en una de ellas:
 *
 * * En un **resaltado**, vaciar el comentario es legítimo: un resaltado sin
 *   comentario sigue siendo un resaltado, y quitarle el texto es una operación
 *   que el usuario puede querer. Se guarda como nulo.
 * * En una **nota**, no: una nota sin texto no es una nota, es una fila que el
 *   usuario tendría que borrar después. Se cancela la edición sin guardar, y
 *   para quitarla de en medio ya está el borrado, que además pide confirmación.
 */
function guardar(estado: EstadoDelPanel, activa: OutlineEntry | null): TransicionDelPanel {
  if (!activa || estado.modo !== 'editando') return { estado };

  const cuerpo = estado.borrador.trim();
  const limpio = { ...estado, modo: 'lista' as const, borrador: '' };

  if (cuerpo.length === 0 && activa.annotation.kind === 'note') return { estado: limpio };

  // Guardar lo mismo que ya había crearía una sucesora idéntica y dejaría la
  // anterior dada de baja en el historial, que es ruido sin información.
  if (cuerpo === (activa.annotation.body ?? '')) return { estado: limpio };

  return {
    estado: limpio,
    efecto: {
      tipo: 'editar',
      id: activa.annotation.id,
      cuerpo: cuerpo.length === 0 ? null : cuerpo
    }
  };
}

/** Escribe en la edición en curso. Fuera del modo `editando` no hace nada. */
export function escribir(estado: EstadoDelPanel, borrador: string): EstadoDelPanel {
  if (estado.modo !== 'editando') return estado;
  return { ...estado, borrador };
}
