/**
 * Lo que no hace falta para enseñar la primera pantalla, cargado cuando se abre.
 *
 * Contrato: `docs/architecture/performance-budgets.md`, sección 4.11,
 * y el criterio 3 en el plan: "si no cabe, lo que se parte es el
 * paquete por destino, no el presupuesto".
 *
 * Se arranca en Inicio (ADR-013), así que el lector, Estudio, Ajustes y Descubrir
 * no se pintan hasta que alguien va a ellos, y hay días en que no se va a
 * ninguno. Cargarlos al arrancar era leer y compilar su código antes de enseñar
 * nada, para nada. Lo mismo vale para lo que se abre a petición: los menús de la
 * barra, la ficha de una obra y el sintetizador de los fondos sonoros.
 *
 * **Por qué no están en `$ui`.** Un componente que se importa de forma estática
 * desde cualquier otro sitio vuelve entero al paquete de arranque sin que nada
 * falle ni se vea: la carga diferida sigue funcionando, solo que ya no difiere
 * nada. Un barril que los reexportara bastaría para eso. Por el mismo motivo
 * `scripts/check-paquete.mjs` comprueba sobre la compilación que cada uno de
 * estos sigue siendo un trozo aparte.
 */

/**
 * El lector, y de paso sus paneles.
 *
 * Los paneles se piden a la vez pero no se esperan: quien abre un libro abre un
 * panel poco después, y así el primero no tiene que cargarse al pulsar la tecla.
 * Se llama también **antes** de pedir el documento al runtime, para que las dos
 * esperas corran juntas y no una detrás de otra.
 */
export const cargarLector = () => {
  void cargarPanelesDelLector();
  return import('./lector/ReaderView.svelte');
};

/** Los siete paneles del lector van juntos: sin documento abierto no se usa ninguno. */
export const cargarPanelesDelLector = () => import('./lector/paneles');

export const cargarEstudio = () => import('./vistas/StudyView.svelte');

export const cargarAjustes = () => import('./vistas/SettingsView.svelte');

export const cargarDescubrir = () => import('./vistas/DiscoverView.svelte');

/** Los menús de la barra superior. Van juntos: son pequeños, y quien abre uno suele abrir otro. */
export const cargarMenus = () => import('./vistas/menus');

/** La ficha de una obra, que solo existe cuando se elige una en la biblioteca. */
export const cargarFicha = () => import('./vistas/DetailPanel.svelte');

/** El sintetizador de los fondos sonoros: no suena nada hasta que alguien lo pide. */
export const cargarFondoSonoro = () => import('./lector/fondoSonoro');

/** La narración y el reproductor de voz preparada: solo hacen falta con el lector abierto. */
export const cargarVoz = () => import('$lib/voz');
export const cargarReproductor = () => import('$lib/voz/reproductor.svelte');
