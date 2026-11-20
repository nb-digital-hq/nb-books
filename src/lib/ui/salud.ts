/**
 * Qué ofrece la interfaz según la salud del runtime.
 *
 * Contrato: `component-model.md`.
 *
 * # La regla, y por qué no es "deshabilitar"
 *
 * Una función cuyo componente no está **se oculta**, no se muestra rota ni en
 * gris. Un control deshabilitado es una promesa a medias: el usuario lo ve, lo
 * intenta y no pasa nada, y lo razonable es concluir que el problema es suyo.
 *
 * # Tres situaciones que no son la misma
 *
 * * **No instalado**: falta un motor y se puede instalar. Se dice, y se dice qué
 *   falta.
 * * **No soportado**: esta plataforma no lo tendrá nunca. Se oculta sin invitar
 *   a nada, porque invitar sería frustrar.
 * * **Degradado o caído**: estaba y ahora no. Se avisa con discreción y el resto
 *   de la aplicación sigue funcionando, que es la regla 1 de la Constitución.
 */

import type { ComponentState, ComponentStatus } from '$lib/bindings';

import type { NombreDeIcono } from './iconos';

/** Componentes que la interfaz consulta. Los nombres los fija el runtime. */
export const COMPONENTES = {
  base: 'database',
  lector: 'reader',
  busqueda: 'search',
  metadatos: 'metadata',
  ia: 'ai'
} as const;

/** Estados con los que un componente sirve para algo. */
const UTILIZABLES: readonly ComponentState[] = ['READY', 'DEGRADED'];

/** Si la función que depende de este componente se puede ofrecer. */
export function seOfrece(estados: readonly ComponentStatus[], componente: string): boolean {
  const estado = estados.find((e) => e.component === componente);
  // Sin noticias, se ofrece: el monitor publica cuando sabe algo, y esconder
  // media aplicación mientras arranca sería peor que ofrecerla un instante.
  return estado === undefined || UTILIZABLES.includes(estado.state);
}

/** Qué hay que contarle al usuario sobre un componente, si es que algo. */
export interface Aviso {
  componente: string;
  icono: NombreDeIcono;
  /** Código canónico; el texto lo pone la interfaz. */
  estado: ComponentState;
  /** Si conviene decir cómo arreglarlo. Solo cuando se puede. */
  accionable: boolean;
}

/**
 * Los avisos que merecen mostrarse, en orden de gravedad.
 *
 * `NOT_SUPPORTED` no genera aviso: esta plataforma no lo tendrá nunca, y
 * anunciarlo cada vez es recordarle al usuario algo que no puede cambiar.
 * `STOPPED` tampoco: es un componente que nadie ha pedido todavía.
 */
export function avisos(estados: readonly ComponentStatus[]): Aviso[] {
  const orden: Record<string, number> = { FAILED: 0, DEGRADED: 1, NOT_INSTALLED: 2 };

  return estados
    .filter((e) => e.state in orden)
    .map((e) => ({
      componente: e.component,
      estado: e.state,
      icono: (e.state === 'FAILED'
        ? 'error'
        : e.state === 'DEGRADED'
          ? 'degradado'
          : 'informacion') as NombreDeIcono,
      // Solo lo que el usuario puede arreglar: instalar lo que falta.
      accionable: e.state === 'NOT_INSTALLED'
    }))
    .sort((a, b) => (orden[a.estado] ?? 9) - (orden[b.estado] ?? 9));
}

/** Si hay algo que merezca el indicador discreto de la barra. */
export function hayAlgoQueDecir(estados: readonly ComponentStatus[]): boolean {
  return avisos(estados).length > 0;
}
