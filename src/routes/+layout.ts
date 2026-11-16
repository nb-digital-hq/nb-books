// NB Books es una aplicación de escritorio: no hay servidor y no hay
// renderizado en servidor. Ver docs/adr/001-tech-stack.md
import { leerCacheDeArranque } from '$core/settings';
import { cargarIdioma } from '$lib/i18n';

import type { LayoutLoad } from './$types';

export const ssr = false;
export const prerender = true;

/**
 * El diccionario del idioma elegido, antes de montar la interfaz.
 *
 * Solo el español viaja en el paquete de arranque, y `idioma.svelte.ts` cuenta
 * por qué. SvelteKit no pinta nada hasta que esto termina, así que quien tiene
 * la interfaz en otro idioma espera lo que tarda en leerse un archivo local y no
 * ve el español ni un fotograma: es el destello que `i18n.md` existe para evitar.
 */
export const load: LayoutLoad = async () => {
  await cargarIdioma(leerCacheDeArranque().language);
};
