/**
 * Los destinos de la navegación primaria y cuáles se ven.
 *
 * Contrato: `ADR-013`, con su criterio y sus cinco reglas.
 *
 * Tres reglas que aquí son código y no buenas intenciones:
 *
 * * **Un destino contextual no se muestra vacío ni atenuado.** El Lector no
 *   aparece hasta que hay algo abierto y Descubrir no aparece sin catálogos. Un
 *   elemento permanentemente en gris es publicidad de algo que el usuario no
 *   tiene.
 * * **Un destino cuya vista todavía no existe tampoco está.** Sería un botón que
 *   no lleva a ninguna parte, que es la misma regla vista del otro lado.
 * * **El Lector no se cierra al navegar.** Ir a Biblioteca durante una lectura
 *   no termina la sesión: se vuelve donde se dejó. Sin esto, consultar algo en
 *   la biblioteca mientras se lee castiga al usuario, y es la conducta más
 *   frecuente que hay.
 */

import type { NombreDeIcono } from '../iconos';

/** Los destinos de ADR-013. */
export type Destino = 'inicio' | 'biblioteca' | 'descubrir' | 'lector' | 'estudio' | 'ajustes';

/** Qué hace falta saber para decidir qué destinos se ven. */
export interface ContextoDeNavegacion {
  /** Hay un documento abierto: el Lector existe. */
  hayDocumentoAbierto: boolean;
  /** Hay catálogos remotos configurados: Descubrir existe. */
  hayCatalogos: boolean;
}

/** Un destino con lo que hace falta para pintarlo. */
export interface EntradaDeNavegacion {
  destino: Destino;
  icono: NombreDeIcono;
  /** Qué tiene que ser cierto para que aparezca. Sin esto, aparece siempre. */
  requiere?: keyof ContextoDeNavegacion;
  /** Va al fondo de la barra, separado de los demás. */
  alPie?: boolean;
}

/**
 * Los destinos, en el orden en que se pintan.
 *
 * El orden es el del recorrido natural: primero dónde se estaba, después lo que
 * se tiene, después lo que se está leyendo, y por último lo que se ha aprendido.
 * Ajustes va al pie porque no es un sitio donde se trabaje.
 */
export const DESTINOS: readonly EntradaDeNavegacion[] = [
  { destino: 'inicio', icono: 'inicio' },
  { destino: 'biblioteca', icono: 'biblioteca' },
  { destino: 'lector', icono: 'lector', requiere: 'hayDocumentoAbierto' },
  { destino: 'estudio', icono: 'estudio' },
  { destino: 'descubrir', icono: 'descubrir', requiere: 'hayCatalogos' },
  { destino: 'ajustes', icono: 'ajustes', alPie: true }
];

/** Contexto de una instalación recién estrenada: solo lo que existe siempre. */
export const SIN_CONTEXTO: ContextoDeNavegacion = {
  hayDocumentoAbierto: false,
  hayCatalogos: false
};

/** Los que se pintan ahora mismo, en orden. */
export function destinosVisibles(contexto: ContextoDeNavegacion): EntradaDeNavegacion[] {
  return DESTINOS.filter((d) => !d.requiere || contexto[d.requiere]);
}

/** Los que van al pie de la barra lateral. */
export function destinosDelPie(contexto: ContextoDeNavegacion): EntradaDeNavegacion[] {
  return destinosVisibles(contexto).filter((d) => d.alPie);
}

/** Los que van arriba, que son el recorrido principal. */
export function destinosPrincipales(contexto: ContextoDeNavegacion): EntradaDeNavegacion[] {
  return destinosVisibles(contexto).filter((d) => !d.alPie);
}

/**
 * A qué destino se va tras cerrar el documento.
 *
 * Cerrar el Lector estando en él deja al usuario sin destino: se vuelve a
 * Inicio, que es la pantalla de arranque. Cerrarlo desde otro sitio no mueve a
 * nadie.
 */
export function destinoTrasCerrar(actual: Destino): Destino {
  return actual === 'lector' ? 'inicio' : actual;
}

/**
 * Si navegar a `destino` debe terminar la sesión de lectura.
 *
 * Siempre falso, y existe como función para que la regla tenga un sitio y una
 * prueba en lugar de ser la ausencia de una línea de código que alguien podría
 * añadir sin saber lo que rompe.
 */
export function navegarCierraElDocumento(_destino: Destino): boolean {
  return false;
}
