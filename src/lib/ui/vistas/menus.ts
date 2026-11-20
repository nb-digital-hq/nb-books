/**
 * Lo que se abre desde la barra superior, en un solo trozo que se carga al abrir
 * el primero.
 *
 * Ver `../diferidos.ts`: ninguno se ve al arrancar, y por eso tampoco se exportan
 * desde `$ui`.
 */

export { default as AudiolibrosPanel } from './AudiolibrosPanel.svelte';
export { default as CollectionsMenu } from './CollectionsMenu.svelte';
export { default as ImportPanel } from './ImportPanel.svelte';
export { default as SearchPanel } from './SearchPanel.svelte';
export { default as ShelvesPanel } from './ShelvesPanel.svelte';
