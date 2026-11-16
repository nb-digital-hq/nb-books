/**
 * El plano del libro: cómo se compone el texto, separado de cómo se ve la
 * aplicación.
 *
 * Compromiso 4 de ADR-012. Son dos decisiones distintas del usuario —el tema de
 * la aplicación y la tipografía del libro— y se pierden juntas si se mezclan.
 * Aquí la separación no es una convención de nombres: **cada preferencia
 * declara su plano** y esta capa solo mira las del libro. Una preferencia de la
 * aplicación que se colara aquí no cambiaría nada, y hay una prueba que lo
 * comprueba en las dos direcciones.
 */

import type { Setting, SettingValue } from '$lib/bindings';

import { type KitAccesible, MODO_POR_DEFECTO, modoPorNombre } from './modos';

/** Las cuatro alineaciones posibles del texto. */
export const ALINEACIONES = ['izquierda', 'centro', 'derecha', 'justificado'] as const;

export type Alineacion = (typeof ALINEACIONES)[number];

/**
 * La clave con la que se guardaba solo "justificado o no".
 *
 * No se borra ni se migra: sigue siendo lo que la exportación lleva, y una
 * biblioteca guardada antes de que existiera la alineación tiene que abrirse
 * como el usuario la dejó. Se lee para deducir la alineación cuando la nueva
 * clave nunca se escribió, y se escribe junto a ella para que las dos no puedan
 * decir cosas distintas.
 */
export const CLAVE_JUSTIFICADO = 'book_justify';

/** Los siete valores que el usuario controla del libro. */
export interface Composicion {
  /** `system` significa la serif de lectura del sistema, no una concreta. */
  familia: string;
  /** Cuerpo del texto, en píxeles. */
  cuerpo: number;
  /** Interlineado, como múltiplo del cuerpo. */
  interlineado: number;
  /** Margen alrededor de la columna, en píxeles. */
  margen: number;
  /** Ancho máximo de la columna, en caracteres por línea. */
  ancho: number;
  /**
   * Cómo se alinea el texto.
   *
   * Justificar es **una** alineación, no un interruptor aparte: tenerlo en dos
   * ajustes dejaba a los dos peleándose por la misma propiedad, y ganaba el que
   * la hoja de estilos declarara último.
   */
  alineacion: Alineacion;
  /** Paginado en lugar de desplazamiento continuo. */
  paginado: boolean;
  /**
   * Modo de lectura con nombre. Es un perfil completo, no una paleta.
   *
   * Cadena y no solo uno de los seis del núcleo: un paquete de
   * personalización aporta los suyos, con el identificador
   * `<extensión>/<modo>`.
   */
  modo: string;
  /** El kit de lectura accesible, que no toca nada mientras no se active. */
  kit: KitAccesible;
}

/**
 * Correspondencia entre cada valor y su preferencia.
 *
 * Es la única fuente: los paneles la recorren para pintar sus controles, así que
 * no se puede añadir un control sin preferencia ni una preferencia sin control.
 */
export const CLAVES: Readonly<Record<keyof Composicion, string>> = {
  familia: 'book_font_family',
  cuerpo: 'book_font_size',
  interlineado: 'book_line_height',
  margen: 'book_margin',
  ancho: 'book_line_width',
  alineacion: 'book_alignment',
  paginado: 'book_pagination',
  modo: 'book_reading_mode',
  kit: 'book_accessible_font'
};

/** Cada ajuste del kit con su preferencia. Se recorre para pintar el panel. */
export const CLAVES_DEL_KIT: Readonly<Record<keyof KitAccesible, string>> = {
  familia: 'book_accessible_font',
  bionica: 'book_bionic',
  regla: 'book_ruler',
  focoDeLinea: 'book_line_focus',
  animarPagina: 'book_page_animation'
};

/**
 * Con lo que se compone mientras el usuario no haya decidido.
 *
 * Coincide con los valores por defecto de Rust, que son la fuente. Aquí existen
 * para poder pintar antes de que responda el puente, y si difirieran el síntoma
 * sería un salto de composición al abrir cada libro.
 */
export const COMPOSICION_POR_DEFECTO: Composicion = {
  familia: 'system',
  cuerpo: 18,
  interlineado: 1.6,
  margen: 32,
  ancho: 65,
  alineacion: 'izquierda',
  paginado: false,
  modo: MODO_POR_DEFECTO,
  kit: {
    familia: 'ninguna',
    bionica: false,
    regla: false,
    focoDeLinea: false,
    animarPagina: true
  }
};

/** Límites de cada valor, para que un ajuste no deje el texto ilegible. */
export const LIMITES = {
  cuerpo: { min: 12, max: 42, paso: 1 },
  interlineado: { min: 1.1, max: 2.4, paso: 0.1 },
  margen: { min: 0, max: 160, paso: 8 },
  ancho: { min: 40, max: 110, paso: 5 }
} as const;

function numero(
  valor: SettingValue | undefined,
  defecto: number,
  limite?: { min: number; max: number }
): number {
  if (typeof valor !== 'number' || !Number.isFinite(valor)) return defecto;
  if (!limite) return valor;
  return Math.min(limite.max, Math.max(limite.min, valor));
}

/**
 * Compone el plano del libro a partir de las preferencias del runtime.
 *
 * **Solo mira las del plano del libro.** Recorrer todas y quedarse con las que
 * tengan el nombre esperado funcionaría igual hoy y dejaría de hacerlo el día
 * que alguien añada una preferencia de la aplicación con un nombre parecido.
 */
export function composicionDe(
  preferencias: readonly Setting[],
  modosDeExtension: readonly string[] = []
): Composicion {
  const valores = new Map<string, SettingValue>();
  for (const preferencia of preferencias) {
    if (preferencia.plane === 'book') valores.set(preferencia.key, preferencia.value);
  }

  const familia = valores.get(CLAVES.familia);
  const paginacion = valores.get(CLAVES.paginado);
  const alineacion = valores.get(CLAVES.alineacion);

  return {
    familia:
      typeof familia === 'string' && familia.length > 0 ? familia : COMPOSICION_POR_DEFECTO.familia,
    cuerpo: numero(valores.get(CLAVES.cuerpo), COMPOSICION_POR_DEFECTO.cuerpo, LIMITES.cuerpo),
    interlineado: numero(
      valores.get(CLAVES.interlineado),
      COMPOSICION_POR_DEFECTO.interlineado,
      LIMITES.interlineado
    ),
    margen: numero(valores.get(CLAVES.margen), COMPOSICION_POR_DEFECTO.margen, LIMITES.margen),
    ancho: numero(valores.get(CLAVES.ancho), COMPOSICION_POR_DEFECTO.ancho, LIMITES.ancho),
    alineacion: alineacionDe(alineacion, valores.get(CLAVE_JUSTIFICADO)),
    paginado: paginacion === 'paged',
    modo: nombreDeModo(valores.get(CLAVES.modo), modosDeExtension),
    kit: {
      familia: familiaAccesible(valores.get(CLAVES_DEL_KIT.familia)),
      bionica: valores.get(CLAVES_DEL_KIT.bionica) === true,
      regla: valores.get(CLAVES_DEL_KIT.regla) === true,
      focoDeLinea: valores.get(CLAVES_DEL_KIT.focoDeLinea) === true,
      // La animación viene puesta salvo que el usuario la apague: transporta
      // significado y cabe. Con movimiento reducido desaparece sola.
      animarPagina: valores.get(CLAVES_DEL_KIT.animarPagina) !== false
    }
  };
}

/**
 * El modo guardado, si existe.
 *
 * `extra` son los que aportan las extensiones instaladas. Sin ellos, el nombre
 * de un modo de extensión no se reconocería y se compondría con el de siempre:
 * el usuario elegiría su modo y al reabrir el libro estaría en "día".
 */
function nombreDeModo(valor: SettingValue | undefined, extra: readonly string[]): string {
  if (typeof valor !== 'string') return MODO_POR_DEFECTO;
  return extra.includes(valor) ? valor : modoPorNombre(valor).nombre;
}

function familiaAccesible(valor: SettingValue | undefined): KitAccesible['familia'] {
  return valor === 'dislexia' || valor === 'bajaVision' ? valor : 'ninguna';
}

/**
 * La alineación, o la que se deduce de la preferencia antigua.
 *
 * El orden importa: manda la alineación si está escrita, y solo si no lo está
 * se mira el interruptor de justificar. Al revés, quien tuviera justificado de
 * antes no habría podido quitarlo nunca desde el control nuevo.
 */
function alineacionDe(
  alineacion: SettingValue | undefined,
  justificado: SettingValue | undefined
): Alineacion {
  if (typeof alineacion === 'string' && (ALINEACIONES as readonly string[]).includes(alineacion)) {
    return alineacion as Alineacion;
  }
  return justificado === true ? 'justificado' : COMPOSICION_POR_DEFECTO.alineacion;
}

/** Valor con el que se guarda un campo de la composición. */
export function valorDe(campo: keyof Composicion, composicion: Composicion): SettingValue {
  if (campo === 'paginado') return composicion.paginado ? 'paged' : 'scroll';
  if (campo === 'kit') return composicion.kit.familia;
  return composicion[campo] as SettingValue;
}
