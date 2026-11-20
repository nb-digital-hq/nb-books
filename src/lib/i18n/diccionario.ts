/**
 * La forma de un diccionario, y quién la define.
 *
 * El español no es "un idioma más": es la **fuente**. `Diccionario` sale de él
 * con `typeof`, así que añadir un texto en español obliga a añadirlo en inglés
 * y en portugués o el proyecto no compila. Sin esto, "está todo traducido"
 * sería una afirmación que nadie comprueba, y la forma en que se rompería
 * —una pantalla en dos idiomas a la vez— solo la vería quien no lee español.
 *
 * Vive en su propio archivo, y no junto a `es.ts`, para que los diccionarios
 * traducidos puedan importar el tipo sin importar el diccionario español
 * entero: un ciclo entre módulos que se resuelve solo en tiempo de ejecución es
 * la clase de dependencia que aparece rota un martes por la mañana.
 *
 * Contrato: `docs/architecture/i18n.md`.
 */

import type { ErrorCode } from '$lib/bindings/errorCodes';

import type { es } from './locales/es';

/**
 * Un diccionario completo.
 *
 * `Mutable` desdobla el `as const` del español: sin eso, los otros idiomas
 * tendrían que declarar cada cadena como literal exacto, que es imposible
 * porque justamente son otras cadenas.
 */
export type Diccionario = {
  -readonly [Grupo in keyof typeof es]: {
    -readonly [Clave in keyof (typeof es)[Grupo]]: string;
  };
};

/** Los idiomas que la interfaz habla. */
export const IDIOMAS = ['es', 'en', 'pt'] as const;

export type Idioma = (typeof IDIOMAS)[number];

/** El que se usa cuando no hay ninguno elegido, o el elegido no se reconoce. */
export const IDIOMA_BASE: Idioma = 'es';

/** Nombre de cada idioma **en ese idioma**, que es como se elige un idioma. */
export const NOMBRE_DEL_IDIOMA: Record<Idioma, string> = {
  es: 'Español',
  en: 'English',
  pt: 'Português'
};

/** Si una cadena cualquiera es uno de los idiomas que se hablan. */
export function esIdioma(valor: unknown): valor is Idioma {
  return typeof valor === 'string' && (IDIOMAS as readonly string[]).includes(valor);
}

/**
 * Comprobación en tiempo de compilación de que **todo** código de error de Rust
 * tiene mensaje.
 *
 * `ERROR_CODES` se genera desde `src-tauri/lector/src/error.rs`, así que añadir una
 * variante allí y no traducirla aquí deja de compilar. La prueba lo
 * comprueba además en los tres idiomas y en tiempo de ejecución, porque un tipo
 * no protege de una traducción vacía.
 */
export type MensajesDeError = Record<ErrorCode, string> & { desconocido: string };
