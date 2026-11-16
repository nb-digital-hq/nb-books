/**
 * Los paneles del lector, en un solo trozo que se carga al abrir el primero.
 *
 * Ver `../diferidos.ts`: ninguno hace falta hasta que hay un documento abierto, y
 * por eso tampoco se exportan desde `$ui`.
 */

export { default as AnnotationsPanel } from './AnnotationsPanel.svelte';
export { default as AudioPanel } from './AudioPanel.svelte';
export { default as BookPanel } from './BookPanel.svelte';
export { default as LookupPanel } from './LookupPanel.svelte';
export { default as MapaDelLibro } from './MapaDelLibro.svelte';
export { default as ModePanel } from './ModePanel.svelte';
export { default as NarracionDelLibro } from './NarracionDelLibro.svelte';
export { default as NombresDelLibro } from './NombresDelLibro.svelte';
export { default as OutlinePanel } from './OutlinePanel.svelte';
export { default as SoundPanel } from './SoundPanel.svelte';
export { default as StudyPanel } from './StudyPanel.svelte';
export { default as VoicePanel } from './VoicePanel.svelte';
