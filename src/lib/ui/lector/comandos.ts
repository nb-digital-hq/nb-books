/**
 * El registro de comandos del lector: qué se puede hacer y con qué tecla.
 *
 * # Por qué existe este archivo y no una tecla suelta en cada componente
 *
 * El criterio de aceptación dice que el flujo completo de lectura y
 * anotación es operable **solo con teclado**. Una afirmación así solo vale si se
 * puede comprobar, y no se puede comprobar mirando componentes uno por uno: hace
 * falta un sitio donde estén todas las acciones y sus teclas a la vez.
 *
 * Este es ese sitio, y de él salen las dos cosas:
 *
 * 1. **Los botones que se pintan.** La barra del lector recorre este registro;
 *    no hay forma de añadir un botón sin comando, ni un comando sin atajo.
 * 2. **Lo que hace una tecla.** [`comandoParaTecla`] es una función pura, así
 *    que el flujo entero se puede ejecutar en una prueba como una secuencia de
 * pulsaciones, sin navegador y sin puente. Es lo mismo que se hizo con la
 *    aritmética de la ventana virtualizada.
 *
 * # La dirección de la dependencia, otra vez
 *
 * Un comando declara **de qué capacidad depende**, y las capacidades las declara
 * el proveedor del formato (`reader-contracts.md`, sección 4). La interfaz no
 * decide que un cómic no tiene tipografía: lo lee. Un control cuya capacidad
 * falta **se omite**, no se atenúa.
 *
 * Contrato: `docs/architecture/ui-components.md` y
 * `docs/adr/012-ui-paradigm.md`.
 */

import { type ControlDelLector, controlesDelLector } from '$core/reader';
import type { ReaderCapabilities } from '$lib/bindings';

import type { NombreDeIcono } from '../iconos';

/** Todo lo que el lector sabe hacer. Lista cerrada. */
export type IdDeComando =
  | 'pagina-siguiente'
  | 'pagina-anterior'
  | 'capitulo-siguiente'
  | 'capitulo-anterior'
  | 'avance-automatico'
  | 'bloque-siguiente'
  | 'bloque-anterior'
  | 'extender-siguiente'
  | 'extender-anterior'
  | 'anotar'
  | 'consultar'
  | 'confirmar'
  | 'resaltar'
  | 'subrayar'
  | 'trazo'
  | 'marcador'
  | 'donde-estoy'
  | 'anotaciones'
  | 'voz'
  | 'indice'
  | 'ajustes-del-libro'
  | 'modo-de-lectura'
  | 'fondo-sonoro'
  | 'buscar'
  | 'estudiar'
  | 'inmersion'
  | 'pantalla-completa'
  | 'acercar'
  | 'alejar'
  | 'dividir'
  | 'cambiar-mitad'
  | 'estrechar-mitad'
  | 'ensanchar-mitad'
  | 'salir';

/**
 * Las familias de la barra del lector, en el orden en que se pintan.
 *
 * # Por qué la barra va agrupada y no seguida
 *
 * Porque sin agrupar era una hilera de veintidós iconos del mismo tamaño, el
 * mismo peso y sin ninguna separación, y una hilera así no se lee: se recorre
 * entera cada vez. Los lectores de referencia reparten estas mismas acciones en
 * dos barras y un panel —Foliate y Readest ponen la navegación abajo, las
 * herramientas arriba y la anotación en el sitio de la selección—, y lo que
 * tienen en común no es dónde las ponen, es que **nunca están todas juntas sin
 * separar**.
 *
 * El orden va de lo que más se usa a lo que menos, que es el orden en que se
 * recorre una barra: moverse, actuar sobre lo elegido, abrir un panel, cambiar
 * cómo se ve, salir.
 */
export type GrupoDeComando = 'navegacion' | 'seleccion' | 'paneles' | 'presentacion' | 'salida';

/** El orden de las familias. Estable: un control que se mueve hay que buscarlo. */
export const GRUPOS: readonly GrupoDeComando[] = [
  'navegacion',
  'seleccion',
  'paneles',
  'presentacion',
  'salida'
];

/** Una combinación de teclas. */
export interface Atajo {
  /** Valor de `KeyboardEvent.key`, tal cual. */
  tecla: string;
  mayus?: boolean;
  control?: boolean;
}

/**
 * Qué panel del lector está abierto.
 *
 * Los paneles son zonas `complementary`, no diálogos: no atrapan el foco. Se
 * cierran con Escape porque eso ayuda, no porque sean modales.
 */
export type PanelDelLector =
  | 'indice'
  | 'anotaciones'
  | 'consulta'
  | 'voz'
  | 'ajustes-del-libro'
  | 'modo-de-lectura'
  | 'fondo-sonoro'
  | 'buscar'
  | 'estudio';

/** Un comando del lector. */
export interface Comando {
  id: IdDeComando;
  atajo: Atajo;
  /**
   * Control del que depende, si depende de alguno.
   *
   * Se resuelve contra `controlesDelLector`, que es la traducción de las
   * capacidades que declara el proveedor. Un comando sin esto existe siempre.
   */
  requiere?: ControlDelLector;
  /** Si necesita que haya algo seleccionado para tener sentido. */
  necesitaSeleccion?: boolean;
  /**
   * Si sigue valiendo mientras se escribe en un campo.
   *
   * Solo dos: guardar y salir. Sin ellos, quien escribe una nota tendría que
   * alcanzar un botón con el ratón para cerrarla, que es exactamente la acción
   * exclusiva de ratón que el criterio prohíbe.
   */
  enCampo?: boolean;
  /** Si abre o cierra un panel, cuál. */
  panel?: PanelDelLector;
  /** La familia con la que se agrupa en la barra. */
  grupo: GrupoDeComando;
  /**
   * Solo tiene sentido con la vista dividida abierta.
   *
   * Cambiar de mitad cuando hay una sola mitad, o mover un separador que no
   * existe, son dos formas de ofrecer un control inerte. Se omiten, que es la
   * regla aplicada a un estado en lugar de a una capacidad.
   */
  soloDividido?: boolean;
  /**
   * Solo se ofrece si hay quien genere.
   *
   * La IA es opcional y viene apagada: un botón permanente que abre un panel para
   * decir "no hay IA" es el anuncio de un componente ajeno que este proyecto ya
   * quitó una vez. Con alguien que genere, se ofrece; sin él, ni botón ni tecla.
   */
  soloConIa?: boolean;
  /**
   * Si se pinta como botón.
   *
   * Uno solo dice que no: **guardar la nota**. Existe como tecla porque
   * `Ctrl+Intro` tiene que funcionar mientras se escribe, pero su botón vivía en
   * la barra permanentemente, con el aspecto de una acción disponible, y no
   * hacía nada salvo en el único momento en que el campo de la nota está
   * abierto; y en ese momento el propio campo ya trae su botón de guardar. Un
   * control siempre visible que casi siempre no hace nada es la promesa a medias
   * que ADR-012 prohíbe, escrita al revés.
   */
  enBarra?: boolean;
  /**
   * Icono del catálogo.
   *
   * Obligatorio: en NB Books no hay letras ni emojis haciendo de icono, así que
   * un comando sin icono no se podría pintar en la barra.
   */
  icono: NombreDeIcono;
}

/**
 * Todos los comandos, en el orden en que se pintan.
 *
 * El panel de anotaciones ocupa la tecla `A` que se le
 * había reservado. **No declara `requiere`**, y no es un descuido: un marcador
 * no necesita seleccionar texto, así que un cómic sin capa de texto tiene
 * anotaciones que listar. Condicionarlo a `anotaciones` escondería el panel
 * justo en los documentos donde el marcador es la única forma de volver.
 *
 * El orden es estable: un control que cambia de sitio entre documentos obliga a
 * buscarlo cada vez.
 *
 * **Anotar antes que resaltar, y con la tecla de aceptar.** No es una
 * preferencia estética: la investigación de la experiencia de lectura, sección
 * 1.1, dice que resaltar por sí solo no mejora el aprendizaje y que quien lee en
 * digital casi no subraya, y de ahí sale un mandato vinculante: anotar tiene que
 * costar **menos** que resaltar. Aquí eso significa que desde una selección se
 * escribe una nota directamente con Enter, sin tener que resaltar primero, que
 * es lo que obligan a hacer casi todos los lectores.
 */
export const COMANDOS: readonly Comando[] = [
  {
    id: 'anotar',
    icono: 'nota',
    atajo: { tecla: 'Enter' },
    requiere: 'anotaciones',
    necesitaSeleccion: true,
    grupo: 'seleccion'
  },
  {
    id: 'confirmar',
    icono: 'comprobado',
    atajo: { tecla: 'Enter', control: true },
    enCampo: true,
    grupo: 'seleccion',
    enBarra: false
  },
  {
    // Consultar va inmediatamente después de anotar porque comparte con él la
    // condición —hay algo seleccionado— y porque es la otra cosa que se hace
    // sobre una selección. Depende de `seleccion` y no de un control propio: sin
    // poder seleccionar texto no hay nada que consultar, y un cómic sin capa de
    // texto no debe enseñar el control.
    id: 'consultar',
    icono: 'diccionario',
    atajo: { tecla: 'c' },
    requiere: 'seleccion',
    necesitaSeleccion: true,
    panel: 'consulta',
    grupo: 'seleccion'
  },
  {
    id: 'resaltar',
    icono: 'resaltar',
    atajo: { tecla: 'r' },
    requiere: 'anotaciones',
    necesitaSeleccion: true,
    grupo: 'seleccion'
  },
  {
    // Subrayar va junto a resaltar y con la misma forma: son hermanos, no uno
    // dentro del otro. `s` de subrayar, que estaba libre.
    id: 'subrayar',
    icono: 'subrayar',
    atajo: { tecla: 's' },
    requiere: 'anotaciones',
    necesitaSeleccion: true,
    grupo: 'seleccion'
  },
  {
    /*
     * El lápiz no necesita selección: se enciende y se dibuja donde sea.
     *
     * Por eso no lleva `necesitaSeleccion` y va en el grupo de la selección de
     * todos modos: es una forma de marcar el texto, y estar donde están las otras
     * es lo que hace que se encuentre.
     */
    id: 'trazo',
    icono: 'trazoLibre',
    atajo: { tecla: 'l' },
    requiere: 'anotaciones',
    grupo: 'seleccion'
  },
  {
    id: 'marcador',
    icono: 'marcador',
    atajo: { tecla: 'b' },
    grupo: 'navegacion'
  },
  {
    /*
     * "¿Dónde estoy?": anuncia capítulo, encabezado y posición para lectores de
     * pantalla sin tener que recorrer el documento.
     *
     * Tecla `p` de posición / paradero, que estaba libre.
     */
    id: 'donde-estoy',
    icono: 'progreso',
    atajo: { tecla: 'p' },
    grupo: 'navegacion'
  },
  {
    id: 'bloque-anterior',
    icono: 'arriba',
    atajo: { tecla: 'ArrowUp' },
    requiere: 'seleccion',
    grupo: 'navegacion',
    // Solo tecla: con la página arriba y abajo en la barra, dos botones más
    // para moverse por párrafos partían la barra en dos filas a 1100 px.
    enBarra: false
  },
  {
    id: 'bloque-siguiente',
    icono: 'abajo',
    atajo: { tecla: 'ArrowDown' },
    requiere: 'seleccion',
    grupo: 'navegacion',
    // Solo tecla: con la página arriba y abajo en la barra, dos botones más
    // para moverse por párrafos partían la barra en dos filas a 1100 px.
    enBarra: false
  },
  {
    id: 'extender-anterior',
    icono: 'extenderArriba',
    atajo: { tecla: 'ArrowUp', mayus: true },
    requiere: 'seleccion',
    grupo: 'seleccion'
  },
  {
    id: 'extender-siguiente',
    icono: 'extenderAbajo',
    atajo: { tecla: 'ArrowDown', mayus: true },
    requiere: 'seleccion',
    grupo: 'seleccion'
  },
  {
    id: 'pagina-anterior',
    icono: 'izquierda',
    atajo: { tecla: 'ArrowLeft' },
    grupo: 'navegacion'
  },
  {
    id: 'pagina-siguiente',
    icono: 'derecha',
    atajo: { tecla: 'ArrowRight' },
    grupo: 'navegacion'
  },
  /*
   * Saltar de capítulo y el avance automático nacieron con Leer en el PC
   * (ADR-016): el teléfono los pide como órdenes, y una orden remota que no
   * tuviera tecla sería una acción que el teclado no alcanza. Solo tecla: la
   * barra ya tiene pasar página, y dos botones más la partían en dos filas.
   *
   * `Mayús` con las flechas laterales porque esas flechas solas ya pasan
   * página y la comparación de atajos exige que el modificador coincida.
   */
  {
    id: 'capitulo-anterior',
    icono: 'atras',
    atajo: { tecla: 'ArrowLeft', mayus: true },
    grupo: 'navegacion',
    enBarra: false
  },
  {
    id: 'capitulo-siguiente',
    icono: 'adelante',
    atajo: { tecla: 'ArrowRight', mayus: true },
    grupo: 'navegacion',
    enBarra: false
  },
  {
    // `g` de girar la página sola: las letras con más sentido ya tenían dueño.
    id: 'avance-automatico',
    icono: 'reproducir',
    atajo: { tecla: 'g' },
    grupo: 'navegacion',
    enBarra: false
  },
  {
    id: 'indice',
    icono: 'indice',
    atajo: { tecla: 'i' },
    panel: 'indice',
    grupo: 'paneles'
  },
  {
    id: 'anotaciones',
    icono: 'anotaciones',
    atajo: { tecla: 'a' },
    panel: 'anotaciones',
    grupo: 'paneles'
  },
  {
    id: 'voz',
    icono: 'voz',
    atajo: { tecla: 'v' },
    // Depende de que haya texto que decir, no de que exista audio sincronizado:
    // el audio sincronizado es la narración que trae el propio libro, y esto es
    // sintetizar la que no trae. Un escaneado sin OCR no ofrece el control, y el
    // motivo se dice en la superficie de lectura.
    requiere: 'lectura-en-voz-alta',
    panel: 'voz',
    grupo: 'paneles'
  },
  {
    id: 'buscar',
    icono: 'buscar',
    atajo: { tecla: 'f', control: true },
    requiere: 'busqueda',
    panel: 'buscar',
    grupo: 'paneles'
  },
  {
    id: 'estudiar',
    icono: 'ia',
    atajo: { tecla: 'e' },
    // Sin texto no hay nada que resumir: la misma condición que buscar.
    requiere: 'busqueda',
    panel: 'estudio',
    grupo: 'paneles',
    soloConIa: true
  },
  {
    id: 'ajustes-del-libro',
    icono: 'tipografia',
    atajo: { tecla: 't' },
    requiere: 'tipografia',
    panel: 'ajustes-del-libro',
    grupo: 'presentacion'
  },
  {
    id: 'modo-de-lectura',
    icono: 'modoLectura',
    atajo: { tecla: 'm' },
    panel: 'modo-de-lectura',
    grupo: 'presentacion'
  },
  {
    id: 'fondo-sonoro',
    icono: 'fondoSonoro',
    // "n" de ruido: la r ya la tiene resaltar y la s, subrayar. Toda orden
    // lleva atajo, y hay una prueba que lo exige: sin él, la única forma de
    // llegar sería el ratón, que es lo que prohíbe la restricción 5 de ADR-012.
    atajo: { tecla: 'n' },
    panel: 'fondo-sonoro',
    grupo: 'presentacion'
  },
  {
    id: 'dividir',
    icono: 'vistaDividida',
    atajo: { tecla: 'd' },
    grupo: 'presentacion'
  },
  {
    id: 'cambiar-mitad',
    icono: 'cambiarMitad',
    atajo: { tecla: 'Tab', control: true },
    grupo: 'presentacion',
    soloDividido: true
  },
  /*
   * Mover el separador. Existen porque el contrato dice que el
   * reparto es aritmética y no un arrastre, "así que el teclado lo mueve igual
   * que el ratón": sin estos dos esa frase era falsa —no había forma ninguna de
   * moverlo— y `repartir` era una función exportada, probada y sin llamar.
   *
   * Con `Control` porque las flechas solas ya pasan página, y la comparación de
   * atajos exige que el modificador coincida exactamente, así que no se pisan.
   */
  {
    id: 'estrechar-mitad',
    icono: 'contraer',
    atajo: { tecla: 'ArrowLeft', control: true },
    grupo: 'presentacion',
    soloDividido: true
  },
  {
    id: 'ensanchar-mitad',
    icono: 'expandir',
    atajo: { tecla: 'ArrowRight', control: true },
    grupo: 'presentacion',
    soloDividido: true
  },
  {
    id: 'inmersion',
    icono: 'inmersion',
    atajo: { tecla: 'z' },
    grupo: 'presentacion'
  },
  {
    // Ampliar una página que se pinta como imagen. El control
    // `ajuste-de-pagina` existía y ningún comando lo usaba: un PDF
    // solo se podía leer al ancho de la columna.
    id: 'acercar',
    icono: 'expandir',
    atajo: { tecla: '+' },
    requiere: 'ajuste-de-pagina',
    grupo: 'presentacion'
  },
  {
    id: 'alejar',
    icono: 'contraer',
    atajo: { tecla: '-' },
    requiere: 'ajuste-de-pagina',
    grupo: 'presentacion'
  },
  {
    id: 'pantalla-completa',
    icono: 'pantallaCompleta',
    atajo: { tecla: 'F11' },
    grupo: 'presentacion'
  },
  {
    id: 'salir',
    icono: 'cerrar',
    atajo: { tecla: 'Escape' },
    enCampo: true,
    grupo: 'salida',
    // Solo tecla: la barra tenía su cruz y la carcasa otra arriba, y las
    // dos parecían lo mismo. Cerrar el documento es la de la carcasa.
    enBarra: false
  }
];

/** Lo que hace falta saber para decidir si una tecla hace algo. */
export interface ContextoDeTeclado {
  /** Lo que el proveedor declara que este documento sabe hacer. */
  capacidades: ReaderCapabilities;
  /** Si hay uno o más bloques seleccionados. */
  haySeleccion: boolean;
  /**
   * Si el foco está dentro de un campo de texto.
   *
   * Con el foco escribiendo, las letras son letras: si `r` resaltara mientras se
   * escribe una nota, escribir "arrastrar" dispararía cuatro resaltados.
   */
  escribiendo: boolean;
  /** Si hay quien genere las funciones de estudio. */
  hayIa?: boolean;
}

/**
 * Comandos que existen para un documento con estas capacidades.
 *
 * Es lo que pinta la barra del lector. Lo que no está aquí **no se pinta**, ni
 * en gris: un control deshabilitado es una promesa a medias, el usuario lo ve,
 * lo intenta y no pasa nada.
 */
/**
 * La pulsación con la que se invoca un comando.
 *
 * Existe para que quien quiera disparar un comando desde algo que no es el
 * teclado —una zona táctil, un botón— no tenga que escribir su tecla a mano.
 * Escrita a mano, la tecla y el comando se separan el día que uno de los dos
 * cambia, y nadie se entera hasta que el gesto deja de hacer nada.
 */
export function pulsacionDe(id: IdDeComando): Pulsacion {
  const comando = COMANDOS.find((c) => c.id === id);
  if (!comando) throw new Error(`no hay comando ${id}`);
  return {
    key: comando.atajo.tecla,
    ctrlKey: comando.atajo.control ?? false,
    shiftKey: comando.atajo.mayus ?? false
  };
}

export function comandosDisponibles(capacidades: ReaderCapabilities): Comando[] {
  const controles = controlesDelLector(capacidades);
  return COMANDOS.filter((c) => !c.requiere || controles.includes(c.requiere));
}

/** Lo que la barra necesita saber del momento para decidir qué ofrece. */
export interface ContextoDeLaBarra {
  capacidades: ReaderCapabilities;
  /** Si hay uno o más bloques elegidos. */
  haySeleccion: boolean;
  /** Si la vista dividida está abierta. */
  dividido: boolean;
  /** Si la ventana da para dos columnas de lectura. */
  cabeDividir: boolean;
  /** Si hay quien genere las funciones de estudio. */
  hayIa?: boolean;
}

/** Una familia de la barra, con lo que hay que pintar dentro. */
export interface FamiliaDeLaBarra {
  grupo: GrupoDeComando;
  comandos: Comando[];
}

/**
 * Lo que la barra del lector pinta, ya agrupado y sin lo que no aplica.
 *
 * Dos cosas que esta función quita y que antes se pintaban siempre:
 *
 * 1. **Los comandos que exigen una selección, cuando no hay ninguna.** Antes se
 *    veían los cuatro —anotar, consultar, resaltar y extender— con el aspecto de
 *    cualquier otro botón, y pulsarlos no hacía nada. Un control que no hace
 *    nada al pulsarlo enseña al usuario que la aplicación no responde, que es
 *    peor que no ofrecerlo.
 * 2. **El botón de guardar la nota**, que solo tiene sentido con el campo
 *    abierto y que ese campo ya trae.
 *
 * Las familias vacías no se devuelven: una separación sin nada a un lado es una
 * raya suelta.
 */
export function barraDelLector(contexto: ContextoDeLaBarra): FamiliaDeLaBarra[] {
  const visibles = comandosDisponibles(contexto.capacidades).filter((c) => {
    if (c.enBarra === false) return false;
    // Lo que solo sirve con dos mitades no se ofrece con una.
    if (c.soloDividido && !contexto.dividido) return false;
    if (c.soloConIa && !contexto.hayIa) return false;
    // Y en una ventana estrecha no se ofrece dividir: dos columnas de lectura
    // ahí no son dos columnas de lectura, son dos columnas ilegibles.
    if (c.id === 'dividir' && !contexto.cabeDividir) return false;
    return true;
  });

  return GRUPOS.map((grupo) => ({
    grupo,
    // La familia de la selección entera aparece y desaparece con la selección,
    // en lugar de hacerlo comando a comando. Es lo mismo que hace el emergente
    // de selección de Foliate, y tiene una ventaja sobre esconder cada botón por
    // separado: la barra no cambia de longitud a trocitos mientras se lee, así
    // que los controles que siempre están no se mueven de sitio.
    //
    // El teclado no depende de esto: `Mayús+Abajo` sigue respondiendo lo mismo
    // que respondía, y quien decide si una tecla hace algo es `comandoParaTecla`.
    comandos:
      grupo === 'seleccion' && !contexto.haySeleccion
        ? []
        : visibles.filter((c) => c.grupo === grupo)
  })).filter((familia) => familia.comandos.length > 0);
}

/** Forma mínima de un evento de teclado. Permite probar sin navegador. */
export interface Pulsacion {
  key: string;
  shiftKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  altKey?: boolean;
}

function coincide(atajo: Atajo, pulsacion: Pulsacion): boolean {
  // Las teclas de letra se comparan sin distinguir mayúsculas: con Bloq Mayús
  // puesto, `r` llega como `R` y el lector dejaría de responder sin motivo
  // visible.
  const tecla = atajo.tecla.length === 1 ? pulsacion.key.toLowerCase() : pulsacion.key;
  if (tecla !== atajo.tecla) return false;

  // El control se compara con `Ctrl` o con `Meta`: en macOS la tecla de los
  // atajos es Command, y exigir Ctrl allí dejaría la búsqueda sin atajo.
  const control = Boolean(pulsacion.ctrlKey || pulsacion.metaKey);
  if (Boolean(atajo.control) !== control) return false;

  // Mayúsculas sí se exige exacta: extender la selección y moverse son dos
  // acciones distintas que se distinguen justo por eso.
  //
  // Un atajo de letra minúscula llega con `shiftKey` cierto cuando el usuario
  // teclea la mayúscula, así que solo se compara donde el atajo lo declara.
  if (atajo.mayus !== undefined && atajo.mayus !== Boolean(pulsacion.shiftKey)) return false;
  if (atajo.mayus === undefined && atajo.tecla.length > 1 && pulsacion.shiftKey) return false;

  return true;
}

/**
 * Qué comando dispara una pulsación, o `null` si ninguno.
 *
 * Es la función que hace comprobable el criterio de teclado: el flujo completo
 * se ejecuta como una lista de pulsaciones y se afirma sobre el resultado, en
 * lugar de mirar la pantalla.
 */
export function comandoParaTecla(
  pulsacion: Pulsacion,
  contexto: ContextoDeTeclado
): Comando | null {
  // Escribiendo, el teclado es del campo: las letras son letras. Solo pasan
  // guardar y salir, que es lo que impide quedarse encerrado en un campo sin
  // poder cerrarlo si no se tiene ratón.
  if (contexto.escribiendo) {
    return COMANDOS.find((c) => c.enCampo && coincide(c.atajo, pulsacion)) ?? null;
  }

  const disponibles = comandosDisponibles(contexto.capacidades);
  const candidato = disponibles.find((c) => coincide(c.atajo, pulsacion));
  if (!candidato) return null;

  // Un comando que necesita selección y no la tiene no es un comando que falla:
  // es un comando que todavía no aplica, y la tecla no hace nada.
  if (candidato.necesitaSeleccion && !contexto.haySeleccion) return null;
  if (candidato.soloConIa && !contexto.hayIa) return null;

  return candidato;
}

/**
 * Cómo se escribe un atajo para mostrarlo junto a su acción.
 *
 * `nombres` traduce las teclas con nombre: la ayuda decía "(ArrowUp)" y
 * "Escape" en una interfaz en español. Sin él, el nombre del navegador.
 */
export function atajoLegible(atajo: Atajo, nombres: Readonly<Record<string, string>> = {}): string {
  const partes: string[] = [];
  if (atajo.control) partes.push(nombres.Control ?? 'Ctrl');
  if (atajo.mayus) partes.push(nombres.Shift ?? 'Mayús');
  partes.push(
    atajo.tecla.length === 1 ? atajo.tecla.toUpperCase() : (nombres[atajo.tecla] ?? atajo.tecla)
  );
  return partes.join('+');
}
