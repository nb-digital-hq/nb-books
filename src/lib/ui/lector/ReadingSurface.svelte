<!--
  @component
  La superficie de lectura: lo que el formato dice que hay que pintar.

  Contrato: docs/architecture/reader-contracts.md, secciones 4.3,
  4.4 y 5..

  Dos formas, porque los formatos son dos cosas distintas:

  * **Marcado**, ya saneado en Rust. Se inserta tal cual y conserva su
    semántica, que es lo que permite que un lector de pantalla lo recorra como
    prosa. El saneado no se repite aquí: la política de contenido del webview
    es la segunda barrera, no la primera.
  * **Imagen**, servida por el protocolo de assets. El hueco se reserva con el
    tamaño que declara el proveedor **antes** de que la imagen llegue, y encima
    va la capa de texto, que es lo que impide que un PDF sea una imagen opaca
    para quien no la ve.

  El cursor de bloque se mueve por unidades del documento y no por caracteres.
  No es una simplificación: no hay navegación por cursor de texto garantizada en
  un webview —WebKitGTK, WebView2 y WKWebView no coinciden—, así que una
  selección que dependiera de ella funcionaría en una plataforma y no en otra.
  Por bloques funciona en las tres, y es además la misma regla para los dos
  formatos.
-->
<script lang="ts">
  import type { Locator, PageSize, Rendered, TextLayer } from '$lib/bindings';

  import { onDestroy, untrack } from 'svelte';

  import { rutaDeImagen } from '$core/ipc';
  import { textos } from '$lib/i18n';
  import { avisarQueElLectorPinta } from '../vistas/significado.svelte';
  import { Spinner } from '$ui';
  import { esRutaDeArchivo } from '$core/reader';
  import type { Bloque } from './estado';
  import type { Composicion } from './libro';
  import {
    KIT_POR_DEFECTO,
    type Modo,
    filtroDe,
    modoPorNombre,
    partirBionica,
    seAnima
  } from './modos';
  import {
    type MarcaDeTinta,
    type Punto as Punto2D,
    aPayload,
    aRuta,
    // `normalizar` ya existe aquí para el texto proyectado, que es otra cosa.
    normalizar as normalizarPunto
  } from './trazos';
  import {
    colocar,
    indiceDe,
    marcaEn,
    normalizar,
    rangoDe,
    recorrer,
    bloqueEn,
    type Anotacion,
    type InformeDeMarcas,
    type Marca,
    type Punto
  } from './proyeccion';
  import { medirMarcas, sePuedePintar, type MarcaMedida } from './resaltados';

  interface Props {
    /** Lo que el proveedor devolvió para esta posición. */
    contenido: Rendered | null;
    /** Tamaño intrínseco, solo para lo que se pinta como imagen. */
    tamano?: PageSize | null;
    /** Capa de texto, solo para lo que se pinta como imagen. */
    capa?: TextLayer | null;
    /** Bloque enfocado, o `-1` si todavía no se ha entrado al texto. */
    cursor?: number;
    /**
     * Bloque que la voz está diciendo, o `-1` si no hay narración.
     *
     * Es el seguimiento visual del texto. Va aparte de la selección
     * a propósito: se puede estar escuchando un párrafo y tener seleccionado
     * otro, y pintarlos igual haría creer que se va a anotar lo que suena.
     */
    narrado?: number;
    /**
     * El carácter del capítulo por el que va un audio que se sigue con el texto
     * -lo preparado, o la narración del propio libro-, o nulo.
     *
     * Llega como desplazamiento y no como bloque porque quien lo sabe es un
     * `Locator`; el bloque que lo contiene se busca aquí, que es donde está el
     * texto proyectado, y se resalta **igual que el que narra la voz**.
     */
    sonando?: number | null;
    /** Rango seleccionado, si lo hay. */
    rango?: { desde: number; hasta: number } | null;
    /** Se avisa cuando cambian los bloques que hay debajo. */
    alCambiarBloques?: (bloques: Bloque[]) => void;
    /** Se avisa al tocar una de las tres zonas de la superficie. */
    alTocar?: (zona: 'izquierda' | 'centro' | 'derecha') => void;
    /**
     * Se avisa, al parar de desplazarse, con el texto del primer bloque visible.
     *
     * Se manda **la cita y no una fracción**: quien convierte texto en posición
     * es el runtime, y una fracción calculada aquí no sabría rellenar los demás
     * selectores del Locator. Ver ADR-004.
     */
    alDesplazarse?: (cita: string) => void;
    /** Se seleccionó texto con el ratón: bloques que toca y texto exacto. */
    alSeleccionarTexto?: (desde: number, hasta: number, texto: string) => void;
    /** Ampliación de la página de imagen; 1 es al ancho de la columna. */
    zoom?: number;
    /**
     * Las anotaciones de este capítulo, para pintarlas sobre el texto.
     *
     * Vienen ya filtradas y en coordenadas del runtime: aquí no se decide a qué
     * texto se refiere ninguna, solo se traduce dónde cae. Ver `proyeccion.ts`.
     */
    anotaciones?: readonly Anotacion[];
    /**
     * Se avisa de qué se pudo pintar y qué no.
     *
     * Lo que no se pudo colocar **no se calla**: sale en el panel marcado, que
     * es lo mismo que el reanclaje hace con lo que no se puede reanclar.
     */
    alColocarMarcas?: (informe: InformeDeMarcas) => void;
    /** Se avisa al señalar una marca en el texto, con la anotación que es. */
    alSenalarMarca?: (id: string) => void;
    /**
     * Marcas que son un punto y no un tramo: marcadores y separador.
     *
     * Van aparte de `anotaciones` porque se resuelven distinto: un tramo se
     * pinta como rango y un punto señala el bloque que lo contiene.
     */
    puntos?: readonly Punto[];
    /**
     * Trazos libres, anclados como los puntos: por el bloque que los contiene.
     *
     * Van aparte de `anotaciones` porque no son un tramo de texto: son tinta
     * sobre una zona, y su geometría vive en el `payload` de la anotación.
     */
    tinta?: readonly MarcaDeTinta[];
    /** El lápiz está encendido: arrastrar dibuja en lugar de seleccionar. */
    dibujando?: boolean;
    /**
     * Se avisa al levantar el lápiz, con el texto del bloque y la geometría.
     *
     * Se manda el **texto** y no el desplazamiento porque anclar por cita es la
     * cascada de ADR-004 que ya usan las demás marcas: el runtime la resuelve, y
     * sobrevive a que el documento cambie. Un desplazamiento suelto habría
     * necesitado un camino de anclaje propio, que es justo lo que G1 prohíbe.
     */
    alDibujar?: (cita: string, tinta: string) => void;
    /** El plano del libro: cómo se compone el texto. */
    composicion?: Composicion;
    /** Los modos que aportan las extensiones: ver `ReaderView`. */
    modosDeExtension?: readonly Modo[];
    /** Textos. Se traducen. */
    etiquetas?: {
      region: string;
      pagina: string;
      sinCapaDeTexto: string;
      cargando: string;
    };
    /** Dirección del pase de página entre capítulos o páginas fijas (1: siguiente, -1: anterior). */
    direccionPase?: 1 | -1 | null;
    /** Se avisa cuando la animación de pase de página ha sido registrada. */
    alAnimarPase?: () => void;
  }

  let {
    contenido,
    tamano = null,
    capa = null,
    cursor = -1,
    narrado = -1,
    sonando = null,
    rango = null,
    alCambiarBloques,
    alTocar,
    alDesplazarse,
    alSeleccionarTexto,
    zoom = 1,
    anotaciones = [],
    puntos = [],
    tinta = [],
    dibujando = false,
    alDibujar,
    alColocarMarcas,
    alSenalarMarca,
    composicion,
    modosDeExtension = [],
    direccionPase = null,
    alAnimarPase,
    etiquetas: etiquetasRecibidas
  }: Props = $props();

  // Los textos del idioma en uso, derivados y no una constante: cambiar de
  // idioma con un libro abierto tiene que llegar también al nombre de la zona del
  // documento, que es lo primero que anuncia un lector de pantalla.
  const etiquetas = $derived({ ...textos().superficie, ...etiquetasRecibidas });

  /** Elementos de bloque del marcado que sirven de unidad anclable. */
  const ANCLABLES = 'p, h1, h2, h3, h4, h5, h6, li, blockquote, pre, dd, dt, figcaption, td, th';

  let lienzo: HTMLDivElement | undefined = $state();
  let contenedor: HTMLElement | undefined = $state();
  let animacion = $state<'siguiente' | 'anterior' | null>(null);
  let animacionTimer: ReturnType<typeof setTimeout> | undefined;

  function esMovimientoReducido(): boolean {
    if (
      typeof document !== 'undefined' &&
      document.documentElement.getAttribute('data-motion') === 'reduced'
    ) {
      return true;
    }
    if (
      typeof window !== 'undefined' &&
      window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
    ) {
      return true;
    }
    return false;
  }

  function debeAnimar(): boolean {
    return seAnima(composicion?.kit ?? KIT_POR_DEFECTO, esMovimientoReducido());
  }

  export function animarPase(dir: 1 | -1) {
    if (!debeAnimar()) return;
    clearTimeout(animacionTimer);
    animacion = dir === 1 ? 'siguiente' : 'anterior';
    animacionTimer = setTimeout(() => {
      animacion = null;
    }, 250);
  }

  $effect(() => {
    if (direccionPase && (composicion?.paginado || contenido?.kind === 'image')) {
      animarPase(direccionPase);
      alAnimarPase?.();
    }
  });

  function avanzarPaginado(direccion: 1 | -1): boolean {
    if (!contenedor) return false;
    const actual = contenedor.scrollLeft;
    const limite = contenedor.scrollWidth - contenedor.clientWidth;
    if (direccion === 1 && actual >= limite - 1) return false;
    if (direccion === -1 && actual <= 1) return false;
    contenedor.scrollBy({ left: contenedor.clientWidth * direccion, behavior: 'auto' });
    if (debeAnimar()) {
      animarPase(direccion);
    }
    return true;
  }

  function avanzarContinuo(direccion: 1 | -1): boolean {
    if (!contenedor) return false;
    const actual = contenedor.scrollTop;
    const limite = contenedor.scrollHeight - contenedor.clientHeight;
    if (direccion === 1 && actual >= limite - 1) return false;
    if (direccion === -1 && actual <= 1) return false;
    const reducido = esMovimientoReducido();
    contenedor.scrollBy({
      top: contenedor.clientHeight * 0.8 * direccion,
      behavior: reducido ? 'auto' : 'smooth'
    });
    return true;
  }

  export function avanzar(direccion: 1 | -1): boolean {
    return composicion?.paginado ? avanzarPaginado(direccion) : avanzarContinuo(direccion);
  }

  /**
   * Desplaza `px` píxeles en continuo. Devuelve si quedaba recorrido.
   *
   * Es lo que mueve el dedo en el teléfono con Leer en el PC y lo que mueve el
   * avance automático. En paginado no aplica: ahí lo que se mueve son páginas.
   */
  export function desplazar(px: number): boolean {
    if (!contenedor || composicion?.paginado) return false;
    const antes = contenedor.scrollTop;
    contenedor.scrollBy({ top: px, behavior: 'auto' });
    return contenedor.scrollTop !== antes;
  }

  /** Si la superficie pasa páginas en lugar de desplazarse. */
  export function paginada(): boolean {
    return Boolean(composicion?.paginado);
  }

  /*
   * El progreso se publica al **parar**, no mientras se desplaza.
   *
   * Cada aviso acaba en una escritura del runtime, y avisar por evento escribiría
   * decenas de veces por segundo mientras el dedo sigue en el ratón. Un cuarto de
   * segundo quieto es lo que separa "estoy buscando algo" de "aquí me he quedado".
   */
  const ESPERA_DE_REPOSO = 300;
  let reposo: ReturnType<typeof setTimeout> | undefined;

  function manejarScroll() {
    // El lector está pintando: la preparación de fondo se aparta. Solo
    // cruza el puente con una en marcha, y como mucho cada medio segundo.
    avisarQueElLectorPinta();
    clearTimeout(reposo);
    reposo = setTimeout(publicarPrimerBloqueVisible, ESPERA_DE_REPOSO);
  }

  onDestroy(() => {
    clearTimeout(reposo);
    clearTimeout(animacionTimer);
  });

  /**
   * Publica el texto del primer bloque que se ve entero.
   *
   * Es el primero **visible** y no el más cercano al centro porque es el que la
   * persona está a punto de leer: al volver, empezar por donde se dejó es
   * empezar por arriba, no por la mitad de lo ya leído.
   */
  /**
   * El primer bloque que se ve, o `null` si no se puede saber.
   *
   * Lo usan dos cosas por el mismo motivo —saber por dónde va quien lee— y por
   * eso el criterio está aquí una sola vez: apuntar la posición al parar de
   * desplazarse, y empezar a leer en alto por donde se está mirando.
   */
  function primerBloqueVisible(selector: string = ANCLABLES): HTMLElement | null {
    if (!contenedor || !lienzo) return null;
    const caja0 = contenedor.getBoundingClientRect();
    // Lo que se ve es el contenedor recortado por la ventana: si quien se
    // desplaza es un antepasado (WebKitGTK no siempre da altura al contenedor),
    // la caja del contenedor abarca el capítulo entero y todo contaría como
    // visible, hasta el principio.
    const marco = {
      top: Math.max(caja0.top, 0),
      bottom: Math.min(caja0.bottom, window.innerHeight),
      left: Math.max(caja0.left, 0),
      right: Math.min(caja0.right, window.innerWidth)
    };
    for (const bloque of lienzo.querySelectorAll<HTMLElement>(selector)) {
      // Un bloque que envuelve a otros (un diario entero dentro de una cita, una
      // lista con sus elementos) está siempre a la vista mientras se lee
      // cualquier parte de él: contarlo anclaba la posición al principio del
      // capítulo. Cuentan los de dentro.
      if (bloque.querySelector(selector)) continue;
      const caja = bloque.getBoundingClientRect();
      const dentro = composicion?.paginado
        ? caja.left >= marco.left - 1 && caja.left < marco.right
        : caja.bottom > marco.top && caja.top < marco.bottom;
      if (dentro) return bloque;
    }
    return null;
  }

  /** Texto comparable: sin espacios repetidos ni diferencias de mayúsculas. */
  function normal(t: string | null | undefined): string {
    return (t ?? '').replace(/\s+/g, ' ').trim().toLowerCase();
  }

  /**
   * Lleva la vista a un locator **dentro** del capítulo ya pintado.
   *
   * Antes nadie lo hacía: pintar un capítulo dejaba la vista
   * arriba, así que reabrir un libro guardado al 47 % lo enseñaba en el título
   * de la escena con el indicador diciendo 47 %. Primero se busca el bloque cuya
   * cita empieza como la del locator, y entre varios el más cercano a su
   * progresión, que es lo que desambigua un "HAMLET" repetido en una obra de
   * teatro. Sin cita que encaje, la progresión sola.
   */
  export function situar(locator: Locator): void {
    if (!contenedor || !lienzo) return;
    const progreso = locator.locations?.progression ?? 0;
    const cita = normal(locator.text?.highlight);
    const bloques = Array.from(lienzo.querySelectorAll<HTMLElement>(ANCLABLES));
    let destino: HTMLElement | null = null;
    if (cita.length >= 4 && bloques.length > 0) {
      let mejor = Number.POSITIVE_INFINITY;
      bloques.forEach((b, i) => {
        const texto = normal(b.textContent);
        if (!(texto.startsWith(cita) || (cita.startsWith(texto.slice(0, 40)) && texto.length >= 8)))
          return;
        const distancia = Math.abs(i / bloques.length - progreso);
        if (distancia < mejor) {
          mejor = distancia;
          destino = b;
        }
      });
    }
    const paginado = Boolean(composicion?.paginado);
    if (destino) {
      (destino as HTMLElement).scrollIntoView({
        block: 'start',
        inline: 'start',
        behavior: 'auto'
      });
    } else if (progreso > 0) {
      if (paginado) contenedor.scrollLeft = progreso * contenedor.scrollWidth;
      else contenedor.scrollTop = progreso * contenedor.scrollHeight;
    } else {
      contenedor.scrollTop = 0;
      contenedor.scrollLeft = 0;
    }
    // En paginado la vista tiene que caer en el borde de una página, no a mitad.
    if (paginado && contenedor.clientWidth > 0) {
      const ancho = contenedor.clientWidth;
      contenedor.scrollLeft = Math.floor(contenedor.scrollLeft / ancho) * ancho;
    }
  }

  /**
   * La cita del bloque por el que se va, para apuntarla sin esperar al reposo.
   *
   * Cerrar justo después de desplazarse cancelaba el temporizador del reposo y
   * la posición se perdía.
   */
  /**
   * Deja lo seleccionado por encima de una franja inferior de `alto` píxeles.
   *
   * El editor de la nota flota abajo, y si el párrafo que se anota estaba en esa
   * parte de la página quedaba tapado: se escribía sobre lo que se quería leer
   *. En paginado no se desplaza en vertical, así que no aplica.
   */
  export function despejar(alto: number): void {
    if (!contenedor || composicion?.paginado) return;
    const marcados = contenedor.querySelectorAll<HTMLElement>(
      '[data-nb-seleccionado="true"], [data-nb-cursor="true"]'
    );
    const ultimo = marcados[marcados.length - 1];
    if (!ultimo) return;
    const sobra =
      ultimo.getBoundingClientRect().bottom - (contenedor.getBoundingClientRect().bottom - alto);
    if (sobra > 0) contenedor.scrollBy({ top: sobra + 16, behavior: 'auto' });
  }

  export function citaActual(): string | null {
    const bloque = (llegoAlFinal() ? ultimoBloque() : null) ?? primerBloqueVisible();
    const cita = bloque?.textContent?.trim() ?? '';
    return cita.length >= 8 ? cita : null;
  }

  /**
   * Por qué bloque empezaría a narrar la voz.
   *
   * **El que se está mirando, no el primero del capítulo.** Empezar siempre por
   * el principio es lo que hacía que pedir que te leyeran en la página doce te
   * devolviera a la uno; en un libro de papel no hay forma de que eso ocurra,
   * porque quien lee en alto empieza donde le señalas.
   */
  export function bloqueParaNarrar(): number {
    // Se busca por `data-nb-bloque` y no por los anclables del marcado: en un
    // PDF las unidades son los fragmentos de la capa de texto, que llevan ese
    // atributo y no son ninguna de esas etiquetas. Narrar desde donde se mira
    // vale para los dos formatos o no vale para ninguno.
    const bloque = primerBloqueVisible('[data-nb-bloque]');
    if (!bloque) return 0;
    const indice = Number(bloque.dataset.nbBloque);
    return Number.isFinite(indice) ? indice : 0;
  }

  function elementoDeReferencia(): HTMLElement | null {
    if (cursor !== undefined && cursor >= 0) {
      const porBloque = lienzo?.querySelector<HTMLElement>(`[data-nb-bloque="${cursor}"]`);
      if (porBloque) return porBloque;
    }
    return primerBloqueVisible();
  }

  function encabezadoPrevio(
    encabezados: HTMLElement[],
    referencia: HTMLElement
  ): HTMLElement | null {
    let previo: HTMLElement | null = null;
    for (const h of encabezados) {
      if (h === referencia || h.contains(referencia)) return h;
      if (referencia.compareDocumentPosition(h) & Node.DOCUMENT_POSITION_PRECEDING) {
        previo = h;
      } else {
        return previo ?? h;
      }
    }
    return previo;
  }

  /**
   * El encabezado más cercano a donde se está leyendo.
   *
   * Permite a quien usa lector de pantalla situarse en el libro ("¿Dónde estoy?")
   * sin tener que recorrerlo. Prioriza el encabezado previo o coincidente con el
   * bloque enfocado (o el primer bloque visible).
   */
  export function encabezadoMasCercano(): string | null {
    if (!lienzo) return null;
    const encabezados = Array.from(
      lienzo.querySelectorAll<HTMLElement>('h1, h2, h3, h4, h5, h6, [role="heading"]')
    );
    if (encabezados.length === 0) return null;

    const referencia = elementoDeReferencia();
    const elegido = referencia
      ? (encabezadoPrevio(encabezados, referencia) ?? encabezados[0])
      : encabezados[0];
    return elegido?.textContent?.trim().replace(/\s+/g, ' ') || null;
  }

  /**
   * Si la vista llegó al final de lo que hay que leer.
   *
   * Hace falta porque "el primer bloque que se ve" no llega nunca al último: con
   * el final del capítulo en pantalla, arriba queda un párrafo de unos cuantos
   * antes, y un libro leído hasta la última línea se quedaba en el 95 %. Nunca
   * pasaba a terminado, y no salía de lo empezado en Inicio.
   */
  function llegoAlFinal(): boolean {
    if (!contenedor) return false;
    const margen = 2;
    return composicion?.paginado
      ? contenedor.scrollLeft + contenedor.clientWidth >= contenedor.scrollWidth - margen
      : contenedor.scrollTop + contenedor.clientHeight >= contenedor.scrollHeight - margen;
  }

  /** El último bloque que se puede anclar, que es donde está quien ya lo vio todo. */
  function ultimoBloque(): HTMLElement | null {
    const bloques = lienzo?.querySelectorAll<HTMLElement>(ANCLABLES);
    return bloques && bloques.length > 0 ? (bloques[bloques.length - 1] ?? null) : null;
  }

  function publicarPrimerBloqueVisible() {
    if (!alDesplazarse) return;
    const bloque = (llegoAlFinal() ? ultimoBloque() : null) ?? primerBloqueVisible();
    if (!bloque) return;

    const cita = bloque.textContent?.trim() ?? '';
    // Una cita vacía no se puede anclar, y una demasiado corta aparece en
    // media obra: el runtime la rechazaría o la anclaría en otro sitio.
    if (cita.length >= 8) alDesplazarse(cita);
  }
  /*
  /*
   * Las marcas del capítulo: se proyectan y se pintan en una capa propia detrás
   * del texto, sin registrar nada en `CSS.highlights`.
   *
   * La proyección (recorrer el árbol, normalizar el texto y comprobar qué citas
   * encajan) solo corre cuando cambia el capítulo, la lista de anotaciones o la
   * lectura biónica; la geometría (medir dónde caen los rectángulos de cada
   * marca y de cada trazo de tinta) vuelve a medirse además cuando cambia la
   * composición tipográfica o el tamaño de la superficie.
   */

  /**
   * La tinta ya colocada: cada trazo con la caja del bloque que lo contiene.
   *
   * Se guarda medida y no se mide al pintar porque medir obliga a resolver la
   * disposición, y hacerlo dentro del marcado lo haría en cada repintado.
   */
  let tintaColocada = $state<
    {
      id: string;
      caja: { left: number; top: number; width: number; height: number };
      marca: MarcaDeTinta;
    }[]
  >([]);
  /** Las marcas del capítulo medidas y listas para pintarse en `.nb-capa-marcas`. */
  let marcasMedidas = $state<MarcaMedida[]>([]);
  let proyectado: ReturnType<typeof normalizar> | null = null;
  let nodosDelTexto: Text[] = [];
  /** Cuántas veces se ha proyectado el texto: lo lee lo que depende de la proyección. */
  let proyecciones = $state(0);
  /** Cuántas veces ha cambiado el tamaño del contenedor o del lienzo. */
  let cambiosDeTamano = $state(0);

  /**
   * El bloque que suena, sea quien sea quien suena.
   *
   * La voz en vivo dice el bloque; un audio que se sigue con el texto dice un
   * carácter, y aquí se convierte en el bloque que lo contiene. A partir de ahí es
   * **el mismo resaltado y el mismo desplazamiento**: dos mecanismos para lo mismo
   * acabarían comportándose distinto.
   */
  const narradoEfectivo = $derived.by(() => {
    if (narrado >= 0) return narrado;
    void proyecciones;
    if (sonando === null || !proyectado) return -1;
    const bloque = bloqueEn(proyectado, nodosDelTexto, sonando);
    const indice = Number(bloque?.dataset.nbBloque);
    return Number.isFinite(indice) ? indice : -1;
  });
  let marcasPintadas: Marca[] = [];

  /**
   * Mide dónde cae cada trazo, una sola vez.
   *
   * Se mide aquí y no al pintar porque medir obliga a resolver la disposición, y
   * hacerlo dentro del marcado lo repetiría en cada repintado.
   */
  function colocarTinta(
    dentro: HTMLElement,
    texto: ReturnType<typeof normalizar>,
    nodos: readonly Text[]
  ): typeof tintaColocada {
    const colocada: typeof tintaColocada = [];
    const cajaDelLienzo = dentro.getBoundingClientRect();
    for (const marca of tinta) {
      const bloque = bloqueEn(texto, nodos, marca.inicio);
      if (!bloque) continue;
      const caja = bloque.getBoundingClientRect();
      colocada.push({
        id: marca.id,
        caja: {
          left: caja.left - cajaDelLienzo.left + dentro.scrollLeft,
          top: caja.top - cajaDelLienzo.top + dentro.scrollTop,
          width: caja.width,
          height: caja.height
        },
        marca
      });
    }
    return colocada;
  }

  /**
   * Si hay texto sobre el que colocar marcas, venga de donde venga.
   *
   * Los dos formatos entran por el mismo camino: el EPUB trae
   * marcado y el PDF trae fragmentos colocados, pero los dos son elementos del
   * documento y el mismo recorrido los encuentra.
   */
  function hayTextoQueMarcar(marcado: string | null, capaDePagina: TextLayer | null): boolean {
    return marcado !== null || (capaDePagina?.spans.length ?? 0) > 0;
  }

  $effect(() => {
    const html = marcadoDelCapitulo;
    /*
     * El PDF también entra por aquí, y esa es la.
     *
     * Antes se salía en cuanto no había marcado, así que sobre un PDF no se
     * pintaba ninguna marca: ni resaltados, ni subrayados, ni marcadores, ni
     * tinta. Su texto no es marcado sino fragmentos colocados, pero **son
     * elementos del documento igual**, así que el mismo recorrido los encuentra y
     * la misma proyección los ordena. No hace falta una segunda maquinaria.
     *
     * Lo que protege de que el sistema de coordenadas no cuadre es lo que ya
     * había: cada marca se valida contra su propia cita antes de pintarse, y la
     * que no cuadra se declara en el panel en lugar de caer en el sitio
     * equivocado. Si el texto plano del proveedor de PDF no coincidiera con lo
     * que proyecta la interfaz, se vería como marcas sin colocar y no como marcas
     * mal puestas.
     */
    const hayTexto = hayTextoQueMarcar(html, capa);
    const lista = anotaciones;
    const senalados = puntos;
    const bionicaActiva = kit?.bionica ?? false;
    /*
     * **Y cuando la superficie publica sus bloques.** Medido el 2026-09-11.
     *
     * Las marcas se colocan recorriendo el marcado ya insertado, y el primer
     * pintado puede llegar antes de que esté. Hasta ese día lo tapaba otra cosa: el
     * lector rehacía las marcas y los puntos con cada cambio de su estado, y alguno
     * llegaba ya con los bloques puestos. Al quitar ese recálculo —costaba el
     * capítulo entero por flecha—, el marcador y el separador dejaron de verse, y lo
     * encontró la suite. Depender de la publicación es el motivo verdadero: pinta
     * cuando el texto está, y no vuelve a pintar al mover el cursor.
     */
    void bloquesPublicados;
    if (!lienzo || !hayTexto) {
      proyectado = null;
      nodosDelTexto = [];
      marcasPintadas = [];
      marcasMedidas = [];
      tintaColocada = [];
      return;
    }

    // La lectura biónica envuelve los arranques de palabra en `<b data-nb-bionica>`
    // antes de recorrer el árbol, para que `nodosDelTexto` apunte siempre a los
    // nodos vivos del documento.
    aplicarBionica(lienzo, bionicaActiva);

    const { eventos, nodos } = recorrer(lienzo);
    const texto = normalizar(eventos);
    proyectado = texto;
    nodosDelTexto = nodos;

    /*
     * Los puntos se colocan **aunque el motor no sepa pintar rangos**.
     *
     * Un marcador y el separador no son un resaltado: se resuelven marcando el
     * bloque que los contiene, que es CSS corriente.
     */
    for (const bloque of lienzo.querySelectorAll<HTMLElement>('[data-nb-marca-punto]')) {
      delete bloque.dataset.nbMarcaPunto;
    }
    for (const punto of senalados) {
      const bloque = bloqueEn(texto, nodos, punto.inicio);
      // El separador no pisa a un marcador: quien puso el marcador lo puso a
      // propósito, y por dónde ibas se sabe además por otros seis sitios.
      if (bloque && bloque.dataset.nbMarcaPunto !== 'marcador') {
        bloque.dataset.nbMarcaPunto = punto.clase;
      }
    }

    if (!sePuedePintar()) {
      marcasPintadas = [];
      untrack(() => (proyecciones += 1));
      alColocarMarcas?.({ soportado: false, pintadas: 0, sinColocar: [] });
      return;
    }

    const { marcas, sinColocar } = colocar(texto, lista);

    const colocadas: Marca[] = [];
    const perdidas = [...sinColocar];
    for (const marca of marcas) {
      const rango = rangoDe(texto, nodos, marca);
      if (!rango) {
        perdidas.push(marca.id);
        continue;
      }
      colocadas.push(marca);
    }

    marcasPintadas = colocadas;
    untrack(() => (proyecciones += 1));
    alColocarMarcas?.({ soportado: true, pintadas: colocadas.length, sinColocar: perdidas });
  });

  $effect(() => {
    if (typeof ResizeObserver === 'undefined' || !lienzo) return;
    const elLienzo = lienzo;
    const elContenedor = contenedor;
    let ignorarPrimero = true;
    const observador = new ResizeObserver(() => {
      if (ignorarPrimero) {
        ignorarPrimero = false;
        return;
      }
      // Al fotograma siguiente: ver AppShell.
      requestAnimationFrame(() => (cambiosDeTamano += 1));
    });
    observador.observe(elLienzo);
    if (elContenedor && elContenedor !== elLienzo) observador.observe(elContenedor);
    return () => observador.disconnect();
  });

  /*
   * La tinta y las marcas van en capas aparte y **no dentro del bloque**.
   * Se miden aquí y se recolocan cuando cambia la proyección del capítulo,
   * la composición tipográfica (cuerpo, interlineado, margen, ancho, familia,
   * alineación, modo paginado, lectura biónica) o el tamaño de la superficie, pero
   * **no** al mover el cursor, extender la selección ni escuchar la voz.
   */
  $effect(() => {
    void proyecciones;
    void cambiosDeTamano;
    void composicion?.cuerpo;
    void composicion?.interlineado;
    void composicion?.margen;
    void composicion?.ancho;
    void composicion?.familia;
    void composicion?.alineacion;
    void composicion?.paginado;
    void kit?.familia;
    void kit?.bionica;
    void tinta;

    if (!lienzo || !proyectado) {
      tintaColocada = [];
      marcasMedidas = [];
      return;
    }

    tintaColocada = colocarTinta(lienzo, proyectado, nodosDelTexto);
    marcasMedidas = sePuedePintar()
      ? medirMarcas(lienzo, proyectado, nodosDelTexto, marcasPintadas)
      : [];
  });
  /**
   * Qué carácter del texto hay bajo un punto de la pantalla.
   *
   * Lo usan dos cosas —señalar una marca y anclar un trazo— y por eso está
   * aparte: son la misma pregunta, y tenerla escrita dos veces habría hecho que
   * una de las dos se quedara sin el repliegue a `caretRangeFromPoint`.
   */
  function indiceBajoElPunto(x: number, y: number): number | null {
    if (!proyectado) return null;
    const documento = document as Document & {
      caretPositionFromPoint?: (
        x: number,
        y: number
      ) => { offsetNode: Node; offset: number } | null;
    };
    let nodo: Node | null = null;
    let desplazamiento = 0;
    const posicion = documento.caretPositionFromPoint?.(x, y);
    if (posicion) {
      nodo = posicion.offsetNode;
      desplazamiento = posicion.offset;
    } else {
      const rango = document.caretRangeFromPoint?.(x, y);
      if (!rango) return null;
      nodo = rango.startContainer;
      desplazamiento = rango.startOffset;
    }
    if (!nodo || nodo.nodeType !== Node.TEXT_NODE) return null;
    return indiceDe(proyectado, nodosDelTexto, nodo as Text, desplazamiento);
  }

  /**
   * El trazo que se está dibujando, si el lápiz está apoyado.
   *
   * Se ancla al bloque donde **se apoya**, no donde se levanta: quien empieza a
   * dibujar sobre un párrafo y se sale por el margen quiere la tinta en ese
   * párrafo. Anclarla al final la movería según dónde levantara la mano.
   */
  let trazoEnCurso = $state<{
    cita: string;
    caja: { left: number; top: number; width: number; height: number };
    puntos: Punto2D[];
  } | null>(null);

  function apoyarLapiz(e: PointerEvent) {
    if (!dibujando || !lienzo || !proyectado) return;
    const indice = indiceBajoElPunto(e.clientX, e.clientY);
    if (indice === null) return;
    const bloque = bloqueEn(proyectado, nodosDelTexto, indice);
    if (!bloque) return;

    /*
     * El puntero se captura, y no es un detalle: sin capturar, sacar la mano del
     * bloque mientras se dibuja corta el trazo a la mitad y el resto se pierde.
     * Con captura, los eventos siguen llegando aquí hasta que se levante.
     */
    (e.target as Element).setPointerCapture?.(e.pointerId);
    e.preventDefault();

    /*
     * El texto del bloque, colapsando los espacios igual que hace el texto plano
     * del runtime: es lo que permite que la cita se encuentre allí.
     */
    const cita = (bloque.textContent ?? '').trim().replace(/\s+/g, ' ');
    if (cita.length === 0) return;

    const caja = bloque.getBoundingClientRect();
    const delLienzo = lienzo.getBoundingClientRect();
    trazoEnCurso = {
      cita,
      caja: {
        left: caja.left - delLienzo.left + lienzo.scrollLeft,
        top: caja.top - delLienzo.top + lienzo.scrollTop,
        width: caja.width,
        height: caja.height
      },
      puntos: [normalizarPunto(e.clientX, e.clientY, caja)]
    };
    cajaDelTrazo = caja;
  }

  /** La caja en coordenadas de ventana, para normalizar mientras se dibuja. */
  let cajaDelTrazo: DOMRect | null = null;

  function moverLapiz(e: PointerEvent) {
    if (!trazoEnCurso || !cajaDelTrazo) return;
    e.preventDefault();
    trazoEnCurso = {
      ...trazoEnCurso,
      puntos: [...trazoEnCurso.puntos, normalizarPunto(e.clientX, e.clientY, cajaDelTrazo)]
    };
  }

  /**
   * Lo que el teclado puede hacer con el lápiz, que no es dibujar.
   *
   * **Dibujar a mano alzada no tiene equivalente de teclado y no lo va a tener**:
   * un trazo es una forma continua que sale del movimiento de una mano, y no hay
   * secuencia de teclas que lo produzca sin convertirlo en otra cosa. Está
   * declarado en `accessibility.md` con su mitigación: lo que un trazo comunica
   * —señalar un pasaje— se alcanza con subrayado, resaltado o nota, y las tres
   * funcionan solo con el teclado.
   *
   * Lo que sí tiene teclado es **salir**: Escape cancela el trazo a medias, para
   * que apoyar sin querer no obligue a levantar y borrar después.
   */
  function teclaDelLapiz(e: KeyboardEvent) {
    if (e.key !== 'Escape' || !trazoEnCurso) return;
    e.preventDefault();
    trazoEnCurso = null;
    cajaDelTrazo = null;
  }

  function levantarLapiz() {
    const enCurso = trazoEnCurso;
    trazoEnCurso = null;
    cajaDelTrazo = null;
    if (!enCurso) return;

    // Un toque sin arrastre no es un trazo: sería un punto de tinta que nadie
    // quiso poner, y aparecería cada vez que alguien tocara el texto con el
    // lápiz encendido.
    if (enCurso.puntos.length < 2) return;

    const tinta = aPayload([{ puntos: enCurso.puntos }]);
    if (tinta) alDibujar?.(enCurso.cita, JSON.stringify(tinta));
  }

  function marcaBajoElPunto(x: number, y: number): Marca | null {
    if (!proyectado || marcasPintadas.length === 0) return null;
    const indice = indiceBajoElPunto(x, y);
    const porTexto = indice === null ? null : marcaEn(marcasPintadas, indice);
    if (porTexto) return porTexto;
    if (!lienzo) return null;
    const cajaDelLienzo = lienzo.getBoundingClientRect();
    const relX = x - cajaDelLienzo.left + lienzo.scrollLeft;
    const relY = y - cajaDelLienzo.top + lienzo.scrollTop;
    for (let i = marcasMedidas.length - 1; i >= 0; i -= 1) {
      const medida = marcasMedidas[i];
      if (!medida) continue;
      const dentro = medida.cajas.some(
        (c) =>
          relX >= c.left && relX <= c.left + c.width && relY >= c.top && relY <= c.top + c.height
      );
      if (dentro) return marcasPintadas.find((m) => m.id === medida.id) ?? null;
    }
    return null;
  }

  /** El bloque que contiene un nodo, por su índice, o `-1`. */
  function indiceDelBloque(nodo: Node | null): number {
    const el = nodo instanceof Element ? nodo : (nodo?.parentElement ?? null);
    const bloque = el?.closest<HTMLElement>('[data-nb-bloque]');
    const indice = Number(bloque?.dataset.nbBloque ?? -1);
    return Number.isFinite(indice) ? indice : -1;
  }

  /**
   * Al soltar el ratón con texto seleccionado, se avisa con sus bloques.
   * Antes seleccionar con el ratón no hacía nada: solo había
   * selección por bloques con el teclado, y una frase no se podía resaltar.
   */
  function soltarSeleccion() {
    if (dibujando || !alSeleccionarTexto || !lienzo) return;
    const seleccion = window.getSelection();
    if (!seleccion || seleccion.isCollapsed) return;
    const texto = seleccion.toString();
    if (texto.trim().length === 0) return;
    if (!lienzo.contains(seleccion.anchorNode) || !lienzo.contains(seleccion.focusNode)) return;
    alSeleccionarTexto(
      indiceDelBloque(seleccion.anchorNode),
      indiceDelBloque(seleccion.focusNode),
      texto
    );
  }

  function manejarClic(e: MouseEvent) {
    const seleccion = window.getSelection();
    if (seleccion && seleccion.toString().length > 0) return;

    // Señalar una marca gana a la zona de paso de página: quien pulsa sobre un
    // resaltado quiere ese resaltado, no la página siguiente.
    if (alSenalarMarca) {
      const marca = marcaBajoElPunto(e.clientX, e.clientY);
      if (marca) {
        /*
         * El clic no sigue subiendo, y esto no es una precaución: un panel
         * momentáneo se cierra al pulsar fuera, y escucha en `window`. Señalar
         * una marca abre el panel de anotaciones, Svelte aplica el cambio antes
         * de que el evento termine de subir, y **el mismo clic que lo abre lo
         * cierra**. Se ve como que pulsar un resaltado no hace nada.
         */
        e.stopPropagation();
        alSenalarMarca(marca.id);
        return;
      }
    }

    if (!alTocar) return;
    const objetivo = e.target as HTMLElement;
    if (objetivo.closest('a') || objetivo.closest('button') || objetivo.closest('.nb-controles'))
      return;
    const currentTarget = e.currentTarget as HTMLElement;
    if (!currentTarget) return;
    const ancho = currentTarget.clientWidth;
    const x = e.clientX - currentTarget.getBoundingClientRect().left;
    if (x < ancho * 0.3) alTocar('izquierda');
    else if (x > ancho * 0.7) alTocar('derecha');
    else alTocar('centro');
  }
  const modo = $derived(modoPorNombre(composicion?.modo ?? 'dia', modosDeExtension));
  const filtro = $derived(filtroDe(modo));
  const kit = $derived(composicion?.kit);

  const proporcion = $derived(
    tamano && tamano.width > 0 && tamano.height > 0 ? `${tamano.width} / ${tamano.height}` : 'auto'
  );

  /**
   * Recorre el marcado insertado y publica sus bloques.
   *
   * Se hace sobre el DOM ya compuesto y no analizando la cadena: el navegador ya
   * la ha analizado, y hacerlo otra vez daría una segunda interpretación que
   * podría no coincidir con la que se ve.
   */
  function bloquesDelMarcado(raiz: HTMLElement): Bloque[] {
    const elementos = Array.from(raiz.querySelectorAll<HTMLElement>(ANCLABLES));
    return elementos.map((elemento, indice) => {
      elemento.dataset.nbBloque = String(indice);
      return { texto: elemento.textContent?.trim() ?? '' };
    });
  }

  /**
   * El marcado del capítulo, con las imágenes ya apuntando a donde están.
   *
   * # Qué arregla
   *
   * El runtime extrae cada recurso del contenedor a su caché y deja en el `src`
   * **la ruta del archivo extraído**, porque el webview no puede leer dentro del
   * EPUB. Una ruta no es una dirección: puesta tal cual, el webview la resuelve
   * contra el origen de la aplicación y pide `http://localhost:1420/home/...`,
   * que no existe. El resultado era un 404 por imagen y ninguna ilustración de
   * ningún libro a la vista, portadilla incluida.
   *
   * # Por qué antes de insertar y no después
   *
   * Porque el navegador **empieza a pedir la imagen en cuanto analiza la
   * etiqueta**. Arreglar el `src` en un efecto posterior corrige la segunda
   * petición y deja la primera, así que el 404 seguiría ahí: medido, con la
   * corrección puesta sobre el DOM ya compuesto seguía saliendo una petición al
   * servidor por imagen.
   *
   * # Por qué con el analizador y no con una expresión regular
   *
   * Porque una expresión regular sobre marcado acierta hasta que un atributo
   * lleva dentro lo que la expresión busca. `DOMParser` construye un documento
   * **inerte**, sin contexto de navegación: nada de lo que hay dentro se pide
   * mientras se transforma. El marcado ya viene saneado de Rust por lista de
   * permitidos, así que aquí no se está confiando en nada nuevo.
   *
   * Es idempotente: lo ya convertido lleva esquema y `esRutaDeArchivo` lo
   * rechaza.
   */
  function conImagenesResueltas(html: string): string {
    // Sin analizador —al pintar en el servidor, en las pruebas de marcado— se
    // devuelve tal cual: allí no hay quien pida una imagen.
    if (typeof DOMParser === 'undefined') return html;

    const documento = new DOMParser().parseFromString(html, 'text/html');
    let cambiado = false;
    for (const imagen of documento.querySelectorAll('img[src]')) {
      const ruta = imagen.getAttribute('src') ?? '';
      if (!esRutaDeArchivo(ruta)) continue;
      imagen.setAttribute('src', rutaDeImagen(ruta));
      cambiado = true;
    }
    // Y la imagen de dentro de un dibujo, que es como muchos libros componen su
    // portada: el runtime la extrae igual que la de un `img`, y sin esto
    // se pedía por su ruta al servidor de la aplicación.
    for (const imagen of documento.querySelectorAll('svg image[href]')) {
      const ruta = imagen.getAttribute('href') ?? '';
      if (!esRutaDeArchivo(ruta)) continue;
      imagen.setAttribute('href', rutaDeImagen(ruta));
      cambiado = true;
    }

    // Sin nada que cambiar se devuelve la cadena original y no la reserializada:
    // volver a serializar un documento que no lo necesitaba es una forma barata
    // de introducir diferencias que nadie pidió.
    return cambiado ? documento.body.innerHTML : html;
  }

  const marcadoDelCapitulo = $derived(
    contenido?.kind === 'markup' ? conImagenesResueltas(contenido.html) : null
  );

  // Lo último que se avisó, para no repetirlo. Deliberadamente **no** es estado
  // reactivo: quien lo lee es el propio efecto que lo escribe, y hacerlo
  // reactivo lo volvería a disparar.
  let ultimosBloques: string | null = null;

  /*
   * Cuántas veces se han publicado bloques nuevos. Lo lee el pintado de marcas para
   * repetirse con el texto ya insertado. Se escribe desde un contador aparte, sin
   * leer el estado, para que el efecto que publica no dependa de él.
   */
  let publicaciones = 0;
  let bloquesPublicados = $state(0);

  $effect(() => {
    if (!lienzo || !contenido) return;

    const bloques =
      contenido.kind === 'markup'
        ? bloquesDelMarcado(lienzo)
        : // Sobre una imagen, las unidades anclables son los fragmentos de la
          // capa de texto: son el mismo texto que el runtime buscará al anclar
          // la cita.
          (capa?.spans ?? []).map((s) => ({ texto: s.text }));

    // Avisar en cada pasada del efecto sería un bucle: quien escucha guarda los
    // bloques en su estado, ese estado vuelve a renderizar esta superficie, y
    // la superficie vuelve a avisar. Se compara antes de hablar.
    const huella = JSON.stringify(bloques);
    if (huella === ultimosBloques) return;
    ultimosBloques = huella;
    publicaciones += 1;
    bloquesPublicados = publicaciones;
    alCambiarBloques?.(bloques);
  });

  /*
   * Mover el cursor sin llevarse el foco dejaría al lector de pantalla leyendo
   * otro sitio del que se está señalando.
   *
   * El desplazamiento se pide **aparte y `nearest`**, y esa es la diferencia
   * entre leer y pasar diapositivas. Dejando que lo hiciera el foco, el
   * navegador centraba cada párrafo al enfocarlo: bajar una línea movía el texto
   * media pantalla, y avanzar por el capítulo parecía ir por páginas en lugar de
   * bajar. Con `nearest` solo se mueve cuando el párrafo se sale, y lo justo
   * para que entre.
   */
  $effect(() => {
    if (!lienzo || cursor < 0) return;
    const destino = lienzo.querySelector<HTMLElement>(`[data-nb-bloque="${cursor}"]`);
    if (!destino) return;
    destino.focus({ preventScroll: true });
    destino.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  });

  /*
   * La selección se marca sobre el marcado insertado, que Svelte no compone: sin
   * esto, extender la selección a tres párrafos solo se vería en el que tiene el
   * foco, y el usuario anotaría sin saber qué está anotando. Los fragmentos del
   * PDF sí los compone Svelte y llevan su marca puesta en la plantilla.
   */
  /**
   * La parada de tabulación del texto, que es **una sola**.
   *
   * Tabulador móvil, igual que el panel de anotaciones: el texto entero es una
   * parada y dentro se navega con el cursor de bloque. Las dos alternativas son
   * peores. Con todos los párrafos en el recorrido, pasar al siguiente control
   * cuesta cien pulsaciones. Con ninguno, no se puede entrar al texto con el
   * tabulador: hay que saber que se entra con la flecha abajo, y eso no lo
   * anuncia nadie. Además dejaba la zona que desplaza sin acceso por teclado,
   * que es una violación seria de WCAG 2.1.1 y la encontró la auditoría de
   * la lectura en modo inmersivo, donde no queda ningún otro control dentro.
   */
  const paradaDeTabulacion = $derived(cursor < 0 ? 0 : cursor);

  /*
   * **Solo se visita lo que puede cambiar.** Medidas del 2026-09-11.
   *
   * Este efecto corre con cada flecha. Reescribía los cuatro atributos de todos los
   * bloques del capítulo aunque solo cambiaran dos: perfilado en Chromium sobre mil
   * seiscientos párrafos, unos 5 ms por tecla. El primer arreglo dejó de escribir lo
   * que no cambiaba, pero seguía **leyendo** los cuatro de cada bloque, y en
   * WebKitGTK, con la tecla pulsada al principio del fotograma, eso eran 6 o 7 ms de
   * efectos por flecha y entre una y dieciocho flechas de cuarenta que se veían un
   * fotograma tarde.
   *
   * Ahora se recuerda qué se escribió la última vez. Si los bloques son los mismos,
   * solo se visitan el cursor, la parada de tabulación y el bloque narrado, de antes
   * y de ahora, y los dos tramos de selección si la selección cambió: todo lo que se
   * escribe depende solo del índice, así que un bloque cuyo índice no está en esa
   * lista ya tiene sus atributos bien. Si no son los mismos —un capítulo nuevo trae
   * bloques sin ningún atributo— se recorren todos, como antes.
   *
   * "Los mismos" se decide mirando la lista: mismo número, y el primero y el último
   * son los mismos nodos. El marcado se inserta de una vez con `{@html}` y los
   * fragmentos del PDF llevan clave por índice, así que ninguno de los dos cambia un
   * bloque del medio dejando los extremos. Se descartó fiarse de `bloquesPublicados`:
   * el aviso no se repite si el texto es idéntico, y un capítulo vuelto a insertar
   * con el mismo texto trae nodos nuevos sin atributos.
   *
   * El `tabindex` se compara con el **atributo** y no con la propiedad: un párrafo
   * sin atributo ya dice `-1`, y sin escribirlo no se le puede dar el foco. El
   * efecto del cursor, que va antes, se quedaría enfocando un párrafo que no lo
   * admite.
   */
  function poner(
    bloque: HTMLElement,
    clave: 'nbSeleccionado' | 'nbNarrado' | 'nbCursor',
    valor: boolean
  ): void {
    const texto = String(valor);
    if (bloque.dataset[clave] !== texto) bloque.dataset[clave] = texto;
  }

  function escribirBloque(
    bloque: HTMLElement,
    indice: number,
    narrando: number,
    parada: number
  ): void {
    poner(bloque, 'nbSeleccionado', seleccionado(indice));
    poner(bloque, 'nbNarrado', indice === narrando);
    // La regla de lectura se dibuja sobre este: el cursor, no la selección.
    // Extender la selección a cinco párrafos no debería dibujar cinco reglas.
    poner(bloque, 'nbCursor', indice === cursor);
    const tabulacion = String(indice === parada ? 0 : -1);
    if (bloque.getAttribute('tabindex') !== tabulacion) {
      bloque.setAttribute('tabindex', tabulacion);
    }
  }

  type Marcas = {
    narrando: number;
    parada: number;
    cursor: number;
    rango: { desde: number; hasta: number } | null;
  };

  // Deliberadamente **no** es estado reactivo, por lo mismo que `ultimosBloques`.
  let escrito:
    (Marcas & { bloques: NodeListOf<HTMLElement>; porIndice: Map<number, HTMLElement> }) | null =
    null;

  function mismosBloques(antes: NodeListOf<HTMLElement>, ahora: NodeListOf<HTMLElement>): boolean {
    const n = ahora.length;
    return (
      antes.length === n && (n === 0 || (antes[0] === ahora[0] && antes[n - 1] === ahora[n - 1]))
    );
  }

  function escribirTodos(
    bloques: NodeListOf<HTMLElement>,
    marcas: Marcas
  ): Map<number, HTMLElement> {
    const porIndice = new Map<number, HTMLElement>();
    for (const bloque of bloques) {
      const indice = Number(bloque.dataset.nbBloque);
      porIndice.set(indice, bloque);
      escribirBloque(bloque, indice, marcas.narrando, marcas.parada);
    }
    return porIndice;
  }

  function indicesQueCambian(antes: Marcas, ahora: Marcas, total: number): Set<number> {
    const indices = new Set([
      antes.narrando,
      ahora.narrando,
      antes.parada,
      ahora.parada,
      antes.cursor,
      ahora.cursor
    ]);
    if (antes.rango?.desde === ahora.rango?.desde && antes.rango?.hasta === ahora.rango?.hasta) {
      return indices;
    }
    for (const tramo of [antes.rango, ahora.rango]) {
      if (!tramo) continue;
      const hasta = Math.min(tramo.hasta, total - 1);
      for (let indice = Math.max(tramo.desde, 0); indice <= hasta; indice += 1) {
        indices.add(indice);
      }
    }
    return indices;
  }

  $effect(() => {
    if (!lienzo) return;
    const ahora: Marcas = { narrando: narradoEfectivo, parada: paradaDeTabulacion, cursor, rango };
    const bloques = lienzo.querySelectorAll<HTMLElement>('[data-nb-bloque]');
    const antes = escrito;

    if (antes === null || !mismosBloques(antes.bloques, bloques)) {
      escrito = { ...ahora, bloques, porIndice: escribirTodos(bloques, ahora) };
      return;
    }
    for (const indice of indicesQueCambian(antes, ahora, bloques.length)) {
      const bloque = antes.porIndice.get(indice);
      if (bloque) escribirBloque(bloque, indice, ahora.narrando, ahora.parada);
    }
    escrito = { ...antes, ...ahora };
  });

  /*
   * El bloque que suena se lleva la vista consigo.
   *
   * Sin esto, escuchar un capítulo entero deja el texto quieto mientras la voz
   * avanza, que es justo lo contrario de "seguir el texto con la voz". No se
   * mueve el foco, solo la vista: mover el foco interrumpiría al lector de
   * pantalla de quien esté usando los dos a la vez.
   */
  $effect(() => {
    if (!lienzo || narradoEfectivo < 0) return;
    const destino = lienzo.querySelector<HTMLElement>(`[data-nb-bloque="${narradoEfectivo}"]`);
    destino?.scrollIntoView({ block: 'center', behavior: 'auto' });
  });

  function seleccionado(indice: number): boolean {
    return rango !== null && indice >= rango.desde && indice <= rango.hasta;
  }

  /**
   * El marcado original de cada bloque, antes de la lectura biónica.
   *
   * Hace falta para poder **apagarla**: la transformación envuelve trozos de
   * texto, y deshacerla adivinando qué envoltorios eran suyos sería frágil. Un
   * `WeakMap` porque las claves son los propios elementos: cuando el navegador
   * los tira al cambiar de página, esto se vacía solo.
   */
  const marcadoOriginal = new WeakMap<HTMLElement, string>();

  /**
   * Pone en negrita el arranque de cada palabra.
   *
   * Es el punto de fijación de la lectura biónica, que el kit accesible de
   * la lectura biónica se ofrecía como interruptor y **no hacía nada**: `partirBionica` estaba
   * escrita y probada, y no la llamaba nadie.
   *
   * Se hace sobre los nodos de texto y no sobre la cadena del documento por lo
   * mismo que la publicación de bloques: el navegador ya analizó el marcado, y
   * volver a analizarlo daría una segunda interpretación que podría no coincidir
   * con la que se ve. Y como solo **envuelve** texto, `textContent` no cambia:
   * la cita que viaja al anclar sigue siendo exactamente la misma.
   */
  /**
   * Los nodos de texto de un bloque, en el orden en que se leen, **sin los de las
   * fórmulas**.
   *
   * Dentro de un `<mi>` de MathML solo puede haber texto: una negrita de HTML
   * metida ahí no se pinta como negrita, rompe la fórmula. La lectura
   * biónica es para la prosa, y una fórmula no se lee palabra a palabra.
   */
  function nodosDeTexto(bloque: HTMLElement): Text[] {
    const recorrido = document.createTreeWalker(bloque, NodeFilter.SHOW_TEXT, {
      acceptNode: (nodo) =>
        nodo.parentElement?.closest('math') ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT
    });
    const textos: Text[] = [];
    for (let nodo = recorrido.nextNode(); nodo; nodo = recorrido.nextNode()) {
      textos.push(nodo as Text);
    }
    return textos;
  }

  /**
   * Un nodo de texto, con el arranque de cada palabra en negrita.
   *
   * Se parte conservando los separadores para no perder los espacios: sin ellos,
   * un párrafo entero se convierte en una sola palabra.
   */
  function trozosBionicos(contenidoDelNodo: string): DocumentFragment {
    const fragmento = document.createDocumentFragment();
    for (const trozo of contenidoDelNodo.split(/(\s+)/)) {
      if (trozo.length === 0) continue;
      if (trozo.trim().length === 0) {
        fragmento.append(trozo);
        continue;
      }
      const [cabeza, cola] = partirBionica(trozo);
      const fuerte = document.createElement('b');
      fuerte.dataset.nbBionica = 'si';
      fuerte.textContent = cabeza;
      fragmento.append(fuerte);
      if (cola) fragmento.append(cola);
    }
    return fragmento;
  }

  function marcarBionica(bloque: HTMLElement): void {
    for (const texto of nodosDeTexto(bloque)) {
      const contenidoDelNodo = texto.nodeValue ?? '';
      if (contenidoDelNodo.trim().length === 0) continue;
      texto.replaceWith(trozosBionicos(contenidoDelNodo));
    }
  }

  function aplicarBionica(raiz: HTMLElement, activa: boolean): void {
    for (const bloque of raiz.querySelectorAll<HTMLElement>('[data-nb-bloque]')) {
      const yaEsta = bloque.dataset.nbBionicaAplicada === 'si';
      if (activa === yaEsta) continue;

      if (activa) {
        marcadoOriginal.set(bloque, bloque.innerHTML);
        marcarBionica(bloque);
        bloque.dataset.nbBionicaAplicada = 'si';
      } else {
        const original = marcadoOriginal.get(bloque);
        if (original !== undefined) bloque.innerHTML = original;
        delete bloque.dataset.nbBionicaAplicada;
      }
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="nb-superficie"
  bind:this={contenedor}
  onclick={manejarClic}
  onscroll={manejarScroll}
  onkeydown={teclaDelLapiz}
  onpointerdown={apoyarLapiz}
  onpointermove={moverLapiz}
  onpointerup={() => {
    levantarLapiz();
    soltarSeleccion();
  }}
  onpointercancel={levantarLapiz}
  data-dibujando={dibujando}
  aria-label={etiquetas.region}
  style:--nb-libro-fondo={modo.fondo}
  style:--nb-libro-texto={modo.texto}
  style:--nb-libro-filtro={filtro}
  data-kit-familia={kit?.familia ?? 'ninguna'}
  data-foco-de-linea={// Sin cursor no hay línea en foco: apagarlo todo dejaba el texto ilegible.
  (kit?.focoDeLinea ?? false) && cursor !== undefined && cursor >= 0}
  data-regla={kit?.regla ?? false}
  style:--nb-libro-familia={composicion?.familia === 'system'
    ? 'var(--nb-font-lectura)'
    : composicion?.familia}
  style:--nb-libro-cuerpo={composicion ? `${composicion.cuerpo}px` : null}
  style:--nb-libro-interlineado={composicion?.interlineado}
  style:--nb-libro-margen={composicion ? `${composicion.margen}px` : null}
  style:--nb-libro-ancho={composicion ? `${composicion.ancho}ch` : null}
  data-alineacion={composicion?.alineacion ?? 'izquierda'}
  data-paginado={composicion?.paginado ?? false}
>
  {#if !contenido}
    <Spinner etiqueta={etiquetas.cargando} />
  {:else if contenido.kind === 'markup'}
    <!--
      El marcado llega saneado desde Rust por lista de permitidos: sin scripts,
      sin recursos remotos y sin nada que navegue fuera del documento.
    -->
    <div
      class="nb-texto"
      class:nb-selectable={!dibujando}
      class:nb-pase-siguiente={animacion === 'siguiente'}
      class:nb-pase-anterior={animacion === 'anterior'}
      onanimationend={() => {
        animacion = null;
      }}
      bind:this={lienzo}
    >
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      {@html marcadoDelCapitulo}
      {#if marcasMedidas.length > 0}
        <div class="nb-capa-marcas" aria-hidden="true">
          {#each marcasMedidas as marca (marca.id)}
            {@const enFoco =
              marca.bloque !== null && (marca.bloque === cursor || seleccionado(marca.bloque))}
            {#each marca.cajas as caja, indice (indice)}
              <span
                class="nb-marca-item"
                data-nb-marca={marca.clase}
                data-nb-marca-id={marca.id}
                data-nb-marca-cita={indice === 0 ? marca.cita : undefined}
                data-en-foco={enFoco}
                style:left="{caja.left}px"
                style:top="{caja.top}px"
                style:width="{caja.width}px"
                style:height="{caja.height}px"
              ></span>
            {/each}
          {/each}
        </div>
      {/if}
      {#if trazoEnCurso}
        <!--
          El trazo mientras se dibuja.

          Va fuera del bloque de los guardados a propósito: el primero se dibuja
          cuando todavía no hay ninguno, y dentro no se vería hasta soltar.
        -->
        <svg
          class="nb-tinta nb-tinta-en-curso"
          aria-hidden="true"
          viewBox="0 0 1 1"
          preserveAspectRatio="none"
          style:left="{trazoEnCurso.caja.left}px"
          style:top="{trazoEnCurso.caja.top}px"
          style:width="{trazoEnCurso.caja.width}px"
          style:height="{trazoEnCurso.caja.height}px"
        >
          <path d={aRuta({ puntos: trazoEnCurso.puntos })} />
        </svg>
      {/if}
      {#if tintaColocada.length > 0}
        <!--
          La tinta: un SVG por trazo, sobre el bloque donde se dibujó.

          `viewBox="0 0 1 1"` con `preserveAspectRatio="none"` hace que las
          coordenadas guardadas de 0 a 1 se estiren con el bloque, que es lo que
          permite que el trazo siga rodeando las mismas palabras después de
          cambiar el cuerpo de letra.

          `aria-hidden` y `pointer-events: none`: es tinta, no contenido. Lo que
          un lector de pantalla necesita saber de ella está en el panel de
          anotaciones, con su clase y su posición.
        -->
        {#each tintaColocada as puesta (puesta.id)}
          <svg
            class="nb-tinta"
            aria-hidden="true"
            viewBox="0 0 1 1"
            preserveAspectRatio="none"
            style:left="{puesta.caja.left}px"
            style:top="{puesta.caja.top}px"
            style:width="{puesta.caja.width}px"
            style:height="{puesta.caja.height}px"
          >
            {#each puesta.marca.tinta.trazos as trazo, indice (indice)}
              <path d={aRuta(trazo)} />
            {/each}
          </svg>
        {/each}
      {/if}
    </div>
  {:else}
    <div
      class="nb-pagina"
      class:nb-pase-siguiente={animacion === 'siguiente'}
      class:nb-pase-anterior={animacion === 'anterior'}
      onanimationend={() => {
        animacion = null;
      }}
      bind:this={lienzo}
      style:aspect-ratio={proporcion}
      style:--nb-zoom={zoom}
    >
      <img src={rutaDeImagen(contenido.path)} alt={etiquetas.pagina} />
      {#if marcasMedidas.length > 0}
        <div class="nb-capa-marcas" aria-hidden="true">
          {#each marcasMedidas as marca (marca.id)}
            {@const enFoco =
              marca.bloque !== null && (marca.bloque === cursor || seleccionado(marca.bloque))}
            {#each marca.cajas as caja, indice (indice)}
              <span
                class="nb-marca-item"
                data-nb-marca={marca.clase}
                data-nb-marca-id={marca.id}
                data-nb-marca-cita={indice === 0 ? marca.cita : undefined}
                data-en-foco={enFoco}
                style:left="{caja.left}px"
                style:top="{caja.top}px"
                style:width="{caja.width}px"
                style:height="{caja.height}px"
              ></span>
            {/each}
          {/each}
        </div>
      {/if}

      {#if capa?.status === 'missing'}
        <!--
          Un escaneado sin OCR **se declara**. Presentar un documento
          silenciosamente ilegible para una tecnología de asistencia es peor que
          decir que no se puede leer: quien lo necesita pierde el tiempo
          intentándolo.
        -->
        <p class="nb-sin-texto">{etiquetas.sinCapaDeTexto}</p>
      {:else if capa}
        <div class="nb-capa">
          {#each capa.spans as span, indice (indice)}
            <span
              class="nb-fragmento"
              data-nb-bloque={indice}
              data-nb-seleccionado={seleccionado(indice)}
              data-nb-narrado={indice === narradoEfectivo}
              style:left="{span.x * 100}%"
              style:top="{span.y * 100}%"
              style:width="{span.width * 100}%"
              style:height="{span.height * 100}%">{span.text}</span
            >
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  /*
   * El modo de lectura manda sobre la superficie, no sobre la aplicación: son
   * los dos planos de personalización de ADR-012, y por eso el fondo del texto
   * sale del modo y el de la carcasa, del tema.
   */
  .nb-superficie {
    height: 100%;
    overflow: auto;
    background: var(--nb-libro-fondo, var(--m3c-surface));
    color: var(--nb-libro-texto, var(--m3c-on-surface));
    container-type: size;
    /*
     * La emulación de brillo y temperatura es esta capa y nada más. No toca el
     * panel: ADR-012, regla 6, obliga a decirlo así en la interfaz.
     */
    filter: var(--nb-libro-filtro, none);
  }

  /*
   * Las familias del kit accesible. No se distribuye ninguna fuente: se piden
   * por nombre y, si no están instaladas, se cae a la genérica. Distribuirlas
   * traería licencias que revisar y megabytes al instalador para algo que casi
   * siempre ya está en el sistema.
   */
  .nb-superficie[data-kit-familia='dislexia'] .nb-texto {
    font-family: 'OpenDyslexic', 'Comic Neue', Verdana, sans-serif;
    letter-spacing: 0.03em;
    word-spacing: 0.12em;
  }
  .nb-superficie[data-kit-familia='bajaVision'] .nb-texto {
    font-family: 'Atkinson Hyperlegible', 'Verdana', sans-serif;
    letter-spacing: 0.02em;
  }

  /*
   * Foco de línea: se atenúa todo menos el bloque en el que se está. Con
   * `opacity` y no ocultando nada, porque el texto tiene que seguir estando ahí
   * para el lector de pantalla y para la búsqueda del navegador.
   */
  /*
   * La regla de lectura: una banda que acompaña al párrafo en curso, de lado a
   * lado de la columna. Era el otro interruptor del kit accesible que
   * no hacía nada: `regla` se guardaba en la composición y no la leía nadie.
   *
   * Va detrás del texto y sin recuadro, porque no es una selección: es un apoyo
   * para no perder el renglón. El borde inferior es la regla propiamente dicha;
   * el fondo, muy tenue, es lo que la hace seguible con el rabillo del ojo.
   */
  .nb-superficie[data-regla='true'] .nb-texto :global([data-nb-cursor='true']) {
    background: color-mix(in oklab, var(--nb-libro-texto, var(--m3c-on-surface)) 5%, transparent);
    box-shadow: 0 2px 0
      color-mix(in oklab, var(--nb-libro-texto, var(--m3c-on-surface)) 35%, transparent);
  }

  /*
   * El marcador y el separador: una cinta en el margen del bloque.
   *
   * # Por qué en el margen y no sobre el texto
   *
   * Porque no son lo mismo que un resaltado. Un resaltado dice "estas palabras";
   * una cinta dice "esta página". En papel se distinguen por dónde están —el
   * marcador asoma por el canto y el subrayado vive en el renglón— y aquí se
   * distinguen igual. Además, un fondo sobre el texto para señalar un punto
   * teñiría un párrafo entero por un sitio que ocupa un carácter.
   *
   * # Por qué no se mueve nada de sitio
   *
   * La cinta se pinta con `position: absolute` dentro del bloque y **fuera** de
   * su caja, en el margen que la columna de lectura ya deja libre. Con un borde
   * o un relleno, poner un marcador movería el texto del párrafo, y esa clase de
   * salto es lo que hace perder el renglón a quien está leyendo.
   *
   * Es un adorno visual y no la vía accesible: quien no lo ve tiene los
   * marcadores en el panel de anotaciones, en orden de lectura y con salto. Por
   * eso no lleva texto alternativo, que sería ruido en cada párrafo marcado.
   */
  .nb-texto :global([data-nb-marca-punto]) {
    position: relative;
  }

  .nb-texto :global([data-nb-marca-punto]::before) {
    content: '';
    position: absolute;
    inset-block: 0.15em auto;
    inset-inline-start: -0.9em;
    width: 4px;
    height: 1.4em;
    border-radius: var(--nb-radius-xs);
    background: var(--m3c-primary);
  }

  /*
   * El separador es más tenue que el marcador, y esa diferencia dice cuál es
   * cuál sin depender del color: uno lo pusiste tú y el otro lo pone la
   * aplicación por ti.
   */
  .nb-texto :global([data-nb-marca-punto='separador']::before) {
    opacity: 0.45;
    width: 2px;
  }

  /* La negrita de la lectura biónica hereda el color: nunca pinta el suyo. */
  .nb-texto :global(b[data-nb-bionica]) {
    font-weight: 700;
  }

  .nb-superficie[data-foco-de-linea='true'] .nb-texto :global([data-nb-bloque]) {
    opacity: 0.35;
    transition: opacity var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-superficie[data-foco-de-linea='true'] .nb-texto :global([data-nb-bloque]:focus-visible),
  .nb-superficie[data-foco-de-linea='true'] .nb-texto :global([data-nb-seleccionado='true']) {
    opacity: 1;
  }

  /*
   * Las marcas del lector (resaltados, notas y subrayados), pintadas en una
   * capa propia detrás del texto sin registrar nada en `CSS.highlights`.
   *
   * # Por qué no `CSS.highlights`
   *
   * Con rangos en `CSS.highlights`, cualquier cambio de texto o inserción de un
   * nodo en cualquier parte del documento cuesta en WebKitGTK en proporción al
   * número de rangos y al tamaño del capítulo (455 ms con 20 marcas en 1.600
   * párrafos frente a 20 ms sin ninguna; performance-budgets.md 4.23).
   *
   * # Por qué el color se mezcla con el fondo del libro y va detrás de los glifos
   *
   * Un velo translúcido sobre el texto baja el contraste de lo que hay debajo.
   * Aquí el color es opaco y sale de mezclar el acento del tema con el fondo del
   * propio libro, detrás de los glifos (`z-index: -1` dentro de `.nb-texto` con
   * `isolation: isolate`), así que en modo noche la marca es oscura y en sepia
   * es cálida, y la tinta sigue contrastando contra ella igual que antes.
   *
   * # Por qué la nota y el subrayado se distinguen por el trazo
   *
   * El subrayado punteado de la nota y el continuo sin fondo del subrayado los
   * diferencian del resaltado sin depender del color.
   */
  .nb-capa-marcas {
    position: absolute;
    top: 0;
    left: 0;
    width: 0;
    height: 0;
    overflow: visible;
    pointer-events: none;
    z-index: -1;
  }

  .nb-pagina .nb-capa-marcas {
    z-index: 1;
  }

  .nb-marca-item {
    position: absolute;
    pointer-events: none;
    border-radius: 2px;
  }

  /*
   * Un 45 % y no un 30 %: sobre el negro del modo Nocturno, el 30 % era un
   * marrón que casi no se distinguía del fondo.
   */
  .nb-marca-item[data-nb-marca='highlight'] {
    background-color: color-mix(
      in oklab,
      var(--nb-accent) 45%,
      var(--nb-libro-fondo, var(--nb-surface))
    );
  }

  .nb-pagina .nb-marca-item[data-nb-marca='highlight'] {
    background-color: color-mix(in oklab, var(--nb-accent) 30%, transparent);
  }

  .nb-marca-item[data-nb-marca='note'] {
    background-color: color-mix(
      in oklab,
      var(--nb-accent) 16%,
      var(--nb-libro-fondo, var(--nb-surface))
    );
    border-bottom: 2px dotted var(--nb-accent);
  }

  .nb-pagina .nb-marca-item[data-nb-marca='note'] {
    background-color: color-mix(in oklab, var(--nb-accent) 16%, transparent);
  }

  .nb-marca-item[data-nb-marca='underline'] {
    border-bottom: 2px solid var(--nb-accent);
    z-index: 1;
  }

  .nb-superficie[data-foco-de-linea='true'] .nb-marca-item {
    opacity: 0.35;
    transition: opacity var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-superficie[data-foco-de-linea='true'] .nb-marca-item[data-en-foco='true'] {
    opacity: 1;
  }

  .nb-texto {
    /*
     * Contexto de posición y de apilamiento para la capa de marcas (`z-index: -1`,
     * entre el fondo de `.nb-superficie` y los glifos del texto) y la capa de tinta.
     */
    position: relative;
    isolation: isolate;
    max-width: var(--nb-libro-ancho, 65ch);
    margin: 0 auto;
    padding: var(--nb-libro-margen, var(--nb-space-6)) var(--nb-space-4);
    font-family: var(--nb-libro-familia, var(--nb-font-lectura));
    font-size: var(--nb-libro-cuerpo, 1.125rem);
    line-height: var(--nb-libro-interlineado, 1.6);
    color: inherit;
  }

  /*
   * La tinta del trazo libre.
   *
   * `pointer-events: none` no es opcional: sin él, la tinta se pondría entre el
   * ratón y el texto y no se podría seleccionar lo que hay debajo, que es la
   * operación principal de la superficie.
   *
   * El trazo no se escala con el bloque aunque el SVG sí: `vector-effect` lo
   * mantiene del mismo grosor. Sin eso, un bloque ancho y bajo daría un trazo
   * aplastado, grueso en vertical y fino en horizontal, que no se parece a la
   * tinta de nada.
   */
  .nb-tinta {
    position: absolute;
    overflow: visible;
    pointer-events: none;
  }

  /*
   * Con el lápiz encendido, el texto deja de seleccionarse y el gesto es dibujar.
   *
   * `touch-action: none` impide que un arrastre con el dedo desplace la página en
   * lugar de dibujar; sin él, en una pantalla táctil el lápiz no serviría de nada.
   * Y el cursor lo dice: un modo que no se ve en el puntero es un modo invisible.
   */
  .nb-superficie[data-dibujando='true'] {
    cursor: crosshair;
    touch-action: none;
  }

  .nb-superficie[data-dibujando='true'] .nb-texto {
    -webkit-user-select: none;
    user-select: none;
  }

  /*
   * El trazo en curso se pinta más tenue que los guardados: dice "esto todavía no
   * está" sin cambiar de color.
   */
  .nb-tinta-en-curso path {
    opacity: 0.55;
  }

  .nb-tinta path {
    fill: none;
    stroke: var(--m3c-primary);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
    opacity: 0.85;
  }

  /* --- Paginación Real (Columnas CSS) --- */
  .nb-superficie[data-paginado='true'] {
    overflow-x: auto;
    overflow-y: hidden;
    /* Ocultamos la barra de scroll horizontal para mayor inmersión */
    scrollbar-width: none;
  }
  .nb-superficie[data-paginado='true']::-webkit-scrollbar {
    display: none;
  }
  /*
   * Cada columna mide exactamente el ancho de la superficie y no hay hueco entre
   * columnas: así el desplazamiento total es un múltiplo exacto de la página. Con
   * hueco y relleno, la última columna no llevaba su hueco detrás, el tope del
   * desplazamiento quedaba 16 px antes del borde de página y la última página
   * salía corrida. El margen de lectura y el tope de ancho pasan a cada
   * bloque, centrado dentro de su columna.
   */
  .nb-superficie[data-paginado='true'] .nb-texto {
    max-width: none;
    height: 100%;
    box-sizing: border-box;
    column-width: 100cqw;
    column-gap: 0;
    column-fill: auto;
    padding: var(--nb-libro-margen, var(--nb-space-6)) 0;
  }
  .nb-superficie[data-paginado='true'] .nb-texto > :global(*) {
    box-sizing: border-box;
    max-width: var(--nb-libro-ancho, 65ch);
    margin-inline: auto;
    padding-inline: var(--nb-space-4);
  }

  /*
   * El contenido del libro lo inyecta Rust, así que estos hijos no llevan la
   * clase del componente y hay que alcanzarlos con `:global`. Va anidado dentro
   * de `.nb-texto` y no suelto: suelto salía del componente y pintaba cualquier
   * imagen de la aplicación.
   */
  .nb-texto :global(img),
  .nb-texto :global(svg),
  .nb-texto :global(video) {
    max-width: 100%;
    height: auto;
    object-fit: contain;
    display: block;
    margin: var(--nb-space-4) auto;
    border-radius: var(--nb-radius-xs);
  }
  .nb-superficie[data-alineacion='izquierda'] .nb-texto {
    text-align: left;
  }

  .nb-superficie[data-alineacion='centro'] .nb-texto {
    text-align: center;
  }

  .nb-superficie[data-alineacion='derecha'] .nb-texto {
    text-align: right;
  }

  .nb-superficie[data-alineacion='justificado'] .nb-texto {
    text-align: justify;
    /* Sin partición, justificar abre ríos de espacio; con ella, no. */
    hyphens: auto;
  }

  /*
   * El bloque enfocado se distingue por su fondo, no solo por el contorno del
   * foco: el contorno lo pinta app.css y aquí hace falta además saber cuál es
   * el trozo que se va a anotar, que puede ser más de un bloque.
   *
   * **El fondo sale del plano del libro y se mezcla con transparente.**
   * Sobre `.nb-superficie` produce el mismo tono del libro que mezclándolo con
   * `--nb-libro-fondo`, pero sin tapar los resaltados y notas pintados detrás
   * en `.nb-capa-marcas`.
   */
  .nb-texto :global([data-nb-bloque]:focus-visible) {
    background: color-mix(in srgb, var(--nb-libro-texto, var(--m3c-on-surface)) 8%, transparent);
  }

  /*
   * Lo que la voz está diciendo se marca **distinto** de lo seleccionado: son
   * dos cosas a la vez y confundirlas haría creer que se va a anotar lo que
   * suena. Un subrayado grueso y no un fondo, para que se distinga incluso
   * cuando el bloque está además seleccionado.
   */
  .nb-texto :global([data-nb-narrado='true']),
  .nb-fragmento[data-nb-narrado='true'] {
    box-shadow: inset 0 -0.2em 0 var(--nb-accent-soft);
  }

  .nb-texto :global([data-nb-seleccionado='true']) {
    background: color-mix(in srgb, var(--nb-libro-texto, var(--m3c-on-surface)) 14%, transparent);
  }

  .nb-pagina {
    position: relative;
    max-width: 100%;
    margin: 0 auto;
  }

  /* Ampliada, la página crece y la superficie la desplaza en los dos ejes. */
  .nb-pagina[style*='--nb-zoom'] {
    width: calc(100% * var(--nb-zoom, 1));
    max-width: none;
  }

  .nb-pagina img {
    display: block;
    width: 100%;
    height: auto;
  }

  /*
   * La capa va encima de la imagen y es transparente: lo que se ve es la página
   * y lo que se lee es el texto. `color: transparent` y no `opacity: 0` porque
   * con opacidad cero algunos navegadores dejan de exponerla al lector de
   * pantalla, que es justo para lo que existe.
   */
  .nb-capa {
    position: absolute;
    inset: 0;
  }

  .nb-fragmento {
    position: absolute;
    overflow: hidden;
    color: transparent;
    white-space: pre;
    line-height: 1;
  }

  .nb-fragmento[data-nb-seleccionado='true'] {
    background: var(--nb-accent-soft);
  }

  .nb-sin-texto {
    position: relative;
    margin: 0;
    padding: var(--nb-space-4);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    background: var(--m3c-surface-container-high);
  }

  /*
   * Animación de pase de página.
   *
   * Transporta dirección y continuidad: la página entra deslizándose suavemente
   * desde el lateral correspondiente. Solo mueve `transform` y `opacity`
   * (aceleración GPU en el compositor, sin recalcular disposición).
   * Con movimiento reducido desaparece por completo (ADR-012, regla 7).
   */
  @keyframes nb-pase-siguiente {
    from {
      opacity: 0.2;
      transform: translateX(32px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  @keyframes nb-pase-anterior {
    from {
      opacity: 0.2;
      transform: translateX(-32px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  .nb-pase-siguiente {
    animation: nb-pase-siguiente var(--nb-motion-base) var(--nb-easing-estandar);
  }

  .nb-pase-anterior {
    animation: nb-pase-anterior var(--nb-motion-base) var(--nb-easing-estandar);
  }

  @media (prefers-reduced-motion: reduce) {
    .nb-pase-siguiente,
    .nb-pase-anterior {
      animation: none;
    }
  }
</style>
