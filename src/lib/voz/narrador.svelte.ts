/**
 * El narrador: la voz de la aplicación, que es **una sola**.
 *
 * # Por qué un singleton y no un narrador por lector
 *
 * Porque hay una salida de audio. Con la vista dividida hay dos lectores, pero
 * no hay dos voces: narrar los dos documentos a la vez no se entiende, y dejar
 * que cada mitad tenga su narrador obligaría a inventar quién calla a quién. Con
 * uno, empezar a narrar en una mitad para la otra, que es lo que ocurre en
 * cualquier reproductor y lo que el usuario espera.
 *
 * # Qué pone cada parte
 *
 * * La máquina de `narracion.ts` decide **qué pasa**: es pura y se prueba sin
 *   navegador.
 * * El proveedor de `contrato.ts` hace que suene.
 * * Este archivo es el pegamento, y no toma ninguna decisión propia: si algo se
 *   decide aquí, está en el sitio equivocado.
 *
 * Contrato: `docs/architecture/voice-services.md`.
 */

import type { Locator } from '$lib/bindings';

import type { DeclaracionDeVoz, ProveedorDeVoz, VozDisponible } from './contrato';
import { elegirVoz as deducirVoz } from './eleccion';
import { enFrases } from './frases';
import {
  ajustar,
  alternar,
  avanzar,
  type BloqueNarrable,
  type EfectoDeVoz,
  type EstadoDeNarracion,
  narracionInicial,
  parar,
  type TransicionDeVoz,
  terminarTramo
} from './narracion';
import { VozDelRuntime } from './runtime';
import { VozDelSistema } from './sistema';

/**
 * El proveedor que no sale del dispositivo.
 *
 * Se guarda aparte del que está en uso porque se vuelve a él: apagar la voz
 * remota en Ajustes tiene que devolver la local, y construir una nueva perdería
 * lo que ya hubiera averiguado sobre el sistema.
 */
let local: ProveedorDeVoz = new VozDelSistema();
let proveedor: ProveedorDeVoz = local;

/**
 * De dónde sale la voz: del sistema, del modelo de aquí dentro, o de un servicio.
 *
 * Son tres, y la de en medio es la que no existía cuando esto se
 * escribió: un modelo neuronal que corre en esta máquina y no manda nada a
 * ninguna parte.
 */
export type OrigenDeVoz = 'local' | 'neural' | 'remote';

/**
 * De qué origen es un proveedor ya construido.
 *
 * **Por lo que declara y no por su clase.** Las dos que no son del sistema —la de
 * un servicio y la neuronal— viven en módulos que se cargan al elegirlas, y
 * comprobar `instanceof` aquí obligaría a traer los dos al arrancar solo para
 * poder preguntar.
 */
function origenDe(proveedor: ProveedorDeVoz): OrigenDeVoz {
  const id = proveedor.declarar().id;
  if (id === 'nube') return 'remote';
  if (id === 'neuronal') return 'neural';
  return 'local';
}

/**
 * Cuál de los tres está en uso, **sin ser estado reactivo**.
 *
 * Es lo mismo que dice `declaracion.id`, y aun así se guarda aparte por un
 * motivo concreto: quien llama a `preferir` es un efecto que lee las
 * preferencias, y `preferir` escribe `declaracion`. Leerla ahí la convertiría en
 * dependencia del efecto que la escribe, y eso es un bucle —Svelte lo corta con
 * `effect_update_depth_exceeded`, que es exactamente lo que ocurrió—.
 */
let elegido: OrigenDeVoz = 'local';

/**
 * Si la última vez había servicio configurado. Tampoco es reactivo, y por el
 * mismo motivo: se compara para decidir si hay que redeclarar, y redeclarar
 * escribe estado que el efecto que llama aquí acabaría leyendo.
 */
let habiaServicio = false;

let estado = $state<EstadoDeNarracion>(narracionInicial());

/**
 * El documento que se puede narrar ahora mismo.
 *
 * Lo registra el lector. No es `$state` porque nadie lo pinta: se lee al empezar
 * un tramo, y hacerlo reactivo obligaría a repintar el panel cada vez que la
 * superficie recalcula sus bloques, que es en cada página.
 */
/**
 * Lo que se manda a decir, **una frase por tramo**.
 *
 * La superficie entrega bloques —párrafos— y aquí se parten. Un párrafo entero
 * llega al sintetizador como un solo enunciado: lo dice de corrido, con una
 * única curva de entonación y sin las pausas que separan una idea de la
 * siguiente, que es lo que se oía como "no vocaliza". Y como ninguna plataforma
 * deja cambiar la velocidad de un enunciado ya encolado, ajustarla obligaba a
 * repetir el párrafo desde el principio: con frases retrocede dos segundos.
 *
 * La máquina de narración no se entera: recorría una lista y sigue recorriendo
 * una lista. Lo único que cambia es de qué está hecha.
 */
let tramos: BloqueNarrable[] = [];
/**
 * A qué bloque del documento pertenece cada tramo.
 *
 * Hace falta para el resaltado: lo que se subraya en pantalla es el párrafo, no
 * la frase, y sin esta correspondencia el índice de la frase se habría tomado
 * por el del párrafo y se habría resaltado el que no era.
 */
let origenes: number[] = [];
let locator: Locator | null = null;

/** Documento registrado, para saber cuándo hay que callar. */
let ultimoDocumento = '';

/**
 * Idioma del documento registrado y el de la interfaz: con qué se elige la voz.
 *
 * No son `$state` por lo mismo que `bloques`: nadie los pinta. Lo que se pinta
 * es la voz que sale de ellos.
 */
let idiomaDelDocumento: string | null = null;
let idiomaDeLaInterfaz = 'es';

/**
 * La voz que el usuario eligió y se guardó, si eligió alguna.
 *
 * Se guarda aparte de `estado.voz` porque son cosas distintas: `estado.voz` es
 * con cuál se está leyendo —deducida o elegida—, y esta es la decisión
 * deliberada, que gana a cualquier deducción y sobrevive a cambiar de libro.
 */
let vozGuardada: string | null = null;

/**
 * Recalcula con qué voz se lee.
 *
 * Se llama al cambiar de documento, al llegar el catálogo del sintetizador y al
 * cargar las preferencias, que son los tres momentos en que cambia alguno de los
 * datos con los que se elige. Sin esto, la voz se quedaba en la que hubiera, y
 * "la que hubiera" era la primera del catálogo: afrikáans.
 */
function ponerLaVozQueToca(): void {
  const elegida = deducirVoz(voces, {
    documento: idiomaDelDocumento,
    interfaz: idiomaDeLaInterfaz,
    guardada: vozGuardada
  });
  if (elegida && elegida !== estado.voz) estado = { ...estado, voz: elegida };
}

/**
 * La declaración se calcula una vez y se guarda.
 *
 * `getVoices()` de algunas plataformas devuelve la lista vacía en la primera
 * llamada y se llena después, así que se recalcula cuando el sistema avisa de
 * que ya las tiene. Sin eso, abrir el panel demasiado pronto declaraba "no hay
 * voces" en un equipo que sí las tiene.
 */
let declaracion = $state<DeclaracionDeVoz>(proveedor.declarar());
let voces = $state<VozDisponible[]>(proveedor.voces());

function releer(): void {
  declaracion = proveedor.declarar();
  voces = proveedor.voces();
  ponerLaVozQueToca();
}

// `voiceschanged` es el aviso del propio navegador. Se registra una vez, al
// importar el módulo, porque el catálogo de voces es del sistema y no de
// ninguna pantalla.
const sintesis = (globalThis as { speechSynthesis?: SpeechSynthesis }).speechSynthesis;
sintesis?.addEventListener?.('voiceschanged', releer);

/**
 * Si el webview no tiene voz, se le pide al runtime.
 *
 * En Linux esto no es el caso raro: es el caso. WebKitGTK declara la
 * característica de síntesis activada y `window.speechSynthesis` llega
 * indefinido de todos modos, así que sin esto la lectura en voz alta no existe
 * en el escritorio donde más falta hace.
 *
 * Se intenta una sola vez y en segundo plano: preguntar al runtime tarda lo que
 * tarda lanzar un proceso, y bloquear el arranque de la aplicación por saber si
 * hay voz sería cobrar el precio a quien nunca la va a usar.
 */
async function buscarVozEnElRuntime(): Promise<void> {
  if (declaracion.disponible) return;

  const delRuntime = new VozDelRuntime();
  const suya = await delRuntime.preparar();
  // Si el runtime tampoco puede, se conserva el motivo del webview: el primero
  // es el que explica por qué no hay voz en esta ventana.
  if (!suya.disponible) return;

  local = delRuntime;
  // Solo se adopta si no hay ya una voz remota en uso: quien la eligió lo hizo
  // a propósito, y sustituirla por la del sistema porque el runtime terminó de
  // arrancar sería deshacer una decisión del usuario.
  if (elegido === 'local') proveedor = delRuntime;
  releer();
}

void buscarVozEnElRuntime();

function ejecutar(efecto: EfectoDeVoz | undefined): void {
  if (!efecto) return;

  switch (efecto.tipo) {
    case 'hablar':
      proveedor.hablar(
        efecto.tramo,
        { voz: estado.voz, velocidad: estado.velocidad },
        {
          alAvanzar: (marca) => {
            estado = avanzar(estado, marca);
          },
          alTerminarTramo: (bloque) => {
            aplicar(terminarTramo(estado, bloque, tramos, locator ?? efecto.tramo.locator));
          },
          alFallar: (motivo) => {
            // Que la interrumpan es lo que pasa al parar: no es un fallo y no
            // puede dejar el estado en "hablando" cuando ya no suena nada.
            if (motivo === 'interrumpida') return;
            estado = { ...estado, estado: 'parada', bloque: -1, marca: null };
          }
        }
      );
      // Se pide el siguiente mientras suena este. Sin esto, con una voz de un
      // servicio el hueco entre frases dura lo que tarda en generarse la
      // siguiente, que es casi lo que dura la frase.
      if (efecto.siguiente) {
        proveedor.adelantar?.(efecto.siguiente, {
          voz: estado.voz,
          velocidad: estado.velocidad
        });
      }
      return;
    case 'pausar':
      proveedor.pausar();
      return;
    case 'reanudar':
      proveedor.reanudar();
      return;
    case 'parar':
      proveedor.parar();
  }
}

/**
 * Pone en uso el proveedor que corresponda, y calla lo que estuviera sonando.
 *
 * Callar antes de sustituir no es una precaución: quien sabe parar es el
 * proveedor que empezó, así que cambiarlo sin callarlo deja lo que suena sonando
 * y sin nadie que pueda detenerlo.
 *
 * Nada de esto se hace si no ha cambiado nada, y ese es el detalle que importa:
 * a esta función la llama un efecto que lee las preferencias, y redeclarar
 * escribe estado que ese mismo efecto acaba leyendo. Hacerlo en cada pasada es
 * un bucle, y Svelte lo corta con `effect_update_depth_exceeded` llevándose por
 * delante la reactividad de la pantalla entera: el síntoma no es un aviso en la
 * consola, es que Ajustes deja de responder.
 */
function ponerElProveedor(quiere: OrigenDeVoz, hayServicio: boolean): void {
  const puedeNube = quiere === 'remote' && hayServicio;

  if (quiere !== elegido) {
    if (estado.estado !== 'parada') aplicar(parar(estado));
    switch (quiere) {
      case 'remote': {
        // Igual que la neuronal: su módulo se trae al elegirla.
        void import('./nube').then(({ VozEnLaNube }) => {
          if (elegido !== 'remote') return;
          const nube = new VozEnLaNube();
          nube.preparar(puedeNube);
          proveedor = nube;
          releer();
        });
        break;
      }
      case 'neural': {
        // El módulo de la voz neuronal se trae **al elegirla**: quien nunca la
        // usa no paga sus kilobytes en el arranque, que tiene presupuesto. Y
        // preguntar al runtime tarda —mide la máquina—, así que se declara lo que
        // hay ahora y se vuelve a declarar cuando conteste; bloquear aquí dejaría
        // el panel de voz en blanco mientras tanto.
        void import('./neuronal').then(async ({ VozNeuronalLocal }) => {
          if (elegido !== 'neural') return;
          const neuronal = new VozNeuronalLocal();
          proveedor = neuronal;
          releer();
          await neuronal.preparar();
          if (proveedor === neuronal) releer();
        });
        break;
      }
      default:
        proveedor = local;
    }
    elegido = quiere;
    habiaServicio = puedeNube;
    releer();
    return;
  }

  // Sigue siendo la nube y solo cambió si hay servicio, que es lo que pasa al
  // escribir el anfitrión en Ajustes: se redeclara sin callar nada.
  if (quiere === 'remote' && puedeNube !== habiaServicio && origenDe(proveedor) === 'remote') {
    void import('./nube').then(({ VozEnLaNube }) => {
      if (proveedor instanceof VozEnLaNube) proveedor.preparar(puedeNube);
      habiaServicio = puedeNube;
      releer();
    });
  }
}

function aplicar(transicion: TransicionDeVoz): void {
  estado = transicion.estado;
  ejecutar(transicion.efecto);
}

/** El narrador, tal y como lo usan el lector y el panel. */
export const narrador = {
  get declaracion(): DeclaracionDeVoz {
    return declaracion;
  },
  get voces(): VozDisponible[] {
    return voces;
  },
  get estado(): EstadoDeNarracion {
    return estado;
  },

  /**
   * Si un bloque del documento registrado se está diciendo ahora.
   *
   * Se traduce del tramo al bloque: la voz avanza por frases y el resaltado va
   * por párrafos, que es la unidad que se ve.
   */
  narrando(bloque: number): boolean {
    if (estado.estado === 'parada' || estado.bloque < 0) return false;
    return origenes[estado.bloque] === bloque;
  },

  /**
   * El lector dice qué hay para narrar.
   *
   * Cambiar de documento **para la voz**: seguir narrando los bloques del
   * anterior diría en voz alta un texto que ya no está en la pantalla.
   */
  registrar(
    nuevos: readonly BloqueNarrable[],
    posicion: Locator,
    documento: string,
    idioma?: string | null
  ): void {
    if (documento !== ultimoDocumento) {
      ultimoDocumento = documento;
      if (estado.estado !== 'parada') aplicar(parar(estado));
      // El idioma se relee al cambiar de libro, que es cuando puede cambiar, y
      // con él la voz: leer una novela en portugués con la voz del libro
      // anterior es el mismo defecto que leerla con la primera de la lista.
      idiomaDelDocumento = idioma ?? null;
      ponerLaVozQueToca();
    }

    // Se parte aquí y no en la superficie: partir en frases es una decisión de
    // cómo suena la voz, no de cómo se pinta el texto.
    const lengua = idiomaDelDocumento ?? idiomaDeLaInterfaz;
    const nuevosTramos: BloqueNarrable[] = [];
    const nuevosOrigenes: number[] = [];
    nuevos.forEach((bloque, indice) => {
      for (const frase of enFrases(bloque.texto, lengua)) {
        nuevosTramos.push({ texto: frase });
        nuevosOrigenes.push(indice);
      }
    });
    tramos = nuevosTramos;
    origenes = nuevosOrigenes;
    locator = posicion;
  },

  /**
   * La aplicación dice en qué idioma está la interfaz y qué voz hay guardada.
   *
   * Va por aquí y no se lee de las preferencias desde dentro porque este módulo
   * no habla con el runtime: quien carga las preferencias es la aplicación, y
   * este archivo es pegamento sin decisiones propias.
   */
  preferir(opciones: {
    interfaz?: string;
    guardada?: string | null;
    /** De dónde sale la voz, según las preferencias. */
    proveedor?: OrigenDeVoz;
    /** Si hay servicio de síntesis configurado. */
    hayServicio?: boolean;
  }): void {
    if (opciones.interfaz) idiomaDeLaInterfaz = opciones.interfaz;
    if (opciones.guardada !== undefined) vozGuardada = opciones.guardada;

    if (opciones.proveedor !== undefined || opciones.hayServicio !== undefined) {
      ponerElProveedor(opciones.proveedor ?? elegido, opciones.hayServicio === true);
    }

    ponerLaVozQueToca();
  },

  /** Empieza, pausa o reanuda. Es lo que hacen la tecla y el botón. */
  alternar(desde = 0): void {
    if (!locator) return;
    // `desde` es un bloque del documento; la máquina avanza por tramos. Se
    // empieza en la primera frase de ese bloque.
    const primerTramo = origenes.indexOf(desde);
    const arranque = estado.bloque >= 0 ? estado.bloque : Math.max(0, primerTramo);
    aplicar(alternar(estado, tramos, arranque, locator));
  },

  parar(): void {
    aplicar(parar(estado));
  },

  elegirVoz(id: string): void {
    // Elegir es una decisión deliberada: gana a la deducción y sobrevive a
    // cambiar de libro. Guardarla en el disco es de quien llama.
    vozGuardada = id;
    if (!locator) {
      estado = { ...estado, voz: id };
      return;
    }
    aplicar(ajustar(estado, { voz: id }, tramos, locator));
  },

  cambiarVelocidad(valor: number): void {
    if (!locator) return;
    aplicar(ajustar(estado, { velocidad: valor }, tramos, locator));
  },

  /** Sustituye el proveedor. Es por donde entrará el motor neuronal. */
  usarProveedor(nuevo: ProveedorDeVoz): void {
    if (estado.estado !== 'parada') aplicar(parar(estado));
    proveedor = nuevo;
    elegido = origenDe(nuevo);
    releer();
  }
};
