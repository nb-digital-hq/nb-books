/**
 * La lectura en voz alta.
 *
 * Un solo punto de entrada, igual que `$lib/i18n` y que `$core/ipc`: quien narra
 * no tiene que saber qué proveedor hay detrás, que es justamente lo que permite
 * que el motor neuronal entre sin tocar a nadie.
 */

export type {
  DeclaracionDeVoz,
  EscuchaDeVoz,
  FinDeTramo,
  MarcaDeVoz,
  OpcionesDeVoz,
  ProveedorDeVoz,
  Tramo,
  VozDisponible
} from './contrato';
export {
  ajustar,
  alternar,
  avanzar,
  type BloqueNarrable,
  type EfectoDeVoz,
  type EstadoDeNarracion,
  type EstadoDeVoz,
  empezar,
  narracionInicial,
  parar,
  seNarra,
  type TransicionDeVoz,
  terminarTramo,
  VELOCIDAD_POR_DEFECTO,
  VELOCIDADES
} from './narracion';
export { narrador, type OrigenDeVoz } from './narrador.svelte';
// Ni la voz de un servicio ni la neuronal se reexportan aquí: sus módulos se
// cargan al elegirlas, y nombrarlas en este barril las traería al arranque. Ver
// `narrador.svelte.ts`.
export { VozDelRuntime } from './runtime';
export { VozDelSistema } from './sistema';
