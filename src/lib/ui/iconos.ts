/**
 * El catálogo de iconos de NB Books.
 *
 * # Por qué existe y por qué es una lista cerrada
 *
 * **Nunca hay emojis ni letras haciendo de icono.** Un emoji lo dibuja el
 * sistema operativo: cambia de forma, de color y de peso entre Windows, macOS y
 * Linux, no hereda el color del tema y en alto contraste se queda como estaba.
 * Una letra dentro de un botón es peor todavía, porque un lector de pantalla la
 * lee. Aquí todos los iconos son trazo sobre `currentColor`, así que cambian con
 * el tema sin que nadie tenga que acordarse.
 *
 * La lista es cerrada por los dos lados: un icono que no está obliga a decidir
 * qué significa antes de dibujarlo, y uno que sobra —el de la paleta de
 * comandos, cuando la paleta se retiró— sale. Setenta y seis iconos coherentes
 * valen más que quinientos con tres lenguajes visuales mezclados.
 *
 * # El lenguaje visual, y por qué estas cifras y no otras
 *
 * Rejilla de 24 con **área segura de 20**: nada se dibuja fuera de `2..22`, y
 * por eso dos iconos puestos uno al lado del otro pesan lo mismo aunque uno sea
 * un círculo y el otro un rectángulo. Trazo de 1,5, extremos y uniones
 * redondeados, sin relleno y sin color propio.
 *
 * Las familias comparten medidas a propósito, que es de donde sale la coherencia
 * y no de que todos estén "bien dibujados":
 *
 * * Círculos de estado y de contenedor: `r = 8.5` centrados en `12,12`.
 * * Contenedores rectangulares: de `3.5` a `20.5`, con `rx = 2`.
 * * Renglones de texto: `x` de 7,5 a 16,5, separados 4,5.
 * * Chevrones: 6,5 de recorrido; flechas rectas: 15 de fuste.
 *
 * Quien sustituya estos trazos por otros dibujos tiene que respetar eso mismo.
 * Contrato: `docs/design/icons.md`.
 */

/** Nombre de un icono del catálogo. */
export type NombreDeIcono = keyof typeof ICONOS;

/**
 * Los trazos de cada icono, en una rejilla de 24 por 24.
 *
 * El valor es el contenido del `<svg>`, sin la etiqueta: la pone `Icon.svelte`,
 * que es también quien decide el tamaño y la accesibilidad. Así no puede haber
 * dos iconos con `stroke-width` distinto.
 */
export const ICONOS = {
  // --- Destinos de la navegación primaria ---------------------------------
  inicio:
    '<path d="M3.5 11.2 12 4l8.5 7.2"/><path d="M5.75 9.9V19a1.5 1.5 0 0 0 1.5 1.5h9.5a1.5 1.5 0 0 0 1.5-1.5V9.9"/><path d="M9.75 20.5v-5.75h4.5v5.75"/>',
  biblioteca:
    '<path d="M3.5 4.5h4.25v15H3.5zM9.75 4.5H14v15H9.75z"/><path d="m16.15 5.6 3.7 1.05-3.4 12.85-3.7-1.05z"/>',
  descubrir: '<circle cx="12" cy="12" r="8.5"/><path d="m15.4 8.6-2.1 4.7-4.7 2.1 2.1-4.7z"/>',
  lector:
    '<path d="M12 6.6C10.1 4.9 7.3 4.5 3.5 4.5v13c3.8 0 6.6.4 8.5 2.1 1.9-1.7 4.7-2.1 8.5-2.1v-13c-3.8 0-6.6.4-8.5 2.1Z"/><path d="M12 6.6v13"/>',
  estudio:
    '<path d="M5.5 3.5h8l5 5v12h-13z"/><path d="M13.5 3.5v5h5"/><path d="M8.5 13.5h7M8.5 17h4.5"/>',
  ajustes:
    '<circle cx="12" cy="12" r="3.25"/><path d="M12 2.5v2.25M12 19.25v2.25M21.5 12h-2.25M4.75 12H2.5M18.72 5.28l-1.6 1.6M6.88 17.12l-1.6 1.6M18.72 18.72l-1.6-1.6M6.88 6.88l-1.6-1.6"/>',

  // --- Carcasa -------------------------------------------------------------
  menu: '<path d="M3.5 7h17M3.5 12h17M3.5 17h17"/>',
  buscar: '<circle cx="10.75" cy="10.75" r="6.75"/><path d="m15.55 15.55 4.95 4.95"/>',
  cerrar: '<path d="m6.5 6.5 11 11M17.5 6.5l-11 11"/>',
  atras: '<path d="M20.5 12H3.5"/><path d="m10 5.5-6.5 6.5 6.5 6.5"/>',
  adelante: '<path d="M3.5 12h17"/><path d="m14 5.5 6.5 6.5-6.5 6.5"/>',
  arriba: '<path d="m5.5 15.25 6.5-6.5 6.5 6.5"/>',
  abajo: '<path d="m5.5 8.75 6.5 6.5 6.5-6.5"/>',
  izquierda: '<path d="m15.25 5.5-6.5 6.5 6.5 6.5"/>',
  derecha: '<path d="m8.75 5.5 6.5 6.5-6.5 6.5"/>',
  expandir:
    '<path d="M9.5 3.5h-6v6M14.5 3.5h6v6M14.5 20.5h6v-6M9.5 20.5h-6v-6"/><path d="m3.5 3.5 5 5M20.5 3.5l-5 5M20.5 20.5l-5-5M3.5 20.5l5-5"/>',
  contraer:
    '<path d="M8.5 3.5v5h-5M15.5 3.5v5h5M15.5 20.5v-5h5M8.5 20.5v-5h-5"/><path d="m3.5 3.5 5 5M20.5 3.5l-5 5M20.5 20.5l-5-5M3.5 20.5l5-5"/>',
  mas: '<path d="M12 3.5v17M3.5 12h17"/>',
  comprobado: '<path d="m3.5 12.5 5.5 5.5L20.5 6.5"/>',

  // --- Biblioteca ----------------------------------------------------------
  cuadricula:
    '<rect x="3.5" y="3.5" width="7.5" height="7.5" rx="1.5"/><rect x="13" y="3.5" width="7.5" height="7.5" rx="1.5"/><rect x="3.5" y="13" width="7.5" height="7.5" rx="1.5"/><rect x="13" y="13" width="7.5" height="7.5" rx="1.5"/>',
  lista:
    '<path d="M8.5 6.5h12M8.5 12h12M8.5 17.5h12"/><path d="M3.5 6.5h1.25M3.5 12h1.25M3.5 17.5h1.25"/>',
  // Nueve celdas frente a las cuatro de `cuadricula`: la misma idea a otra
  // densidad, que es justo lo que distingue las dos vistas.
  mosaico:
    '<rect x="3.5" y="3.5" width="4.5" height="4.5" rx="1"/><rect x="9.75" y="3.5" width="4.5" height="4.5" rx="1"/><rect x="16" y="3.5" width="4.5" height="4.5" rx="1"/><rect x="3.5" y="9.75" width="4.5" height="4.5" rx="1"/><rect x="9.75" y="9.75" width="4.5" height="4.5" rx="1"/><rect x="16" y="9.75" width="4.5" height="4.5" rx="1"/><rect x="3.5" y="16" width="4.5" height="4.5" rx="1"/><rect x="9.75" y="16" width="4.5" height="4.5" rx="1"/><rect x="16" y="16" width="4.5" height="4.5" rx="1"/>',
  ordenar:
    '<path d="M6.5 3.5v17M3.5 17.5 6.5 20.5l3-3"/><path d="M13 6.5h7.5M13 12h5.5M13 17.5h3.5"/>',
  // El mismo dibujo al revés: la flecha sube y los renglones crecen. Con una
  // punta de flecha suelta, junto a un desplegable, se leía como su triángulo.
  ordenarAscendente:
    '<path d="M6.5 20.5v-17M3.5 6.5 6.5 3.5l3 3"/><path d="M13 6.5h3.5M13 12h5.5M13 17.5h7.5"/>',
  filtrar: '<path d="M3.5 4.5h17l-6.75 8v6.25l-3.5-2.25V12.5z"/>',
  estanteria:
    '<path d="M3.5 20.5h17"/><path d="M6 20.5v-9h3.25v9M12 20.5v-13h3.25v13M17.75 20.5v-6h2.75v6"/>',
  autor:
    '<circle cx="12" cy="8" r="3.75"/><path d="M4.75 20.5c0-3.75 3.25-6.25 7.25-6.25s7.25 2.5 7.25 6.25"/>',
  serie:
    '<rect x="3.5" y="5.5" width="9" height="13" rx="1.5"/><path d="M15.25 7.5v9M18.75 9.25v5.5"/>',
  materia:
    '<path d="M20 12.5 12.5 20a1.5 1.5 0 0 1-2.1 0l-6.9-6.9V4.5a1 1 0 0 1 1-1h8.6l6.9 6.9a1.5 1.5 0 0 1 0 2.1Z"/><circle cx="8.25" cy="8.25" r="1.25"/>',
  duplicado:
    '<rect x="3.5" y="3.5" width="12" height="12" rx="2"/><path d="M8.5 20.5h10a2 2 0 0 0 2-2v-10"/>',
  noDisponible: '<circle cx="12" cy="12" r="8.5"/><path d="m6 6 12 12"/>',
  portadaAusente:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="m3.5 16.25 4.75-4.75 3.5 3.5 3-3 5.75 5.75"/><circle cx="9" cy="9" r="1.5"/>',
  progreso:
    '<circle cx="12" cy="12" r="8.5"/><path d="M12 3.5a8.5 8.5 0 0 1 8.5 8.5"/><path d="M12 7.5V12l3 2"/>',
  importar:
    '<path d="M12 3.5v10.5M8 10.25l4 4 4-4"/><path d="M3.5 16.5v2.5a1.5 1.5 0 0 0 1.5 1.5h14a1.5 1.5 0 0 0 1.5-1.5v-2.5"/>',
  exportar:
    '<path d="M12 14V3.5M8 7.25l4-3.75 4 3.75"/><path d="M3.5 16.5v2.5a1.5 1.5 0 0 0 1.5 1.5h14a1.5 1.5 0 0 0 1.5-1.5v-2.5"/>',
  carpeta:
    '<path d="M3.5 6.5A1.5 1.5 0 0 1 5 5h4.25l2 2.5H19A1.5 1.5 0 0 1 20.5 9v9a1.5 1.5 0 0 1-1.5 1.5H5A1.5 1.5 0 0 1 3.5 18z"/>',

  // --- Lector --------------------------------------------------------------
  indice: '<path d="M3.5 6.5h17M7 12h13.5M7 17.5h13.5"/>',
  anotaciones: '<path d="M3.5 5.5h17v10h-9.5l-4.5 4v-4h-3z"/><path d="M8 10.5h8"/>',
  nota: '<path d="M4.5 4.5h15v9.75l-5.25 5.25H4.5z"/><path d="M19.5 14.25h-5.25v5.25"/><path d="M8.5 9.5h7"/>',
  resaltar:
    '<path d="m5 18.5 2.25-4.75 7.25-7.25 4.5 4.5-7.25 7.25z"/><path d="M14.5 6.5 19 11"/><path d="M3.5 21.5h17"/>',
  marcador: '<path d="M6.5 3.5h11v17l-5.5-4.5-5.5 4.5z"/>',
  // Subrayado: los renglones de la familia de texto, con una regla debajo. Se
  // distingue de `resaltar` en que ahí la línea la traza un rotulador en
  // diagonal y aquí es recta y va bajo las palabras, que es el gesto.
  subrayar: '<path d="M7.5 5.5h9M7.5 10h9M7.5 14.5h5"/><path d="M4.5 19.5h15"/>',
  // Trazo libre: tinta a mano alzada. Sin regla debajo a propósito, para no
  // confundirlo con el subrayado a tamaño pequeño.
  trazoLibre: '<path d="M3.5 15.5c3-6 5.5 4.5 8.5 0s4.5-7 8-2.5"/>',
  tipografia:
    '<path d="M3.5 19 8.75 5.5 14 19M5.6 14.75h6.3"/><path d="M16.5 10.5h4M18.5 10.5V19"/>',
  modoLectura:
    '<circle cx="6.5" cy="14.5" r="3.75"/><circle cx="17.5" cy="14.5" r="3.75"/><path d="M10.25 14.5h3.5"/><path d="M2.75 14.5V10l2.25-3.5M21.25 14.5V10L19 6.5"/>',
  inmersion: '<rect x="3.5" y="3.5" width="17" height="17" rx="2"/><path d="M8 12h8"/>',
  pantallaCompleta:
    '<path d="M3.5 8.5v-4a1 1 0 0 1 1-1h4M15.5 3.5h4a1 1 0 0 1 1 1v4M20.5 15.5v4a1 1 0 0 1-1 1h-4M8.5 20.5h-4a1 1 0 0 1-1-1v-4"/>',
  vistaDividida: '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M12 4.5v15"/>',
  // Cambiar de mitad es la vista dividida con el chevrón de "derecha" dentro de
  // la mitad que recibe el foco: la misma familia, no un dibujo nuevo.
  cambiarMitad:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M12 4.5v15"/><path d="m15.25 9.5 2.5 2.5-2.5 2.5"/>',
  // Extender la selección: el chevrón de mover, con el tope al que llega. Sin
  // esta pareja, extender y moverse compartían el icono de `expandir` y la barra
  // del lector enseñaba dos botones idénticos uno al lado del otro.
  extenderArriba: '<path d="M4.5 3.5h15"/><path d="m5.5 17.25 6.5-6.5 6.5 6.5"/>',
  extenderAbajo: '<path d="M4.5 20.5h15"/><path d="m5.5 6.75 6.5 6.5 6.5-6.5"/>',
  diccionario:
    '<path d="M5.5 5.5A2 2 0 0 1 7.5 3.5h13v14h-13a2 2 0 0 0-2 2z"/><path d="M9.5 8h6.5M9.5 11.5h4.5"/>',
  voz: '<rect x="9" y="3" width="6" height="10.5" rx="3"/><path d="M5.5 11.5a6.5 6.5 0 0 0 13 0"/><path d="M12 18v3"/>',
  reproducir: '<path d="M7.5 4.75 19.5 12l-12 7.25z"/>',
  pausa: '<path d="M9 4.5v15M15 4.5v15"/>',
  // Tres ondas de amplitud creciente: es un fondo sonoro, no una voz, así que
  // no repite el icono del micrófono.
  fondoSonoro:
    '<path d="M4 12h1.5"/><path d="M8 8.5v7"/><path d="M12 5.5v13"/><path d="M16 8.5v7"/><path d="M19.5 10.5v3"/>',

  // --- Estudio -------------------------------------------------------------
  vocabulario:
    '<rect x="3.5" y="4.5" width="13.5" height="11" rx="2"/><path d="M20.5 8v9.5a2 2 0 0 1-2 2H7"/><path d="M7.5 12.75 10 7.75l2.5 5M8.4 11.2h3.2"/>',
  metas:
    '<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="4.5"/><path d="M12 11.75h.01"/>',
  plan: '<rect x="3.5" y="5" width="17" height="15.5" rx="2"/><path d="M8 3.5v3M16 3.5v3M3.5 10.25h17"/>',
  tablero:
    '<rect x="3.5" y="3.5" width="17" height="17" rx="2"/><path d="M7.75 7.5v8M12 7.5v4.5M16.25 7.5v10"/>',
  enlace:
    '<path d="M10 14.25a4.5 4.5 0 0 0 6.4 0l2.9-2.9a4.5 4.5 0 0 0-6.4-6.4L11.4 6.45"/><path d="M14 9.75a4.5 4.5 0 0 0-6.4 0l-2.9 2.9a4.5 4.5 0 0 0 6.4 6.4l1.5-1.5"/>',
  retroenlace: '<path d="M20.5 12H8.5"/><path d="m14 6.5-5.5 5.5 5.5 5.5"/><path d="M3.5 4.5v15"/>',

  // --- Estado --------------------------------------------------------------
  correcto: '<circle cx="12" cy="12" r="8.5"/><path d="m8 12.25 2.75 2.75L16 9.5"/>',
  advertencia: '<path d="M12 3.5 20.75 20.5H3.25z"/><path d="M12 10v4.25M12 17.5h.01"/>',
  error: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5v5.25M12 16h.01"/>',
  informacion: '<circle cx="12" cy="12" r="8.5"/><path d="M12 11.25v5.25M12 8h.01"/>',
  sinConexion:
    '<path d="m3.5 3.5 17 17"/><path d="M8.5 15.5a5 5 0 0 1 6.15-.6"/><path d="M5.25 11.75a10 10 0 0 1 3.6-2.3M18.75 11.75a10 10 0 0 0-6.2-2.65"/><path d="M12 18.75h.01"/>',
  degradado: '<circle cx="12" cy="12" r="8.5"/><path d="M8 12h8"/>',
  cargando: '<path d="M12 3.5a8.5 8.5 0 1 1-6.01 2.49"/>',

  // --- Acciones ------------------------------------------------------------
  editar:
    '<path d="M4.5 19.5h3.25l9.75-9.75-3.25-3.25L4.5 16.25z"/><path d="m14.25 6.5 3.25 3.25"/><path d="m16.75 4 3.25 3.25-2.5 2.5"/>',
  borrar:
    '<path d="M3.5 6.5h17M9.5 6.5V4.5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v2"/><path d="m6 6.5 1.1 13.1a1.5 1.5 0 0 0 1.5 1.4h6.8a1.5 1.5 0 0 0 1.5-1.4L18 6.5"/><path d="M10.25 10.5v6.25M13.75 10.5v6.25"/>',
  copiar:
    '<rect x="8.5" y="8.5" width="12" height="12" rx="2"/><path d="M15.5 5.5a2 2 0 0 0-2-2h-8a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2"/>',
  reintentar: '<path d="M20.5 12a8.5 8.5 0 1 1-2.9-6.4"/><path d="M20.5 3.5v5.25h-5.25"/>',

  // --- Ajustes -------------------------------------------------------------
  apariencia: '<circle cx="12" cy="12" r="8.5"/><path d="M12 3.5v17"/>',
  accesibilidad:
    '<circle cx="12" cy="4.75" r="1.75"/><path d="M4.5 9.25h15"/><path d="M12 9.25v5.5M12 14.75 8.75 20.5M12 14.75l3.25 5.75"/>',
  idioma:
    '<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17"/><path d="M12 3.5c2.4 2.6 3.7 5.4 3.7 8.5s-1.3 5.9-3.7 8.5c-2.4-2.6-3.7-5.4-3.7-8.5s1.3-5.9 3.7-8.5Z"/>',
  almacenamiento:
    '<ellipse cx="12" cy="6.25" rx="7.5" ry="2.75"/><path d="M4.5 6.25v11.5c0 1.52 3.36 2.75 7.5 2.75s7.5-1.23 7.5-2.75V6.25"/><path d="M4.5 12c0 1.52 3.36 2.75 7.5 2.75s7.5-1.23 7.5-2.75"/>',
  telefono: '<rect x="6.5" y="2.5" width="11" height="19" rx="2"/><path d="M10.5 18h3"/>',
  red: '<path d="M12 18.75h.01"/><path d="M8.5 15.5a5 5 0 0 1 7 0"/><path d="M5.25 11.75a10 10 0 0 1 13.5 0"/>',
  privacidad:
    '<path d="M12 3.5 5 6.25v6.15c0 4.2 2.9 7.35 7 8.6 4.1-1.25 7-4.4 7-8.6V6.25z"/><path d="m9.25 12.25 2 2 3.5-3.75"/>',
  extensiones:
    '<rect x="3.5" y="3.5" width="7.5" height="7.5" rx="1.5"/><rect x="13" y="13" width="7.5" height="7.5" rx="1.5"/><path d="M13 7.25h7.5M7.25 13v7.5"/>',
  ia: '<rect x="7" y="7" width="10" height="10" rx="2"/><path d="M10 3.5V7M14 3.5V7M10 17v3.5M14 17v3.5M3.5 10H7M3.5 14H7M17 10h3.5M17 14h3.5"/>',
  cuenta:
    '<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="9.75" r="3"/><path d="M6.4 19.25a6.5 6.5 0 0 1 11.2 0"/>'
} as const satisfies Record<string, string>;

/** Cuántos iconos tiene el catálogo. Lo usa la prueba que lo compara con la documentación. */
export const CUANTOS_ICONOS = Object.keys(ICONOS).length;
