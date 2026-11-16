/**
 * La vista dividida: dos superficies de lectura a la vez.
 *
 * Contrato: ADR-012, que la nombra como la respuesta a la simultaneidad tras
 * descartar las ventanas flotantes.
 *
 * # Por qué el reparto es aritmética y no un arrastre
 *
 * Arrastrar el separador es un gesto de puntero, y una acción exclusiva del
 * ratón está prohibida. El reparto vive aquí como un número que se mueve a
 * pasos, así que el teclado lo cambia igual que el ratón, y además se puede
 * comprobar en una prueba en lugar de mirando la pantalla.
 *
 * # Qué no cabe
 *
 * En la clase de ancho compacta la división **no se ofrece**: dos columnas de
 * lectura en una ventana estrecha no son dos columnas de lectura, son dos
 * columnas ilegibles. Se omite, no se muestra rota.
 */

import type { ClaseDeAncho } from '../anchos';

/** Cuál de las dos mitades manda. */
export type Mitad = 'principal' | 'secundaria';

/** Estado de la división. */
export interface Division {
  activa: boolean;
  /** Qué documento hay en la segunda mitad. Nulo si todavía ninguno. */
  secundario: string | null;
  /** Qué fracción del ancho ocupa la mitad principal, de 0,2 a 0,8. */
  reparto: number;
  /** Cuál recibe los comandos del lector. */
  foco: Mitad;
}

export const DIVISION_INICIAL: Division = {
  activa: false,
  secundario: null,
  reparto: 0.5,
  foco: 'principal'
};

/** Cuánto se mueve el separador por pulsación. */
export const PASO = 0.05;

/** Límites del reparto: por debajo, la mitad deja de servir para leer. */
export const REPARTO_MINIMO = 0.2;
export const REPARTO_MAXIMO = 0.8;

/** Si la división se puede ofrecer en esta clase de ancho. */
export function cabeLaDivision(clase: ClaseDeAncho): boolean {
  return clase !== 'compacto';
}

/** Mueve el separador, sin salirse de los límites. */
export function repartir(division: Division, pasos: number): Division {
  const reparto = Math.min(
    REPARTO_MAXIMO,
    Math.max(REPARTO_MINIMO, Number((division.reparto + pasos * PASO).toFixed(4)))
  );
  return { ...division, reparto };
}

/** Cambia la mitad que recibe los comandos. */
export function alternarFoco(division: Division): Division {
  return { ...division, foco: division.foco === 'principal' ? 'secundaria' : 'principal' };
}

/**
 * Abre o cierra la división.
 *
 * Cerrarla devuelve el foco a la mitad principal: dejarlo en una mitad que ya no
 * existe haría que las teclas siguientes no fueran a ninguna parte.
 */
export function alternarDivision(division: Division): Division {
  const activa = !division.activa;
  return {
    ...division,
    activa,
    foco: activa ? division.foco : 'principal',
    secundario: activa ? division.secundario : null
  };
}

/** Pone un documento en la segunda mitad y le da el foco. */
export function abrirEnSecundaria(division: Division, fileId: string): Division {
  return { ...division, activa: true, secundario: fileId, foco: 'secundaria' };
}

/** Anchos de las dos mitades, en porcentaje, listos para el estilo. */
export function anchos(division: Division): { principal: string; secundaria: string } {
  const principal = Math.round(division.reparto * 100);
  return { principal: `${principal}%`, secundaria: `${100 - principal}%` };
}
