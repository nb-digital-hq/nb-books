/**
 * Componentes base y utilidades del sistema de diseño.
 *
 * Contrato: `docs/architecture/ui-components.md`.
 */

export { default as AppShell } from './AppShell.svelte';
export {
  ANCHO_MINIMO,
  type ClaseDeAncho,
  clasificarAncho,
  elPanelSeSuperpone,
  laNavegacionVaAbajo,
  panelesAcoplados
} from './anchos';
export { default as Button } from './Button.svelte';
export {
  contrasteHex,
  interpretarHex,
  luminanciaRelativa,
  type Rgb,
  relacionDeContraste,
  UMBRAL_INTERFAZ,
  UMBRAL_TEXTO
} from './contraste';
export { default as Desplegable } from './Desplegable.svelte';
export {
  cargarAjustes,
  cargarDescubrir,
  cargarEstudio,
  cargarFicha,
  cargarFondoSonoro,
  cargarLector,
  cargarMenus,
  cargarPanelesDelLector,
  cargarReproductor,
  cargarVoz
} from './diferidos';
export { default as Emergente } from './Emergente.svelte';
export { default as Field } from './Field.svelte';
export { default as Icon } from './Icon.svelte';
export { default as IconButton } from './IconButton.svelte';
export { CUANTOS_ICONOS, ICONOS, type NombreDeIcono } from './iconos';
export { default as LibraryView } from './LibraryView.svelte';
export { default as Logo } from './Logo.svelte';
// El lector. Su vista, su superficie y sus paneles no se exportan desde
// aquí: se cargan al abrir un documento, ver `diferidos.ts`. Desde aquí solo
// viajan los tipos que la carcasa necesita, para que el barril no devuelva su
// lógica al paquete de arranque (performance-budgets.md, sección 4.11).
export type { PanelDelLector } from './lector/comandos';
export type { EstadoDeConsulta } from './lector/consulta';
export {
  type ContextoDeNavegacion,
  DESTINOS,
  type Destino,
  destinosDelPie,
  destinosPrincipales,
  destinosVisibles,
  destinoTrasCerrar,
  type EntradaDeNavegacion,
  navegarCierraElDocumento,
  SIN_CONTEXTO
} from './lector/destinos';
export {
  abrirEnSecundaria,
  alternarDivision,
  alternarFoco,
  anchos,
  cabeLaDivision,
  DIVISION_INICIAL,
  type Division,
  type Mitad,
  PASO,
  REPARTO_MAXIMO,
  REPARTO_MINIMO,
  repartir
} from './lector/division';
// El sintetizador se carga al pedir el primer fondo (`diferidos.ts`); desde aquí
// solo viajan sus tipos, o el barril lo devolvería entero al arranque.
export type { ClaseDeRuido, Sonando } from './lector/fondoSonoro';
export {
  ALINEACIONES,
  type Alineacion,
  CLAVE_JUSTIFICADO,
  CLAVES,
  CLAVES_DEL_KIT,
  COMPOSICION_POR_DEFECTO,
  type Composicion,
  composicionDe,
  LIMITES,
  valorDe
} from './lector/libro';
export { aModo, contrasteDe, legibles, type ModoIlegible } from './lector/modoDeExtension';
export {
  filtroDe,
  KIT_POR_DEFECTO,
  type KitAccesible,
  MODO_POR_DEFECTO,
  MODOS,
  type Modo,
  modoPorNombre,
  type NombreDeModo,
  partirBionica,
  seAnima
} from './lector/modos';
export { default as NavItem } from './NavItem.svelte';
export { default as Notificaciones } from './Notificaciones.svelte';
export {
  type ClaseDeNotificacion,
  caducar,
  DURACION,
  descartar,
  encolar,
  type Notificacion,
  type PeticionDeAviso,
  VISIBLES
} from './notificaciones';
export { notificaciones, notificar } from './notificaciones.svelte';
export { default as Panel } from './Panel.svelte';
export {
  acotarPagina,
  NODOS_POR_ELEMENTO,
  nodosDe,
  numerosVisibles,
  type Peticion,
  PRESUPUESTO_DE_NODOS,
  paginas,
  peticionDe,
  TAMANO_POR_DEFECTO,
  TAMANOS_DE_PAGINA
} from './paginacion';
export { default as Range } from './Range.svelte';
export { default as Select } from './Select.svelte';
export { default as Spinner } from './Spinner.svelte';
export { default as Switch } from './Switch.svelte';
export { type Aviso, avisos, COMPONENTES, hayAlgoQueDecir, seOfrece } from './salud';
export { default as TarjetaLateral } from './TarjetaLateral.svelte';
export { default as Tooltip } from './Tooltip.svelte';
export {
  APARIENCIA_POR_DEFECTO,
  type Apariencia,
  aplicarApariencia,
  atributosDeApariencia,
  ESCALA_MAXIMA,
  ESCALA_MINIMA,
  type Esquema,
  elSistemaPrefiereOscuro,
  esquemaDe,
  leerApariencia,
  resolverTema,
  TEMAS,
  type Tema,
  type TemaDeExtensionDisponible,
  type TemaResuelto,
  type TokensDeTema
} from './tema';
export { type FalloDeContraste, fallosDeContraste } from './temaDeExtension';
export { default as Cover } from './vistas/Cover.svelte';
export { default as EmptyState } from './vistas/EmptyState.svelte';
export { default as Fila } from './vistas/Fila.svelte';
export { default as HomeView } from './vistas/HomeView.svelte';
export { default as Seccion } from './vistas/Seccion.svelte';
export type { AccionDeIa, EstadoDeIa, SeccionDeAjustes } from './vistas/SettingsView.svelte';
export type { Seccion as SeccionDeEstanteria } from './vistas/ShelvesPanel.svelte';
