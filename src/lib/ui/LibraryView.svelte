<!--
  @component
  La vista de biblioteca: cuadrícula, mosaico o lista, **paginada**.

  Contrato: docs/architecture/ui-components.md.

  Tres cosas que este componente **no** hace, y que son la razón de que quepa
  en el presupuesto:

  1. **No ordena ni filtra.** Eso lo hace SQL, donde están los índices. Con
     152.080 obras, traerlas para ordenarlas aquí no cabe en memoria ni en
     tiempo. La consecuencia importante es que **el orden es del catálogo
     entero**, no de la página: la primera página del orden por título empieza
     por la A de las ciento cincuenta mil, no por la A de las cincuenta que se
     estén viendo.
  2. **No decide cuánto pinta.** Lo decide el tamaño de página, que elige quien
     lee entre una lista cerrada. Por eso el presupuesto de nodos se cumple
     **por construcción** y no por una aritmética que haya que demostrar: lo
     que se pinta es lo que se pidió.
  3. **No mide nada del navegador.** Ni alturas de fila, ni columnas, ni
     desplazamiento. Medir en el hilo de pintado es lo que rompe el presupuesto
     de fotograma, y era además de dónde salía que la lista se rehiciera sola
     mientras se bajaba.

  El presupuesto de nodos se reparte por elemento: cada uno son **nueve** nodos
  —el botón, la portada con su icono, el título, el autor, la marca con el suyo y
  la barra de progreso con su relleno—, y `NODOS_POR_ELEMENTO` lo declara para
  que la prueba pueda multiplicar. Añadir un elemento a la tarjeta obliga a
  revisar ese número: si no, el presupuesto se calcula sobre una cuenta que ya no
  es cierta.
-->
<script lang="ts">
  import { autoresBreves } from '$core/autores';
  import { porcentajeLeido } from '$core/progreso';
  import { untrack } from 'svelte';

  import { rutaDeImagen } from '$core/ipc';
  import { idioma, textos } from '$lib/i18n';
  import type {
    BrowseFilter,
    BrowseItem,
    BrowsePage,
    BrowseQuery,
    BrowseSort,
    LibraryFacets
  } from '$lib/bindings';

  import Button from './Button.svelte';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';
  import Select from './Select.svelte';
  import Spinner from './Spinner.svelte';
  import Cover from './vistas/Cover.svelte';
  import EmptyState from './vistas/EmptyState.svelte';
  import {
    acotarPagina,
    numerosVisibles,
    paginaEscrita,
    paginas,
    peticionDe,
    TAMANO_POR_DEFECTO,
    TAMANOS_DE_PAGINA
  } from './paginacion';

  /** Criterios de orden, en el orden en que se ofrecen. */
  const ORDENES = [
    ['addedAt', 'porFecha'],
    ['title', 'porTitulo'],
    ['author', 'porAutor'],
    ['series', 'porSerie'],
    ['progress', 'porProgreso'],
    ['lastRead', 'porLectura'],
    ['published', 'porPublicacion']
  ] as const;

  /**
   * En qué sentido empieza cada criterio al elegirlo.
   *
   * Antes el sentido no se podía elegir y valía para todos el de
   * la fecha, de lo más nuevo a lo más viejo: "por título" empezaba por la Z.
   * Cada criterio tiene su sentido natural —las letras de la A a la Z, lo
   * reciente y lo avanzado primero— y desde ahí se puede dar la vuelta.
   */
  const SENTIDO_NATURAL: Record<BrowseSort, boolean> = {
    addedAt: false,
    title: true,
    author: true,
    series: true,
    progress: false,
    lastRead: false,
    published: false
  };

  /**
   * Filtros. Los duplicados y lo no disponible **se pueden mirar**: son dos de
   * las cuatro quejas que la investigación recogió sobre la herramienta
   * dominante, y el runtime ya las resolvía sin que se vieran.
   */
  const FILTROS = [
    ['all', 'todos'],
    ['inProgress', 'empezados'],
    ['audiobook', 'audiolibros'],
    ['duplicates', 'duplicados'],
    ['unavailable', 'noDisponibles']
  ] as const;

  /**
   * Modos de la vista, con su icono y su clave de texto.
   *
   * Tres, y cada uno resuelve una pregunta distinta: la cuadrícula es para
   * elegir mirando —portada grande y ficha—, el mosaico para reconocer de un
   * vistazo cuando ya sabes lo que buscas, y la lista para comparar por datos
   * en columnas alineadas.
   */
  const MODOS = [
    ['cuadricula', 'cuadricula', 'cuadricula'],
    ['mosaico', 'mosaico', 'mosaico'],
    ['lista', 'lista', 'lista']
  ] as const;

  type Modo = (typeof MODOS)[number][0];

  interface Props {
    /** Pide una ventana al runtime. Se inyecta para poder probar la vista sin puente. */
    cargar: (query: BrowseQuery) => Promise<BrowsePage>;
    consulta?: BrowseQuery;
    /**
     * Señal para volver a pedir la página actual.
     *
     * Cambiar este número recarga. Existe porque la biblioteca la puede cambiar
     * algo que ocurre **fuera** de esta vista —completar una ficha desde un
     * catálogo, por ejemplo—, y sin esto la rejilla seguiría enseñando el título
     * viejo hasta que el usuario tocara un filtro.
     */
    recarga?: number;
    /**
     * Cuántos elementos por página, y en qué forma se ven.
     *
     * Entran como valor y se avisa al cambiarlos, en lugar de enlazarse: son
     * **preferencias**, no estado de la pantalla, y quien las guarda es la
     * aplicación. Una preferencia que no sobrevive a cerrar la aplicación no es
     * una preferencia.
     */
    porPagina?: number;
    modo?: Modo;
    /**
     * En qué página se está, empezando por la uno.
     *
     * Vive **fuera** de esta vista, y no por gusto: cambiar de destino desmonta
     * el componente, así que un estado interno se pierde. Estar en la página
     * 1.500 de ciento cincuenta mil obras, ir a Estudio y volver al principio no
     * es un detalle, es perder el sitio en el que estabas. Es la mitad de
     * sesión: lo que sobrevive a cambiar de vista lo guarda la
     * carcasa, no la vista.
     */
    pagina?: number;
    alCambiarPagina?: (valor: number) => void;
    /**
     * Las extensiones que revisan metadatos por lotes.
     *
     * Vacío mientras no haya ninguna instalada: revisar por lotes es una
     * contribución, no una función del núcleo.
     */
    revisoresDeMetadatos?: readonly { id: string; name: string }[];
    alRevisarMetadatos?: (extension: string, ediciones: string[]) => void;
    alCambiarModo?: (valor: Modo) => void;
    /**
     * Cuántas obras hay detrás de cada filtro fijo.
     *
     * Un filtro sin obras **no se ofrece**: con la biblioteca sana, "Duplicados"
     * y "No disponibles" son cero, y una ficha que al pulsarla vacía la vista
     * miente sobre lo que hay. Mientras no se sepa -nulo- se ofrecen todos, que
     * es lo que había antes y no esconde nada que exista.
     */
    facetas?: LibraryFacets | null;
    /**
     * Se avisa al cambiar el orden, para guardarlo. El orden es una decisión
     * que se toma una vez, igual que la forma y el tamaño de página.
     */
    alCambiarOrden?: (criterio: BrowseSort, ascendente: boolean) => void;
    /**
     * Nombre de la estantería abierta, si hay alguna.
     *
     * El botón que las abre **no está aquí**: vive en la barra superior, con
     * buscar e importar, porque es de esa clase —abre un panel momentáneo— y no
     * de la de los controles que cambian lo que se ve en esta misma vista. Lo
     * que sí está aquí es qué estantería está puesta, que es un dato de la
     * vista.
     */
    estanteriaActiva?: string | null;
    alQuitarEstanteria?: () => void;
    alAbrir?: (item: BrowseItem) => void;
    /** Se avisa al elegir una obra, para que el panel de detalle la muestre. */
    alSeleccionar?: (item: BrowseItem) => void;
    /**
     * Se avisa cuando una recarga trae otra vez la obra elegida, con lo que
     * tenga ahora. Sin esto, corregir su título dejaba la ficha con el
     * de antes: la rejilla se releía y la ficha era una copia.
     */
    alActualizar?: (item: BrowseItem) => void;
    /** Textos. Se traducen. */
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    cargar,
    consulta = $bindable({
      sort: 'addedAt',
      ascending: false,
      filter: { kind: 'all' },
      offset: 0,
      limit: TAMANO_POR_DEFECTO
    }),
    recarga = 0,
    porPagina = TAMANO_POR_DEFECTO,
    modo = 'cuadricula' as Modo,
    pagina = $bindable(1),
    alCambiarPagina,
    alCambiarModo,
    facetas = null,
    alCambiarOrden,
    revisoresDeMetadatos = [],
    alRevisarMetadatos,
    estanteriaActiva = null,
    alQuitarEstanteria,
    alAbrir,
    alSeleccionar,
    alActualizar,
    etiquetas
  }: Props = $props();

  /** Un texto que falte se queda en su clave, que es visible y se corrige. */
  // El diccionario del idioma en uso, con las anulaciones de quien monta el
  // componente encima. `$derived` y no una constante: cambiar de idioma tiene
  // que repintar, y una constante capturaría el diccionario de un solo momento.
  const t = $derived({ ...textos().biblioteca, ...etiquetas } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  /*
   * Las cifras se agrupan **en el idioma de la interfaz**, no en el del
   * navegador. `toLocaleString()` sin argumento usa el del navegador, que en un
   * escritorio en inglés escribiría `152,080` dentro de una aplicación en
   * español. Con ciento cincuenta mil obras la cifra es lo que da la escala de
   * la biblioteca, y una coma donde va un punto la hace ilegible.
   */
  const cifra = $derived(new Intl.NumberFormat(idioma()));

  let contenedor: HTMLDivElement | undefined = $state();
  let seleccionada = $state<string | null>(null);

  let total = $state(0);
  let cargando = $state(true);
  let fallo = $state(false);
  let items = $state<BrowseItem[]>([]);

  /*
   * Anclas alfabéticas. Dependen del tamaño de la biblioteca: aparecen cuando la
   * lista ocupa al menos `PAGINAS_PARA_ANCLAS` páginas con el tamaño elegido, y
   * solo con los órdenes alfabéticos. Todo lo demás vive en su componente, que se
   * carga aparte: el arranque va al límite de su presupuesto.
   */
  const PAGINAS_PARA_ANCLAS = 4;
  const conAnclas = $derived(
    (consulta.sort === 'title' || consulta.sort === 'author') &&
      total >= porPagina * PAGINAS_PARA_ANCLAS
  );
  const totalDePaginas = $derived(paginas(total, porPagina));

  /**
   * Cuántas casillas de número caben en el pie, medido.
   *
   * Es una medida del pie y no de la rejilla: se observa su tamaño, que solo
   * cambia al cambiar la ventana, y no se lee nada en el hilo de pintado mientras
   * se recorre la biblioteca. Siete de partida, que es lo que se pintaba antes
   * de medir y lo que cabe en la ventana más estrecha que se admite.
   */
  let casillas = $state(7);
  let hueco: HTMLDivElement | undefined = $state();
  /** Lo que ocupa una casilla con su separación: 2,25 rem y 2 px, a 16 px el rem. */
  const ANCHO_DE_CASILLA = 38;

  $effect(() => {
    if (!hueco || typeof ResizeObserver === 'undefined') return;
    const observador = new ResizeObserver(([entrada]) => {
      if (!entrada) return;
      // Las dos flechas ocupan dos casillas.
      const caben = Math.floor(entrada.contentRect.width / ANCHO_DE_CASILLA) - 2;
      // Al fotograma siguiente: ver AppShell.
      requestAnimationFrame(() => (casillas = Math.max(5, Math.min(21, caben))));
    });
    observador.observe(hueco);
    return () => observador.disconnect();
  });

  const numeros = $derived(numerosVisibles(pagina, totalDePaginas, casillas));

  /** Lo escrito en "ir a la página". Solo cifras, y nunca fuera de la biblioteca. */
  let paginaPedida = $state('');

  /**
   * Qué se cuenta. Con "Duplicados" dice qué se considera duplicado: el
   * nombre del filtro solo no lo dice, y antes significaba otra
   * cosa -el mismo archivo dos veces-.
   */
  const formatoDeFecha = $derived(
    new Intl.DateTimeFormat(idioma(), { day: 'numeric', month: 'short', year: 'numeric' })
  );
  function fechaCorta(iso: string): string {
    const fecha = new Date(iso);
    return Number.isNaN(fecha.getTime()) ? iso : formatoDeFecha.format(fecha);
  }

  const queCuenta = $derived(
    // "1 documentos" no es español: con uno, en singular.
    consulta.filter.kind === 'duplicates'
      ? `${texto(total === 1 ? 'unDocumento' : 'documentos')} ${texto('duplicadosCriterio')}`
      : texto(total === 1 ? 'unDocumento' : 'documentos')
  );

  /**
   * Los filtros que se ofrecen: todos los que llevan a alguna obra, y el que
   * esté puesto aunque se haya quedado sin ninguna, para poder quitarlo.
   */
  const filtrosOfrecidos = $derived(
    FILTROS.filter(([valor]) => {
      if (valor === 'all' || consulta.filter.kind === valor || !facetas) return true;
      const cuenta = {
        inProgress: facetas.inProgress,
        audiobook: facetas.audiobook,
        duplicates: facetas.duplicates,
        unavailable: facetas.unavailable
      }[valor];
      return cuenta > 0;
    })
  );

  /** Columnas de la vista de lista: criterio de orden, si lo hay, y su rótulo. */
  const COLUMNAS: ReadonlyArray<readonly [BrowseSort | null, string]> = [
    ['title', 'porTitulo'],
    ['author', 'porAutor'],
    [null, 'columnaMarca'],
    [null, 'columnaFormato'],
    [null, 'columnaTamano'],
    ['progress', 'porProgreso']
  ];

  const FORMATOS: Record<string, string> = {
    'application/epub+zip': 'EPUB',
    'application/pdf': 'PDF',
    'application/vnd.comicbook+zip': 'CBZ'
  };
  function formatoLegible(tipo: string | null): string {
    if (!tipo) return '';
    return FORMATOS[tipo] ?? (tipo.split('/').pop() ?? tipo).toUpperCase();
  }

  const formatoDeTamano = $derived(
    new Intl.NumberFormat(idioma(), { style: 'unit', unit: 'megabyte', maximumFractionDigits: 1 })
  );
  function tamanoLegible(bytes: number | null): string {
    return typeof bytes === 'number' ? formatoDeTamano.format(bytes / 1_000_000) : '';
  }

  function ordenarPor(criterio: BrowseSort, ascendente: boolean) {
    consulta = { ...consulta, sort: criterio, ascending: ascendente };
    alCambiarOrden?.(criterio, ascendente);
  }

  /* El primero y el último que se están viendo, contando desde uno. */
  const primero = $derived(total === 0 ? 0 : (pagina - 1) * porPagina + 1);
  const ultimo = $derived(Math.min(pagina * porPagina, total));

  /**
   * Un clic elige, dos abren.
   *
   * Es lo que hace cualquier lista de archivos del sistema, y aquí importa más
   * que en una lista cualquiera: elegir cambia la ficha y abrir cambia de
   * pantalla. Con un solo clic para abrir, mirar de qué va un libro obligaba a
   * abrirlo y volver.
   */
  function elegir(item: BrowseItem) {
    seleccionada = item.editionId;
    alSeleccionar?.(item);
  }

  /**
   * La página siguiente, ya pedida y con sus portadas descodificadas.
   *
   * # Por qué esto y no una caché de páginas
   *
   * Porque está medido en qué se va el tiempo, y no es donde parecía. Un cambio
   * de página cuesta unos 250 ms en el motor real, y el reparto es:
   * contestar la consulta, **menos de 4 %** —entre 2,7 y 9,3 ms desde los índices
   *—; rehacer el DOM y descodificar las cincuenta portadas, todo lo
   * demás. Guardar lo que el runtime ya contestó ahorra lo que ya era barato.
   *
   * Lo que sí se puede adelantar es el trabajo caro: pedir la página siguiente y
   * **descodificar sus portadas** mientras alguien mira la actual. Cuando pulse,
   * los mapas de bits ya están hechos.
   *
   * # Qué la invalida, que es la mitad difícil
   *
   * Cambiar de consulta, de tamaño de página o recibir una recarga. Las tres
   * dejan obsoleto lo guardado, y una caché que no sabe cuándo tirar es un
   * defecto de pérdida disfrazado de rendimiento. Por eso guarda **una sola**
   * página y no un historial: cuanto menos se guarda, menos hay que invalidar.
   */
  let adelantada: { pagina: number; datos: BrowsePage } | null = null;

  /**
   * Cuántas portadas se adelantan como mucho.
   *
   * # Por qué no todas las de la página
   *
   * Porque adelantarlas todas convierte el ahorro en un coste, y está medido. Una
   * portada de 400x600 ocupa 960 KB descodificada; con cincuenta vivas en la
   * página actual y otras cincuenta adelantadas son casi cien megas de mapas de
   * bits, y el motor empieza a desalojar. Desaloja lo que le parece, así que a
   * veces tira lo adelantado —y el trabajo fue en balde— y a veces tira lo que se
   * está mirando. Medido con las cincuenta: cuatro cambios de página de 79 a 169
   * ms y uno de **425**, peor que los 265 de no adelantar nada. La mediana
   * mejoraba y la cola empeoraba, que en algo que se recorre es mal negocio.
   *
   * Veinticuatro es lo que cabe en pantalla en la ventana más grande que se mide
   * (1600x900, cincuenta por página). Las demás llevan `loading="lazy"` y no se
   * cargan hasta que alguien baja, así que adelantarlas es trabajo para algo que
   * quizá nadie mire.
   */
  const PORTADAS_ADELANTADAS = 24;

  /**
   * Descodifica por adelantado las portadas que se van a ver.
   *
   * `decode()` sobre una imagen suelta deja el mapa de bits en la caché del
   * motor, y la que se monte después con la misma dirección lo reutiliza. No se
   * espera el resultado: si falla, la portada se descodificará cuando toque, que
   * es lo que pasa hoy.
   */
  function adelantarPortadas(datos: BrowsePage) {
    for (const item of datos.items.slice(0, PORTADAS_ADELANTADAS)) {
      if (!item.coverPath) continue;
      const imagen = new Image();
      imagen.src = rutaDeImagen(item.coverPath);
      void imagen.decode().catch(() => undefined);
    }
  }

  /**
   * Pide la siguiente y la deja lista, cuando el navegador no tenga nada mejor.
   *
   * Va detrás de `requestIdleCallback` porque adelantar trabajo mientras la
   * página actual todavía se está pintando es competir con ella: se ganaría en el
   * cambio siguiente lo que se pierde en el de ahora.
   */
  function adelantarSiguiente(desde: number) {
    const proxima = desde + 1;
    if (proxima > paginas(total, porPagina)) return;
    if (adelantada?.pagina === proxima) return;

    const trabajo = () => {
      void cargar({ ...consulta, ...peticionDe(proxima, porPagina) })
        .then((datos) => {
          adelantada = { pagina: proxima, datos };
          adelantarPortadas(datos);
        })
        .catch(() => undefined);
    };
    if (typeof requestIdleCallback === 'function') requestIdleCallback(trabajo, { timeout: 1000 });
    else setTimeout(trabajo, 200);
  }

  async function pedir(numero: number) {
    fallo = false;

    // Si es la que se adelantó, no hay nada que pedir.
    if (adelantada?.pagina === numero) {
      const datos = adelantada.datos;
      adelantada = null;
      total = datos.total;
      items = datos.items;
      cargando = false;
      adelantarSiguiente(numero);
      return;
    }

    try {
      const resultado = await cargar({ ...consulta, ...peticionDe(numero, porPagina) });
      total = resultado.total;
      items = resultado.items;
      const elegida = items.find((i) => i.editionId === seleccionada);
      if (elegida) alActualizar?.(elegida);

      /*
       * La página se acota **con el total que acaba de llegar**, no antes.
       *
       * Antes no se puede: el total de un filtro no se sabe hasta pedirlo. Si al
       * acotar cambia, se vuelve a pedir una sola vez, y esa segunda petición ya
       * cae dentro del rango porque el total no ha cambiado entre las dos.
       */
      const acotada = acotarPagina(numero, resultado.total, porPagina);
      if (acotada !== numero) {
        pagina = acotada;
        const segunda = await cargar({ ...consulta, ...peticionDe(acotada, porPagina) });
        total = segunda.total;
        items = segunda.items;
      }
      adelantarSiguiente(pagina);
    } catch {
      // Un fallo al leer la biblioteca no puede dejar una pantalla en blanco:
      // se dice, y se ofrece reintentar.
      fallo = true;
    } finally {
      cargando = false;
    }
  }

  /** Va a una página y sube arriba del todo: la nueva empieza por su primero. */
  function irA(numero: number) {
    const destino = acotarPagina(numero, total, porPagina);
    if (destino === pagina) return;
    pagina = destino;
    contenedor?.scrollTo({ top: 0 });
  }

  /**
   * La consulta con la que se pidió la última vez, sin la ventana.
   *
   * Existe para distinguir **cambiar de consulta** de **volver a montarse**, que
   * eran la misma cosa para este efecto: al volver a la biblioteca
   * desde otro destino, el componente se monta de nuevo, el efecto corre, y
   * reiniciaba la página a la uno. Con la página guardada en la carcasa eso se
   * veía como que guardarla no servía de nada.
   *
   * `offset` y `limit` se dejan fuera a propósito: los pone `peticionDe` en cada
   * llamada, así que entran y salen solos y no dicen nada sobre si la consulta
   * cambió.
   */
  let firmaAnterior: string | null = null;

  /** La última página que se pidió, para no pedir dos veces la misma. */
  let ultimaPedida = 0;

  $effect(() => {
    const firma = `${JSON.stringify({ ...consulta, offset: 0, limit: 0 })}|${porPagina}`;
    const cambio = firmaAnterior !== null && firma !== firmaAnterior;
    firmaAnterior = firma;

    if (cambio) {
      // Quedarse en la página 300 al cambiar de filtro deja al usuario en un
      // sitio que no eligió y que puede no existir; y con otro tamaño de página,
      // el mismo número señala a otros elementos.
      pagina = 1;
      // Otra consulta u otro tamaño hacen de lo adelantado otra biblioteca.
      adelantada = null;
    }

    cargando = true;
    // Se apunta antes de pedir para que el efecto de abajo no pida lo mismo otra
    // vez al montarse con una página que no sea la primera.
    ultimaPedida = pagina;
    void pedir(pagina);
  });

  /*
   * Y cambiar de página pide la suya. Va en su propio efecto para que cambiar
   * de filtro no dispare dos peticiones: el de arriba ya pide la primera.
   */
  $effect(() => {
    const numero = pagina;
    if (numero === ultimaPedida) return;
    ultimaPedida = numero;
    void pedir(numero);
  });

  // Recargar sin moverse: se vuelve a pedir **la misma página**. Cambiar una
  // ficha no es motivo para devolver al usuario al principio de una biblioteca
  // de ciento cincuenta mil obras.
  //
  // Solo cuando **cambia**, no al montarse: montarse ya pide su página, y con la
  // cuenta por encima de cero cada entrada a la biblioteca la pedía dos veces
  //.
  let recargaAtendida = untrack(() => recarga);
  $effect(() => {
    const pedida = recarga;
    if (pedida === recargaAtendida) return;
    recargaAtendida = pedida;
    // La biblioteca cambió por fuera: lo adelantado ya no dice la verdad.
    adelantada = null;
    void pedir(pagina);
  });
</script>

<section class="nb-biblioteca" aria-label={texto('region')}>
  <div class="nb-barra" role="toolbar" aria-label={texto('controles')}>
    <div class="nb-fichas">
      {#each filtrosOfrecidos as [valor, clave] (valor)}
        <button
          class="nb-ficha"
          type="button"
          aria-pressed={consulta.filter.kind === valor}
          onclick={() => (consulta = { ...consulta, filter: { kind: valor } as BrowseFilter })}
        >
          {texto(clave)}
        </button>
      {/each}
    </div>

    {#if estanteriaActiva}
      <!--
        Qué estantería está puesta, y cómo se quita. Sin esto, una biblioteca
        acotada se ve igual que una vacía: la cuenta baja y nada dice por qué.
      -->
      <button class="nb-ficha nb-ficha-activa" type="button" onclick={() => alQuitarEstanteria?.()}>
        <Icon nombre="cerrar" tamano={13} />
        {estanteriaActiva}
      </button>
    {/if}

    <!--
      Los controles van juntos y en el mismo orden siempre: cuántas hay, cómo se
      ordenan, cuántas por página y en qué forma. Si no caben al lado de los
      filtros bajan **juntos** a la fila siguiente, en lugar de partirse por donde
      caiga. Antes cualquier botón de más -el de una extensión, por
      ejemplo- empujaba la forma de la vista sola a la segunda fila.
    -->
    <div class="nb-controles">
      <span class="nb-cuenta">{cifra.format(total)} {queCuenta}</span>

      <Select
        etiqueta={texto('ordenar')}
        prefijo={texto('ordenar')}
        valor={consulta.sort}
        opciones={ORDENES.map(([valor, clave]) => [valor, texto(clave)] as const)}
        alCambiar={(valor) => ordenarPor(valor as BrowseSort, SENTIDO_NATURAL[valor as BrowseSort])}
        alineacion="derecha"
      />

      <IconButton
        etiqueta={consulta.ascending ? texto('ascendente') : texto('descendente')}
        onclick={() => ordenarPor(consulta.sort, !consulta.ascending)}
      >
        <Icon nombre={consulta.ascending ? 'ordenarAscendente' : 'ordenar'} />
      </IconButton>

      <!--
        Cuántas obras por página va **arriba**, con lo demás que decide qué se ve.

        Estaba en el pie, junto a los números de página, y ahí se leía como parte
        de la navegación cuando es lo contrario: navegar es moverse por lo que hay,
        y esto decide cuánto hay.
      -->
      <Select
        etiqueta={texto('porPagina')}
        prefijo={texto('porPagina')}
        valor={String(porPagina)}
        opciones={TAMANOS_DE_PAGINA.map((n) => [String(n), String(n)] as const)}
        alCambiar={(valor) => alCambiarPagina?.(Number(valor))}
        alineacion="derecha"
      />

      <!--
        Cuadrícula, mosaico y lista son una **elección**, no algo que se despliegue:
        van con `activo`, que es `aria-pressed`. Con `aria-expanded` el lector de
        pantalla anunciaba "contraído" sobre el modo en curso, y en pantalla los
        tres se veían igual porque ninguna regla los distinguía.
      -->
      <div class="nb-modos" role="group" aria-label={texto('vista')}>
        {#each MODOS as [valor, icono, clave] (valor)}
          <button
            class="nb-modo"
            type="button"
            aria-label={texto(clave)}
            aria-pressed={modo === valor}
            onclick={() => alCambiarModo?.(valor)}
          >
            <Icon nombre={icono} />
          </button>
        {/each}
      </div>

      <!--
        Revisar metadatos actúa sobre **lo que se está viendo**, no sobre la
        biblioteca entera, y no escribe nada: lo que la extensión proponga queda
        como propuesta. Va el último y en un botón de icono porque lo aporta una
        extensión: lo del núcleo no cambia de sitio por tenerla instalada.
      -->
      {#each revisoresDeMetadatos as revisor (revisor.id)}
        <IconButton
          etiqueta={`${texto('revisarMetadatos')}: ${revisor.name}`}
          onclick={() =>
            alRevisarMetadatos?.(
              revisor.id,
              items.map((i) => i.editionId)
            )}
        >
          <Icon nombre="extensiones" />
        </IconButton>
      {/each}
    </div>
  </div>

  {#if conAnclas}
    {#await import('./vistas/AnclasAlfabeticas.svelte') then { default: Anclas }}
      <Anclas {consulta} {porPagina} {pagina} etiqueta={texto('anclas')} alIr={irA} />
    {/await}
  {/if}

  <div class="nb-lienzo" bind:this={contenedor}>
    {#if fallo}
      <p class="nb-estado" role="alert">
        {texto('error')}
        <button class="nb-modo nb-reintentar" type="button" onclick={() => pedir(pagina)}>
          {texto('reintentar')}
        </button>
      </p>
    {:else if cargando}
      <div class="nb-estado-cargando">
        <Spinner etiqueta={texto('cargando')} tamano="3rem" />
      </div>
    {:else if total === 0}
      <EmptyState
        icono={consulta.filter.kind === 'all' ? 'biblioteca' : 'filtrar'}
        mensaje={consulta.filter.kind === 'all' ? texto('vacio') : texto('sinResultados')}
      />
    {:else}
      {#if modo === 'lista'}
        <!-- Cabecera de columnas. Las que tienen orden lo cambian al pulsarlas,
         y la segunda pulsación invierte el sentido, como en cualquier tabla. -->
        <div class="nb-cabecera-lista">
          <span></span>
          {#each COLUMNAS as [criterio, clave] (clave)}
            {#if criterio}
              <button
                class="nb-columna"
                type="button"
                aria-pressed={consulta.sort === criterio}
                data-sentido={consulta.sort === criterio
                  ? consulta.ascending
                    ? 'ascendente'
                    : 'descendente'
                  : undefined}
                onclick={() =>
                  ordenarPor(
                    criterio,
                    consulta.sort === criterio ? !consulta.ascending : criterio !== 'progress'
                  )}
              >
                {texto(clave)}
                {#if consulta.sort === criterio}
                  <Icon nombre={consulta.ascending ? 'arriba' : 'abajo'} tamano={12} />
                {/if}
              </button>
            {:else}
              <span class="nb-columna">{texto(clave)}</span>
            {/if}
          {/each}
        </div>
      {/if}
      <ul class="nb-rejilla" data-modo={modo}>
        {#each items as item (item.editionId)}
          <li class="nb-fila">
            <button
              class="nb-tarjeta"
              type="button"
              aria-current={item.editionId === seleccionada ? 'true' : undefined}
              onfocus={() => elegir(item)}
              onclick={() => elegir(item)}
              ondblclick={() => alAbrir?.(item)}
              onkeydown={(evento) => {
                // Con el teclado, Enter abre: no hay doble pulsación que
                // hacer, y llegar hasta aquí con el tabulador ya es haber
                // elegido. Es lo mismo que hace cualquier lista del sistema.
                if (evento.key === 'Enter') {
                  evento.preventDefault();
                  alAbrir?.(item);
                }
              }}
            >
              <Cover
                titulo={item.title}
                autor={modo === 'lista'
                  ? null
                  : autoresBreves(item.authors, textos().ficha.autoresMas)}
                imagen={item.coverPath ? rutaDeImagen(item.coverPath) : null}
                ancho={24}
                fluida={modo !== 'lista'}
                progreso={modo === 'lista' ? null : item.progression}
              />
              <span class="nb-tarjeta-texto">
                <span class="nb-titulo" title={item.title}>{item.title}</span>
                <span class="nb-autor"
                  >{autoresBreves(item.authors, textos().ficha.autoresMas)}</span
                >
                <span class="nb-marca">
                  {#if item.availability !== 'available'}
                    <Icon nombre="noDisponible" tamano={13} /> {texto('noDisponible')}
                  {:else if item.copies > 1}
                    <Icon nombre="duplicado" tamano={13} /> {item.copies} {texto('copias')}
                  {:else if consulta.filter.kind === 'duplicates'}
                    <!-- Dos tarjetas con el mismo título y autor se distinguen por
                     cuándo entró cada una; el resto -formato, tamaño, ruta- está
                     en su ficha. -->
                    {texto('porFecha')}
                    {fechaCorta(item.addedAt)}
                  {/if}
                </span>
              </span>
              {#if modo === 'lista'}
                <span class="nb-dato">{formatoLegible(item.mediaType)}</span>
                <span class="nb-dato nb-cifra">{tamanoLegible(item.byteSize)}</span>
                <span class="nb-progreso-columna">
                  {#if item.progression !== null}
                    <span class="nb-progreso">
                      <span
                        class="nb-progreso-barra"
                        style:width="{porcentajeLeido(item.progression)}%"
                      ></span>
                    </span>
                    <span class="nb-cifra">{porcentajeLeido(item.progression)} %</span>
                  {/if}
                </span>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <!--
    El pie de paginación: dónde estás y cómo moverte.

    Solo cuando hay más de una página. Con una sola no hay a dónde ir, y una
    barra de navegación con un único destino ocupa sitio para no ofrecer nada. El
    tamaño de página ya no vive aquí, así que esconder el pie no esconde ningún
    ajuste.
  -->
  {#if !cargando && !fallo && totalDePaginas > 1}
    <nav class="nb-paginacion" aria-label={texto('paginacion')}>
      <span class="nb-rango">
        {cifra.format(primero)}–{cifra.format(ultimo)}
        {texto('deTotal')}
        {cifra.format(total)}
      </span>

      <div class="nb-paginas" bind:this={hueco}>
        <IconButton
          etiqueta={texto('anterior')}
          deshabilitado={pagina <= 1}
          onclick={() => irA(pagina - 1)}
        >
          <Icon nombre="izquierda" />
        </IconButton>

        {#each numeros as numero, i (numero === null ? `hueco-${i}` : numero)}
          {#if numero === null}
            <!--
              Un hueco, no un botón. Se puede pulsar y no lleva a un sitio
              concreto, así que pintarlo como control sería ofrecer un destino
              que no existe. `aria-hidden` porque leerlo en voz alta no dice
              nada: la posición ya la da el botón que está marcado.
            -->
            <span class="nb-elipsis" aria-hidden="true">…</span>
          {:else}
            <button
              class="nb-pagina"
              type="button"
              aria-current={numero === pagina ? 'page' : undefined}
              onclick={() => irA(numero)}
            >
              {cifra.format(numero)}
            </button>
          {/if}
        {/each}

        <IconButton
          etiqueta={texto('siguiente')}
          deshabilitado={pagina >= totalDePaginas}
          onclick={() => irA(pagina + 1)}
        >
          <Icon nombre="derecha" />
        </IconButton>
      </div>

      <!--
        Ir a una página escribiéndola. Con mil quinientas páginas, llegar a la
        setecientos a base de flechas no es una forma de llegar.

        El campo **no admite nada que no lleve a una página**: solo cifras, y lo
        que pase de la última se queda en la última mientras se escribe. Así no
        hay error que explicar ni número que falle al pulsar Intro. Al ir, se
        vacía, y la página queda marcada en los números.
      -->
      <input
        class="nb-ir-a"
        type="text"
        inputmode="numeric"
        autocomplete="off"
        aria-label={texto('irAPagina')}
        placeholder={texto('irAPagina')}
        value={paginaPedida}
        oninput={(evento) => {
          paginaPedida = paginaEscrita(evento.currentTarget.value, totalDePaginas);
          // Se reescribe el campo aunque el estado no cambie: con "12" puesto y
          // una letra tecleada, el estado sigue siendo "12" y Svelte no
          // repintaría, así que la letra se quedaría a la vista.
          evento.currentTarget.value = paginaPedida;
        }}
        onkeydown={(evento) => {
          if (evento.key !== 'Enter' || paginaPedida === '') return;
          evento.preventDefault();
          irA(Number(paginaPedida));
          paginaPedida = '';
        }}
      />
    </nav>
  {/if}
</section>

<style>
  .nb-estado-cargando {
    display: flex;
    justify-content: center;
    align-items: center;
    height: 100%;
    min-height: 200px;
    color: var(--m3c-primary);
  }

  /*
   * El contenedor que se desplaza, y el único.
   *
   * Las tres declaraciones van juntas y ninguna sobra: `flex: 1 1 auto` con
   * `min-height: 0` es lo que hace que un hijo flexible pueda ser más bajo que
   * su contenido, que es la condición para que `overflow` signifique algo. Sin
   * ellas, la página crece hacia abajo y se lleva por delante el pie de
   * paginación, que es justo el control con el que se sale de ahí.
   */
  .nb-lienzo {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .nb-cuenta {
    font-size: var(--nb-text-sm);
    font-variant-numeric: tabular-nums;
    color: var(--m3c-on-surface-variant);
  }

  .nb-progreso {
    display: block;
    height: 3px;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-highest);
  }

  .nb-progreso-barra {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--m3c-primary);
  }

  .nb-estado {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--nb-space-4);
    height: 100%;
    min-height: 200px;
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }
  .nb-biblioteca {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }

  /*
   * La barra envuelve en lugar de desplazarse. La decisión de producto es
   * explícita: nada se desplaza en horizontal, por estrecha que sea la ventana.
   * Un control que se sale de la ventana no está oculto, está perdido: nadie
   * arrastra una barra que no sabe que existe.
   */
  .nb-barra {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-2) var(--nb-space-3);
    padding: var(--nb-space-3) var(--nb-space-4);
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }

  /*
   * El pie de paginación. Mismo peso visual que la barra de arriba y en el
   * borde opuesto: son los dos extremos de la misma vista, y lo que se elige
   * arriba (qué y en qué orden) decide lo que se recorre abajo.
   */
  .nb-paginacion {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-2) var(--nb-space-3);
    padding: var(--nb-space-2) var(--nb-space-4);
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }

  .nb-rango {
    font-variant-numeric: tabular-nums;
  }

  /*
   * Los números ocupan el sitio que hay entre el rango y el campo de ir a una
   * página, y es ese sitio el que se mide: la base es cero para que su ancho lo
   * decida el pie y no los números que ya estaban pintados.
   */
  .nb-paginas {
    display: flex;
    flex: 1 1 0;
    justify-content: flex-end;
    align-items: center;
    gap: 2px;
    min-width: 0;
  }

  .nb-ir-a {
    box-sizing: border-box;
    width: 7.5rem;
    min-height: 2.25rem;
    padding: 0 var(--nb-space-3);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-sm);
    font-variant-numeric: tabular-nums;
  }

  .nb-ir-a::placeholder {
    color: var(--m3c-on-surface-variant);
  }

  .nb-ir-a:focus-visible {
    border-color: var(--m3c-primary);
  }

  .nb-controles {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: var(--nb-space-2) var(--nb-space-3);
    margin-inline-start: auto;
  }

  .nb-pagina {
    min-width: 2.25rem;
    min-height: 2.25rem;
    padding: 0 var(--nb-space-2);
    border: none;
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    font: inherit;
    font-size: var(--nb-text-sm);
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    transition:
      background var(--nb-motion-fast) var(--nb-easing),
      color var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-pagina:hover,
  .nb-pagina:focus-visible {
    color: var(--m3c-on-surface);
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  /*
   * La página en curso se marca con `aria-current="page"`, que es lo que un
   * lector de pantalla anuncia, y el estilo cuelga del mismo atributo: así no
   * hay forma de que lo que se ve y lo que se anuncia discrepen.
   */
  .nb-pagina[aria-current='page'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font-weight: 600;
  }

  /*
   * La estantería puesta se ve como una ficha marcada con una cruz. No es un
   * filtro más de la fila de la izquierda: aquellos son cuatro y fijos, y esta
   * aparece y desaparece con lo que se elija.
   */
  .nb-ficha-activa {
    display: inline-flex;
    align-items: center;
    gap: var(--nb-space-1);
    max-width: 16rem;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    border-color: transparent;
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font-weight: 600;
  }

  .nb-elipsis {
    padding: 0 var(--nb-space-1);
    color: var(--m3c-on-surface-variant);
  }

  /* Reintentar toma el aspecto de los controles de la barra, sin ser uno. */
  .nb-reintentar {
    border-color: color-mix(in srgb, var(--m3c-outline-variant) 60%, transparent);
  }

  .nb-fichas {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
  }

  .nb-modos {
    display: flex;
    gap: 2px;
    padding: var(--nb-space-1);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
  }

  /*
   * Las fichas de filtro son las de Material: con su contorno en reposo y el
   * contenedor de la secundaria al elegirse. Los modos, un grupo segmentado sobre
   * su capa.
   */
  .nb-modo,
  .nb-ficha {
    min-height: 2rem;
    padding: 0 var(--nb-space-3);
    border: 1px solid transparent;
    border-radius: var(--nb-radius-sm);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    font: inherit;
    font-size: var(--nb-text-sm);
    font-weight: 500;
    cursor: pointer;
    transition:
      background var(--nb-motion-fast) var(--nb-easing),
      color var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-ficha {
    border-color: var(--m3c-outline-variant);
  }

  .nb-modo {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 2.5rem;
    padding: 0;
  }

  .nb-modo:hover,
  .nb-ficha:hover,
  .nb-modo:focus-visible,
  .nb-ficha:focus-visible {
    color: var(--m3c-on-surface);
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  :global(.nb-ficha[aria-pressed='true']) {
    border-color: transparent;
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font-weight: 600;
  }

  :global(.nb-modo[aria-pressed='true']) {
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface);
  }

  .nb-rejilla {
    display: grid;
    /*
     * **Las columnas las cuenta el navegador, no nosotros.**
     *
     * Antes se calculaban en JavaScript dividiendo el ancho observado entre un
     * tamaño supuesto de tarjeta. Esa cuenta acierta casi siempre y falla justo
     * donde no se puede comprobar desde aquí: con la barra de desplazamiento
     * apareciendo en el mismo fotograma, con una escala de pantalla que no sea
     * del cien por cien, o con un tema que cambie el hueco entre tarjetas.
     * Cuando falla, sobra una columna y la última fila se sale por la derecha,
     * justo por debajo de la ficha.
     *
     * `auto-fill` no puede equivocarse: pone las que caben en el ancho que hay
     * de verdad, después de la barra y a la escala que sea. La cuenta sigue
     * haciendo falta para la virtualización, y por eso se **lee de vuelta** de
     * la rejilla ya compuesta en lugar de calcularse aparte: dos cuentas del
     * mismo número acaban discrepando, y aquí discrepar significa pedir la
     * página equivocada.
     */
    grid-template-columns: repeat(auto-fill, minmax(var(--card-width, 200px), 1fr));
    gap: var(--nb-space-3);
    margin: 0;
    padding: var(--nb-space-3);
    list-style: none;
  }

  /*
   * En lista hay **una** columna, y hay que decirlo.
   *
   * Antes lo decía `--nb-columnas: 1` puesto en línea. Al pasar las columnas a
   * `auto-fill`, esta regla se quedó sin declararlas y la lista heredó las de la
   * cuadrícula: cinco tarjetas por fila, cada una en modo lista —con su autor de
   * ancho fijo— dentro de una columna que no le llega. El resultado era una
   * hilera de nombres corriendo hacia la derecha, la ventana desplazándose en
   * horizontal y la ficha empujada fuera de la pantalla.
   */
  .nb-lienzo {
    /* Una sola definición para la cabecera y las filas: si divergieran, los
       rótulos dejarían de caer encima de su columna. */
    --nb-columnas-lista: 24px minmax(0, 10fr) minmax(0, 6fr) 40px 52px 72px 100px;
  }

  .nb-cabecera-lista {
    display: grid;
    grid-template-columns: var(--nb-columnas-lista);
    gap: var(--nb-space-2);
    padding: var(--nb-space-1) var(--nb-space-4);
    border-bottom: 1px solid var(--m3c-outline-variant);
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
  }

  .nb-columna {
    display: inline-flex;
    align-items: center;
    gap: var(--nb-space-1);
    min-width: 0;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    text-align: start;
    white-space: nowrap;
    overflow: hidden;
  }
  button.nb-columna {
    cursor: pointer;
  }
  .nb-columna[aria-pressed='true'] {
    color: var(--m3c-on-surface);
    font-weight: 600;
  }
  .nb-columna:last-child {
    justify-content: flex-end;
  }

  .nb-dato {
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
    white-space: nowrap;
  }

  :global(.nb-rejilla[data-modo='lista']) {
    grid-template-columns: minmax(0, 1fr);
    gap: 0;
    padding: 0;
  }

  /*
   * Lo que queda fuera de la vista no se maqueta ni se pinta hasta que se acerca
   *. Entrar en Biblioteca costaba un fotograma de 210 a 230 ms en
   * WebKitGTK, maquetando las cincuenta tarjetas de la página aunque se vieran
   * doce; con esto, de 56 a 74 ms, medido en la aplicación instalada. `auto` en el
   * tamaño intrínseco hace que, una vez pintada, recuerde su alto real y la
   * barra de desplazamiento no salte.
   */
  .nb-fila {
    min-width: 0;
    content-visibility: auto;
    contain-intrinsic-size: auto 18rem;
  }
  :global(.nb-rejilla[data-modo='lista']) .nb-fila {
    contain-intrinsic-size: auto 3rem;
  }

  /*
   * Mosaico: la misma tarjeta sin caja y más pequeña.
   *
   * Es el marcado de la cuadrícula, no otro: un segundo bloque de elementos
   * habría duplicado la tarjeta entera —portada, título, autor, marca,
   * progreso— para cambiarle el tamaño, y dos copias del mismo elemento acaban
   * divergiendo justo en el detalle que una de las dos arregló. Lo que cambia
   * es el ancho de columna y que aquí no hay caja, así que el relieve lo lleva
   * la portada, que es la que sí tiene borde.
   */
  :global(.nb-rejilla[data-modo='mosaico']) {
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: var(--nb-space-4) var(--nb-space-3);
  }

  :global(.nb-rejilla[data-modo='mosaico']) .nb-tarjeta {
    gap: var(--nb-space-2);
    padding: 0;
    border: none;
    background: none;
    box-shadow: none;
  }

  :global(.nb-rejilla[data-modo='mosaico']) .nb-tarjeta:hover,
  :global(.nb-rejilla[data-modo='mosaico']) .nb-tarjeta:focus-visible {
    background: none;
    box-shadow: none;
    transform: none;
  }

  :global(.nb-rejilla[data-modo='mosaico']) .nb-tarjeta[aria-current='true'] {
    background: none;
  }

  :global(.nb-rejilla[data-modo='mosaico']) .nb-tarjeta-texto {
    height: auto;
  }

  /* Lo elegido se marca en la portada, que es lo que aquí tiene contorno. */
  :global(.nb-rejilla[data-modo='mosaico']) .nb-tarjeta[aria-current='true'] :global(.nb-portada) {
    box-shadow: 0 0 0 2px var(--m3c-primary);
  }

  /*
   * La obra no va en una caja: la portada es la que tiene relieve, y alrededor
   * solo hay una capa de estado que aparece al pasar o al elegir. Con cincuenta
   * cajas con borde y sombra, la rejilla era una hoja de cálculo de tarjetas.
   */
  .nb-tarjeta {
    display: flex;
    flex-direction: column;
    width: 100%;
    padding: var(--nb-space-2);
    gap: var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-lg);
    background: transparent;
    color: var(--m3c-on-surface);
    font: inherit;
    text-align: left;
    cursor: pointer;
    position: relative;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  /*
   * Al recibir el ratón **no se escala**, y esto es la segunda mitad del arreglo
   * del parpadeo.
   *
   * Quitar la transición de `box-shadow` se llevó casi todo, y quedó un destello
   * de un solo fotograma —"casi frame perfect", Se describió—. Un fotograma
   * suelto no es coste por fotograma: es algo que ocurre **una vez** por
   * transición, y eso es la promoción a capa de composición. Al empezar a escalar,
   * el motor sube la tarjeta a su propia capa, y la tarjeta lleva dentro la
   * portada ya descodificada: promocionarla obliga a re-rasterizarla. De ahí que
   * se note más cuantas más haya.
   *
   * El arreglo que recomienda todo el mundo es pre-promocionar con `will-change`
   * o `translateZ(0)`. Aquí no vale: `check-ui.mjs` rechaza `will-change` por una
   * medida propia —cincuenta y cuatro capas vivas en reposo y el fotograma
   * mediano en 18 ms sobre 16,7— y `translateZ(0)` es lo mismo con otro nombre,
   * así que usarlo sería evadir la regla, no cumplirla.
   *
   * La salida es no dar motivo para promocionar. La elevación la da la sombra,
   * que es lo que se lee como relieve; es el mismo argumento que ya estaba escrito
   * aquí para quitar el `translateY`, aplicado un paso más allá.
   */
  /*
   * Al recibir el ratón **no se escala ni se levanta**. Levantarla sacaba el área
   * sensible de debajo del puntero y el hover parpadeaba; escalarla promocionaba
   * la tarjeta a su propia capa con la portada dentro, y eso se veía como un
   * destello de un fotograma. Una capa de estado es solo color.
   */
  .nb-tarjeta:hover,
  .nb-tarjeta:focus-visible {
    background: color-mix(in srgb, var(--m3c-on-surface) 6%, transparent);
  }

  .nb-tarjeta:active {
    background: color-mix(in srgb, var(--m3c-on-surface) 10%, transparent);
  }

  :global(.nb-tarjeta[aria-current='true']) {
    background: color-mix(in srgb, var(--m3c-secondary-container) 80%, transparent);
    color: var(--m3c-on-secondary-container);
  }

  .nb-tarjeta :global(.nb-portada) {
    transition:
      transform 0.2s cubic-bezier(0.2, 0, 0, 1),
      box-shadow 0.2s cubic-bezier(0.2, 0, 0, 1);
    /*
     * Sin `will-change`. Aquí no hay una portada: hay las que quepan en la
     * ventana, y anunciar el cambio en todas fija una capa de composición por
     * cada una. Medido sobre el catálogo entero eran cincuenta y cuatro capas
     * vivas en reposo, con el fotograma mediano en 18 ms sobre un presupuesto
     * de 16,7. `box-shadow` además no se compone, así que la mitad del anuncio
     * no podía cumplirse ni queriendo.
     */
  }

  .nb-tarjeta:hover :global(.nb-portada),
  .nb-tarjeta:focus-visible :global(.nb-portada) {
    transform: none;
    box-shadow: var(--m3-elevation-3);
  }

  .nb-tarjeta-texto {
    display: flex;
    flex-direction: column;
    justify-content: flex-start;
    gap: var(--nb-space-1);
    height: 48px;
    container-type: inline-size;
    overflow: hidden;
    width: 100%;
  }

  :global(.nb-rejilla[data-modo='lista']) .nb-tarjeta {
    display: grid;
    grid-template-columns: var(--nb-columnas-lista);
    align-items: center;
    gap: var(--nb-space-2);
    padding: var(--nb-space-2) var(--nb-space-4);
    border-radius: var(--nb-radius-md);
    border: none;
    background: transparent;
    box-shadow: none;
    transform: none;
  }
  :global(.nb-rejilla[data-modo='lista']) .nb-tarjeta:hover,
  :global(.nb-rejilla[data-modo='lista']) .nb-tarjeta:focus-visible {
    background: color-mix(in srgb, var(--m3c-on-surface) 4%, transparent);
    box-shadow: none;
    transform: none;
  }
  :global(.nb-rejilla[data-modo='lista']) .nb-tarjeta :global(.nb-portada) {
    box-shadow: var(--m3-elevation-1);
    transform: none;
  }
  :global(.nb-rejilla[data-modo='lista']) .nb-tarjeta:hover :global(.nb-portada),
  :global(.nb-rejilla[data-modo='lista']) .nb-tarjeta:focus-visible :global(.nb-portada) {
    box-shadow: var(--m3-elevation-1);
    transform: none;
  }
  :global(.nb-rejilla[data-modo='lista']) .nb-tarjeta-texto {
    display: contents;
  }

  /* La marca vacía no reserva hueco en una fila: ahí el alto es fijo. */

  /*
   * El título se corta con puntos suspensivos y no se desplaza al pasar el ratón.
   *
   * Tenía una marquesina que al recibir el hover cambiaba `width` de `100%` a
   * `max-content`, y eso rehace la disposición al entrar **y otra vez al salir**:
   * 6,5 ms medidos por vez en WebKitGTK, sobre un presupuesto de fotograma de 16.
   * En una rejilla de cincuenta tarjetas eso ocurre una vez por cada una que se
   * roce, y con la rueda girando se rozan muchas por segundo sin que nadie mueva
   * el ratón adrede.
   *
   * `performance-budgets.md` ya decía qué hacer con una animación que no cabe:
   * se elimina, no se recorta. Se recortó de infinita a dos pasadas y
   * seguía costando. Y era movimiento que solo llegaba con el ratón, que esta
   * interfaz no admite como vía única. El título entero está en la ficha, a un
   * clic, y es el mismo argumento que `Cover.svelte` ya tenía escrito para las
   * portadas.
   */
  .nb-titulo {
    font-size: var(--nb-text-sm);
    display: block;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    container-type: inline-size;
  }

  /*
   * En la rejilla, dos líneas y no una: "Cómo tomas las mejo…" no dejaba
   * saber de qué libro se trataba. Se reservan las dos para que las tarjetas
   * queden alineadas; la lista sigue en una sola línea, que es lo que es.
   */
  :global(.nb-rejilla:not([data-modo='lista'])) .nb-titulo {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    white-space: normal;
    min-height: 2lh;
  }

  .nb-autor {
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
    min-width: 0;
    display: block;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    container-type: inline-size;
  }

  .nb-marca {
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }

  .nb-progreso-columna {
    display: flex;
    align-items: center;
    gap: var(--nb-space-3);
    justify-content: flex-end;
  }

  .nb-cifra {
    white-space: nowrap;
    min-width: 3ch;
    text-align: right;
  }
</style>
