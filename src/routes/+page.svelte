<!--
  @component
  La aplicación: la carcasa con sus destinos y sus vistas.

  Contrato: ADR-013 y docs/design/ui-conventions.md.

  Cuatro reglas de ADR-013 viven aquí y no en un comentario:

  * **Se arranca en Inicio**, no en Biblioteca: una cuadrícula de portadas
    responde a "qué tengo", y quien abre un lector quiere "seguir donde estaba".
  * El **Lector es contextual**: no está en la barra hasta que hay algo abierto,
    y no aparece en gris.
  * El **Lector no se cierra al navegar**: ir a Biblioteca a mitad de un capítulo
    y volver devuelve donde se dejó, porque la sesión y el destino son dos cosas
    distintas.
  * **Lo que necesita red vive donde corresponde**, marcado, no en una sección
    aparte.
-->
<script lang="ts">
  import { coalescer, espaciar } from '$core/coalescer';
  import { porcentajeLeido } from '$core/progreso';
  import { onMount, tick } from 'svelte';

  import type {
    StudyCounts,
    ImportDuplicate,
    ImportRejection,
    ImportRun,
    Annotation,
    AnnotationKind,
    AvailabilityReport,
    BrowseFilter,
    BrowseItem,
    BrowsePage,
    BrowseQuery,
    BrowseSort,
    DiagnosticsPackage,
    LibraryFacets,
    ImportReport,
    IndexingReport,
    Lectura,
    LibraryAnnotation,
    LibraryRootView,
    Locator,
    HomeRow,
    Note,
    OutlineEntry,
    RecoveryReport,
    Board,
    AnnotationCounts,
    BoardColumn,
    BoardLimits,
    CitationFormat,
    CollectionSummary,
    ExtensionSoundscape,
    Withdrawal,
    SearchStatus,
    SessionInfo,
    Setting,
    Shelf,
    ShelfKind
  } from '$lib/bindings';

  import {
    annotationOutline,
    browseLibrary,
    libraryFacets,
    diagnosticsPreview,
    guardarDiagnostico,
    guardarReportesDeIa,
    guardarBiblioteca,
    cancelImport,
    pendingImport,
    lastImport,
    importDuplicates,
    importRejections,
    discardPendingImport,
    deleteAnnotation,
    editAnnotation,
    libraryAnnotations,
    elegirParaImportar,
    search,
    closeDocument,
    flushProgress,
    rescanRoots,
    componentStates,
    createNote,
    deleteNote,
    healthCheck,
    importPath,
    allowSpeechHost,
    speechEndpoint,
    speechCredentialStatus,
    setSpeechCredential,
    clearSpeechCredential,
    libraryShelves,
    listNotes,
    POR_PAGINA,
    noteBacklinks,
    acceptSuggestion,
    addCatalog,
    allowLookupHost,
    aiModels,
    aiProviderStatus,
    studyAvailability,
    aiReports,
    aiSends,
    allowAiProvider,
    allowCatalogHost,
    allowModelHost,
    clearAiCredential,
    clearAiReports,
    clearAiSends,
    forgetAiReport,
    probeAiProvider,
    semanticSearch,
    setAiCredential,
    cancelAiModelInstall,
    installAiModel,
    uninstallAiModel,
    avisoDeAudioRetirado,
    annotationHistory,
    citeAnnotation,
    citedAnnotations,
    notesCiting,
    unciteAnnotation,
    elegirCarpetaDeRaiz,
    guardarAnotaciones,
    guardarCitas,
    formatosDeCita as formatosDeCitaIpc,
    revisarMetadatosPorLotes,
    tableros as tablerosIpc,
    colecciones as coleccionesIpc,
    crearColeccion as crearColeccionIpc,
    renombrarColeccion as renombrarColeccionIpc,
    borrarColeccion as borrarColeccionIpc,
    anadirAColeccion,
    quitarDeColeccion,
    coleccionesDeLaEdicion,
    rutaDeImagen,
    crearTablero as crearTableroIpc,
    borrarTablero as borrarTableroIpc,
    columnasDeTablero,
    limitesDeTablero,
    anadirColumna,
    renombrarColumna,
    borrarColumna,
    moverColumna,
    ponerNotaEnColumna,
    enlazarEnColumna,
    crearNotaEnColumna,
    moverTarjeta,
    quitarTarjeta,
    cuentasDeAnotaciones as cuentasDeAnotacionesIpc,
    cuentasDeEstudio,
    findNote,
    indexingProgress,
    reindex,
    searchStatus,
    setIndexingPaused,
    libraryRoots,
    refreshAvailability,
    relocateLibraryRoot,
    applyOnlineMetadata,
    browseCatalog,
    clearVocabulary,
    deleteVocabulary,
    downloadFromCatalog,
    elegirDiccionario,
    fetchOnlineCover,
    installDictionary,
    listCatalogs,
    listDictionaries,
    extensionesInstaladas,
    versionDeApiDeExtensiones,
    elegirExtension,
    instalarExtension as instalarExtensionIpc,
    concederAExtension,
    revocarAExtension,
    habilitarExtension,
    desinstalarExtension,
    generacionesDeExtensiones as generacionesDeExtensionesIpc,
    retiradasDeExtensiones as retiradasDeExtensionesIpc,
    modosDeExtension as modosDeExtensionIpc,
    fondosSonoros as fondosSonorosIpc,
    comprobarRetiradasDeExtensiones,
    volverAtrasExtensiones,
    vistasDeExtension as vistasDeExtensionIpc,
    componerVistaDeExtension,
    temasDeExtension as temasDeExtensionIpc,
    ordenesDeExtension as ordenesDeExtension_,
    ejecutarOrdenDeExtension,
    listVocabulary,
    networkReport,
    onDomainEvent,
    openDocument,
    pendingSuggestions,
    rejectSuggestion,
    removeCatalog,
    revokeNetworkConsent,
    searchOnlineMetadata,
    allowMetadataHost,
    uninstallDictionary,
    whatIsSent,
    readingStats,
    recoveryReport,
    homeRows,
    updateNote,
    type ComponentStatus,
    type Catalog,
    type DictionaryManifest,
    type ExtensionApi,
    type ExtensionCommand,
    type ExtensionRenderedView,
    type ExtensionStudyView,
    type ExtensionTheme,
    type ExtensionInstall,
    type ExtensionPending,
    type ExtensionView,
    type Feed,
    type FeedEntry,
    type HealthReport,
    type MetadataCandidate,
    type MetadataSuggestion,
    type CredentialStatus,
    type AiInstallProgress,
    type AiModels,
    type NetworkReport,
    type VocabularyItem,
    esperarRuntime
  } from '$core/ipc';
  // No se llama a `updateSetting` ni a `settingsSnapshot` directamente: esta capa
  // es la que mantiene al día la caché del primer pintado, y saltársela deja el
  // tema elegido sin sobrevivir al siguiente arranque. Regla 1 de ADR-010.
  import {
    codigoDeError,
    idioma as idiomaActual,
    mensajeDeError,
    textos,
    traducirError,
    usarIdioma
  } from '$lib/i18n';
  import type { narrador as tipoNarrador, OrigenDeVoz } from '$lib/voz';
  import type { ReproductorDePreparado } from '$lib/voz/reproductor.svelte';
  import { guardaDeRespuesta } from '$core/carga';
  import { type Cambios, escucharCambios, type Tema } from '$core/vivo';
  import { cargarPreferencias, guardarPreferencia as escribirPreferencia } from '$core/settings';
  import {
    AppShell,
    Button,
    Spinner,
    Desplegable,
    Emergente,
    type AccionDeIa,
    type EstadoDeIa,
    type SeccionDeAjustes,
    aplicarApariencia,
    fallosDeContraste,
    avisos,
    cargarAjustes,
    cargarDescubrir,
    cargarEstudio,
    cargarFicha,
    cargarFondoSonoro,
    cargarLector,
    cargarMenus,
    cargarPanelesDelLector,
    cargarReproductor,
    cargarVoz,
    CLAVE_JUSTIFICADO,
    CLAVES,
    CLAVES_DEL_KIT,
    type Composicion,
    composicionDe,
    legibles,
    type ClaseDeRuido,
    type Sonando,
    type Modo,
    type ContextoDeNavegacion,
    type Destino,
    type EstadoDeConsulta,
    destinosDelPie,
    destinosPrincipales,
    destinoTrasCerrar,
    elSistemaPrefiereOscuro,
    EmptyState,
    HomeView,
    Icon,
    IconButton,
    abrirEnSecundaria,
    alternarDivision,
    alternarFoco,
    anchos,
    cabeLaDivision,
    clasificarAncho,
    ANCHO_MINIMO,
    Cover,
    type Division,
    DIVISION_INICIAL,
    hayAlgoQueDecir,
    type KitAccesible,
    leerApariencia,
    Logo,
    type NombreDeModo,
    NavItem,
    notificar,
    Panel,
    type PanelDelLector,
    repartir,
    type SeccionDeEstanteria,
    TAMANO_POR_DEFECTO,
    LibraryView,
    TarjetaLateral,
    valorDe
  } from '$ui';
  import type ReaderView from '$ui/lector/ReaderView.svelte';

  /** Página vacía, para pintar antes de que responda el runtime. */
  const VACIA: BrowsePage = { total: 0, items: [] };

  let report = $state<HealthReport | null>(null);
  /** El runtime todavía se prepara: una migración larga, por ejemplo. */
  let poniendoAlDia = $state(false);
  let components = $state<ComponentStatus[]>([]);
  let error = $state<string | null>(null);
  /**
   * Las estanterías del panel: una sección por clase, con cuántas hay de cada.
   *
   * No se cargan al arrancar. Son tres consultas de agrupación sobre la
   * biblioteca entera, y hacerlas para un panel que puede no abrirse nunca es
   * pagar el arranque por adelantado. Se piden al abrir y al escribir.
   */
  /**
   * Lo que la biblioteca está enseñando.
   *
   * Vive aquí y no dentro de la vista porque hay dos sitios que lo cambian: la
   * propia barra de la biblioteca y el panel de estanterías, que es de la
   * carcasa. Con el estado dentro de la vista, elegir una estantería no habría
   * tenido forma de llegar hasta ella.
   */
  let consultaDeBiblioteca = $state<BrowseQuery>({
    sort: 'addedAt',
    ascending: false,
    filter: { kind: 'all' },
    offset: 0,
    limit: TAMANO_POR_DEFECTO
  });

  let estanterias = $state<SeccionDeEstanteria[]>([]);
  let busquedaDeEstanteria = $state('');
  /*
   * Una guarda por cosa que se pide, no una por aplicación: dos filtros
   * distintos no compiten entre sí, y compartir la cuenta haría que escribir en
   * uno descartara la respuesta del otro.
   */
  const guardaDeEstanterias = guardaDeRespuesta();
  let cargandoEstanterias = $state(false);
  /** La estantería abierta: se recuerda para poder marcarla y para nombrarla. */
  let estanteriaActiva = $state<Shelf | null>(null);
  let preferencias = $state<Setting[]>([]);

  /** Valor de una preferencia de texto, o el que se pase si no está guardada. */
  function texto(clave: string, siNoEsta = ''): string {
    const valor = preferencias.find((p) => p.key === clave)?.value;
    return typeof valor === 'string' ? valor : siNoEsta;
  }

  /**
   * Qué tarjetas de la barra lateral están plegadas.
   *
   * Se guarda en preferencias: quien pliega la del índice porque ya sabe cómo va
   * no quiere volver a plegarla cada vez que abre la aplicación.
   */
  const tarjetasPlegadas = $derived(
    new Set(
      texto('sidebar_folded_cards')
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean)
    )
  );

  function tarjetaPlegada(id: string): boolean {
    return tarjetasPlegadas.has(id);
  }

  function plegarTarjeta(id: string, plegada: boolean) {
    const siguiente = new Set(tarjetasPlegadas);
    if (plegada) siguiente.add(id);
    else siguiente.delete(id);
    void guardarPreferencia('sidebar_folded_cards', [...siguiente].sort().join(','));
  }

  /** Valor de una preferencia numérica. */
  function numero(clave: string, siNoEsta: number): number {
    const valor = preferencias.find((p) => p.key === clave)?.value;
    return typeof valor === 'number' ? valor : siNoEsta;
  }

  /*
   * La biblioteca guarda cómo se mira: cuántas obras por página y en qué forma.
   *
   * Van por preferencias y no por estado de la vista porque son decisiones que
   * se toman una vez: quien decide ver la biblioteca en lista con veinticinco
   * por página no quiere volver a decidirlo cada vez que abre la aplicación.
   */
  let porPaginaBiblioteca = $state(TAMANO_POR_DEFECTO);

  /**
   * Cuántas obras hay detrás de cada filtro fijo, para no ofrecer los vacíos.
   *
   * Se piden al entrar en la biblioteca y cada vez que algo la cambia, no al
   * arrancar: son cuatro recuentos, baratos desde V29, pero para una vista que
   * puede no abrirse.
   */
  let facetasDeBiblioteca = $state<LibraryFacets | null>(null);

  const refrescarFacetas = coalescer(releerFacetas, 1500);

  async function releerFacetas() {
    try {
      facetasDeBiblioteca = await libraryFacets();
    } catch {
      // Sin recuentos se ofrecen todos los filtros, que es lo de antes: no
      // saber cuántas hay no es motivo para esconder ninguno.
      facetasDeBiblioteca = null;
    }
  }

  $effect(() => {
    if (destino !== 'biblioteca') return;
    void recargaDeBiblioteca;
    void refrescarFacetas();
  });

  const CRITERIOS: readonly BrowseSort[] = [
    'addedAt',
    'title',
    'author',
    'series',
    'progress',
    'lastRead',
    'published'
  ];

  /** Lee el orden guardado, `criterio:asc` o `criterio:desc`. Lo que no se entienda no se aplica. */
  function ordenGuardado(valor: string): Pick<BrowseQuery, 'sort' | 'ascending'> | null {
    const [criterio, sentido] = valor.split(':');
    if (!CRITERIOS.includes(criterio as BrowseSort)) return null;
    if (sentido !== 'asc' && sentido !== 'desc') return null;
    return { sort: criterio as BrowseSort, ascending: sentido === 'asc' };
  }

  /**
   * En qué página de la biblioteca se estaba.
   *
   * Vive aquí y no dentro de la vista porque los destinos se montan con un
   * `{#if}`: cambiar de destino **desmonta el componente**, y con él todo su
   * estado. Estar en la página 1.500 de ciento cincuenta mil obras, ir a Estudio
   * y volver al principio no es un detalle, es perder el sitio.
   *
   * No se guarda en disco a propósito: es estado de **sesión**, como la sesión de
   * lectura. Sobrevive a moverse por la aplicación y no a cerrarla, que es lo que
   * cualquiera espera de "dónde estaba mirando".
   */
  let paginaDeBiblioteca = $state(1);

  /**
   * La importación que quedó a medias, y el informe de la última.
   *
   * Las dos se consultan al arrancar y no al abrir el panel: la aplicación tiene
   * que poder **decir** que quedó algo a medias sin que nadie vaya a buscarlo.
   * Antes no se guardaba ninguna de las dos, así que reanudar era gratis
   * solo si te acordabas, y saber qué quedó fuera exigía no haber cerrado el
   * panel. Es la pregunta J12.
   */
  let importacionPendiente = $state<ImportRun | null>(null);
  let informeDeImportacion = $state<{
    ejecucion: ImportRun;
    rechazos: ImportRejection[];
    copias: ImportDuplicate[];
  } | null>(null);
  let vistaBiblioteca = $state<'cuadricula' | 'mosaico' | 'lista'>('cuadricula');

  /*
   * Y el narrador recibe con qué elegir la voz: el idioma de la interfaz y la
   * que el usuario haya guardado. El idioma del documento se lo da el lector.
   */
  $effect(() => {
    /*
     * De dónde sale la voz lo decide una preferencia, y el narrador se entera
     * aquí y no por su cuenta: este archivo es quien carga las preferencias, y
     * `$lib/voz` no habla con el runtime salvo por su propio proveedor.
     */
    const guardado = texto('voice_provider', 'local');
    const proveedorDeVoz: OrigenDeVoz =
      guardado === 'remote' ? 'remote' : guardado === 'neural' ? 'neural' : 'local';
    const config = {
      interfaz: idiomaActual(),
      guardada: texto('voice_name') || null,
      proveedor: proveedorDeVoz,
      // Sin anfitrión no hay servicio, y el panel lo dice en lugar de ofrecer
      // botones que no harían nada.
      hayServicio: texto('speech_host').trim().length > 0
    };
    if (narradorInstancia) {
      narradorInstancia.preferir(config);
    } else if (destino === 'lector') {
      void cargarVoz().then(({ narrador }) => {
        narradorInstancia = narrador;
        narrador.preferir(config);
      });
    }
  });

  let destino = $state<Destino>('inicio');
  let plegada = $state(false);
  /** Lo que la barra está pintando; durante el cambio va detrás de `plegada`. */
  let plegadaVisible = $state(false);
  /**
   * El menú de la barra superior que está abierto, si hay uno.
   *
   * Uno solo a la vez, y por eso es un valor y no una bandera por menú: abrir
   * Colecciones con Importar abierto cierra Importar, que es lo que se espera de
   * dos menús que nacen de la misma barra.
   */
  let menuAbierto = $state<
    'importar' | 'audiolibros' | 'buscar' | 'estanterias' | 'colecciones' | null
  >(null);
  /** Lo escrito en la búsqueda de la barra superior. Sobrevive a cerrar los resultados. */
  let consultaGlobal = $state('');
  let buscador: HTMLElement | undefined = $state();
  /** La sección abierta de Ajustes, para poder llevar a una desde fuera. */
  let seccionDeAjustes = $state<SeccionDeAjustes>('apariencia');

  /*
   * Las colecciones del usuario. Se piden al arrancar con lo demás: son
   * pocas, y la ficha las necesita en cuanto se elige una obra.
   */
  let colecciones = $state<CollectionSummary[]>([]);
  let cargandoColecciones = $state(false);
  /** La colección que filtra la biblioteca, para marcarla y nombrarla. */
  let coleccionActiva = $state<CollectionSummary | null>(null);
  /** En qué colecciones está la obra enfocada. */
  let enColecciones = $state<string[]>([]);

  /**
   * Abre o cierra un menú de la barra.
   *
   * Cerrar solo cierra **si es el que está abierto**. Pulsar Importar con
   * Colecciones abierto dispara las dos cosas en el mismo clic —Importar se abre,
   * y Colecciones se entera de que se ha pulsado fuera—, y sin esta comprobación
   * el cierre de la segunda deshacía la apertura de la primera.
   */
  function alternarMenu(cual: NonNullable<typeof menuAbierto>, abierto: boolean) {
    if (abierto) menuAbierto = cual;
    else if (menuAbierto === cual) menuAbierto = null;
  }

  // Cambiar de destino cierra los menús: lo que se abrió para otra pantalla no
  // tiene nada que decir en esta.
  $effect(() => {
    void destino;
    menuAbierto = null;
  });
  /** Avance de la importación en curso. Nulo cuando no hay ninguna. */
  /** La obra que el foco de la biblioteca tiene encima, para su ficha. */
  let enfocada = $state<BrowseItem | null>(null);

  let avanceImportacion = $state<{
    processed: number;
    imported: number;
    skipped: number;
    rejected: number;
    duplicates: number;
    total: number;
  } | null>(null);

  /** La sesión sobrevive a cambiar de destino: eso es la regla 2 de ADR-013. */
  let sesion = $state<SessionInfo | null>(null);

  /**
   * Por dónde va cada documento abierto, para volver ahí al volver al lector.
   *
   * El lector no se cierra al ir a otro destino, pero su vista se desmonta; sin
   * esto, al volver empezaba donde estaba el libro al abrirlo. No es
   * reactivo a propósito: se lee al montar la vista y nada más.
   */
  const ultimaPosicion = new Map<string, Locator>();
  /** La segunda mitad de la vista dividida. Es otro lector, con su posición. */
  let sesionSecundaria = $state<SessionInfo | null>(null);
  let division = $state<Division>(DIVISION_INICIAL);
  /**
   * Ancho de la ventana, para decidir si la división se ofrece.
   *
   * Se observa aquí y no dentro de la carcasa porque quien decide qué comandos
   * existen es esta capa: la carcasa no sabe nada del lector.
   */
  let anchoDeLaVentana = $state(1440);
  const cabeDividir = $derived(cabeLaDivision(clasificarAncho(anchoDeLaVentana)));

  $effect(() => {
    const medir = () => (anchoDeLaVentana = globalThis.innerWidth ?? 1440);
    medir();
    globalThis.addEventListener?.('resize', medir);
    return () => globalThis.removeEventListener?.('resize', medir);
  });

  /**
   * Abre o cierra la vista dividida.
   *
   * Antes esto no existía: `alternarDivision` estaba escrita, probada y
   * exportada, y **no la llamaba nadie**. La tecla `D` y su botón estaban en la
   * barra y no hacían nada, así que la vista dividida no
   * se podía abrir por ningún camino. La única vía que la miraba era abrir un
   * segundo documento con la división ya activa, y nunca podía estarlo.
   */
  /**
   * Pasa el foco a la otra mitad, **si hay otra mitad**.
   *
   * Con la segunda vacía no se mueve: allí no hay lector que gobierne el
   * teclado, así que llevar el foco ahí dejaba la aplicación sin responder a
   * ninguna tecla hasta volver.
   */
  function cambiarDeMitad() {
    if (!division.activa || !sesionSecundaria) return;
    division = alternarFoco(division);
  }

  function alternarLaDivision() {
    if (!division.activa && !cabeDividir) return;
    division = alternarDivision(division);
    // Cerrarla suelta el documento de la derecha: dejarlo abierto en el runtime
    // mientras no se ve es progreso que nadie está moviendo.
    if (!division.activa && sesionSecundaria) {
      const abierto = sesionSecundaria.fileId;
      sesionSecundaria = null;
      void closeDocument(abierto).catch(avisarDeFallo);
    }
  }
  let panelDelLector = $state<PanelDelLector | null>(null);
  /**
   * El lector principal, para poder pedirle que cierre su panel.
   *
   * El estado del panel es **suyo**, no de la carcasa: aquí solo se refleja para
   * saber cuál pintar. Cerrarlo desde aquí sin decírselo dejaba el panel cerrado
   * y su botón de la barra marcado como abierto.
   */
  let lectorPrincipal = $state<ReaderView | undefined>();
  /** El nombre del teléfono que maneja el lector con Leer en el PC, o nulo. */
  let controladoDesde = $state<string | null>(null);

  /**
   * Si hay quien genere las funciones de estudio. Sin eso el lector no
   * ofrece estudiar: la IA es opcional y no se anuncia.
   */
  let hayIaParaEstudiar = $state(false);
  /** Dónde se estaba y qué había seleccionado al abrir el panel de estudio. */
  let paraEstudiar = $state<Awaited<ReturnType<ReaderView['paraEstudiar']>> | null>(null);

  let narradorInstancia = $state<typeof tipoNarrador | null>(null);

  /**
   * El reproductor de lo preparado. Vive aquí y no en el panel de
   * voz, que es momentáneo: dentro del panel dejaría de sonar al tocar el texto.
   * Mueve el lector al fragmento que suena, y para la narración en vivo al empezar.
   */
  let reproductor = $state<ReproductorDePreparado | null>(null);

  async function asegurarVoz(): Promise<{
    narrador: typeof tipoNarrador;
    reproductor: ReproductorDePreparado;
  }> {
    if (!narradorInstancia) {
      const { narrador } = await cargarVoz();
      narradorInstancia = narrador;
      const guardado = texto('voice_provider', 'local');
      const proveedorDeVoz: OrigenDeVoz =
        guardado === 'remote' ? 'remote' : guardado === 'neural' ? 'neural' : 'local';
      narrador.preferir({
        interfaz: idiomaActual(),
        guardada: texto('voice_name') || null,
        proveedor: proveedorDeVoz,
        hayServicio: texto('speech_host').trim().length > 0
      });
    }
    if (!reproductor) {
      const { ReproductorDePreparado: Ctor } = await cargarReproductor();
      reproductor = new Ctor(
        (locator) => (objetivo = locator),
        () => narradorInstancia?.parar()
      );
    }
    return { narrador: narradorInstancia, reproductor };
  }

  async function refrescarIaParaEstudiar() {
    try {
      hayIaParaEstudiar = (await studyAvailability()).ready;
    } catch {
      hayIaParaEstudiar = false;
    }
  }

  $effect(() => {
    if (sesion?.fileId) void refrescarIaParaEstudiar();
  });
  let inmersivo = $state(false);
  let objetivo = $state<Locator | null>(null);
  /**
   * Anotaciones del documento abierto, en orden de lectura.
   *
   * Vive aquí y no dentro del panel porque el panel es una zona de la carcasa y
   * el lector es otra: quien anota es el lector, y quien lo pinta es el panel.
   * Con la lista en medio, anotar la refresca sin que ninguno de los dos tenga
   * que saber del otro.
   */
  let anotaciones = $state<OutlineEntry[]>([]);

  /*
   * Qué se pudo marcar sobre el texto y qué no.
   *
   * Vive aquí y no dentro del lector porque quien lo enseña es el panel de
   * anotaciones, que es de la carcasa. Una marca que el runtime colocó y que la
   * página no pudo pintar **no es lo mismo que una perdida**, y callarla haría
   * que "no la veo en el libro" y "no existe" se parecieran demasiado.
   */
  let sinPintar = $state<Set<string>>(new Set());
  let sinSoporteDeMarcas = $state(false);
  /** Anotación señalada en la página, para abrir el panel justo en ella. */
  let anotacionSenalada = $state<string | null>(null);
  /**
   * Lo que el panel de consulta enseña.
   *
   * Vive aquí por el mismo motivo que las anotaciones: consultar es del lector,
   * pintarlo es del panel, y el panel lo monta la carcasa. `null` mientras no se
   * ha consultado nada.
   */
  let consulta = $state<EstadoDeConsulta | null>(null);
  /**
   * Palabra que el panel pide consultar.
   *
   * Se pone a nulo en cuanto el lector avisa de que ya está consultando esa: sin
   * eso, volver a elegir la misma palabra no cambiaría el valor y el efecto no
   * se dispararía.
   */
  let palabraPedida = $state<string | null>(null);

  // Estudio: vocabulario y los diccionarios de Ajustes.
  let vocabulario = $state<VocabularyItem[]>([]);
  let diccionarios = $state<DictionaryManifest[]>([]);

  // Extensiones. `pendienteExtension` va aparte de la lista porque
  // todavía **no está instalada**: mezclarla daría a entender que ya entró.
  let extensiones = $state<ExtensionView[]>([]);
  let apiDeExtensiones = $state<ExtensionApi | null>(null);
  // Lo que las extensiones aportan sobre un documento. Se pide una vez
  // y no por ficha: sale del manifiesto y no cambia al cambiar de documento.
  let ordenesDeExtension = $state<ExtensionCommand[]>([]);
  // Las vistas que las extensiones aportan a Estudio. Las pestañas
  // salen del manifiesto; el contenido se compone al abrirlas, y componerlo es
  // lo que carga el código de la extensión.
  let vistasDeExtension = $state<ExtensionStudyView[]>([]);
  let vistaDeExtension = $state<ExtensionRenderedView | null>(null);
  // Temas aportados por extensiones.
  let temasDeExtension = $state<ExtensionTheme[]>([]);
  let temaDeExtensionActivo = $state<string | null>(null);
  let pendienteExtension = $state<ExtensionPending | null>(null);
  let rutaPendienteExtension = $state<string | null>(null);
  let generacionesDeExtensiones = $state<number[]>([]);
  let retiradasDeExtensiones = $state<Withdrawal[]>([]);
  /**
   * Los modos de lectura que aportan los paquetes instalados, ya filtrados por
   * contraste.
   *
   * Van al panel de modos junto a los seis del núcleo, y sus nombres al mapeo
   * de composición: sin ellos, elegir un modo de extensión y reabrir el libro
   * devolvería a "día", porque el nombre guardado no se reconocería.
   */
  let modosDeExtension = $state<Modo[]>([]);
  /**
   * Los fondos sonoros que aportan los paquetes instalados.
   *
   * Nada de esto se guarda —ni cuál suena ni a qué volumen—, y es deliberado:
   * abrir la aplicación y que empiece a sonar ruido por su cuenta sería
   * hostil. De paso, desinstalar el paquete no deja rastro sin tener que
   * limpiar nada.
   */
  let fondosSonoros = $state<ExtensionSoundscape[]>([]);
  /** Los formatos de cita que aportan las extensiones instaladas. */
  let formatosDeCita = $state<CitationFormat[]>([]);
  /**
   * Los tableros de notas.
   *
   * Un tablero es una **vista** sobre las notas que ya hay: crearlo y borrarlo
   * no toca ninguna.
   */
  let tableros = $state<Board[]>([]);
  let tableroActivo = $state<string | null>(null);
  let columnasDelTablero = $state<BoardColumn[]>([]);
  let limitesDelTablero = $state<BoardLimits>({ columns: 8, cards: 120 });
  /** Cuántas anotaciones hay de cada clase, para no ofrecer filtros vacíos. */
  let cuentasDeAnotaciones = $state<AnnotationCounts | null>(null);

  /*
   * Cuántas filas se piden de cada lista de Estudio.
   *
   * Crecen de página en página cuando alguien pulsa "Cargar Más", y no se
   * reinician al refrescar: quien ha pedido ver doscientas notas no quiere que
   * borrar una lo devuelva a cincuenta. La regla 1 del contrato de carga no
   * prohíbe llegar a doscientas, prohíbe **pedirlas sin haberlas pedido**: lo
   * que no puede haber es una consulta cuyo tamaño lo decida la biblioteca.
   */
  let limiteNotas = $state(POR_PAGINA);
  let limiteAnotaciones = $state(POR_PAGINA);
  let limiteVocabulario = $state(POR_PAGINA);
  let limiteTableros = $state(POR_PAGINA);

  /**
   * Si una lista puede tener más de lo que trajo.
   *
   * Se deduce de que venga llena, sin preguntarle al runtime cuántas hay: un
   * `COUNT(*)` por página es justo lo que J8 desaconseja. El caso en que el
   * total es exactamente el límite ofrece un botón que no trae nada, y se
   * corrige solo en cuanto se pulsa.
   */
  const hayMas = (cuantas: number, limite: number) => cuantas >= limite;

  /** Relee las notas con el límite puesto. */
  async function refrescarNotas() {
    notas = await listNotes(limiteNotas);
  }

  async function refrescarTableros() {
    tableros = await tablerosIpc(limiteTableros);
    // Si el que estaba abierto ya no está -se borró-, se cierra en lugar de
    // dejar un tablero que señala a algo que no existe.
    if (tableroActivo && !tableros.some((t) => t.id === tableroActivo)) {
      tableroActivo = null;
      columnasDelTablero = [];
    }
  }

  async function abrirTablero(id: string) {
    try {
      tableroActivo = id;
      columnasDelTablero = await columnasDeTablero(id);
    } catch (e) {
      avisarDeFallo(e);
    }
  }
  /**
   * Las extensiones que revisan metadatos por lotes.
   *
   * Salen del manifiesto: declarar el proveedor `bulk-metadata` es lo que las
   * pone en la biblioteca, sin compilar una sola instrucción suya.
   */
  const revisoresDeMetadatos = $derived(
    extensiones
      // Una a la que le falta su módulo no se ofrece: pulsar su botón solo podría
      // fallar, y Ajustes ya dice qué le pasa.
      .filter((e) => e.enabled && e.readable && e.missingModules.length === 0)
      .filter((e) => e.providers.includes('bulk-metadata'))
      .map((e) => ({ id: e.id, name: e.name }))
  );
  let fondoSonando = $state<string | null>(null);
  let volumenDelFondo = $state(0.4);
  let minutosDelFondo = $state(0);
  let contextoDeAudio: AudioContext | null = null;
  let sonando: Sonando | null = null;
  let relojDelFondo: ReturnType<typeof setTimeout> | null = null;
  /** Cuenta los fondos pedidos y parados, para que uno que tarda en cargar no suene tarde. */
  let pedidoDelFondo = 0;

  /**
   * Pone un fondo sonoro.
   *
   * El `AudioContext` se crea la primera vez y se reutiliza: cada uno es un
   * hilo de audio del sistema, y abrir uno por pulsación es la forma de
   * quedarse sin ninguno.
   */
  async function ponerFondo(id: string) {
    const fondo = fondosSonoros.find((f) => f.id === id);
    if (!fondo) return;
    pararFondo();
    const pedido = pedidoDelFondo;

    // El contexto se crea y se reanuda antes de esperar a nada: el audio solo se
    // deja arrancar desde una pulsación, y después de una espera ya no lo es.
    const contexto = (contextoDeAudio ??= new AudioContext());
    void contexto.resume();
    const { sonar } = await cargarFondoSonoro();
    // Mientras se cargaba se pudo parar, o pedir otro: entonces este ya no se quiere.
    if (pedido !== pedidoDelFondo) return;
    sonando = sonar(contexto, fondo.kind as ClaseDeRuido, volumenDelFondo);
    fondoSonando = id;
    ultimoFondo = id;

    if (minutosDelFondo > 0) {
      relojDelFondo = setTimeout(() => pararFondo(), minutosDelFondo * 60_000);
    }
  }

  /** El último fondo que sonó, para que el mando lo vuelva a poner. */
  let ultimoFondo: string | null = null;

  /**
   * El fondo sonoro desde el teléfono (ADR-016). La tecla abre el panel para
   * elegir; el mando no ve el panel, así que alterna: para lo que suena o
   * vuelve a poner el último fondo, o el primero.
   */
  function alternarFondoRemoto() {
    if (fondoSonando) return pararFondo();
    const id = ultimoFondo ?? fondosSonoros[0]?.id;
    if (id) void ponerFondo(id);
  }

  function pararFondo() {
    pedidoDelFondo++;
    if (relojDelFondo) {
      clearTimeout(relojDelFondo);
      relojDelFondo = null;
    }
    sonando?.parar();
    sonando = null;
    fondoSonando = null;
  }

  // Red: lo autorizado y lo que ha salido, y qué se envía al consultar
  //. El texto lo dice el runtime, donde vive junto al propósito.
  let red = $state<NetworkReport | null>(null);

  /** Los modelos de IA del catálogo y su instalación. */
  let modelosDeIa = $state<AiModels | null>(null);
  let avanceDeModelo = $state<AiInstallProgress | null>(null);
  /** La red de distribución a la que la descarga necesita salir, si falta su permiso. */
  let modeloPideAutorizar = $state<string | null>(null);
  /** De qué modelo era esa descarga, para seguirla al autorizar. */
  let modeloQueEspera = $state<string | null>(null);
  let falloDeModelo = $state<string | null>(null);

  /**
   * El proveedor de IA externo: su estado, la última prueba y lo que se
   * le ha enviado. La prueba guarda el mensaje que se enseña antes del primer
   * envío, y se descarta al decidir.
   */
  let ia = $state<EstadoDeIa>({
    proveedor: null,
    prueba: null,
    probando: false,
    envios: [],
    reportes: []
  });
  let credencialDeVoz = $state<CredentialStatus | null>(null);
  /** A dónde saldría el texto con el servidor escrito, como lo entiende el runtime. */
  let puntoDeVoz = $state<string | null>(null);
  let queSeEnviaAlConsultar = $state<string | null>(null);

  // Completar la ficha desde un catálogo abierto. Vacío mientras nadie
  // lo pida: la consulta es explícita, nunca automática al importar.
  let candidatos = $state<MetadataCandidate[]>([]);
  let buscandoEnLinea = $state(false);
  let errorEnLinea = $state<string | null>(null);
  let propuestas = $state<MetadataSuggestion[]>([]);
  /**
   * Señal de recarga de la biblioteca.
   *
   * Un contador y no una bandera: hay que poder pedir dos recargas seguidas, y
   * una bandera que ya estaba a cierto no dispara nada la segunda vez.
   */
  let recargaDeBiblioteca = $state(0);

  // Descubrir: catálogos OPDS.
  let catalogos = $state<Catalog[]>([]);

  /**
   * El camino recorrido dentro del catálogo abierto, de la raíz hacia dentro.
   *
   * Existe por dos cosas que sin ella no se pueden hacer: saber **en qué
   * catálogo estás** cuando ya has entrado en una sección —la raíz es el primer
   * elemento, y no cambia al navegar— y **volver**, que es lo que convierte
   * entrar en una sección en algo reversible.
   */
  let rutaDelCatalogo = $state<string[]>([]);
  const catalogoRaiz = $derived(rutaDelCatalogo[0] ?? null);
  let raices = $state<LibraryRootView[]>([]);
  let revisandoRaices = $state(false);
  let revisionDeRaices = $state<AvailabilityReport | null>(null);
  let feed = $state<Feed | null>(null);
  let catalogoAbierto = $state<string | null>(null);
  let cargandoCatalogo = $state(false);
  let errorDeCatalogo = $state<string | null>(null);
  /**
   * El anfitrión al que falta autorizar, **con su nombre**, y si es en claro.
   *
   * Antes la interfaz solo sabía que faltaba un permiso, y
   * autorizaba el anfitrión de la dirección abierta. Casi todo catálogo redirige
   * a otro, así que se autorizaba uno que ya lo estaba y el catálogo seguía sin
   * abrir, en bucle. Ahora lo dice el runtime.
   */
  let permisoQueFalta = $state<{ host: string; cleartext: boolean } | null>(null);
  let descargando = $state(false);
  let ultimaDescarga = $state<{ titulo: string; estado: 'ok' | 'repetido' | 'rechazado' } | null>(
    null
  );

  // Inicio
  let enCurso = $state<BrowsePage>(VACIA);
  /**
   * La obra del documento abierto, tal como se eligió. La tarjeta lateral la
   * necesita cuando lo abierto todavía no cuenta como empezado (no llega al 1 %)
   * y no está en `enCurso`: sin ella decía el título de otro libro.
   */
  let obraAbierta = $state<BrowseItem | null>(null);
  let novedades = $state<BrowsePage>(VACIA);
  /** Las filas de Inicio de esta visita. */
  let filasDeInicio = $state<HomeRow[]>([]);
  /**
   * La semilla de la visita a Inicio.
   *
   * Cambia **al salir** de Inicio, no al entrar, y las filas de la visita
   * siguiente se piden en ese momento: al volver ya están, y la pantalla no se
   * rehace delante de quien la mira. Mientras se está en Inicio no cambia, así que
   * un refresco —terminar una importación, guardar un progreso— no baraja lo que se
   * está leyendo.
   */
  let semillaDeInicio = nuevaSemilla();
  let estabaEnInicio = true;
  let cifras = $state<Lectura | null>(null);
  /** Las materias con más obras, para la fila de Inicio que explora por ellas. */
  let materiasDeInicio = $state<Shelf[]>([]);

  // Estudio
  let notas = $state<Note[]>([]);
  let notaAbierta = $state<Note | null>(null);
  let retroenlaces = $state<Note[]>([]);
  let vistaDeEstudio = $state('notas');
  /**
   * Anotaciones de toda la biblioteca.
   *
   * Es el otro alcance del mismo dato que pinta el panel del lector: allí, las
   * del documento abierto; aquí, las de todos. Se piden aparte porque la
   * consulta es otra, no porque sean otra cosa.
   */
  let anotacionesDeLaBiblioteca = $state<LibraryAnnotation[]>([]);
  let anotacionAbiertaEnEstudio = $state<LibraryAnnotation | null>(null);
  let palabraAbiertaEnEstudio = $state<VocabularyItem | null>(null);

  /**
   * Diagnóstico preparado, o nulo si no se ha pedido.
   *
   * No se prepara al abrir el panel: prepararlo lee el registro del disco, y
   * quien mira el estado de los componentes no está pidiendo eso. Se prepara
   * cuando alguien lo pide, y entonces se enseña **antes** de guardarlo.
   */
  let diagnostico = $state<DiagnosticsPackage | null>(null);

  /**
   * Qué pasó al abrir la biblioteca, si hubo que recuperarla.
   *
   * Se pregunta una sola vez al arrancar. Cuando trae algo hay que **decirlo**:
   * la biblioteca está vacía y el usuario no lo pidió, y una biblioteca vacía sin
   * explicación es la pérdida silenciosa que el producto existe para impedir.
   */
  let recuperacion = $state<RecoveryReport | null>(null);
  let diagnosticoGuardado = $state(false);
  let claseDeAnotacion = $state<AnnotationKind | null>(null);

  // Los nombres de los modos que aportan las extensiones entran en el mapeo:
  // sin ellos, elegir uno y reabrir el libro devolvería a "día", porque el
  // nombre guardado no se reconocería y se compondría con el de siempre.
  const composicion = $derived(
    composicionDe(
      preferencias,
      modosDeExtension.map((m) => m.nombre)
    )
  );

  // Lo que el runtime confirma gana a la caché de arranque: regla 2 de ADR-010.
  // Sin esto, cambiar el idioma se guardaba en la base y no cambiaba nada de lo
  // que se ve hasta el siguiente arranque, que es la peor versión de un ajuste.
  $effect(() => {
    const elegido = preferencias.find((p) => p.key === 'language')?.value;
    if (typeof elegido !== 'string') return;
    // El diccionario puede tener que cargarse, y el atributo cambia con el texto y
    // no antes: el lector de pantalla leería español con la pronunciación inglesa.
    void usarIdioma(elegido).then(() => {
      document.documentElement.lang = idiomaActual();
    });
  });

  /*
   * Un cambio de preferencia se ve al instante. Nunca hay que reiniciar.
   *
   * `+layout.svelte` pinta la apariencia una vez desde la caché de arranque,
   * para que no haya destello mientras SQLite responde. A partir de ahí manda
   * esto: lo que el runtime confirmó. Sin este efecto, elegir un tema lo
   * guardaba en la base y no cambiaba nada de lo que se ve, que es la peor
   * versión de un ajuste: parece que funciona y no funciona.
   */
  let oscuroDelSistema = $state(elSistemaPrefiereOscuro());

  $effect(() => {
    const valores = Object.fromEntries(preferencias.map((p) => [p.key, p.value]));
    void aplicarApariencia(
      document.documentElement,
      leerApariencia(valores, temasDeExtension),
      oscuroDelSistema
    );
  });

  // Y si el tema es el del sistema, cambiarlo en el sistema también se ve al
  // instante: preguntarlo solo al arrancar dejaría la aplicación en claro toda
  // la noche.
  $effect(() => {
    const consulta = globalThis.matchMedia?.('(prefers-color-scheme: dark)');
    if (!consulta) return;

    oscuroDelSistema = consulta.matches;
    const alCambiar = () => (oscuroDelSistema = consulta.matches);
    consulta.addEventListener('change', alCambiar);
    return () => consulta.removeEventListener('change', alCambiar);
  });

  /**
   * Qué se está leyendo, para la barra superior.
   *
   * Sale del índice del documento y no del catálogo: es lo que la sesión ya
   * trajo al abrir, y pedir el título por separado sería una consulta más en el
   * camino de abrir un libro.
   */
  const tituloDelDocumento = $derived(
    // Un documento de páginas no tiene capítulos con nombre: su índice dice
    // "Página 1", y eso se quedaba fijo arriba pasara la página que pasara. Ahí
    // va el título de la obra, que la sesión ya trae.
    sesion && sesion.mediaType !== 'application/epub+zip'
      ? sesion.title
      : (sesion?.navigation.entries[0]?.locator.title ?? sesion?.locator.title ?? null)
  );

  const contexto = $derived<ContextoDeNavegacion>({
    hayDocumentoAbierto: sesion !== null,
    // Descubrir aparece en cuanto hay un catálogo, y no antes: un destino cuya
    // única pantalla es un formulario de configuración es la regla 5 de ADR-013
    // vista del otro lado. Los catálogos se añaden en Ajustes.
    hayCatalogos: catalogos.length > 0
  });

  // Los nombres de los destinos salen del diccionario y no de una tabla escrita
  // aquí: la barra, la cabecera y el nombre accesible tienen que decir lo mismo,
  // y con dos tablas acabarían diciendo cosas distintas en algún idioma.
  const T = $derived(textos());
  const TITULOS = $derived(textos().destinos as Record<Destino, string>);
  const cifra = $derived(new Intl.NumberFormat(idiomaActual()));
  const cifraCorta = $derived(new Intl.NumberFormat(idiomaActual(), { notation: 'compact' }));
  /** Sin sitio para el texto de los botones de la barra: se quedan en su icono. */
  // Los menús de la barra pasan a icono hasta tener sitio de sobra: con cuatro
  // menús con nombre a 1100 px, el buscador se quedaba en "Busca" y no dejaba
  // ver lo que se escribía. Los 320 px son lo que ocupan los nombres.
  const barraEstrecha = $derived(anchoDeLaVentana < ANCHO_MINIMO.amplio + 320);
  /** Cómo va el índice de la biblioteca entera, para su tarjeta. */
  const indiceDeLaTarjeta = $derived.by(() => {
    if (!avanceDelIndice || avanceDelIndice.pending === 0) return null;
    // Cuánto de la biblioteca se puede buscar, contado en la base. Antes
    // salía de lo indexado en esta sesión más la cola, que sobre una biblioteca
    // indexada a medias en sesiones anteriores no era un porcentaje de nada.
    const { searchable, total, paused, pauseReason } = avanceDelIndice;
    return {
      total,
      hechas: searchable,
      fraccion: searchable / Math.max(1, total),
      paused,
      sinSitio: pauseReason === 'STORAGE_BUDGET_REACHED'
    };
  });

  /**
   * Lo que se hace con un fallo del runtime, en un solo sitio.
   *
   * Antes cada `catch` escribía en `error`, y `error` solo se pintaba **dentro
   * del panel de estado**, que a su vez solo aparece cuando algún componente
   * está en mal estado. El resultado medido: abrir un documento que falla,
   * guardar una nota que falla o importar una carpeta que falla dejaban la
   * pantalla idéntica a si hubiera ido bien. Ahora el fallo se ve siempre, y la
   * variable sigue existiendo para el panel de estado, que es donde se mira
   * cuando ya se sabe que algo va mal.
   *
   * El detalle es el **código canónico**, nunca `error.message`: en un `NbError`
   * el mensaje es el código, y el mensaje en palabras lo pone `mensajeDeError`.
   */
  function avisarDeFallo(causa: unknown): void {
    error = mensajeDeError(causa);
    notificar.error(error, codigoDeError(causa) ?? undefined);
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let unlistenImportacion: (() => void) | undefined;
    let unlistenFinal: (() => void) | undefined;
    let unlistenOrdenes: (() => void) | undefined;
    let unlistenControlador: (() => void) | undefined;
    let unlistenIndice: (() => void) | undefined;
    let unlistenCambios: (() => void) | undefined;
    let unlistenModelo: (() => void) | undefined;
    let unlistenModeloFinal: (() => void) | undefined;

    void (async () => {
      try {
        // Si tarda más de un instante, se dice por qué: sin esto, una migración
        // sobre una biblioteca grande dejaba la carcasa vacía sin explicación.
        const aviso = setTimeout(() => (poniendoAlDia = true), 400);
        await esperarRuntime();
        clearTimeout(aviso);
        poniendoAlDia = false;
        report = await healthCheck();
        components = await componentStates();
        // Lo primero que hay que saber: si esta biblioteca es la tuya o una
        // vacía porque la tuya no se pudo abrir.
        recuperacion = await recoveryReport();
        preferencias = await cargarPreferencias();
        // Cómo se mira la biblioteca sale de las preferencias una sola vez: a
        // partir de ahí manda lo que se pulse, que además se guarda.
        porPaginaBiblioteca = numero('library_page_size', TAMANO_POR_DEFECTO);
        // Y el orden, que antes no se guardaba y volvía a "por
        // fecha" en cada arranque.
        const orden = ordenGuardado(texto('library_sort', 'addedAt:desc'));
        if (orden) consultaDeBiblioteca = { ...consultaDeBiblioteca, ...orden };
        const forma = texto('library_view', 'cuadricula');
        if (forma === 'cuadricula' || forma === 'mosaico' || forma === 'lista') {
          vistaBiblioteca = forma;
        }
        await refrescar();
        await refrescarImportaciones();
        // El audio preparado con una voz que una actualización retiró se borra
        // solo al arrancar; aquí se dice, una vez.
        const audioRetirado = await avisoDeAudioRetirado().catch(() => null);
        if (audioRetirado) notificar.informacion(T.avisos.audioRetirado, String(audioRetirado));
        // Se dice al arrancar, con un aviso que no interrumpe: la acción vive en
        // el panel de importar, que es donde importar tiene sentido, pero
        // enterarse no puede depender de abrirlo.
        if (importacionPendiente) notificar.informacion(T.avisos.importacionAMedias);
      } catch (e) {
        avisarDeFallo(e);
      }

      // Las suscripciones van dentro del intento: no poder escuchar el bus deja
      // la interfaz sin actualizarse sola, que es un modo degradado, pero un
      // rechazo sin capturar la dejaría con una promesa colgando y un error en
      // la consola que nadie sabría de dónde viene.
      try {
        unlisten = await onDomainEvent('ComponentStateChanged', ({ component, state }) => {
          components = components.map((c) => (c.component === component ? { ...c, state } : c));
        });

        // El avance lo escucha la aplicación y no el panel: un panel que se
        // suscribe por su cuenta deja de recibir avances en cuanto se cierra, y
        // cerrarlo no para la importación.
        unlistenImportacion = await onDomainEvent(
          'BulkImportProgress',
          ({ processed, imported, skipped, rejected, duplicates, total }) => {
            avanceImportacion = { processed, imported, skipped, rejected, duplicates, total };
          }
        );

        /*
         * El avance del índice se relee al moverse, con un freno: el runtime
         * publica un evento por documento, y repintar la tarjeta en cada uno
         * sería trabajo para enseñar lo mismo. Uno cada cuatro segundos basta
         * para que la cifra no se quede quieta toda la tarde.
         */
        let indiceProgramado = false;
        unlistenIndice = await onDomainEvent('IndexingProgress', () => {
          if (indiceProgramado) return;
          indiceProgramado = true;
          setTimeout(() => {
            indiceProgramado = false;
            void refrescarAvanceDelIndice();
          }, 4000);
        });

        /*
         * La descarga de un modelo, igual que la importación: la escucha la
         * aplicación y no Ajustes, porque salir de Ajustes no para la descarga y
         * volver tiene que encontrarla donde iba.
         */
        unlistenModelo = await onDomainEvent('AiModelProgress', ({ modelId, done, total }) => {
          avanceDeModelo = { modelId, done, total };
        });
        unlistenModeloFinal = await onDomainEvent('AiModelFinished', alTerminarUnModelo);

        // Leer en el PC: las órdenes del teléfono y quién lo maneja.
        // Abrir es de la carcasa, que monta el lector, y llega con el archivo ya
        // resuelto; lo demás entra por el registro de comandos del lector.
        unlistenOrdenes = await onDomainEvent('RemoteOrder', ({ order, fileId }) => {
          if (order.tipo === 'abrir') void abrirResultado(fileId);
          else if (destino === 'lector') lectorPrincipal?.ordenRemota(order);
        });
        unlistenControlador = await onDomainEvent(
          'RemoteControllerChanged',
          ({ deviceName }) => (controladoDesde = deviceName)
        );

        unlistenFinal = await onDomainEvent('BulkImportFinished', () => {
          avanceImportacion = null;
          notificar.exito(T.avisos.importacionTerminada);
          // Lo que la importación cambió en la biblioteca llega por `alCambiar`,
          // como cualquier otro cambio; aquí solo lo que es de la importación: el
          // informe guardado, que es el que sobrevive a cerrar el panel.
          void refrescarImportaciones();
        });

        /*
         * Lo que cambió, lo diga quien lo diga.
         *
         * Una sola suscripción para todo lo que se pinta: un documento importado,
         * un metadato corregido, una portada recuperada, un progreso guardado,
         * una colección tocada o una extensión instalada. Quien escribe es el
         * runtime y lo anuncia después de confirmar; aquí solo se decide qué
         * volver a leer. Ver `$core/vivo`.
         */
        unlistenCambios = await escucharCambios(alCambiar);
        hayBus = true;
      } catch {
        // Sin bus, la interfaz responde igual a lo que el usuario hace; lo que
        // pierde es enterarse sola de lo que pasa por detrás.
      }
    })();

    // Volver a la ventana desde otra aplicación: el momento en que más probable es
    // que algo haya cambiado en el disco.
    const alVolverALaVentana = () => {
      // Volver a la ventana sí repasa siempre: es cuando alguien acaba de copiar
      // un libro con el explorador. El runtime frena dos seguidos.
      if (document.visibilityState === 'visible') void rescanRoots().catch(() => undefined);
    };
    window.addEventListener('focus', alVolverALaVentana);

    return () => {
      window.removeEventListener('focus', alVolverALaVentana);
      unlisten?.();
      unlistenImportacion?.();
      unlistenFinal?.();
      unlistenOrdenes?.();
      unlistenControlador?.();
      unlistenIndice?.();
      unlistenModelo?.();
      unlistenModeloFinal?.();
      unlistenCambios?.();
    };
  });

  /**
   * Pide las estanterías de las tres clases, con lo que haya escrito.
   *
   * Tres consultas y no una: el runtime devuelve una clase por petición para que
   * el límite se reparta entre las tres en lugar de gastarlo la primera. Con una
   * sola consulta, buscar "gar" en una biblioteca con muchos autores no habría
   * dejado sitio para ninguna materia.
   */
  async function refrescarEstanterias() {
    cargandoEstanterias = true;
    try {
      const clases: ShelfKind[] = ['author', 'series', 'subject', 'language', 'format', 'year'];
      const texto = busquedaDeEstanteria;
      const respuesta = await guardaDeEstanterias(() =>
        Promise.all(clases.map((kind) => libraryShelves({ kind, search: texto, limit: 20 })))
      );
      // La respuesta de una búsqueda que ya se borró no pinta: si la tercera
      // pulsación contesta antes que la segunda, lo que se queda en pantalla es
      // el resultado de algo que nadie escribió. Regla 3 del contrato de carga.
      if (!respuesta.vigente) return;

      estanterias = clases.map((kind, i) => ({
        kind,
        shelves: respuesta.valor[i]?.shelves ?? [],
        total: respuesta.valor[i]?.total ?? 0
      }));
    } catch (e) {
      estanterias = [];
      avisarDeFallo(e);
    } finally {
      cargandoEstanterias = false;
    }
  }

  /** Abre una estantería: es un filtro de la biblioteca, no otra pantalla. */
  function abrirEstanteria(estanteria: Shelf) {
    estanteriaActiva = estanteria;
    const filtro: BrowseFilter = { kind: estanteria.kind, value: estanteria.key };
    coleccionActiva = null;
    consultaDeBiblioteca = { ...consultaDeBiblioteca, filter: filtro };
    menuAbierto = null;
    destino = 'biblioteca';
  }

  /** Vuelve a la biblioteca entera. */
  function quitarEstanteria() {
    estanteriaActiva = null;
    coleccionActiva = null;
    consultaDeBiblioteca = { ...consultaDeBiblioteca, filter: { kind: 'all' } };
  }

  async function refrescarColecciones() {
    cargandoColecciones = true;
    try {
      colecciones = await coleccionesIpc();
      // Si la que filtraba ya no está —se borró—, la biblioteca vuelve a
      // enseñarlo todo en lugar de filtrar por algo que no existe.
      if (coleccionActiva) {
        const sigue = colecciones.find((c) => c.id === coleccionActiva?.id) ?? null;
        if (sigue) coleccionActiva = sigue;
        else quitarEstanteria();
      }
    } catch (e) {
      avisarDeFallo(e);
    } finally {
      cargandoColecciones = false;
    }
  }

  /** Abre una colección: es un filtro de la biblioteca, igual que una estantería. */
  function abrirColeccion(coleccion: CollectionSummary) {
    estanteriaActiva = null;
    coleccionActiva = coleccion;
    consultaDeBiblioteca = {
      ...consultaDeBiblioteca,
      filter: { kind: 'collection', value: coleccion.id }
    };
    menuAbierto = null;
    destino = 'biblioteca';
  }

  /** La ficha de una obra, que vive en la biblioteca: es el "Detalles" de Inicio. */
  function verFicha(item: BrowseItem) {
    enfocada = item;
    candidatos = [];
    errorEnLinea = null;
    void refrescarPropuestas(item.editionId);
    destino = 'biblioteca';
  }

  /** La biblioteca con lo empezado: el "Ver Todo" de lo que se está leyendo. */
  function verEmpezados() {
    estanteriaActiva = null;
    coleccionActiva = null;
    consultaDeBiblioteca = { ...consultaDeBiblioteca, filter: { kind: 'inProgress' } };
    destino = 'biblioteca';
  }

  async function crearColeccion(nombre: string) {
    try {
      await crearColeccionIpc(nombre);
      sinBus('colecciones');
      notificar.exito(T.avisos.coleccionCreada, nombre);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function renombrarColeccion(id: string, nombre: string) {
    try {
      await renombrarColeccionIpc(id, nombre);
      sinBus('colecciones');
      notificar.exito(T.avisos.coleccionRenombrada, nombre);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function borrarColeccion(id: string) {
    try {
      await borrarColeccionIpc(id);
      enColecciones = enColecciones.filter((c) => c !== id);
      sinBus('colecciones');
      notificar.exito(T.avisos.coleccionBorrada);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /**
   * Pone o quita la obra enfocada de una colección.
   *
   * No se relee nada aquí: el runtime anuncia el cambio, y con él se releen la
   * pertenencia, el recuento y la biblioteca, que si está filtrando por esa
   * colección tiene que dejar de enseñar el libro que se quitó.
   */
  async function cambiarColeccion(coleccion: string, dentro: boolean) {
    const obra = enfocada;
    if (!obra) return;
    const nombre = colecciones.find((c) => c.id === coleccion)?.name ?? '';
    try {
      if (dentro) await anadirAColeccion(coleccion, [obra.editionId]);
      else await quitarDeColeccion(coleccion, obra.editionId);
      sinBus('colecciones');
      notificar.exito(dentro ? T.avisos.anadidoAColeccion : T.avisos.quitadoDeColeccion, nombre);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  // La ficha dice en qué colecciones está la obra que se enfoca. La respuesta de
  // una obra que ya no está enfocada no pinta: sería la pertenencia de otra.
  $effect(() => {
    const edicion = enfocada?.editionId;
    // Y cuando cambian las colecciones, aunque la obra sea la misma.
    void versionDeColecciones;
    if (!edicion) {
      enColecciones = [];
      return;
    }
    coleccionesDeLaEdicion(edicion)
      .then((ids) => {
        if (enfocada?.editionId === edicion) enColecciones = ids;
      })
      .catch(() => (enColecciones = []));
  });

  /**
   * Si el bus respondió al suscribirse.
   *
   * Sin bus la interfaz no se entera de nada que pase por detrás, pero **sí de
   * lo que hace el propio usuario**: cada acción que escribe avisa entonces por su
   * cuenta con `sinBus`. Con bus no lo hace, y es a propósito: si lo hiciera
   * siempre, un anuncio que el runtime dejara de publicar no se notaría en ninguna
   * pantalla que el usuario toque, y el defecto aparecería solo con los cambios
   * que hacen otros.
   */
  let hayBus = false;

  /**
   * Repasar las carpetas referenciadas, como mucho cada dos minutos: cada
   * visita a Inicio o a la Biblioteca recorría las 19.350 carpetas de la
   * biblioteca real, y el runtime ya repasa por su cuenta cada pocos minutos.
   */
  const repasarRaices = espaciar(() => void rescanRoots().catch(() => undefined), 120_000);

  /** Inicio necesita releerse la próxima vez que se vea. */
  let inicioDesfasado = false;

  /**
   * Cuántas veces cambiaron las colecciones. Lo leen las pertenencias de la
   * obra enfocada, que dependen de las colecciones y no de la obra.
   */
  let versionDeColecciones = $state(0);

  /**
   * Relee lo que depende de lo que cambió.
   *
   * **Lo que se ve se relee ya; lo que no se ve, al volver a verse.** Mientras se
   * lee, el progreso se anuncia cada pocos segundos, y releer Inicio entero con
   * cada anuncio sería trabajo para una pantalla que nadie está mirando. Lo que
   * no se admite es que al volver enseñe la cifra de antes: por eso se apunta
   * como desfasada, y se relee al entrar.
   */
  function alCambiar(cambios: Cambios) {
    const { temas } = cambios;

    if (temas.has('progreso') && enfocada && cambios.progresos.has(enfocada.editionId)) {
      // La ficha enseña la barra de la obra enfocada: se mueve con el dato que
      // trae el anuncio, sin pedir nada.
      enfocada = { ...enfocada, progression: cambios.progresos.get(enfocada.editionId) ?? null };
    }

    if (temas.has('obras') || temas.has('progreso') || temas.has('colecciones')) {
      // La biblioteca solo vuelve a pedir si está montada; si no, al montarse ya
      // pide la página entera.
      recargaDeBiblioteca += 1;
      if (destino === 'inicio') void refrescarInicio();
      else inicioDesfasado = true;
    }
    if (temas.has('obras') && destino === 'biblioteca') void refrescarFacetas();
    if (temas.has('colecciones')) {
      versionDeColecciones += 1;
      void refrescarColecciones();
    }
    if (temas.has('extensiones')) void refrescarExtensiones();
  }

  /**
   * Avisa de un cambio propio cuando no hay bus que lo anuncie. Ver `hayBus`.
   */
  function sinBus(tema: Tema) {
    if (hayBus) return;
    alCambiar({ temas: new Set([tema]), ediciones: new Set(), progresos: new Map() });
  }

  /*
   * Dejar de ver el lector con el libro abierto escribe por dónde se iba.
   *
   * El runtime agrupa las escrituras del progreso cada cinco segundos, y la última
   * posición se queda apuntada hasta el siguiente movimiento. Mientras se lee da
   * igual; quien va a la biblioteca a mitad de capítulo vería allí la barra de hace
   * un momento y en el lector la de ahora. Escribirla anuncia el progreso, y con
   * él se mueve la barra en todas partes.
   */
  let estabaEnElLector = false;
  $effect(() => {
    const enElLector = destino === 'lector';
    if (estabaEnElLector && !enElLector) {
      for (const abierta of [sesion, sesionSecundaria]) {
        if (abierta) void flushProgress(abierta.fileId).catch(() => undefined);
      }
    }
    estabaEnElLector = enElLector;
  });

  /*
   * Ajustes enseña dos registros que crecen desde el lector: lo enviado fuera
   * -una consulta en línea, un envío a un proveedor de IA- y lo marcado como
   * incorrecto. Nadie los anuncia porque solo se leen aquí, así que se releen al
   * entrar. Antes los ponía al día, sin que nadie lo hubiera decidido, la
   * recarga completa que se hacía al cerrar un documento.
   */
  $effect(() => {
    if (destino !== 'ajustes') return;
    void refrescarProveedorDeIa();
    void refrescarRed();
  });

  /*
   * Lo que cambió en las carpetas referenciadas se mira al volver a la ventana y
   * al entrar en la biblioteca o en Inicio: es cuando alguien que acaba de
   * copiar un libro con el explorador de archivos espera verlo. El runtime frena
   * los repasos seguidos, así que pedirlo en los dos momentos no cuesta dos.
   */
  $effect(() => {
    if (destino !== 'biblioteca' && destino !== 'inicio') return;
    repasarRaices();
  });

  // Volver a Inicio con algo desfasado lo relee al entrar.
  $effect(() => {
    if (destino !== 'inicio' || !inicioDesfasado) return;
    inicioDesfasado = false;
    void refrescarInicio();
  });

  /**
   * Lo que pinta Inicio, releído con **la misma semilla**: lo que cambia son las
   * cifras y las portadas, no qué filas salen. Barajar Inicio porque se guardó un
   * progreso sería rehacer la pantalla delante de quien la mira.
   */
  // Una relectura en marcha absorbe las que lleguen mientras dura.
  const refrescarInicio = coalescer(releerInicio, 1500);

  async function releerInicio() {
    try {
      // Por lo último que se leyó, no por lo más avanzado: "sigue leyendo" es
      // el libro de anoche, no el que va por el 90 % desde hace un año.
      let empezados = await browseLibrary({
        sort: 'lastRead',
        ascending: false,
        filter: { kind: 'inProgress' },
        offset: 0,
        limit: 12
      });
      if (empezados.total === 0) {
        // Lo recién abierto no llega al 1 % y no cuenta como empezado, pero
        // Inicio decía "todavía no has empezado ningún documento" justo encima de
        // una fila titulada "porque estás leyendo Hamlet". Se ofrece lo último
        // que se abrió.
        const ultimo = await browseLibrary({
          sort: 'lastRead',
          ascending: false,
          filter: { kind: 'all' },
          offset: 0,
          limit: 1
        });
        const abierto = ultimo.items[0];
        if (abierto && abierto.progression === 0) empezados = { total: 1, items: [abierto] };
      }
      enCurso = empezados;
      novedades = await browseLibrary({
        sort: 'addedAt',
        ascending: false,
        filter: { kind: 'all' },
        offset: 0,
        limit: 12
      });
      await cargarFilasDeInicio();
      cifras = await readingStats();
    } catch {
      // Se queda lo que había: una cifra de hace un momento es mejor que una
      // pantalla vacía, y el siguiente anuncio lo vuelve a intentar.
    }
    try {
      materiasDeInicio = (await libraryShelves({ kind: 'subject', search: '', limit: 6 })).shelves;
    } catch {
      // Sin materias, Inicio no pinta esa fila: no es motivo para dejar de pintar el resto.
      materiasDeInicio = [];
    }
  }

  /** Todo lo que Inicio y Biblioteca muestran, en una sola pasada. */
  async function refrescar() {
    await refrescarInicio();
    await refrescarNotas();
    // La rejilla la decide el runtime: la interfaz la pinta, no la elige.
    limitesDelTablero = await limitesDeTablero();
    await refrescarTableros();
    await refrescarColecciones();
    await refrescarVocabulario();
    await refrescarDiccionarios();
    await refrescarExtensiones();
    await refrescarRed();
    await refrescarModelos();
    await refrescarProveedorDeIa();
    await refrescarCredencialDeVoz();
    await refrescarCatalogos();
    await refrescarRaices();
    await refrescarAvanceDelIndice();
    try {
      queSeEnviaAlConsultar = await whatIsSent('lookup');
    } catch {
      // Sin el texto del runtime, el panel usa el suyo. Lo que no hace es
      // ofrecer el permiso sin decir qué sale.
      queSeEnviaAlConsultar = null;
    }
  }

  async function abrir(item: BrowseItem) {
    if (!item.fileId) return;
    // El lector se pide ya: carga mientras el runtime abre el documento, y no después.
    void cargarLector();
    try {
      // Con la división abierta y ya un documento en la izquierda, lo siguiente
      // que se abre va a la derecha: es lo que hace que comparar dos textos sea
      // abrir dos textos y no un procedimiento.
      if (division.activa && sesion) {
        sesionSecundaria = await openDocument(item.fileId);
        division = abrirEnSecundaria(division, item.fileId);
      } else {
        sesion = await openDocument(item.fileId);
        obraAbierta = item;
        // El salto pendiente era de otro documento. Sin soltarlo, abrir un libro
        // después de saltar a un resultado de búsqueda le aplicaba al nuevo la
        // posición del anterior: un PDF se abría en la página 11 con el rótulo
        // "Sección 6" del libro de antes.
        objetivo = null;
      }
      destino = 'lector';
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  function nuevaSemilla(): number {
    return Math.floor(Math.random() * 0x1_0000_0000);
  }

  async function cargarFilasDeInicio() {
    const semilla = semillaDeInicio;
    try {
      const filas = await homeRows(semilla);
      // La respuesta de una visita que ya no es la actual no pinta.
      if (semilla === semillaDeInicio) filasDeInicio = filas;
    } catch {
      // Sin filas, Inicio sigue teniendo la marquesina y lo empezado.
      if (semilla === semillaDeInicio) filasDeInicio = [];
    }
  }

  $effect(() => {
    const enInicio = destino === 'inicio';
    if (estabaEnInicio && !enInicio) {
      semillaDeInicio = nuevaSemilla();
      void cargarFilasDeInicio();
    }
    estabaEnInicio = enInicio;
  });

  /**
   * Recarga las anotaciones del documento abierto.
   *
   * Se pide siempre al runtime en lugar de retocar la lista en memoria: cada
   * llamada reancla, y una anotación puede cambiar de sitio o de aviso porque
   * cambió el documento, no porque el usuario la tocara. Mantener una copia
   * local sería una segunda verdad sobre dónde está cada cosa.
   */
  async function refrescarAnotaciones() {
    if (!sesion) {
      anotaciones = [];
      return;
    }
    try {
      anotaciones = await annotationOutline(sesion.fileId);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function editarAnotacion(id: string, cuerpo: string | null) {
    try {
      // Lo que vuelve es la **sucesora**, con identificador nuevo: la anterior
      // queda dada de baja. Por eso se recarga la lista entera en lugar de
      // sustituir la fila, que dejaría a la vista apuntando a un identificador
      // que ya no está vivo.
      await editAnnotation(id, { body: cuerpo });
      await refrescarAnotaciones();
      notificar.exito(T.avisos.anotacionEditada);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function borrarAnotacion(id: string) {
    try {
      await deleteAnnotation(id);
      await refrescarAnotaciones();
      notificar.exito(T.avisos.anotacionBorrada);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function prepararDiagnostico() {
    diagnosticoGuardado = false;
    try {
      diagnostico = await diagnosticsPreview();
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function guardarElDiagnostico() {
    try {
      const nombre = `nb-books-diagnostico-${new Date().toISOString().slice(0, 10)}.json`;
      diagnosticoGuardado = await guardarDiagnostico(nombre, T.ajustes.guardarDiagnostico);
      if (diagnosticoGuardado) notificar.exito(T.avisos.diagnosticoGuardado);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  let exportandoBiblioteca = $state(false);

  /**
   * Exporta la biblioteca entera a un archivo.
   *
   * Sobre una biblioteca grande tarda segundos, así que el botón dice que está en
   * ello y no se puede pedir dos veces: dos exportaciones a la vez escribirían
   * sobre el mismo archivo a medias.
   */
  async function exportarBiblioteca() {
    if (exportandoBiblioteca) return;
    exportandoBiblioteca = true;
    try {
      const nombre = `nb-books-biblioteca-${new Date().toISOString().slice(0, 10)}.json`;
      const documentos = await guardarBiblioteca(nombre, T.ajustes.exportarBoton);
      if (documentos !== null) notificar.exito(T.avisos.bibliotecaExportada, String(documentos));
    } catch (e) {
      avisarDeFallo(e);
    } finally {
      exportandoBiblioteca = false;
    }
  }

  /**
   * Sincroniza el estado local de la página tras un borrado en Ajustes.
   *
   * El runtime ya avisa por el bus de eventos, pero si había un libro o una nota
   * abiertos y se acaban de vaciar, la página los cierra en el acto para no dejar
   * una sesión apuntando a filas que ya no existen.
   */
  function trasBorrarDatos(alcance: 'historial' | 'anotaciones' | 'biblioteca') {
    if (alcance === 'anotaciones' || alcance === 'biblioteca') {
      notaAbierta = null;
      retroenlaces = [];
      anotaciones = [];
      anotacionesDeLaBiblioteca = [];
      void refrescarNotas();
      void refrescarTableros();
    }
    if (alcance === 'biblioteca') {
      sesion = null;
      objetivo = null;
      enfocada = null;
      vocabulario = [];
      revisionDeRaices = null;
      void refrescarRaices();
      sinBus('colecciones');
    }
    sinBus('obras');
  }

  async function refrescarAnotacionesDeLaBiblioteca() {
    try {
      anotacionesDeLaBiblioteca = await libraryAnnotations(
        claseDeAnotacion ?? undefined,
        limiteAnotaciones
      );
      cuentasDeAnotaciones = await cuentasDeAnotacionesIpc();
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /*
   * Las cuentas de las pestañas se piden al entrar en Estudio, no al abrir cada
   * pestaña: si no, "Anotaciones" no diría cuántas hay hasta pulsarla, y
   * "Tarjetas" y "Conceptos" no lo dirían nunca.
   */
  let cuentasDeTarjetas = $state<StudyCounts | null>(null);
  $effect(() => {
    if (destino !== 'estudio') return;
    void (async () => {
      try {
        cuentasDeAnotaciones = await cuentasDeAnotacionesIpc();
        cuentasDeTarjetas = await cuentasDeEstudio();
        // Notas y tableros también se releen al entrar: pueden haber cambiado
        // fuera de Estudio (desde el lector o desde una extensión), y lo que se
        // ve tiene que ser lo que hay sin reiniciar.
        await refrescarNotas();
        await refrescarTableros();
      } catch (e) {
        avisarDeFallo(e);
      }
    })();
  });

  // Se piden al entrar en Estudio y al cambiar el filtro, no al arrancar: en una
  // biblioteca con miles de anotaciones es una consulta que no hace falta hasta
  // que alguien va a mirarlas.
  $effect(() => {
    if (destino !== 'estudio' || vistaDeEstudio !== 'anotaciones') return;
    void claseDeAnotacion;
    void refrescarAnotacionesDeLaBiblioteca();
  });

  /**
   * Abre el documento de una anotación y lleva la lectura a su posición.
   *
   * El locator viaja entero, con sus selectores: el reanclaje se resuelve **al
   * abrir**, que es cuando hay documento contra el que resolverlo. Por eso la
   * lista de Estudio no promete una posición y esta función sí la alcanza.
   */
  async function saltarAAnotacion(anotacion: LibraryAnnotation) {
    if (!anotacion.fileId) return;
    void cargarLector();
    try {
      sesion = await openDocument(anotacion.fileId);
      objetivo = anotacion.annotation.target;
      destino = 'lector';
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /**
   * Abre lo que un resultado de búsqueda señala, y si trae posición, va a ella.
   *
   * # Por qué hace falta, y qué pasaba antes
   *
   * **Antes pulsar un resultado de búsqueda no abría nada.** El
   * resultado de catálogo llegaba aquí con su edición y esta función no
   * existía: el manejador ponía `destino = 'biblioteca'` y descartaba el
   * identificador con un `void`. Ir a la biblioteca sin llevar el libro es, para
   * quien pulsa, exactamente lo mismo que no hacer nada.
   *
   * Y el salto de texto tenía el defecto hermano: ponía el objetivo sin abrir el
   * documento, lo que solo funciona si ya está abierto **y es el mismo**. Con el
   * alcance de corpus los resultados vienen de toda la biblioteca, así que
   * saltaba en el libro equivocado o en ninguno.
   *
   * Los dos salen del mismo hueco: el resultado no traía su archivo. Ahora lo
   * trae, igual que ya lo traían una anotación, una palabra del vocabulario y
   * una recomendación, y este es el mismo camino que aquellas: abrir el
   * documento y, si hay locator, poner el objetivo.
   *
   * Sin archivo disponible no se hace nada: el botón no se ofrece, así que
   * llegar aquí sin él sería un defecto de la vista.
   */
  async function abrirResultado(fileId: string | null, locator?: Locator) {
    if (!fileId) return;
    void cargarLector();
    try {
      sesion = await openDocument(fileId);
      objetivo = locator ?? null;
      destino = 'lector';
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /**
   * Salta al punto exacto donde apareció una palabra.
   *
   * Es el mismo camino que el salto desde una anotación, y a propósito: los dos
   * abren el documento y ponen el objetivo, porque los dos anclan con un
   * Locator. Que sean el mismo camino es lo que hace que un cambio de tipografía
   * los afecte igual.
   */
  async function saltarAPalabra(palabra: VocabularyItem) {
    if (!palabra.fileId) return;
    void cargarLector();
    try {
      sesion = await openDocument(palabra.fileId);
      objetivo = palabra.entry.locator;
      destino = 'lector';
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /** Abre el libro en la fuente de algo guardado en Estudio. */
  async function abrirFuenteDeEstudio(fileId: string, locator: Locator) {
    void cargarLector();
    try {
      sesion = await openDocument(fileId);
      objetivo = locator;
      destino = 'lector';
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function refrescarVocabulario() {
    try {
      vocabulario = await listVocabulary(limiteVocabulario);
    } catch {
      // Sin vocabulario, Estudio enseña la lista vacía con su explicación. No es
      // motivo para dejar de pintar el resto de la vista.
      vocabulario = [];
    }
  }

  /**
   * Si cabe guardar una clave y si el servidor configurado ya tiene una.
   *
   * Se vuelve a preguntar cada vez que cambia el servidor, porque la respuesta
   * depende de cuál sea: la clave está guardada **para un anfitrión**, así que
   * apuntar a otro sitio deja de tenerla, que es exactamente lo que se quiere.
   */
  async function refrescarCredencialDeVoz() {
    try {
      puntoDeVoz = await speechEndpoint();
    } catch {
      puntoDeVoz = null;
    }
    try {
      credencialDeVoz = await speechCredentialStatus();
    } catch {
      // Sin respuesta, Ajustes no ofrece guardar. Es el lado seguro: mejor no
      // ofrecer un campo que no sabemos si puede cumplir.
      credencialDeVoz = null;
    }
  }

  async function refrescarProveedorDeIa() {
    try {
      ia.proveedor = await aiProviderStatus();
      ia.envios = await aiSends(50);
      ia.reportes = await aiReports(50);
    } catch {
      // Sin estado, la sección no ofrece probar: es el lado seguro.
      ia.proveedor = null;
      ia.envios = [];
      ia.reportes = [];
    }
  }

  /**
   * Lo que se pide sobre el proveedor externo. Probar la primera vez **no envía
   * nada**: el runtime devuelve el mensaje que enviaría, y Ajustes lo enseña para
   * decidir. Autorizar se hace con ese mensaje delante, y entonces sale.
   */
  async function accionDeIa(accion: AccionDeIa, secreto = '') {
    const aviso = {
      guardarClave: T.avisos.credencialGuardada,
      borrarClave: T.avisos.credencialBorrada,
      borrarEnvios: T.avisos.enviosDeIaBorrados,
      borrarReportes: T.avisos.reportesDeIaBorrados
    }[accion as string];
    ia.prueba = null;
    if (accion === 'descartarPrueba') return;
    try {
      if (accion === 'autorizarYProbar') await allowAiProvider().then(refrescarRed);
      if (accion === 'guardarClave') await setAiCredential(secreto);
      if (accion === 'borrarClave') await clearAiCredential();
      if (accion === 'borrarEnvios') await clearAiSends();
      if (accion === 'borrarReportes') await clearAiReports();
      if (accion === 'exportarReportes') {
        const nombre = `nb-books-avisos-de-ia-${new Date().toISOString().slice(0, 10)}.json`;
        if (await guardarReportesDeIa(nombre, T.ajustes.guardarAvisosDeIa)) {
          notificar.exito(T.avisos.reportesDeIaGuardados);
        }
      }
      if (accion === 'probar' || accion === 'autorizarYProbar') {
        ia.probando = true;
        ia.prueba = await probeAiProvider();
      }
      await refrescarProveedorDeIa();
      if (aviso) notificar.exito(aviso);
    } catch (e) {
      avisarDeFallo(e);
    } finally {
      ia.probando = false;
    }
  }

  /** Quita un aviso suelto de los marcados como incorrectos. */
  async function quitarReporteDeIa(id: number) {
    try {
      await forgetAiReport(id);
      await refrescarProveedorDeIa();
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function refrescarModelos() {
    try {
      modelosDeIa = await aiModels();
    } catch {
      // Sin la lista, la sección enseña su explicación y no ofrece descargar.
      modelosDeIa = null;
    }
  }

  /**
   * Qué hacer cuando una descarga de modelo termina, sea como sea.
   *
   * Está aparte del registro del oyente porque son cinco desenlaces y el registro
   * ya tenía los suyos: juntos pasaban de lo que el proyecto admite en una sola
   * función, y una función que hace cinco cosas se lee peor que cinco líneas.
   */
  function alTerminarUnModelo({
    modelId,
    outcome,
    host,
    reasonCode
  }: {
    modelId: string;
    outcome: string;
    host: string | null;
    reasonCode: string | null;
  }) {
    avanceDeModelo = null;
    if (outcome === 'installed') notificar.exito(T.avisos.modeloInstalado);
    if (outcome === 'cancelled') notificar.informacion(T.avisos.modeloCancelado);
    if (outcome === 'needsConsent' && host) {
      modeloPideAutorizar = host;
      modeloQueEspera = modelId;
    }
    if (outcome === 'failed') falloDeModelo = reasonCode ?? 'INTERNAL';
    void refrescarModelos();
  }

  /**
   * Autoriza el anfitrión del catálogo y empieza a descargar.
   *
   * Se llega aquí **desde la confirmación**, que ya enseñó la licencia, de quién es
   * el modelo, a dónde se sale y qué se envía. Autorizar y descargar van juntos
   * porque son la misma decisión: nadie autoriza un anfitrión de modelos para otra
   * cosa que descargar uno.
   */
  async function descargarModelo(id: string, host: string) {
    falloDeModelo = null;
    modeloPideAutorizar = null;
    try {
      await allowModelHost(host);
      await installAiModel(id);
      await refrescarModelos();
      await refrescarRed();
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /** Autoriza la red de distribución que pidió la descarga, y la sigue. */
  async function autorizarModeloYSeguir(host: string) {
    const id = modeloQueEspera;
    modeloPideAutorizar = null;
    modeloQueEspera = null;
    if (!id) return;
    try {
      await allowModelHost(host);
      await installAiModel(id);
      await refrescarModelos();
      await refrescarRed();
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function quitarModelo(id: string) {
    falloDeModelo = null;
    try {
      await uninstallAiModel(id);
      await refrescarModelos();
      notificar.exito(T.avisos.modeloQuitado);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function refrescarRed() {
    try {
      red = await networkReport();
    } catch {
      // Sin informe, Ajustes enseña las listas vacías con su explicación. No es
      // motivo para dejar de pintar el resto.
      red = null;
    }
  }

  /**
   * Autoriza un destino que un proveedor de consulta pidió.
   *
   * El anfitrión es **el que vino en el omitido**: quien decide a dónde sale la
   * aplicación es el runtime, y al conceder comprueba que algún proveedor lo
   * reconoce como suyo.
   */
  async function autorizarConsulta(host: string) {
    try {
      await allowLookupHost(host);
      await refrescarRed();
      notificar.exito(T.avisos.permisoConcedido, host);
      // Se vuelve a consultar la misma palabra: sin esto, el permiso quedaría
      // concedido y el panel seguiría enseñando el hueco hasta que alguien
      // consultara otra cosa.
      if (consulta) palabraPedida = consulta.termino;
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function refrescarPropuestas(editionId: string) {
    try {
      propuestas = await pendingSuggestions(editionId);
    } catch {
      propuestas = [];
    }
  }

  /** Busca en el catálogo abierto candidatos para la obra enfocada. */
  async function buscarEnLinea() {
    if (!enfocada) return;
    buscandoEnLinea = true;
    errorEnLinea = null;
    candidatos = [];
    try {
      candidatos = await searchOnlineMetadata(enfocada.editionId);
    } catch (e) {
      // El código, no el mensaje: la ficha distingue "falta autorizar" de
      // "modo sin red" de "no respondió", y son tres textos con tres acciones.
      errorEnLinea = e instanceof Error && 'code' in e ? String(e.code) : 'NETWORK_FAILURE';
    } finally {
      buscandoEnLinea = false;
    }
  }

  /**
   * Aplica un candidato y, si trae portada, la descarga.
   *
   * La portada va aparte porque es **otro anfitrión y otro permiso**: pedir un
   * dato de texto y descargar una imagen no son la misma decisión, así que fallar
   * al traer la imagen no puede deshacer los metadatos que sí entraron.
   */
  async function aplicarCandidato(candidato: MetadataCandidate) {
    if (!enfocada) return;
    const editionId = enfocada.editionId;
    try {
      await applyOnlineMetadata(editionId, candidato);
      if (candidato.coverUrl) {
        try {
          await fetchOnlineCover(editionId, candidato.coverUrl);
        } catch {
          // Sin portada, la ficha se queda con la que tenía. No se avisa con un
          // error porque los metadatos sí entraron.
        }
      }
      candidatos = [];
      await refrescarPropuestas(editionId);
      // La biblioteca, Inicio y la ficha se releen con el anuncio del runtime.
      sinBus('obras');
      notificar.exito(T.avisos.fichaCompletada);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function resolverPropuesta(id: string, aceptar: boolean) {
    const editionId = enfocada?.editionId;
    try {
      await (aceptar ? acceptSuggestion(id) : rejectSuggestion(id));
      if (editionId) await refrescarPropuestas(editionId);
      if (aceptar) sinBus('obras');
      notificar.exito(aceptar ? T.avisos.propuestaAceptada : T.avisos.propuestaRechazada);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /**
   * Guarda las anotaciones del documento abierto donde el usuario diga.
   *
   * El nombre por defecto sale del título de la sesión, no del identificador:
   * quien guarda un archivo lo va a buscar después por su nombre.
   */
  async function exportarAnotaciones(formato?: 'markdown' | 'text') {
    if (!sesion) return;
    const extension = formato === 'markdown' ? 'md' : formato === 'text' ? 'txt' : 'json';
    try {
      const guardado = await guardarAnotaciones(
        sesion.editionId,
        `${sesion.title}.anotaciones.${extension}`,
        T.anotaciones.exportar,
        formato,
        idiomaActual()
      );
      if (guardado) notificar.exito(T.avisos.anotacionesGuardadas);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function crearTablero(titulo: string, columnas: string[]) {
    try {
      const tablero = await crearTableroIpc(titulo, columnas);
      await refrescarTableros();
      await abrirTablero(tablero.id);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function borrarTablero(id: string) {
    try {
      await borrarTableroIpc(id);
      await refrescarTableros();
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /**
   * Todo lo que cambia un tablero devuelve sus columnas ya actualizadas.
   *
   * Así la interfaz no tiene que recomponer el tablero por su cuenta: una
   * segunda fuente de verdad sobre dónde está cada nota es exactamente lo que
   * acaba enseñando una cosa distinta de lo que hay guardado.
   */
  async function conElTablero(accion: (id: string) => Promise<BoardColumn[]>) {
    if (!tableroActivo) return;
    try {
      columnasDelTablero = await accion(tableroActivo);
      await refrescarTableros();
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /**
   * Revisa por lotes los metadatos de lo que se está viendo.
   *
   * No escribe nada: lo que la extensión proponga queda como propuesta, y se
   * dice cuántas han quedado. Enseñar el número importa: "hecho" sobre mil
   * documentos no dice si hay algo que mirar.
   */
  async function revisarMetadatos(extension: string, ediciones: string[]) {
    if (ediciones.length === 0) return;
    try {
      const resumen = await revisarMetadatosPorLotes(extension, ediciones);
      notificar.exito(T.avisos.metadatosRevisados, String(resumen.recorded));
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /**
   * Guarda las citas del documento abierto en el formato que se elija.
   *
   * El nombre por defecto lleva la extensión que declara el formato: quien
   * exporta a BibTeX espera un `.bib`, y un gestor bibliográfico decide por la
   * extensión qué analizador usar.
   */
  async function exportarCitas(formato: string) {
    if (!sesion) return;
    const elegido = formatosDeCita.find((f) => f.id === formato);
    if (!elegido) return;
    try {
      const guardado = await guardarCitas(
        sesion.editionId,
        formato,
        `${sesion.title}.${elegido.fileExtension}`,
        T.anotaciones.exportar
      );
      if (guardado) notificar.exito(T.avisos.anotacionesGuardadas);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /**
   * Cómo está el índice de búsqueda del documento abierto.
   *
   * Se consulta al abrir y al pedir que se prepare, no en un temporizador: el
   * estado solo cambia por esas dos cosas, y sondear tendría a la aplicación
   * preguntando por algo que casi siempre es lo mismo.
   */
  let estadoDelIndice = $state<SearchStatus | null>(null);

  /**
   * Versiones anteriores de una anotación.
   *
   * Se piden al pulsar y no al seleccionar: son otra consulta, y la mayoría de
   * las anotaciones nunca se editaron. La lista se vacía al cambiar de anotación
   * para que no se enseñe el historial de la anterior.
   */
  let historialDeAnotacion = $state<Annotation[]>([]);

  /**
   * Los pasajes que cita la nota abierta, y las notas que citan la anotación
   * abierta.
   *
   * Son los dos lados del mismo enlace, y se piden por separado porque en
   * Estudio se mira uno u otro: quien tiene una nota abierta no está mirando una
   * anotación.
   */
  let pasajesCitados = $state<LibraryAnnotation[]>([]);
  let notasQueCitanLaAnotacion = $state<Note[]>([]);

  async function refrescarPasajes() {
    const nota = notaAbierta;
    if (!nota) {
      pasajesCitados = [];
      return;
    }
    try {
      pasajesCitados = await citedAnnotations(nota.id);
    } catch {
      pasajesCitados = [];
    }
  }

  async function refrescarNotasQueCitan() {
    const anotacion = anotacionAbiertaEnEstudio;
    if (!anotacion) {
      notasQueCitanLaAnotacion = [];
      return;
    }
    try {
      notasQueCitanLaAnotacion = await notesCiting(anotacion.annotation.id);
    } catch {
      notasQueCitanLaAnotacion = [];
    }
  }

  /**
   * Cita un pasaje en una nota.
   *
   * Se refrescan los dos lados: quien acaba de citar puede estar mirando
   * cualquiera de ellos, y uno que no se entera de lo que acaba de hacer parece
   * que no lo hizo.
   */
  async function citarPasaje(noteId: string, annotationId: string) {
    try {
      await citeAnnotation(noteId, annotationId);
      await refrescarNotasQueCitan();
      await refrescarPasajes();
      notificar.exito(T.avisos.pasajeCitado);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function quitarPasaje(noteId: string, annotationId: string) {
    try {
      await unciteAnnotation(noteId, annotationId);
      await refrescarPasajes();
      await refrescarNotasQueCitan();
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  $effect(() => {
    void notaAbierta?.id;
    void refrescarPasajes();
  });

  $effect(() => {
    void anotacionAbiertaEnEstudio?.annotation.id;
    void refrescarNotasQueCitan();
  });

  async function verHistorial(id: string) {
    try {
      historialDeAnotacion = await annotationHistory(id);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  $effect(() => {
    void anotacionAbiertaEnEstudio?.annotation.id;
    historialDeAnotacion = [];
  });

  async function refrescarEstadoDelIndice() {
    if (!sesion) {
      estadoDelIndice = null;
      return;
    }
    try {
      estadoDelIndice = await searchStatus(sesion.editionId);
    } catch {
      // Sin estado, el panel no avisa de nada, que es como estaba antes: no
      // poder decir cómo va el índice no es motivo para no dejar buscar.
      estadoDelIndice = null;
    }
  }

  /**
   * Cómo va la construcción del índice de toda la biblioteca.
   *
   * Es un dato de la biblioteca entera, no del documento abierto: por eso vive
   * en Ajustes y no en el panel de búsqueda, donde lo que importa es si **este**
   * libro se puede buscar ya.
   */
  let avanceDelIndice = $state<IndexingReport | null>(null);
  let indicePausado = $state(false);

  async function refrescarAvanceDelIndice() {
    try {
      avanceDelIndice = await indexingProgress();
    } catch {
      avanceDelIndice = null;
    }
  }

  /**
   * Rehace el índice de búsqueda entero.
   *
   * No hay nada que perder: el índice es caché y los documentos siguen donde
   * estaban. Lo que sí hay es tiempo, y por eso lo pide el usuario y no ocurre
   * solo: sobre una biblioteca de ciento cincuenta mil documentos son horas.
   */
  async function rehacerIndice() {
    try {
      const encoladas = await reindex();
      notificar.exito(T.avisos.indiceEncolado, `${encoladas}`);
      await refrescarAvanceDelIndice();
      await refrescarEstadoDelIndice();
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /** Pausa o reanuda la indexación. Pausar no pierde lo ya indexado. */
  async function pausarIndice(pausado: boolean) {
    try {
      await setIndexingPaused(pausado);
      indicePausado = pausado;
      await refrescarAvanceDelIndice();
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function refrescarRaices() {
    try {
      raices = await libraryRoots();
    } catch {
      // Sin raíces que enseñar, la sección dice que no hay ninguna. No es un
      // fallo que merezca un aviso: los ajustes siguen sirviendo para el resto.
      raices = [];
    }
  }

  /**
   * Revisa dónde está cada archivo y enseña el recuento.
   *
   * Se pide, no se hace sola al abrir los ajustes: sobre una biblioteca de
   * ciento cincuenta mil archivos es trabajo de disco, y quien entra a cambiar
   * el tema no lo ha pedido.
   */
  async function revisarRaices() {
    revisandoRaices = true;
    try {
      revisionDeRaices = await refreshAvailability();
      await refrescarRaices();
      recargaDeBiblioteca += 1;
    } catch (e) {
      avisarDeFallo(e);
    } finally {
      revisandoRaices = false;
    }
  }

  /**
   * Reapunta una raíz a donde estén ahora sus archivos.
   *
   * La carpeta la elige el usuario en el diálogo del sistema, igual que al
   * importar: esta capa nunca escribe una ruta que no venga de un gesto suyo.
   */
  async function reapuntarRaiz(rootId: string) {
    const carpeta = await elegirCarpetaDeRaiz();
    if (!carpeta) return;
    revisandoRaices = true;
    try {
      revisionDeRaices = await relocateLibraryRoot(rootId, carpeta);
      await refrescarRaices();
      // La biblioteca vuelve a pedir su página: las obras que estaban ausentes
      // ya no lo están, y la rejilla seguiría enseñándolas como perdidas.
      recargaDeBiblioteca += 1;
      notificar.exito(T.avisos.raizReapuntada, `${revisionDeRaices.available}`);
    } catch (e) {
      avisarDeFallo(e);
    } finally {
      revisandoRaices = false;
    }
  }

  async function refrescarCatalogos() {
    try {
      catalogos = await listCatalogs();
    } catch {
      catalogos = [];
    }
  }

  /**
   * Autoriza la salida hacia el anfitrión de un catálogo.
   *
   * El anfitrión sale de la dirección que el propio catálogo ya tenía anotada,
   * no de nada que se escriba aquí. Añadir un catálogo no autoriza nada: esta
   * es la única puerta, y la abre el usuario.
   */
  async function autorizarCatalogo(host: string, cleartext = false) {
    try {
      await allowCatalogHost(host, cleartext);
      await refrescarRed();
      notificar.exito(T.avisos.permisoConcedido, host);
      // Se reabre el mismo catálogo: sin esto el permiso quedaría concedido y la
      // vista seguiría enseñando el aviso hasta que alguien eligiera otro.
      if (catalogoAbierto) void abrirFeed(catalogoAbierto);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /*
   * Autorizar **desde Ajustes** también reabre lo que Descubrir tenía parado por
   * ese permiso. Era la otra mitad de "funciona raro": se concedía fuera, se
   * volvía a Descubrir y seguía el aviso, porque nadie le decía a la vista que ya
   * no hacía falta.
   */
  $effect(() => {
    const falta = permisoQueFalta;
    const permisos = red?.consents ?? [];
    if (!falta || !catalogoAbierto) return;
    const proposito = falta.cleartext ? 'catalog-cleartext' : 'catalog';
    if (permisos.some((p) => p.host === falta.host && p.purpose === proposito)) {
      void abrirFeed(catalogoAbierto);
    }
  });

  /** Abre un catálogo desde su raíz: empieza un camino nuevo. */
  async function abrirCatalogo(url: string) {
    rutaDelCatalogo = [url];
    await abrirFeed(url);
  }

  /** Entra en una sección del catálogo abierto, apuntando de dónde se viene. */
  async function navegarEnCatalogo(url: string) {
    rutaDelCatalogo = [...rutaDelCatalogo, url];
    await abrirFeed(url);
  }

  /** Vuelve al feed anterior. Sin sitio al que volver no hace nada. */
  async function volverEnCatalogo() {
    if (rutaDelCatalogo.length < 2) return;
    const camino = rutaDelCatalogo.slice(0, -1);
    rutaDelCatalogo = camino;
    await abrirFeed(camino[camino.length - 1] ?? '');
  }

  /** Abre un feed. La dirección la propone el catálogo, nunca esta capa. */
  async function abrirFeed(url: string) {
    cargandoCatalogo = true;
    errorDeCatalogo = null;
    permisoQueFalta = null;
    ultimaDescarga = null;
    // Cuál se intentó abrir se apunta antes de saber si se pudo. Apuntarlo solo
    // al acertar dejaba sin dirección precisamente el caso que la necesita: el
    // de "falta autorizar", donde hay que decir **a qué** se autoriza.
    catalogoAbierto = url;
    try {
      const pagina = await browseCatalog(url);
      if (pagina.estado === 'feed') {
        feed = pagina.detalle;
      } else {
        feed = null;
        if (pagina.estado === 'needsConsent') {
          permisoQueFalta = pagina.detalle;
          errorDeCatalogo = 'CONSENT_REQUIRED';
        } else {
          errorDeCatalogo = 'NEEDS_LOGIN';
        }
      }
    } catch (e) {
      feed = null;
      // El código, no el mensaje: inalcanzable, sin permiso y corrupto piden
      // tres acciones distintas, y con un solo texto se ofrecería la que no es.
      errorDeCatalogo = e instanceof Error && 'code' in e ? String(e.code) : 'INTERNAL';
    } finally {
      cargandoCatalogo = false;
    }
  }

  /**
   * Descarga una entrada del catálogo.
   *
   * Lo que se descarga **no es distinto** de un archivo que el usuario suelta
   * sobre la ventana: entra por la misma canalización, que reconoce el formato
   * por contenido. Por eso un rechazo se enseña como rechazo y no como error.
   */
  async function descargarDelCatalogo(entrada: FeedEntry) {
    if (!entrada.download) return;
    descargando = true;
    ultimaDescarga = null;
    try {
      const resultado = await downloadFromCatalog(entrada.download);
      if (resultado.estado === 'needsConsent') {
        // Las descargas viven a menudo en otro servidor que el feed: se dice cuál
        // falta, igual que al abrir un catálogo.
        permisoQueFalta = resultado.detalle;
        errorDeCatalogo = 'CONSENT_REQUIRED';
        return;
      }
      const informe = resultado.detalle;
      ultimaDescarga = {
        titulo: entrada.title,
        estado:
          informe.imported > 0 ? 'ok' : informe.alreadyInLibrary > 0 ? 'repetido' : 'rechazado'
      };
      // Lo que entró llega por el anuncio de la importación, como cualquier otra.
      if (informe.imported > 0) sinBus('obras');
      // Lo que pasó se dice en la esquina además de en la fila del catálogo: el
      // usuario acaba de pulsar y puede estar mirando a otra parte de la lista.
      if (informe.imported > 0) notificar.exito(T.avisos.descargaHecha, entrada.title);
      else if (informe.alreadyInLibrary > 0)
        notificar.informacion(T.avisos.descargaRepetida, entrada.title);
    } catch (e) {
      avisarDeFallo(e);
    } finally {
      descargando = false;
    }
  }

  async function anadirCatalogo(titulo: string, url: string) {
    try {
      await addCatalog(titulo, url);
      await refrescarCatalogos();
      notificar.exito(T.avisos.catalogoAnadido, titulo);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function quitarCatalogo(id: string) {
    try {
      await removeCatalog(id);
      await refrescarCatalogos();
      notificar.exito(T.avisos.catalogoQuitado);
      // El feed abierto puede ser el del catálogo que se acaba de quitar:
      // dejarlo a la vista sería enseñar algo que ya no está configurado.
      feed = null;
      catalogoAbierto = null;
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function refrescarDiccionarios() {
    try {
      diccionarios = await listDictionaries();
    } catch {
      diccionarios = [];
    }
  }

  async function ejecutarOrdenDeExtensionEn(extension: string, orden: string) {
    try {
      // Pulsar es lo que carga su código: hasta ahora el botón estaba en la
      // ficha sin que existiera una sola instrucción compilada de la extensión.
      const dicho = await ejecutarOrdenDeExtension(extension, orden);
      if (dicho) notificar.exito(dicho);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  /**
   * Compone la vista de una extensión al abrir su pestaña.
   *
   * Se pide al cambiar y no al arrancar: hasta que alguien la abre, la pestaña
   * existe y el código de la extensión no.
   */
  async function abrirVistaDeExtension(id: string) {
    const cual = vistasDeExtension.find((v) => v.id === id);
    if (!cual) {
      vistaDeExtension = null;
      return;
    }
    try {
      vistaDeExtension = await componerVistaDeExtension(cual.extension, cual.id);
    } catch (e) {
      // Una vista que no se puede componer no puede dejar Estudio en blanco: se
      // avisa y se vuelve a las nativas.
      vistaDeExtension = null;
      vistaDeEstudio = 'notas';
      avisarDeFallo(e);
    }
  }

  /**
   * Pone o quita un tema de extensión.
   *
   * Si sus colores no llegan al contraste que exige el sistema de diseño **no
   * se aplica nada** y se dice cuál falla. Aplicarlo a medias dejaría texto del
   * tema nuevo sobre fondos del viejo, que es justo el tramo ilegible que
   * `app.css` describe.
   */
  function elegirTemaDeExtension(extension: string | null) {
    if (!extension) {
      temaDeExtensionActivo = null;
      void guardarPreferencia('extension_theme', '');
      return;
    }

    const elegido = temasDeExtension.find((t) => t.extension === extension);
    if (!elegido) return;

    /*
     * El contraste se comprueba **antes** de dejar que se ponga: un tema que no
     * llega al umbral no se aplica a medias ni se aplica y se retira, que sería un
     * destello ilegible. Lo demás -los tokens, el esquema y la paleta entera- lo
     * pinta `aplicarApariencia` desde la preferencia, que es la única que pinta.
     */
    const fallos = fallosDeContraste(elegido.tokens);
    if (fallos.length > 0) {
      notificar.error(T.ajustes.temaSinContraste);
      return;
    }

    temaDeExtensionActivo = extension;
    // Elegirlo se guarda. Si no, se olvidaría al cerrar y habría que volver a
    // ponerlo cada vez; y lo que se guarda es **quién lo aporta**, no sus
    // colores, que son suyos y cambian cuando se actualiza. Al desinstalarla,
    // el runtime vacía esta preferencia solo.
    void guardarPreferencia('extension_theme', extension);
  }

  async function refrescarExtensiones() {
    extensiones = await extensionesInstaladas();
    // La versión de la API se pide con el resto: es lo que decide si Ajustes
    // enseña que todavía no es estable, y eso tiene que estar antes de que se
    // pinte la lista de lo instalado.
    try {
      apiDeExtensiones = await versionDeApiDeExtensiones();
    } catch {
      apiDeExtensiones = null;
    }
    ordenesDeExtension = await ordenesDeExtension_('library/document');
    vistasDeExtension = await vistasDeExtensionIpc();
    temasDeExtension = await temasDeExtensionIpc();
    try {
      generacionesDeExtensiones = await generacionesDeExtensionesIpc();
    } catch {
      generacionesDeExtensiones = [];
    }
    retiradasDeExtensiones = await retiradasDeExtensionesIpc();
    fondosSonoros = await fondosSonorosIpc();
    formatosDeCita = await formatosDeCitaIpc();
    // Si el paquete que lo aportaba ya no está, lo que sonaba se para: seguir
    // sonando sin nada en la lista que lo pare sería un ruido del que no se
    // puede salir.
    if (fondoSonando && !fondosSonoros.some((f) => f.id === fondoSonando)) {
      pararFondo();
    }
    const { modos, descartados } = legibles(await modosDeExtensionIpc());
    modosDeExtension = modos;
    // Un modo que no se lee no se ofrece, y se dice cuál: callarlo dejaría a su
    // autor sin saber por qué el suyo no aparece.
    for (const malo of descartados) {
      notificar.error(T.ajustes.modoSinContraste, malo.modo);
    }

    /*
     * Y cuál está puesto sale de la preferencia, que es la que pinta: aquí solo se
     * pone al día lo que Ajustes marca. Si la extensión ya no está -se desinstaló,
     * o se volvió a una generación anterior-, no queda marcado ninguno y el núcleo
     * pinta lo suyo, que es lo que el usuario tenía antes.
     */
    const guardado = texto('extension_theme');
    temaDeExtensionActivo = temasDeExtension.some((t) => t.extension === guardado)
      ? guardado
      : null;
  }

  /**
   * Pregunta al registro qué ha retirado.
   *
   * Se refresca la lista entera después, y no solo los avisos: lo que resulte
   * malicioso se apaga, así que las filas de al lado cambian de estado y
   * enseñarlas encendidas sería mentir sobre lo que está corriendo.
   */
  async function comprobarRetiradas() {
    try {
      const avisos = await comprobarRetiradasDeExtensiones();
      sinBus('extensiones');
      if (avisos.length === 0) {
        notificar.exito(T.ajustes.retiradasSinNovedad);
      } else {
        notificar.error(T.ajustes.extensionRetirada, avisos[0]?.extension ?? '');
      }
    } catch (error) {
      // Que no haya registro configurado no es un fallo del que avisar como
      // tal: es el estado de hoy, y el texto lo dice sin alarmar.
      notificar.error(
        codigoDeError(error) === 'COMPONENT_UNAVAILABLE'
          ? T.ajustes.sinRegistroConfigurado
          : mensajeDeError(error)
      );
    }
  }

  /**
   * Aplica el resultado de intentar instalar.
   *
   * Que pida consentimiento **no es un fallo**: es el sistema funcionando, y
   * por eso no se avisa como error sino que se enseña qué falta por aprobar.
   */
  function aplicarInstalacion(resultado: ExtensionInstall, ruta: string) {
    if (resultado.estado === 'needsConsent') {
      pendienteExtension = resultado.detalle;
      rutaPendienteExtension = ruta;
      return;
    }
    pendienteExtension = null;
    rutaPendienteExtension = null;
    /*
     * Un paquete incompleto **no es un manifiesto mal escrito**, y hasta el
     * 2026-09-18 se decía así: quien instalaba desde la carpeta de fuentes de una
     * extensión recibía "el manifiesto no es válido" y se iba a revisar un
     * archivo que estaba bien. Ahora se dice qué módulo falta y dónde está el
     * paquete que sí lo trae.
     */
    if (resultado.estado === 'incomplete') {
      notificar.error(T.ajustes.extensionPaqueteIncompleto, resultado.detalle.modules.join(', '));
      return;
    }
    notificar.exito(T.ajustes.extensionInstalada);
  }

  async function instalarExtension() {
    try {
      const ruta = await elegirExtension();
      // Cancelar no es un error: cambiar de opinión es una respuesta.
      if (!ruta) return;
      aplicarInstalacion(await instalarExtensionIpc(ruta), ruta);
      sinBus('extensiones');
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function concederAPendiente(anfitrion: string) {
    const ruta = rutaPendienteExtension;
    if (!ruta) return;
    try {
      await concederAExtension(ruta, anfitrion);
      // Se reintenta sola: quien acaba de conceder lo último que falta no tiene
      // por qué volver a buscar la carpeta.
      aplicarInstalacion(await instalarExtensionIpc(ruta), ruta);
      sinBus('extensiones');
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function habilitarExtensionEn(id: string, activa: boolean) {
    try {
      extensiones = await habilitarExtension(id, activa);
      sinBus('extensiones');
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function desinstalarExtensionEn(id: string) {
    try {
      extensiones = await desinstalarExtension(id);
      sinBus('extensiones');
      // Desinstalar no pide confirmación porque se deshace: se dice dónde.
      notificar.exito(T.avisos.extensionDesinstalada);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function revocarAExtensionEn(id: string, anfitrion: string) {
    try {
      await revocarAExtension(id, anfitrion);
      sinBus('extensiones');
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function volverAtrasExtensionesEn() {
    try {
      extensiones = await volverAtrasExtensiones();
      sinBus('extensiones');
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function instalarDiccionario() {
    try {
      const ruta = await elegirDiccionario();
      // Cancelar no es un error: cambiar de opinión es una respuesta.
      if (!ruta) return;
      await installDictionary(ruta, null);
      await refrescarDiccionarios();
      notificar.exito(T.avisos.diccionarioInstalado);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function desinstalarDiccionario(id: string) {
    try {
      await uninstallDictionary(id);
      await refrescarDiccionarios();
      notificar.exito(T.avisos.diccionarioQuitado);
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  async function borrarAnotacionDeLaBiblioteca(id: string) {
    try {
      await deleteAnnotation(id);
      await refrescarAnotacionesDeLaBiblioteca();
      notificar.exito(T.avisos.anotacionBorrada);
      // La misma anotación puede estar en el panel del documento abierto: si no
      // se refresca, quedaría a la vista una que ya no existe.
      await refrescarAnotaciones();
    } catch (e) {
      avisarDeFallo(e);
    }
  }

  // Abrir otro documento trae otras anotaciones. La dependencia es el archivo y
  // no la sesión entera: la sesión se reemplaza también al guardar progreso, y
  // volver a pedir el panel en cada página sería una consulta por página.
  $effect(() => {
    const abierto = sesion?.fileId;
    if (!abierto) {
      anotaciones = [];
      return;
    }
    void refrescarAnotaciones();
    // Y con otro documento, otro índice: el estado del anterior no dice nada
    // del nuevo, y dejarlo puesto avisaría de algo que no es de este libro.
    void refrescarEstadoDelIndice();
  });

  async function cerrarDocumento() {
    if (!sesion) return;
    // Lo primero, apuntar por dónde se va: cerrar con la cruz no pasa por el
    // comando `salir`, que era el único que lo hacía.
    await lectorPrincipal?.guardarPosicion();
    // Lo preparado es de este libro: cerrar el libro lo para y suelta su audio.
    // Y la narración del libro, que suena por el mismo reproductor.
    reproductor?.parar();
    narradorInstancia?.parar();
    const abierto = sesion.fileId;
    ultimaPosicion.delete(abierto);
    sesion = null;
    objetivo = null;
    panelDelLector = null;
    inmersivo = false;
    anotaciones = [];
    destino = destinoTrasCerrar(destino);
    // Cerrar no es opcional: es lo que vacía el progreso pendiente, y sin ello
    // el usuario vuelve unos segundos más atrás de donde lo dejó. Lo que eso
    // cambia en Inicio y en la biblioteca llega con el anuncio del progreso.
    await closeDocument(abierto);
    sinBus('progreso');
    // Sin aviso de "Documento cerrado": la vista ya cambió y decirlo además era
    // ruido que tapaba el texto de abajo.
  }

  async function cambiarComposicion(
    campo: keyof Composicion,
    valor: Composicion[keyof Composicion]
  ) {
    const siguiente = { ...composicion, [campo]: valor };
    await guardarPreferencia(CLAVES[campo], valorDe(campo, siguiente));

    // La alineación se escribe también en la clave antigua. Es lo que va en la
    // exportación y lo que lee una biblioteca de antes: dejarla atrás haría que
    // las dos dijeran cosas distintas sobre lo mismo.
    if (campo === 'alineacion') {
      await guardarPreferencia(CLAVE_JUSTIFICADO, siguiente.alineacion === 'justificado');
    }
  }

  async function guardarPreferencia(clave: string, valor: Setting['value']) {
    // Gana lo que responda el runtime, no lo que se creía estar escribiendo:
    // regla 2 de ADR-010.
    const guardada = await escribirPreferencia(clave, valor, preferencias);
    // Se reemplaza o se añade. Solo reemplazar perdía en silencio cualquier
    // preferencia que el runtime todavía no hubiera devuelto en el listado.
    preferencias = preferencias.some((p) => p.key === guardada.key)
      ? preferencias.map((p) => (p.key === guardada.key ? guardada : p))
      : [...preferencias, guardada];

    // Tres preferencias cambian lo que Ajustes está enseñando debajo: el
    // interruptor general y los dos servidores. Sin releer, el usuario apaga el
    // modo sin red y el registro de salidas sigue contando lo de antes.
    if (clave === 'offline_mode' || clave === 'translation_host' || clave === 'speech_host') {
      await refrescarRed();
    }
    // Y la clave está guardada **para un anfitrión**: al apuntar a otro
    // servidor deja de haberla, y el campo tiene que decirlo en el momento y no
    // seguir enseñando "ya hay una" con la del servidor anterior.
    if (clave === 'speech_host') {
      await refrescarCredencialDeVoz();
    }
    // Cambiar el tope despierta al indexador; se relee para que Ajustes deje de
    // decir que se detuvo en cuanto vuelve a haber sitio.
    if (clave === 'derived_budget') {
      setTimeout(() => void refrescarAvanceDelIndice(), 1500);
    }
    // El presupuesto de la IA cambia lo que la sección enseña debajo: un tope de
    // memoria por debajo del modelo lo deja sin cargar, y eso lo dice el runtime.
    // El tope y los hilos cambian lo que la sección enseña debajo; el modelo elegido
    // cambia cuál dice que está en uso, y eso lo decide el runtime.
    if (clave === 'ai_threads' || clave === 'ai_memory_limit_mb' || clave === 'ai_model_local') {
      await refrescarModelos();
    }
    // Y el proveedor externo: cambiar la dirección cambia a quién se autorizó y de
    // quién es la clave, y una prueba de antes ya no dice nada del nuevo.
    if (clave === 'ai_provider' || clave === 'ai_host' || clave === 'ai_model') {
      ia.prueba = null;
      await refrescarProveedorDeIa();
    }
    if (clave.startsWith('ai_')) void refrescarIaParaEstudiar();
  }

  async function importar(ruta: string, copiar: boolean): Promise<ImportReport> {
    const informe = await importPath(ruta, copiar);
    if (informe.running) {
      // Una carpeta sigue en marcha: la biblioteca se refresca con el avance,
      // no al volver de la llamada, que devuelve enseguida a propósito.
      avanceImportacion = {
        processed: 0,
        imported: 0,
        skipped: 0,
        rejected: 0,
        duplicates: 0,
        total: 0
      };
    } else {
      // Un archivo suelto entra de una vez, y el runtime lo anuncia al confirmar.
      sinBus('obras');
    }
    return informe;
  }

  /** Relee qué quedó a medias y qué dejó fuera la última importación. */
  async function refrescarImportaciones() {
    importacionPendiente = await pendingImport();
    const ultima = await lastImport();
    informeDeImportacion = ultima
      ? {
          ejecucion: ultima,
          rechazos: await importRejections(ultima.id),
          // Solo se piden si las hubo: es lo normal, y así no cuesta una llamada.
          copias: ultima.duplicates > 0 ? await importDuplicates(ultima.id) : []
        }
      : null;
  }

  /**
   * Continúa una importación que quedó a medias.
   *
   * Es volver a lanzarla sobre la misma carpeta y con el mismo modo: lo ya
   * importado se reconoce por su contenido y se salta, así que no duplica nada.
   * El modo tiene que ser el que se pidió y no el que esté por defecto ahora, o
   * la segunda mitad de la biblioteca acabaría guardada de otra forma que la
   * primera.
   */
  async function continuarImportacion(ejecucion: ImportRun) {
    importacionPendiente = null;
    try {
      await importar(ejecucion.root, ejecucion.storageMode === 'managed');
    } catch (e) {
      avisarDeFallo(e);
      await refrescarImportaciones();
    }
  }

  /** Da por zanjada una importación a medias sin continuarla. */
  async function descartarImportacion(ejecucion: ImportRun) {
    importacionPendiente = null;
    try {
      await discardPendingImport(ejecucion.id);
    } catch (e) {
      avisarDeFallo(e);
    }
    await refrescarImportaciones();
  }

  async function abrirNota(nota: Note) {
    notaAbierta = nota;
    retroenlaces = await noteBacklinks(nota.id);
  }
</script>

{#if poniendoAlDia}
  <div class="nb-poniendo-al-dia" role="status">
    <Spinner etiqueta={T.avisos.poniendoAlDia} tamano="3rem" />
    <p>{T.avisos.poniendoAlDia}</p>
  </div>
{/if}

<AppShell {inmersivo} bind:plegada bind:plegadaVisible>
  {#snippet marca()}
    <!--
      El logotipo preside la barra desplegada sin comerse su ancho.
      `tamano` mide el disco, y la marca va dentro al 62 %, así que un solo valor
      reduce las dos cosas a la vez.
    -->
    <Logo tamano={plegadaVisible ? '2.25rem' : '7.5rem'} />
  {/snippet}

  {#snippet navegacion()}
    {#each destinosPrincipales(contexto) as entrada (entrada.destino)}
      <NavItem
        icono={entrada.icono}
        etiqueta={TITULOS[entrada.destino]}
        activo={destino === entrada.destino}
        plegado={plegadaVisible}
        extra={entrada.destino === 'biblioteca' && novedades.total > 0
          ? cifraCorta.format(novedades.total)
          : undefined}
        onclick={() => (destino = entrada.destino)}
      />
    {/each}
  {/snippet}

  {#snippet tarjetas()}
    <!--
      Lo que está abierto, si está entre lo empezado; si no, lo más reciente. Con
      un PDF abierto la tarjeta seguía diciendo el último EPUB.
    -->
    {@const enLectura =
      enCurso.items.find((i) => i.editionId === sesion?.editionId) ??
      (sesion && obraAbierta) ??
      enCurso.items[0]}
    {#if enLectura}
      <!--
        Lo que da vida a la barra no es una animación: es que aquí abajo pase
        algo real mientras se lee.
      -->
      <TarjetaLateral
        titulo={enLectura.title}
        cifra={enLectura.progression !== null
          ? `${porcentajeLeido(enLectura.progression)} %`
          : undefined}
        plegada={tarjetaPlegada('leyendo')}
        etiquetaPlegar={T.carcasa.plegarTarjeta}
        etiquetaDesplegar={T.carcasa.desplegarTarjeta}
        alAlternar={(valor) => plegarTarjeta('leyendo', valor)}
        onclick={() => {
          if (sesion) destino = 'lector';
          else void abrir(enLectura);
        }}
      >
        <span class="nb-leyendo">
          <Cover
            titulo={enLectura.title}
            imagen={enLectura.coverPath ? rutaDeImagen(enLectura.coverPath) : null}
            ancho={36}
          />
          <span class="nb-leyendo-texto">
            <span class="nb-lateral-tarjeta-detalle">{T.carcasa.leyendo}</span>
            {#if enLectura.progression !== null}
              <span class="nb-leyendo-barra">
                <span style:width="{porcentajeLeido(enLectura.progression)}%"></span>
              </span>
            {/if}
          </span>
        </span>
      </TarjetaLateral>
    {/if}

    {#if avanceImportacion}
      <!--
        Una importación larga se ve desde cualquier sitio sin tener que estar en
        el panel de importar. Al terminar o cancelarse desaparece.
      -->
      <TarjetaLateral
        icono="importar"
        titulo={T.carcasa.tarjetaImportacion}
        cifra={avanceImportacion.total > 0
          ? `${Math.floor((avanceImportacion.processed / avanceImportacion.total) * 100)} %`
          : `${cifra.format(avanceImportacion.processed)}`}
        plegada={tarjetaPlegada('importacion')}
        etiquetaPlegar={T.carcasa.plegarTarjeta}
        etiquetaDesplegar={T.carcasa.desplegarTarjeta}
        alAlternar={(valor) => plegarTarjeta('importacion', valor)}
        onclick={async () => {
          if (destino === 'lector') {
            destino = 'biblioteca';
            await tick();
          }
          alternarMenu('importar', true);
        }}
      >
        {#if avanceImportacion.total > 0}
          <span class="nb-leyendo-barra">
            <span
              style:width="{Math.min(
                100,
                (avanceImportacion.processed / avanceImportacion.total) * 100
              )}%"
            ></span>
          </span>
          <span class="nb-lateral-tarjeta-detalle">
            {cifra.format(avanceImportacion.processed)}
            {T.carcasa.tarjetaImportacionDe}
            {cifra.format(avanceImportacion.total)}
            {T.carcasa.tarjetaImportacionDocumentos}
          </span>
        {:else}
          <span class="nb-lateral-tarjeta-detalle">
            {cifra.format(avanceImportacion.processed)}
            {T.carcasa.tarjetaImportacionDocumentos}
          </span>
        {/if}
      </TarjetaLateral>
    {/if}

    {#if indiceDeLaTarjeta}
      <!--
        El índice se construye durante horas sobre una biblioteca grande, y
        mientras no termina buscar dentro de los libros no encuentra todo. Se dice
        aquí, donde se ve sin buscarlo, y lleva a su sección de Ajustes.
      -->
      <TarjetaLateral
        icono="progreso"
        titulo={T.carcasa.tarjetaIndice}
        cifra={indiceDeLaTarjeta.sinSitio
          ? T.carcasa.tarjetaIndiceSinSitio
          : indiceDeLaTarjeta.paused
            ? T.carcasa.tarjetaIndicePausado
            : `${Math.floor(indiceDeLaTarjeta.fraccion * 100)} %`}
        plegada={tarjetaPlegada('indice')}
        etiquetaPlegar={T.carcasa.plegarTarjeta}
        etiquetaDesplegar={T.carcasa.desplegarTarjeta}
        alAlternar={(valor) => plegarTarjeta('indice', valor)}
        onclick={() => {
          seccionDeAjustes = 'indice';
          destino = 'ajustes';
        }}
      >
        <span class="nb-leyendo-barra">
          <span style:width="{indiceDeLaTarjeta.fraccion * 100}%"></span>
        </span>
        <span class="nb-lateral-tarjeta-detalle">
          {cifra.format(indiceDeLaTarjeta.hechas)}
          {T.carcasa.tarjetaIndiceDe}
          {cifra.format(indiceDeLaTarjeta.total)}
          {T.carcasa.tarjetaIndiceObras}
        </span>
      </TarjetaLateral>
    {/if}

    {#if hayAlgoQueDecir(components)}
      <!--
        Lo que antes era un botón de la barra superior que aparecía cuando algo
        fallaba. Ahora es una tarjeta que lleva a Acerca de, donde el estado de
        cada componente se ve siempre.
      -->
      <TarjetaLateral
        icono={avisos(components)[0]?.icono ?? 'informacion'}
        titulo={T.carcasa.tarjetaEstado}
        plegada={tarjetaPlegada('estado')}
        etiquetaPlegar={T.carcasa.plegarTarjeta}
        etiquetaDesplegar={T.carcasa.desplegarTarjeta}
        alAlternar={(valor) => plegarTarjeta('estado', valor)}
        onclick={() => {
          seccionDeAjustes = 'acerca';
          destino = 'ajustes';
        }}
      >
        <span class="nb-lateral-tarjeta-detalle">{T.carcasa.tarjetaEstadoAyuda}</span>
      </TarjetaLateral>
    {/if}
  {/snippet}

  {#snippet pie()}
    {#each destinosDelPie(contexto) as entrada (entrada.destino)}
      <NavItem
        icono={entrada.icono}
        etiqueta={TITULOS[entrada.destino]}
        activo={destino === entrada.destino}
        plegado={plegadaVisible}
        onclick={() => (destino = entrada.destino)}
      />
    {/each}
  {/snippet}

  {#snippet banner()}
    <IconButton
      etiqueta={plegada ? T.carcasa.desplegar : T.carcasa.plegar}
      onclick={() => (plegada = !plegada)}
    >
      <Icon nombre="menu" />
    </IconButton>

    <!--
      El nombre de la vista es el encabezado de la página para quien la oye, y en
      la barra no se ve: ya lo dice el destino marcado de la barra lateral, y
      repetirlo arriba era el título de una web. En el lector sí se ve, porque ahí
      el nombre es el del libro, y eso no lo dice nada más.
    -->
    {#if destino === 'lector' && tituloDelDocumento}
      <h1 class="nb-titulo">{tituloDelDocumento}</h1>
    {:else}
      <h1 class="nb-titulo nb-solo-lector">{TITULOS[destino]}</h1>
    {/if}

    {#if destino !== 'lector'}
      <!--
        La búsqueda es una caja en la barra y no un botón que abre otra caja: es
        lo más usado de la pantalla, y un paso de más delante de lo más usado es
        el que más cuesta. Los resultados se abren debajo al escribir.
      -->
      <div class="nb-buscador" bind:this={buscador} data-abierto={menuAbierto === 'buscar'}>
        <span class="nb-buscador-icono"><Icon nombre="buscar" /></span>
        <input
          class="nb-buscador-entrada"
          type="search"
          aria-label={T.carcasa.buscar}
          placeholder={T.carcasa.buscarAyuda}
          autocomplete="off"
          spellcheck="false"
          bind:value={consultaGlobal}
          onfocus={() => alternarMenu('buscar', true)}
          oninput={() => alternarMenu('buscar', true)}
        />
        {#if consultaGlobal}
          <span class="nb-buscador-borrar">
            <IconButton etiqueta={T.carcasa.borrarBusqueda} onclick={() => (consultaGlobal = '')}>
              <Icon nombre="cerrar" tamano={20} />
            </IconButton>
          </span>
        {/if}
        {#if menuAbierto === 'buscar'}
          <Emergente
            titulo={T.paneles.busqueda}
            alineacion="inicio"
            ancho="40rem"
            ancla={buscador}
            alCerrar={() => alternarMenu('buscar', false)}
          >
            {#await cargarMenus() then M}
              <M.SearchPanel
                conCaja={false}
                bind:consulta={consultaGlobal}
                alBuscar={(consulta, alcance) => search(consulta, alcance)}
                alBuscarPorSignificado={semanticSearch}
                documentoAbierto={sesion
                  ? {
                      editionId: sesion.editionId,
                      titulo: TITULOS.lector,
                      navegacion: sesion.navigation.entries
                    }
                  : null}
                estadoDeBusqueda={estadoDelIndice}
                alAbrirEdicion={(fileId) => {
                  menuAbierto = null;
                  void abrirResultado(fileId);
                }}
                alSaltar={(fileId, locator) => {
                  menuAbierto = null;
                  void abrirResultado(fileId, locator);
                }}
              />
            {/await}
          </Emergente>
        {/if}
      </div>
    {/if}

    <span class="nb-separador"></span>

    <!--
      Las estanterías solo en la biblioteca: en el lector o en Estudio abrirían
      algo que no tiene nada que ver con lo que hay delante. Colecciones e
      Importar valen en cualquier pantalla menos en el lector, donde la barra es
      del libro.
    -->
    {#if destino === 'biblioteca'}
      <Desplegable
        texto={T.estanterias.titulo}
        titulo={T.estanterias.titulo}
        icono="estanteria"
        soloIcono={barraEstrecha}
        ancho="24rem"
        abierto={menuAbierto === 'estanterias'}
        alAlternar={(abierto) => {
          alternarMenu('estanterias', abierto);
          if (abierto) void refrescarEstanterias();
        }}
      >
        {#await cargarMenus() then M}
          <M.ShelvesPanel
            secciones={estanterias}
            busqueda={busquedaDeEstanteria}
            cargando={cargandoEstanterias}
            activa={estanteriaActiva}
            alBuscar={(texto) => {
              busquedaDeEstanteria = texto;
              void refrescarEstanterias();
            }}
            alElegir={abrirEstanteria}
            alQuitar={() => {
              quitarEstanteria();
              menuAbierto = null;
            }}
          />
        {/await}
      </Desplegable>
    {/if}

    {#if destino !== 'lector'}
      <Desplegable
        texto={T.carcasa.colecciones}
        titulo={T.paneles.colecciones}
        icono="carpeta"
        soloIcono={barraEstrecha}
        ancho="22rem"
        abierto={menuAbierto === 'colecciones'}
        alAlternar={(abierto) => {
          alternarMenu('colecciones', abierto);
          if (abierto) void refrescarColecciones();
        }}
      >
        {#await cargarMenus() then M}
          <M.CollectionsMenu
            {colecciones}
            activa={coleccionActiva?.id ?? null}
            cargando={cargandoColecciones}
            alElegir={abrirColeccion}
            alQuitar={() => {
              quitarEstanteria();
              menuAbierto = null;
            }}
            alCrear={(nombre) => void crearColeccion(nombre)}
            alRenombrar={(id, nombre) => void renombrarColeccion(id, nombre)}
            alBorrar={(id) => void borrarColeccion(id)}
          />
        {/await}
      </Desplegable>

      <Desplegable
        texto={T.carcasa.audiolibrosCorto}
        etiqueta={T.carcasa.audiolibros}
        titulo={T.paneles.audiolibros}
        icono="voz"
        soloIcono={barraEstrecha}
        ancho="28rem"
        abierto={menuAbierto === 'audiolibros'}
        alAlternar={(abierto) => alternarMenu('audiolibros', abierto)}
      >
        {#await cargarMenus() then M}
          <M.AudiolibrosPanel
            libro={enfocada ? { editionId: enfocada.editionId, title: enfocada.title } : null}
            coleccion={coleccionActiva
              ? { id: coleccionActiva.id, name: coleccionActiva.name }
              : null}
            alVerPreparados={() => {
              consultaDeBiblioteca = {
                ...consultaDeBiblioteca,
                filter: { kind: 'audiobook' },
                offset: 0
              };
              destino = 'biblioteca';
              menuAbierto = null;
            }}
          />
        {/await}
      </Desplegable>

      <Desplegable
        texto={T.carcasa.importarCorto}
        etiqueta={T.carcasa.importar}
        titulo={T.paneles.importar}
        icono="importar"
        principal
        soloIcono={barraEstrecha}
        ancho="28rem"
        abierto={menuAbierto === 'importar'}
        alAlternar={(abierto) => alternarMenu('importar', abierto)}
      >
        {#await cargarMenus() then M}
          <M.ImportPanel
            alImportar={importar}
            avance={avanceImportacion}
            alCancelar={() => void cancelImport()}
            alElegir={elegirParaImportar}
            pendiente={importacionPendiente}
            alContinuar={(ejecucion) => void continuarImportacion(ejecucion)}
            alDescartar={(ejecucion) => void descartarImportacion(ejecucion)}
            guardado={informeDeImportacion}
          />
        {/await}
      </Desplegable>
    {/if}

    {#if sesion}
      <IconButton etiqueta={T.carcasa.cerrarDocumento} onclick={() => void cerrarDocumento()}>
        <Icon nombre="cerrar" />
      </IconButton>
    {/if}
  {/snippet}

  {#snippet principal()}
    {#if recuperacion}
      <!--
        Lo primero que se ve, y en todas las vistas. No es un aviso más: el
        usuario está mirando una biblioteca vacía que no ha vaciado él, y sin
        esto creería que ha perdido todo. Dice qué pasó y **dónde está lo suyo**.
      -->
      <div class="nb-recuperacion" role="alert">
        <Icon nombre="advertencia" />
        <div>
          <p class="nb-recuperacion-titulo">{T.carcasa.recuperada}</p>
          <p class="nb-recuperacion-ayuda">{T.carcasa.recuperadaAyuda}</p>
          <p class="nb-recuperacion-archivo nb-selectable">{recuperacion.isolatedAs}</p>
          <p class="nb-recuperacion-ayuda">
            {T.carcasa.recuperadaMotivo}: {traducirError(recuperacion.reasonCode)}
          </p>
        </div>
      </div>
    {/if}

    {#if destino === 'lector' && sesion}
      <!--
        Dos mitades, cada una con su lector y su posición. El reparto es un
        número que se mueve a pasos, no un arrastre: arrastrar un separador es un
        gesto de puntero, y una acción exclusiva del ratón está prohibida.
      -->
      {#await cargarLector() then { default: Lector }}
        <div class="nb-division" data-activa={division.activa}>
          <div
            class="nb-mitad"
            style:width={division.activa ? anchos(division).principal : '100%'}
            data-enfocada={!division.activa || division.foco === 'principal'}
          >
            <Lector
              bind:this={lectorPrincipal}
              {controladoDesde}
              {sesion}
              {composicion}
              {modosDeExtension}
              {objetivo}
              sonandoEn={reproductor?.sonandoEn ?? null}
              desde={ultimaPosicion.get(sesion.fileId) ?? null}
              alMoverse={(l) => {
                if (sesion) ultimaPosicion.set(sesion.fileId, l);
              }}
              {cabeDividir}
              hayIa={hayIaParaEstudiar}
              dividido={division.activa}
              fondoSonando={fondoSonando !== null}
              activo={!division.activa ||
                division.foco === 'principal' ||
                sesionSecundaria === null}
              alSalir={() => void cerrarDocumento()}
              alDividir={alternarLaDivision}
              alCambiarMitad={cambiarDeMitad}
              alComponer={(campo, valor) => void cambiarComposicion(campo, valor)}
              alAlternarFondo={alternarFondoRemoto}
              alRepartir={(pasos) => (division = repartir(division, pasos))}
              alCambiarModo={(modo) => {
                inmersivo = modo.inmersivo;
                panelDelLector = modo.panel;
                // **También al abrir la voz**, que es donde está preparar el capítulo.
                // Antes solo se tomaba al abrir Estudiar con IA, así que
                // preparar el capítulo no aparecía sin haber abierto antes el otro
                // panel, y si se había abierto, preparaba el capítulo de entonces.
                if (modo.panel === 'estudio' || modo.panel === 'voz') {
                  void lectorPrincipal?.paraEstudiar().then((v) => (paraEstudiar = v));
                }
                // Al cerrar el panel se olvida a quién se había señalado. Sin
                // esto, volver a pulsar la misma marca no movería nada: el valor
                // no habría cambiado, y lo que mueve el cursor es el cambio.
                if (modo.panel !== 'anotaciones') anotacionSenalada = null;
              }}
              {anotaciones}
              alColocarMarcas={(informe) => {
                sinSoporteDeMarcas = !informe.soportado;
                sinPintar = new Set(informe.sinColocar);
              }}
              alSenalarMarca={(id) => {
                // Señalar una marca abre el panel **en esa anotación**: quien
                // pulsa un resaltado pregunta por ese, no por los cuarenta.
                anotacionSenalada = id;
                lectorPrincipal?.abrirPanel('anotaciones');
              }}
              alAnotar={() => void refrescarAnotaciones()}
              alConsultar={(nueva) => {
                consulta = nueva;
                if (nueva.cargando) palabraPedida = null;
                // Una consulta con respuesta deja la palabra apuntada en el
                // vocabulario. Se relee para que Estudio no enseñe una cuenta
                // vieja: el runtime ya escribió, y la vista lo ignoraba hasta el
                // siguiente arranque.
                else if (nueva.resultado && nueva.resultado.entries.length > 0) {
                  void refrescarVocabulario();
                }
              }}
              {palabraPedida}
              seleccionConsultada={consulta?.seleccion ?? ''}
            />
          </div>
          {#if division.activa}
            <div
              class="nb-mitad"
              style:width={anchos(division).secundaria}
              data-enfocada={division.foco === 'secundaria'}
            >
              {#if sesionSecundaria}
                <Lector
                  sesion={sesionSecundaria}
                  desde={ultimaPosicion.get(sesionSecundaria.fileId) ?? null}
                  alMoverse={(l) => {
                    if (sesionSecundaria) ultimaPosicion.set(sesionSecundaria.fileId, l);
                  }}
                  {composicion}
                  {modosDeExtension}
                  {cabeDividir}
                  dividido
                  activo={division.foco === 'secundaria'}
                  alSalir={alternarLaDivision}
                  alDividir={alternarLaDivision}
                  alCambiarMitad={cambiarDeMitad}
                  alRepartir={(pasos) => (division = repartir(division, pasos))}
                />
              {:else}
                <!--
                La segunda mitad sin documento dice qué falta y lleva a hacerlo.
                Antes, con la división abierta y una sola obra, no se pintaba
                nada: media pantalla en blanco sin explicación.
              -->
                <EmptyState icono="lector" mensaje={T.lector.segundaMitad}>
                  {#snippet accion()}
                    <Button variante="primario" onclick={() => (destino = 'biblioteca')}>
                      {T.inicio.irABiblioteca}
                    </Button>
                  {/snippet}
                </EmptyState>
              {/if}
            </div>
          {/if}
        </div>
      {/await}
    {:else if destino === 'biblioteca'}
      <LibraryView
        cargar={browseLibrary}
        bind:consulta={consultaDeBiblioteca}
        bind:pagina={paginaDeBiblioteca}
        porPagina={porPaginaBiblioteca}
        modo={vistaBiblioteca}
        facetas={facetasDeBiblioteca}
        alCambiarOrden={(criterio, ascendente) =>
          void guardarPreferencia('library_sort', `${criterio}:${ascendente ? 'asc' : 'desc'}`)}
        estanteriaActiva={estanteriaActiva?.label ?? coleccionActiva?.name ?? null}
        {revisoresDeMetadatos}
        alRevisarMetadatos={(extension, ediciones) => void revisarMetadatos(extension, ediciones)}
        alQuitarEstanteria={quitarEstanteria}
        alCambiarPagina={(n) => {
          // Se aplica antes de guardarlo: escribir en el disco tarda, y esperar
          // a que vuelva dejaría el control sin responder al pulsarlo.
          porPaginaBiblioteca = n;
          void guardarPreferencia('library_page_size', n);
        }}
        alCambiarModo={(m) => {
          vistaBiblioteca = m;
          void guardarPreferencia('library_view', m);
        }}
        alAbrir={(item) => void abrir(item)}
        recarga={recargaDeBiblioteca}
        alActualizar={(item) => {
          if (enfocada?.editionId === item.editionId) enfocada = item;
        }}
        alSeleccionar={(item) => {
          enfocada = item;
          // Cambiar de obra trae sus propuestas y borra lo que se estaba
          // mirando de la anterior: enseñar candidatos de otro libro sería peor
          // que no enseñar ninguno.
          candidatos = [];
          errorEnLinea = null;
          void refrescarPropuestas(item.editionId);
        }}
      />
    {:else if destino === 'estudio'}
      {#await cargarEstudio() then { default: Estudio }}
        <Estudio
          {notas}
          mas={{
            notas: hayMas(notas.length, limiteNotas)
              ? () => {
                  limiteNotas += POR_PAGINA;
                  void refrescarNotas();
                }
              : undefined,
            anotaciones: hayMas(anotacionesDeLaBiblioteca.length, limiteAnotaciones)
              ? () => {
                  limiteAnotaciones += POR_PAGINA;
                  void refrescarAnotacionesDeLaBiblioteca();
                }
              : undefined,
            vocabulario: hayMas(vocabulario.length, limiteVocabulario)
              ? () => {
                  limiteVocabulario += POR_PAGINA;
                  void refrescarVocabulario();
                }
              : undefined,
            tableros: hayMas(tableros.length, limiteTableros)
              ? () => {
                  limiteTableros += POR_PAGINA;
                  void refrescarTableros();
                }
              : undefined
          }}
          vistas={[
            { id: 'notas', etiqueta: T.estudio.notas, cuenta: notas.length },
            {
              id: 'anotaciones',
              etiqueta: T.estudio.anotaciones,
              cuenta: cuentasDeAnotaciones?.all
            },
            // El vocabulario se anuncia, que es cuando empezó a
            // llevar a alguna parte: antes habría sido un botón vacío, que es la
            // regla 5 de ADR-013 vista del otro lado.
            { id: 'vocabulario', etiqueta: T.estudio.vocabulario, cuenta: vocabulario.length },
            // Lo guardado al estudiar con IA. Se ofrece siempre,
            // como el vocabulario: vacío, dice cómo se llena.
            { id: 'tarjetas', etiqueta: T.estudio.tarjetas, cuenta: cuentasDeTarjetas?.cards },
            {
              id: 'conceptos',
              etiqueta: T.estudio.conceptos,
              cuenta: cuentasDeTarjetas?.concepts
            },
            // Los tableros: una vista **sobre las notas que ya hay**.
            { id: 'tablero', etiqueta: T.tableros.tableros, cuenta: tableros.length },
            // Y las que aporten las extensiones, con el mismo aspecto que las
            // nativas y marcadas como suyas: se marca, no se esconde.
            ...vistasDeExtension.map((vista) => ({
              id: vista.id,
              etiqueta: vista.title,
              deExtension: true
            }))
          ]}
          vistaActiva={vistaDeEstudio}
          {vistaDeExtension}
          {tableros}
          {tableroActivo}
          {columnasDelTablero}
          {limitesDelTablero}
          {cuentasDeAnotaciones}
          alAbrirTablero={(id) => void abrirTablero(id)}
          alCerrarTablero={() => {
            tableroActivo = null;
            columnasDelTablero = [];
          }}
          alCrearTablero={(titulo, columnas) => void crearTablero(titulo, columnas)}
          alBorrarTablero={(id) => void borrarTablero(id)}
          alAnadirColumna={(titulo) => void conElTablero((id) => anadirColumna(id, titulo))}
          alRenombrarColumna={(columna, titulo) =>
            void conElTablero((id) => renombrarColumna(id, columna, titulo))}
          alBorrarColumna={(columna) => void conElTablero((id) => borrarColumna(id, columna))}
          alMoverColumna={(columna, lugar) =>
            void conElTablero((id) => moverColumna(id, columna, lugar))}
          alPonerNotaEnColumna={(columna, nota) =>
            void conElTablero((id) => ponerNotaEnColumna(id, columna, nota))}
          alEnlazarEnColumna={(columna, destino) =>
            void conElTablero((id) => enlazarEnColumna(id, columna, destino))}
          alCrearNotaEnColumna={(columna, titulo) =>
            void conElTablero(async (id) => {
              const columnas = await crearNotaEnColumna(id, columna, titulo, '');
              await refrescarNotas();
              return columnas;
            })}
          alMoverTarjeta={(tarjeta, columna, lugar) =>
            void conElTablero((id) => moverTarjeta(id, tarjeta, columna, lugar))}
          alQuitarTarjeta={(tarjeta) => void conElTablero((id) => quitarTarjeta(id, tarjeta))}
          alAbrirNotaPorId={(id) =>
            void findNote(id)
              .then(async (nota) => {
                if (!nota) return;
                vistaDeEstudio = 'notas';
                await abrirNota(nota);
              })
              .catch(avisarDeFallo)}
          alAbrirFuente={(fileId, locator) => void abrirFuenteDeEstudio(fileId, locator)}
          alCambiarVista={(id) => {
            vistaDeEstudio = id;
            void abrirVistaDeExtension(id);
          }}
          anotaciones={anotacionesDeLaBiblioteca}
          clase={claseDeAnotacion}
          alFiltrar={(nueva) => (claseDeAnotacion = nueva)}
          alSaltar={(anotacion) => void saltarAAnotacion(anotacion)}
          alBorrarAnotacion={(id) => void borrarAnotacionDeLaBiblioteca(id)}
          {retroenlaces}
          abierta={notaAbierta}
          alAbrir={(nota) => void abrirNota(nota)}
          bind:anotacionActiva={anotacionAbiertaEnEstudio}
          historial={historialDeAnotacion}
          alVerHistorial={(id) => void verHistorial(id)}
          citadas={pasajesCitados}
          notasQueCitan={notasQueCitanLaAnotacion}
          alCitarEnNota={(noteId, annotationId) => void citarPasaje(noteId, annotationId)}
          alQuitarPasaje={(noteId, annotationId) => void quitarPasaje(noteId, annotationId)}
          bind:palabraActiva={palabraAbiertaEnEstudio}
          alCrear={(titulo, cuerpo, silencioso) =>
            void createNote(titulo, cuerpo)
              .then(async (nota) => {
                await refrescarNotas();
                await abrirNota(nota);
                if (!silencioso) notificar.exito(T.avisos.notaGuardada);
              })
              .catch(avisarDeFallo)}
          alGuardar={(id, titulo, cuerpo, silencioso) =>
            void updateNote(id, titulo, cuerpo)
              .then(async (nota) => {
                await refrescarNotas();
                // Solo si sigue abierta la misma: el autoguardado de la nota que
                // se deja no debe volver a abrirla encima de la que se elige.
                if (!silencioso || notaAbierta?.id === nota.id) notaAbierta = nota;
                if (!silencioso) notificar.exito(T.avisos.notaGuardada);
              })
              .catch(avisarDeFallo)}
          alBorrar={(id) =>
            void deleteNote(id)
              .then(async () => {
                await refrescarNotas();
                notaAbierta = null;
                retroenlaces = [];
                notificar.exito(T.avisos.notaBorrada);
              })
              .catch(avisarDeFallo)}
          {vocabulario}
          alSaltarAPalabra={(palabra) => void saltarAPalabra(palabra)}
          alBorrarPalabra={(id) =>
            void deleteVocabulary(id)
              .then(refrescarVocabulario)
              .then(() => notificar.exito(T.avisos.palabraBorrada))
              .catch(avisarDeFallo)}
          alBorrarVocabulario={() =>
            void clearVocabulary()
              .then(refrescarVocabulario)
              .then(() => notificar.exito(T.avisos.vocabularioBorrado))
              .catch(avisarDeFallo)}
        />
      {/await}
    {:else if destino === 'ajustes'}
      {#await cargarAjustes() then { default: Ajustes }}
        <Ajustes
          {preferencias}
          esquemaOscuro={oscuroDelSistema}
          alCambiar={(clave, valor) => void guardarPreferencia(clave, valor)}
          {diagnostico}
          {diagnosticoGuardado}
          alPrepararDiagnostico={() => void prepararDiagnostico()}
          alQuitarReporte={(id) => void quitarReporteDeIa(id)}
          alGuardarDiagnostico={() => void guardarElDiagnostico()}
          {diccionarios}
          alInstalarDiccionario={() => void instalarDiccionario()}
          alDesinstalarDiccionario={(id) => void desinstalarDiccionario(id)}
          {extensiones}
          api={apiDeExtensiones}
          {temasDeExtension}
          {temaDeExtensionActivo}
          alElegirTemaDeExtension={(extension) => elegirTemaDeExtension(extension)}
          pendiente={pendienteExtension}
          {generacionesDeExtensiones}
          retiradas={retiradasDeExtensiones}
          alComprobarRetiradas={() => void comprobarRetiradas()}
          alInstalarExtension={() => void instalarExtension()}
          alConcederPendiente={(anfitrion) => void concederAPendiente(anfitrion)}
          alHabilitarExtension={(id, activa) => void habilitarExtensionEn(id, activa)}
          alDesinstalarExtension={(id) => void desinstalarExtensionEn(id)}
          alRevocarExtension={(id, anfitrion) => void revocarAExtensionEn(id, anfitrion)}
          alVolverAtrasExtensiones={() => void volverAtrasExtensionesEn()}
          {red}
          alRevocar={(host, purpose) =>
            void revokeNetworkConsent(host, purpose)
              .then(refrescarRed)
              .then(() => notificar.exito(T.avisos.permisoRevocado, host))
              .catch(avisarDeFallo)}
          alAutorizar={(host, cleartext) => void autorizarCatalogo(host, cleartext)}
          credencial={credencialDeVoz}
          {puntoDeVoz}
          alGuardarCredencial={(secreto) =>
            void setSpeechCredential(secreto)
              .then(refrescarCredencialDeVoz)
              .then(() => notificar.exito(T.avisos.credencialGuardada))
              .catch(avisarDeFallo)}
          alBorrarCredencial={() =>
            void clearSpeechCredential()
              .then(refrescarCredencialDeVoz)
              .then(() => notificar.exito(T.avisos.credencialBorrada))
              .catch(avisarDeFallo)}
          alAutorizarVoz={() =>
            void allowSpeechHost()
              .then(refrescarRed)
              .then(() => notificar.exito(T.avisos.permisoConcedido, texto('speech_host')))
              .catch(avisarDeFallo)}
          {catalogos}
          alAnadirCatalogo={(titulo, url) => void anadirCatalogo(titulo, url)}
          alQuitarCatalogo={(id) => void quitarCatalogo(id)}
          indice={avanceDelIndice}
          {indicePausado}
          alPausarIndice={(pausado) => void pausarIndice(pausado)}
          alRehacerIndice={() => void rehacerIndice()}
          {raices}
          {revisandoRaices}
          revision={revisionDeRaices}
          alRevisarRaices={() => void revisarRaices()}
          alReapuntarRaiz={(rootId) => void reapuntarRaiz(rootId)}
          alExportarBiblioteca={() => void exportarBiblioteca()}
          {exportandoBiblioteca}
          alBorrarDatos={(alcance) => trasBorrarDatos(alcance)}
          bind:seccion={seccionDeAjustes}
          informe={report}
          componentes={components}
          ultimoFallo={error}
          modelos={modelosDeIa}
          {avanceDeModelo}
          {modeloPideAutorizar}
          {falloDeModelo}
          alDescargarModelo={(id, host) => void descargarModelo(id, host)}
          alAutorizarModelo={(host) => void autorizarModeloYSeguir(host)}
          alCancelarModelo={() => void cancelAiModelInstall().catch(avisarDeFallo)}
          alQuitarModelo={(id) => void quitarModelo(id)}
          {ia}
          alIa={(accion, secreto) => void accionDeIa(accion, secreto)}
        />
      {/await}
    {:else if destino === 'descubrir'}
      {#await cargarDescubrir() then { default: Descubrir }}
        <Descubrir
          {catalogos}
          {feed}
          abierto={catalogoAbierto}
          raiz={catalogoRaiz}
          puedeVolver={rutaDelCatalogo.length > 1}
          alVolver={() => void volverEnCatalogo()}
          cargando={cargandoCatalogo}
          error={errorDeCatalogo}
          descarga={ultimaDescarga}
          {descargando}
          alAbrirCatalogo={(url) => void abrirCatalogo(url)}
          permiso={permisoQueFalta}
          alAutorizar={(host, cleartext) => void autorizarCatalogo(host, cleartext)}
          alNavegar={(url) => void navegarEnCatalogo(url)}
          alDescargar={(entrada) => void descargarDelCatalogo(entrada)}
        />
      {/await}
    {:else if destino === 'inicio'}
      <HomeView
        {enCurso}
        {novedades}
        filas={filasDeInicio}
        sugerir={preferencias.find((p) => p.key === 'home_suggestions')?.value !== false}
        lectura={cifras}
        materias={materiasDeInicio}
        alAbrir={(item) => void abrir(item)}
        alVerFicha={verFicha}
        alVerEmpezados={verEmpezados}
        alAbrirMateria={abrirEstanteria}
        alAbrirColeccion={(id) => {
          const coleccion = colecciones.find((c) => c.id === id);
          if (coleccion) abrirColeccion(coleccion);
        }}
        alIrABiblioteca={() => (destino = 'biblioteca')}
        alImportar={() => alternarMenu('importar', true)}
      />
    {:else}
      <EmptyState icono="informacion" mensaje={T.carcasa.sinIteracion} />
    {/if}
  {/snippet}

  {#snippet paneles()}
    {#if destino === 'lector' && sesion && panelDelLector === 'indice'}
      <!--
        Los paneles con los que se lee -índice, anotaciones, consulta, voz,
        búsqueda y estudio- se acoplan y estrechan la columna cuando hay sitio:
        superpuestos tapaban el texto que se estaba leyendo. Los de ajustar
        -modo, ajustes del libro, fondo sonoro- siguen siendo momentáneos.
      -->
      <Panel titulo={T.paneles.indice} alCerrar={() => lectorPrincipal?.cerrarPanel()}>
        {#await cargarPanelesDelLector() then P}
          <!--
            El mapa del libro va arriba del índice: es la misma pregunta, "dónde
            estoy en el libro", con el libro entero a la vista. Y los dos
            miran dónde se está **ahora**, no dónde se estaba al abrir el libro.
          -->
          <P.MapaDelLibro
            entradas={sesion.navigation.entries}
            {anotaciones}
            actual={ultimaPosicion.get(sesion.fileId) ?? sesion.locator}
            alSaltar={(locator) => (objetivo = locator)}
          />
          {#if sesion.capabilities.textExtraction}
            <P.NombresDelLibro
              fileId={sesion.fileId}
              alSaltar={(locator) => (objetivo = locator)}
            />
          {/if}
          <P.OutlinePanel
            entradas={sesion.navigation.entries}
            actual={ultimaPosicion.get(sesion.fileId) ?? sesion.locator}
            alSaltar={(locator) => (objetivo = locator)}
          />
        {/await}
      </Panel>
    {:else if destino === 'lector' && sesion && panelDelLector === 'anotaciones'}
      <Panel titulo={T.paneles.anotaciones} alCerrar={() => lectorPrincipal?.cerrarPanel()}>
        {#await cargarPanelesDelLector() then P}
          <P.AnnotationsPanel
            entradas={anotaciones}
            {sinPintar}
            {sinSoporteDeMarcas}
            destacada={anotacionSenalada}
            alSaltar={(locator) => (objetivo = locator)}
            alEditar={(id, cuerpo) => void editarAnotacion(id, cuerpo)}
            alBorrar={(id) => void borrarAnotacion(id)}
            alExportar={(formato) => void exportarAnotaciones(formato)}
            {formatosDeCita}
            alExportarCitas={(formato) => void exportarCitas(formato)}
          />
        {/await}
      </Panel>
    {:else if destino === 'lector' && sesion && panelDelLector === 'consulta'}
      <Panel titulo={T.paneles.consulta} alCerrar={() => lectorPrincipal?.cerrarPanel()}>
        {#await cargarPanelesDelLector() then P}
          <P.LookupPanel
            termino={consulta?.termino ?? ''}
            seleccion={consulta?.seleccion ?? ''}
            resultado={consulta?.resultado ?? null}
            cargando={consulta?.cargando ?? false}
            error={consulta?.error ?? null}
            alElegirPalabra={(palabra) => (palabraPedida = palabra)}
            alAutorizar={(host) => void autorizarConsulta(host)}
            queSeEnvia={queSeEnviaAlConsultar}
          />
        {/await}
      </Panel>
    {:else if destino === 'lector' && sesion && panelDelLector === 'voz'}
      <Panel titulo={T.paneles.voz} alCerrar={() => lectorPrincipal?.cerrarPanel()}>
        {#await Promise.all( [cargarPanelesDelLector(), asegurarVoz()] ) then [P, { narrador: n, reproductor: rep }]}
          {#if sesion.capabilities.synchronizedAudio}
            <!--
              La narración del propio libro va primero, como la recomendada, y la
              voz sintética debajo, sin esconderla.
            -->
            <P.NarracionDelLibro
              reproductor={rep}
              fileId={sesion.fileId}
              entradas={sesion.navigation.entries}
              desde={paraEstudiar?.locator ?? null}
              alIr={(locator) => (objetivo = locator)}
              alEmpezar={() => n.parar()}
            />
          {/if}
          <P.VoicePanel
            declaracion={n.declaracion}
            servicio={texto('speech_host') || null}
            voces={n.voces}
            narracion={n.estado}
            alAlternar={() => {
              rep.pausar();
              n.alternar(lectorPrincipal?.bloqueParaNarrar() ?? 0);
            }}
            alParar={() => n.parar()}
            alElegirVoz={(id) => {
              n.elegirVoz(id);
              void guardarPreferencia('voice_name', id);
            }}
            alCambiarVelocidad={(valor) => n.cambiarVelocidad(valor)}
          />
          <!--
            Y debajo, preparar el capítulo. Va aquí y no en un destino
            propio porque es de la voz: es la forma de escuchar en un equipo que no
            narra en directo. Solo con la voz de aquí dentro elegida: con la del
            sistema no hay nada que preparar, y con la de un servicio prepararlo
            sería mandar el libro entero por la red sin que nadie lo haya pedido.
          -->
          {#if texto('voice_provider', 'local') === 'neural' && paraEstudiar}
            <P.AudioPanel
              editionId={sesion.editionId}
              titulo={sesion.title ?? ''}
              entradas={sesion.navigation.entries}
              actual={paraEstudiar.locator}
              voz={texto('voice_name') || null}
              reproductor={rep}
            />
          {/if}
        {/await}
      </Panel>
    {:else if destino === 'lector' && sesion && panelDelLector === 'buscar'}
      <!--
        Buscar dentro del libro abierto. El botón y Ctrl+F
        ponen el estado en `buscar`, pero aquí no había rama que lo pintara:
        se iluminaba el botón y no aparecía nada.
      -->
      <Panel titulo={T.paneles.busqueda} alCerrar={() => lectorPrincipal?.cerrarPanel()}>
        {#await cargarMenus() then M}
          <M.SearchPanel
            alcanceInicial="document"
            alBuscar={(consulta, alcance) => search(consulta, alcance)}
            alBuscarPorSignificado={semanticSearch}
            documentoAbierto={{
              editionId: sesion.editionId,
              titulo: sesion.title ?? TITULOS.lector,
              navegacion: sesion.navigation.entries
            }}
            estadoDeBusqueda={estadoDelIndice}
            alSaltar={(_fileId, locator) => {
              objetivo = locator;
            }}
          />
        {/await}
      </Panel>
    {:else if destino === 'lector' && sesion && panelDelLector === 'estudio'}
      <Panel titulo={T.paneles.estudio} alCerrar={() => lectorPrincipal?.cerrarPanel()}>
        {#await cargarPanelesDelLector() then P}
          {#if paraEstudiar}
            <P.StudyPanel
              editionId={sesion.editionId}
              entradas={sesion.navigation.entries}
              actual={paraEstudiar.locator}
              seleccion={paraEstudiar.seleccion}
              alSaltar={(locator) => (objetivo = locator)}
            />
          {/if}
        {/await}
      </Panel>
    {:else if destino === 'lector' && panelDelLector === 'fondo-sonoro'}
      <Panel
        titulo={T.fondoSonoro.fondoSonoro}
        momentaneo
        alCerrar={() => lectorPrincipal?.cerrarPanel()}
      >
        {#await cargarPanelesDelLector() then P}
          <P.SoundPanel
            fondos={fondosSonoros}
            sonando={fondoSonando}
            volumen={volumenDelFondo}
            minutos={minutosDelFondo}
            alSonar={(id) => void ponerFondo(id)}
            alParar={() => pararFondo()}
            alCambiarVolumen={(valor) => {
              volumenDelFondo = valor;
              // Se aplica a lo que ya suena en lugar de esperar a la próxima vez:
              // el volumen se ajusta escuchando.
              if (fondoSonando) void ponerFondo(fondoSonando);
            }}
            alCambiarMinutos={(valor) => {
              minutosDelFondo = valor;
              if (fondoSonando) void ponerFondo(fondoSonando);
            }}
          />
        {/await}
      </Panel>
    {:else if destino === 'lector' && panelDelLector === 'modo-de-lectura'}
      <Panel
        titulo={T.paneles.modoDeLectura}
        momentaneo
        alCerrar={() => lectorPrincipal?.cerrarPanel()}
      >
        {#await cargarPanelesDelLector() then P}
          <P.ModePanel
            modo={composicion.modo}
            kit={composicion.kit}
            {modosDeExtension}
            alCambiarModo={(modo: string) => void guardarPreferencia(CLAVES.modo, modo)}
            alCambiarKit={(campo: keyof KitAccesible, valor: KitAccesible[keyof KitAccesible]) =>
              void guardarPreferencia(CLAVES_DEL_KIT[campo], valor as Setting['value'])}
          />
        {/await}
      </Panel>
    {:else if destino === 'lector' && panelDelLector === 'ajustes-del-libro'}
      <Panel
        titulo={T.paneles.ajustesDelLibro}
        momentaneo
        alCerrar={() => lectorPrincipal?.cerrarPanel()}
      >
        {#await cargarPanelesDelLector() then P}
          <P.BookPanel
            {composicion}
            alCambiar={(campo, valor) => void cambiarComposicion(campo, valor)}
          />
        {/await}
      </Panel>
    {:else if destino === 'biblioteca' && enfocada}
      <!--
        La ficha existe **cuando hay una obra**, no siempre.

        Antes se montaba en cuanto se entraba en Biblioteca y, sin nada elegido,
        ocupaba 288 píxeles para decir "elige una obra". Peor: en una ventana
        donde la columna de contenido baja de 720 píxeles el panel se superpone,
        así que a 800 de ancho la biblioteca **desaparecía entera** detrás de un
        panel vacío. Un panel que no tiene nada que enseñar no es un panel: es un
        hueco con un título.
      -->
      <Panel titulo={T.paneles.ficha} alCerrar={() => (enfocada = null)}>
        {#await cargarFicha() then { default: Ficha }}
          <Ficha
            item={enfocada}
            alAbrir={(item) => void abrir(item)}
            {candidatos}
            {buscandoEnLinea}
            {errorEnLinea}
            {propuestas}
            alBuscarEnLinea={() => void buscarEnLinea()}
            alAutorizarEnLinea={() =>
              void allowMetadataHost()
                .then(() => buscarEnLinea())
                .catch(avisarDeFallo)}
            alAplicarCandidato={(candidato) => void aplicarCandidato(candidato)}
            {ordenesDeExtension}
            alEjecutarOrden={(extension, orden) =>
              void ejecutarOrdenDeExtensionEn(extension, orden)}
            alAceptarPropuesta={(id) => void resolverPropuesta(id, true)}
            alRechazarPropuesta={(id) => void resolverPropuesta(id, false)}
            {colecciones}
            {enColecciones}
            alCambiarColeccion={(coleccion, dentro) => void cambiarColeccion(coleccion, dentro)}
            alAntesDeQuitar={async (id) => {
              // Si es el que se está leyendo, la sesión se cierra antes de la
              // baja: cerrar guarda la posición, y guardarla después sería
              // escribir sobre un documento que ya no existe.
              if (sesion?.editionId === id) await cerrarDocumento();
            }}
            alQuitado={() => {
              enfocada = null;
              sinBus('obras');
            }}
          />
        {/await}
      </Panel>
    {/if}
  {/snippet}
</AppShell>

<style>
  .nb-poniendo-al-dia {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--nb-space-4);
    background: var(--m3c-surface);
    color: var(--m3c-on-surface);
  }

  .nb-division {
    display: flex;
    height: 100%;
    min-height: 0;
  }

  .nb-division[data-activa='true'] .nb-mitad + .nb-mitad {
    border-left: 1px solid var(--nb-border);
  }

  /*
   * Cuál de las dos manda. Sin marcarlo, con dos lectores idénticos en pantalla
   * no hay forma de saber a cuál va la siguiente tecla, y `Ctrl+Tab` parecería
   * no hacer nada.
   */
  .nb-division[data-activa='true'] .nb-mitad[data-enfocada='false'] {
    opacity: 0.72;
  }

  /*
   * La marca va en una capa encima y no como sombra interior del contenedor: el
   * lector pinta su propio fondo, así que una `inset` del padre queda debajo y
   * no se ve. `pointer-events: none` para que la raya no se coma ningún clic.
   */
  .nb-division[data-activa='true'] .nb-mitad {
    position: relative;
  }

  .nb-division[data-activa='true'] .nb-mitad[data-enfocada='true']::after {
    content: '';
    position: absolute;
    inset: 0 0 auto 0;
    height: 2px;
    background: var(--nb-accent);
    pointer-events: none;
    z-index: 5;
  }

  .nb-mitad {
    min-width: 0;
    height: 100%;
  }

  .nb-titulo {
    margin: 0 var(--nb-space-2);
    min-width: 0;
    font-family: var(--nb-font-display);
    font-size: var(--nb-text-lg);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Para quien lo oye, fuera de la vista. Ver el comentario del marcado. */
  .nb-solo-lector {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .nb-separador {
    flex: 1 1 auto;
  }

  /*
   * La caja entera es el campo, y el icono y el botón de borrar van encima.
   *
   * Con el campo dentro de una píldora, el anillo de foco rodeaba solo el texto:
   * un rectángulo dentro de una cápsula, que se leía como un defecto. Siendo el
   * campo la cápsula, el anillo sigue su forma.
   */
  .nb-buscador {
    position: relative;
    display: flex;
    align-items: center;
    flex: 0 1 40rem;
    min-width: 10rem;
    /*
     * **Lo que hace que escribir no cueste una disposición de la pantalla
     * entera.** Medido en el motor real el 2026-09-18: cada letra tardaba 133 ms
     * en la biblioteca y 109 en Inicio, y de ellos entre 95 y 114 eran
     * disposición; un campo suelto costaba 17 ms y cero de disposición. El texto
     * que se escribe cambia el tamaño intrínseco del campo, y sin contención eso
     * ensucia a todos sus antepasados hasta la raíz, que aquí son la barra
     * superior y la carcasa enteras.
     */
    contain: layout;
  }

  .nb-buscador-icono {
    position: absolute;
    left: var(--nb-space-5);
    display: flex;
    color: var(--m3c-on-surface-variant);
    pointer-events: none;
  }

  .nb-buscador-entrada {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 3.5rem;
    padding: 0 var(--nb-space-7) 0 calc(var(--nb-space-5) + var(--nb-space-6));
    border: none;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-md);
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-buscador[data-abierto='true'] .nb-buscador-entrada {
    background: var(--m3c-surface-container-highest);
  }

  .nb-buscador-entrada::placeholder {
    color: var(--m3c-on-surface-variant);
  }

  /* El borrar del navegador sobra: hay uno propio, con nombre y con foco. */
  .nb-buscador-entrada::-webkit-search-cancel-button {
    appearance: none;
    -webkit-appearance: none;
    /* WebKitGTK lo pintaba igual con solo `appearance`, al lado del propio. */
    display: none;
  }

  .nb-buscador-borrar {
    position: absolute;
    right: var(--nb-space-2);
    display: flex;
  }

  .nb-leyendo {
    display: flex;
    align-items: center;
    gap: var(--nb-space-3);
  }

  .nb-leyendo-texto {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
  }

  .nb-leyendo-barra {
    display: block;
    height: 4px;
    border-radius: 2px;
    background: var(--m3c-surface-container-highest);
    overflow: hidden;
  }

  .nb-leyendo-barra span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--m3c-primary);
  }

  .nb-lateral-tarjeta-detalle {
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }

  /*
   * El aviso de recuperación no se puede cerrar y va arriba de todo: mientras
   * esté, la biblioteca que se ve no es la del usuario, y eso no es un detalle
   * que se pueda descartar y olvidar.
   */
  .nb-recuperacion {
    display: flex;
    gap: var(--nb-space-3);
    margin: var(--nb-space-3);
    padding: var(--nb-space-3);
    border: 1px solid var(--nb-danger);
    border-radius: var(--nb-radius-md);
    background: var(--nb-surface-raised);
    color: var(--nb-text);
  }

  .nb-recuperacion-titulo {
    margin: 0;
    font-weight: 600;
  }

  .nb-recuperacion-ayuda {
    margin: var(--nb-space-1) 0 0;
    color: var(--nb-text-muted);
    font-size: var(--nb-text-sm);
  }

  .nb-recuperacion-archivo {
    margin: var(--nb-space-1) 0 0;
    font-family: var(--nb-font-mono);
    font-size: var(--nb-text-sm);
  }
</style>
