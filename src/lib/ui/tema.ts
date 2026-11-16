/**
 * Apariencia de la interfaz: tema, contraste, movimiento y escala tipográfica.
 *
 * Contrato: `docs/architecture/ui-components.md`, sección 4, plano
 * de aplicación. La persistencia es la de
 * `docs/adr/010-settings-persistence.md`: SQLite manda, y la
 * caché del webview solo sirve para pintar la primera pantalla sin destello.
 *
 * La resolución está separada de la aplicación a propósito. `resolverApariencia`
 * y `atributosDeApariencia` son funciones puras y por eso se pueden comprobar
 * sin un navegador; `aplicarApariencia` es la única línea que toca el documento.
 * Sin esa separación, comprobar que el alto contraste se aplica exigiría montar
 * una interfaz entera, y en la práctica no se comprobaría.
 */

import type { SettingValue } from '$lib/bindings';

import { interpretarHex, luminanciaRelativa } from './contraste.js';

/**
 * Los temas de la capa inicial, más `system`.
 *
 * Un tema no es una paleta: fija también la tipografía de la interfaz, el ritmo
 * del movimiento, la textura y el redondeo. El motivo está en
 * `ui-components.md`, sección 5.1, y se resume en que nadie sabe qué va a leer
 * cada persona, así que la aplicación no busca la interfaz correcta para todos
 * —no existe— sino adaptarse a lo que cada uno espera sentir.
 */
export type Tema =
  | 'grafito'
  | 'gabinete'
  | 'papel'
  | 'tinta'
  | 'pergamino'
  | 'vigilia'
  | 'musgo'
  | 'system';

/**
 * Los siete, en el orden en que se ofrecen. `system` va aparte.
 *
 * El tipo es `TemaResuelto` y no `Tema`: aquí no hay comodín, y decir que sí lo
 * hay obligaba a convertir en cada uso, que es la forma de que el tipo deje de
 * impedir lo que existe para impedir.
 */
export const TEMAS: readonly TemaResuelto[] = [
  'grafito',
  'gabinete',
  'papel',
  'tinta',
  'pergamino',
  'vigilia',
  'musgo'
];

/**
 * Semillas de Material 3 para cada tema base.
 *
 * Se exporta porque `scripts/generar-paletas.mjs` deriva de aquí las catorce
 * paletas que van a `src/paletas.css`. Si se copiaran allí, las dos listas se
 * separarían el día que alguien cambiara un tema, y la diferencia se vería como
 * un destello de otro color al arrancar.
 */
export const SEMILLAS_POR_TEMA: Readonly<Record<TemaResuelto, string>> = {
  grafito: '#d97757',
  gabinete: '#d9a441',
  papel: '#8c2a20',
  tinta: '#4ec9b0',
  pergamino: '#7a4715',
  vigilia: '#8fa9dd',
  musgo: '#93c3a5'
};

/**
 * Los temas cuyos grises no toman el matiz de la semilla.
 *
 * Material tiñe de la semilla también los fondos, los textos y los trazos: con
 * una semilla naranja, la superficie sale color café. Grafito es el tema de
 * partida y lo quiere al revés —Se fijó el 2026-09-11—: grises de verdad,
 * y el color solo en el acento. Es una propiedad del tema y no de la semilla, así
 * que elegir otro acento sobre Grafito deja los grises en gris.
 */
export const NEUTROS_GRISES: readonly TemaResuelto[] = ['grafito'];

const CLAROS: readonly Tema[] = ['papel', 'pergamino'];

export type TemaResuelto = Exclude<Tema, 'system'>;
export type Esquema = 'light' | 'dark';

/** Los colores que declara un tema de extensión, como los trae su manifiesto. */
export type TokensDeTema = Readonly<Record<string, string>>;

/** Un tema de extensión instalado: quién lo aporta y qué colores trae. */
export interface TemaDeExtensionDisponible {
  extension: string;
  tokens: TokensDeTema;
}

export interface Apariencia {
  tema: Tema;
  altoContraste: boolean;
  movimientoReducido: boolean;
  escalaTipografica: number;
  acento: string | null;
  /**
   * El tema que aporta una extensión, si hay uno puesto.
   *
   * **Va aquí y no aparte, y eso es el arreglo del 2026-09-12.** Hasta entonces se
   * aplicaba por su cuenta, escribiendo en línea los doce tokens `--nb-*` que
   * declara. Con el rediseño media interfaz pasó a leer las variables
   * de Material, que un tema no declara, así que quedaba mitad y mitad: medido en
   * WebKitGTK, un tema oscuro sobre una paleta clara dejaba un texto a **1,13** de
   * contraste. Siendo parte de la apariencia, de él sale también el esquema y la
   * paleta entera.
   */
  temaDeExtension: TokensDeTema | null;
}

export const ESCALA_MINIMA = 0.75;
export const ESCALA_MAXIMA = 2;

export const APARIENCIA_POR_DEFECTO: Readonly<Apariencia> = {
  tema: 'system',
  altoContraste: false,
  movimientoReducido: false,
  escalaTipografica: 1,
  acento: null,
  temaDeExtension: null
};

function esTema(valor: unknown): valor is Tema {
  return valor === 'system' || (typeof valor === 'string' && TEMAS.includes(valor as TemaResuelto));
}

const HEREDADOS: Readonly<Record<string, Tema>> = {
  light: 'papel',
  dark: 'grafito',
  /*
   * Brasa se retiró el 2026-09-11 al llegar Grafito: su semilla era el mismo
   * naranja, y dos temas con el mismo color son un tema con dos nombres. Quien lo
   * tenía elegido pasa al que conserva su acento, en lugar de caer en el tema por
   * defecto y creer que perdió su elección.
   */
  brasa: 'grafito'
};

export function leerApariencia(
  valores: Readonly<Record<string, SettingValue>>,
  temasDeExtension: readonly TemaDeExtensionDisponible[] = []
): Apariencia {
  const escalaCruda = valores.font_scale;
  const escala =
    typeof escalaCruda === 'number' && Number.isFinite(escalaCruda)
      ? Math.min(ESCALA_MAXIMA, Math.max(ESCALA_MINIMA, escalaCruda))
      : APARIENCIA_POR_DEFECTO.escalaTipografica;

  const temaCrudo = valores.theme;
  const tema = esTema(temaCrudo)
    ? temaCrudo
    : (typeof temaCrudo === 'string' && HEREDADOS[temaCrudo]) || APARIENCIA_POR_DEFECTO.tema;

  const acento = valores.accent_color;

  /*
   * Cuál está puesto lo dice la preferencia, y sus colores los trae la extensión
   * instalada: lo que se guarda es **quién lo aporta**, no sus colores, que son
   * suyos y cambian cuando se actualiza. Si la extensión ya no está, la
   * preferencia sobra sola en lugar de dejar colores de nadie.
   */
  const puesto = valores.extension_theme;
  const temaDeExtension =
    typeof puesto === 'string' && puesto.length > 0
      ? (temasDeExtension.find((t) => t.extension === puesto)?.tokens ?? null)
      : null;

  return {
    tema,
    altoContraste: valores.high_contrast === true,
    movimientoReducido: valores.reduced_motion === true,
    escalaTipografica: escala,
    acento: typeof acento === 'string' && /^#[0-9a-f]{6}$/i.test(acento) ? acento : null,
    temaDeExtension
  };
}

/**
 * El esquema que declara un tema de extensión: lo dice su superficie.
 *
 * Un tema que no declare superficie no declara esquema, y entonces manda el del
 * núcleo: es lo único que se puede afirmar sin inventarle un color.
 *
 * El umbral es 0,2 y no 0,5 porque la luminancia relativa no es lineal: el gris
 * medio está en 0,216, no a mitad de camino. Con 0,5 un gris de fondo contaría
 * como claro y la mitad de los temas oscuros entrarían por el lado equivocado.
 */
export function esquemaDelTemaDeExtension(tokens: TokensDeTema): Esquema | null {
  const superficie = tokens['--nb-surface'] ?? tokens['--nb-surface-raised'];
  const rgb = superficie ? interpretarHex(superficie) : null;
  if (!rgb) return null;
  return luminanciaRelativa(rgb) < 0.2 ? 'dark' : 'light';
}

/**
 * Los dos temas del núcleo que hacen de base bajo un tema de extensión.
 *
 * Una extensión declara catorce colores como mucho y la hoja del núcleo pone los
 * doscientos y pico que faltan, así que la base **tiene que ser del mismo
 * esquema** o el resto de la interfaz se queda del contrario. Se eligen los dos
 * neutros para no teñir con el matiz de otro tema lo que el autor no declaró.
 */
const BASES: Readonly<Record<Esquema, TemaResuelto>> = { dark: 'grafito', light: 'papel' };

/**
 * El tema del núcleo sobre el que se pinta una apariencia.
 *
 * Con un tema de extensión puesto es la base de su esquema; sin él, el elegido.
 * Lo usan los atributos, la paleta y la clave de la caché, que tienen que
 * coincidir: si uno de los tres eligiera otro, la paleta y la hoja discreparían.
 */
export function temaBase(apariencia: Apariencia, elSistemaPrefiereOscuro: boolean): TemaResuelto {
  const esquema = apariencia.temaDeExtension
    ? esquemaDelTemaDeExtension(apariencia.temaDeExtension)
    : null;
  return esquema ? BASES[esquema] : resolverTema(apariencia.tema, elSistemaPrefiereOscuro);
}

export function resolverTema(tema: Tema, elSistemaPrefiereOscuro: boolean): TemaResuelto {
  if (tema === 'system') return elSistemaPrefiereOscuro ? 'grafito' : 'papel';
  return tema;
}

export function esquemaDe(tema: TemaResuelto): Esquema {
  return CLAROS.includes(tema) ? 'light' : 'dark';
}

export interface AtributosDeApariencia {
  'data-theme': TemaResuelto;
  'data-resolved-scheme': Esquema;
  'data-contrast': 'high' | null;
  'data-motion': 'reduced' | null;
  '--nb-font-scale': string;
  '--nb-accent': string | null;
  [key: string]: string | null;
}

/**
 * Lo que la apariencia escribe sobre el elemento raíz.
 *
 * **Sin la paleta de Material**, ahora. Las de los siete temas, con
 * y sin alto contraste, ya están en `src/paletas.css` y las eligen `data-theme` y
 * `data-contrast`: calcularlas aquí era cargar la utilería de color en el
 * arranque para escribir en línea exactamente lo que la hoja ya tenía. Solo un
 * acento propio necesita calcularse, y eso lo hace `aplicarApariencia` a demanda;
 * la paleta que se pinta la da `paletaDe`, en `paleta.ts`.
 */
export function atributosDeApariencia(
  apariencia: Apariencia,
  elSistemaPrefiereOscuro: boolean
): AtributosDeApariencia {
  const resuelto = temaBase(apariencia, elSistemaPrefiereOscuro);
  const esquema = esquemaDe(resuelto);
  /*
   * El acento entra por dos caminos y solo uno es seguro con alto contraste.
   *
   * Como **semilla**, Material lo pasa por la curva de contraste y deriva de él
   * una paleta medida: por ahí puede entrar siempre. Como **variable en línea**
   * -`--nb-accent`, que le gana a cualquier hoja de estilos- se pinta tal cual,
   * sin que nadie lo haya medido contra ese fondo, y esa es justo la forma de
   * que elegir un color le quite el contraste a quien lo necesita. Así que ese
   * segundo camino se sigue cerrando, y el primero se abre.
   */
  const acentoEnLinea = apariencia.altoContraste ? null : apariencia.acento;

  return {
    'data-theme': resuelto,
    'data-resolved-scheme': esquema,
    'data-contrast': apariencia.altoContraste ? 'high' : null,
    'data-motion': apariencia.movimientoReducido ? 'reduced' : null,
    '--nb-font-scale': String(apariencia.escalaTipografica),
    '--nb-accent': acentoEnLinea,
    /*
     * Los colores del tema de extensión van al final a propósito: los que declara
     * ganan a lo anterior, incluido el acento elegido. Un tema es una elección más
     * concreta que un acento, y aplicar los dos a medias es lo que producía la
     * mezcla que esto arregla.
     */
    ...(apariencia.temaDeExtension ?? {})
  };
}

/**
 * Dónde se guarda la última paleta de acento calculada.
 *
 * Es una caché del primer fotograma por el mismo motivo que la de
 * `$core/settings`, y va aparte porque aquella solo admite las preferencias que
 * declara el runtime: esto no es una preferencia, se deriva de tres.
 */
const CLAVE_PALETA = 'nb-books.paleta-del-acento.v1';

/** Qué variables de Material hay escritas en línea sobre cada raíz. */
const EN_LINEA = new WeakMap<HTMLElement, string[]>();

/**
 * Y qué tokens de un tema de extensión.
 *
 * Se lleva la cuenta por el mismo motivo que la de Material: escribir en línea es
 * fácil, pero al cambiar de tema hay que **retirar** lo que el anterior puso. Sin
 * esto, pasar de un tema que declara catorce colores a otro que declara siete
 * dejaba los siete restantes del primero, que es la forma exacta de tener dos
 * temas puestos a la vez.
 */
const TOKENS_DE_TEMA_EN_LINEA = new WeakMap<HTMLElement, string[]>();

/**
 * La última apariencia pedida. Calcular un acento espera a una carga, y una
 * paleta que llega tarde no puede pisar a la que se eligió después.
 */
let ultimaPedida = 0;

/**
 * Aplica la apariencia al elemento raíz.
 *
 * Devuelve una promesa que se cumple cuando la paleta está puesta. Sin acento
 * propio ya lo está al volver, así que quien no la espera no pierde nada; existe
 * para que las pruebas puedan esperar al acento.
 */
export function aplicarApariencia(
  raiz: HTMLElement,
  apariencia: Apariencia,
  elSistemaPrefiereOscuro: boolean
): Promise<void> {
  const atributos = atributosDeApariencia(apariencia, elSistemaPrefiereOscuro);
  const turno = ++ultimaPedida;

  raiz.setAttribute('data-tema-cambiando', '');
  retirarTokensQueSobran(raiz, atributos, apariencia.temaDeExtension);

  /*
   * Un valor nulo se retira en lugar de escribirse: `--nb-accent` sin acento
   * elegido tiene que dejar de existir, no valer la cadena "null". Es la razón
   * de que este bucle distinga los tres casos en vez de asignar y ya está.
   */
  for (const [clave, valor] of Object.entries(atributos)) {
    if (clave.startsWith('--')) {
      if (valor === null) raiz.style.removeProperty(clave);
      else raiz.style.setProperty(clave, String(valor));
    } else if (valor === null) {
      raiz.removeAttribute(clave);
    } else {
      raiz.setAttribute(clave, valor);
    }
  }

  /*
   * Sin acento propio, la paleta la pone la hoja. Lo que hubiera en línea se
   * retira: el estilo del elemento le gana a cualquier hoja, así que volver al
   * acento del tema no cambiaría nada.
   */
  if (!apariencia.acento && !apariencia.temaDeExtension) {
    quitarPaleta(raiz);
    devolverTransiciones(raiz, turno);
    return Promise.resolve();
  }

  /*
   * **Con acento, la utilería de color se carga ahora y no al arrancar.**
   *
   * Cargar tarda, y el primer fotograma no puede esperar ni salir con el acento
   * del tema para cambiar al elegido un instante después: sería el destello que
   * prohíbe la regla 2 de ADR-010. Así que se pinta enseguida la última paleta
   * calculada, si es la de esta apariencia, y el cálculo la confirma encima. Una
   * actualización que cambie cómo se deriva el color queda corregida en ese
   * mismo arranque.
   */
  const clave = claveDePaleta(apariencia, elSistemaPrefiereOscuro);
  const guardada = paletaGuardada(clave);
  if (guardada) ponerPaleta(raiz, guardada);

  return import('./paleta.js')
    .then(({ paletaDe }) => {
      if (turno !== ultimaPedida) return;
      const variables = paletaDe(apariencia, elSistemaPrefiereOscuro);
      ponerPaleta(raiz, variables);
      guardarPaleta(clave, variables);
    })
    .finally(() => devolverTransiciones(raiz, turno));
}

/**
 * Retira los tokens que dejó el tema anterior y que el nuevo no trae.
 *
 * Sin esto, pasar de un tema que declara catorce colores a otro que declara siete
 * dejaba los siete restantes del primero, que es la forma exacta de tener dos
 * temas puestos a la vez.
 */
function retirarTokensQueSobran(
  raiz: HTMLElement,
  atributos: AtributosDeApariencia,
  tema: TokensDeTema | null
): void {
  for (const nombre of TOKENS_DE_TEMA_EN_LINEA.get(raiz) ?? []) {
    if (!(nombre in atributos)) raiz.style.removeProperty(nombre);
  }
  TOKENS_DE_TEMA_EN_LINEA.set(raiz, Object.keys(tema ?? {}));
}

/**
 * Las transiciones vuelven cuando la paleta ya está pintada, y no antes: con el
 * acento calculándose, devolverlas antes animaría el paso del color del tema al
 * elegido.
 */
function devolverTransiciones(raiz: HTMLElement, turno: number): void {
  const devolver = () => {
    if (turno === ultimaPedida) raiz.removeAttribute('data-tema-cambiando');
  };
  if (typeof requestAnimationFrame === 'function') {
    requestAnimationFrame(() => requestAnimationFrame(devolver));
  } else {
    devolver();
  }
}

function ponerPaleta(raiz: HTMLElement, variables: Readonly<Record<string, string>>): void {
  for (const nombre of EN_LINEA.get(raiz) ?? []) {
    if (!(nombre in variables)) raiz.style.removeProperty(nombre);
  }
  for (const [nombre, valor] of Object.entries(variables)) raiz.style.setProperty(nombre, valor);
  EN_LINEA.set(raiz, Object.keys(variables));
}

function quitarPaleta(raiz: HTMLElement): void {
  for (const nombre of EN_LINEA.get(raiz) ?? []) raiz.style.removeProperty(nombre);
  EN_LINEA.delete(raiz);
}

function claveDePaleta(apariencia: Apariencia, elSistemaPrefiereOscuro: boolean): string {
  const resuelto = temaBase(apariencia, elSistemaPrefiereOscuro);
  const tema = apariencia.temaDeExtension;
  return [
    resuelto,
    apariencia.acento,
    // Del tema de extensión basta lo que cambia la paleta: su semilla y su esquema.
    tema ? `${tema['--nb-accent'] ?? ''}@${tema['--nb-surface'] ?? ''}` : '',
    apariencia.altoContraste ? 'alto' : 'normal'
  ].join('|');
}

function almacen(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

function paletaGuardada(clave: string): Record<string, string> | null {
  try {
    const crudo = almacen()?.getItem(CLAVE_PALETA);
    if (!crudo) return null;
    const leido: unknown = JSON.parse(crudo);
    if (typeof leido !== 'object' || leido === null) return null;
    const { clave: suya, variables } = leido as { clave?: unknown; variables?: unknown };
    if (suya !== clave || typeof variables !== 'object' || variables === null) return null;

    // Solo variables de Material: lo que se lee de aquí se escribe en línea sobre
    // la raíz, y no puede tocar ninguna otra propiedad.
    const limpias: Record<string, string> = {};
    for (const [nombre, valor] of Object.entries(variables)) {
      if (nombre.startsWith('--m3c-') && typeof valor === 'string') limpias[nombre] = valor;
    }
    return limpias;
  } catch {
    // Ilegible: se pinta con el acento del tema un instante y se reescribe.
    return null;
  }
}

function guardarPaleta(clave: string, variables: Readonly<Record<string, string>>): void {
  try {
    almacen()?.setItem(CLAVE_PALETA, JSON.stringify({ clave, variables }));
  } catch {
    // Cuota agotada o almacenamiento en solo lectura: cuesta un destello al
    // arrancar, no la aplicación.
  }
}

export function elSistemaPrefiereOscuro(): boolean {
  try {
    return globalThis.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;
  } catch {
    return false;
  }
}
