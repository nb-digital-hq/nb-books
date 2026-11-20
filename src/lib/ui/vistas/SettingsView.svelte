<!--
  @component
  Ajustes: el plano de la aplicación, separado del plano del libro.

  Contrato: ui-components.md, secciones 4 y 5.1.

  **Los dos planos están separados a propósito** y aquí solo se toca el de la
  aplicación: la tipografía del libro se ajusta leyendo, que es donde se ve el
  efecto. Cambiar el tema no altera cómo se ve un libro, y al revés.

  El tema se elige **viéndolo**, no por su nombre en una lista: cada uno enseña
  su paleta, su letra y su redondeo en una miniatura de la propia interfaz. Un
  desplegable con siete palabras obligaría a probarlos de uno en uno.

  **Ahora se recorre por secciones**, como la configuración de Windows: a
  la izquierda las secciones y a la derecha la que está abierta. Con doce
  secciones seguidas en una sola columna, llegar al diagnóstico era desplazarse
  por todo lo demás, y quien entraba a cambiar el tema tenía delante los permisos
  de red de las extensiones. Y aquí vive **Acerca de**: la versión, el estado de
  cada componente del runtime y el diagnóstico, que antes estaban en un panel de
  la barra superior que solo aparecía cuando algo iba mal.
-->
<script lang="ts" module>
  import type { AiProbe, AiProviderStatus, AiReport, AiSend } from '$lib/bindings';

  /**
   * El proveedor de IA externo, como lo pinta Ajustes. Va en un solo
   * objeto y con una sola acción porque todo esto vive en el paquete de arranque,
   * que tiene un presupuesto de 250 KB, y diez propiedades sueltas lo pasaban.
   */
  export interface EstadoDeIa {
    /** Nunca trae la credencial. */
    proveedor: AiProviderStatus | null;
    /**
     * La última prueba. Con `needsConsent`, el mensaje entero que saldría: se
     * enseña antes del primer envío, y no se ha enviado nada.
     */
    prueba: AiProbe | null;
    probando: boolean;
    /** Lo que se ha enviado, de lo más reciente a lo más antiguo. */
    envios: AiSend[];
    /** Lo que se ha marcado como incorrecto. */
    reportes: AiReport[];
  }

  export type AccionDeIa =
    | 'probar'
    | 'autorizarYProbar'
    | 'descartarPrueba'
    | 'guardarClave'
    | 'borrarClave'
    | 'borrarEnvios'
    | 'borrarReportes'
    | 'exportarReportes';

  /** Las secciones de Ajustes. */
  export type SeccionDeAjustes =
    | 'apariencia'
    | 'accesibilidad'
    | 'idioma'
    | 'voz'
    | 'modelos'
    | 'red'
    | 'telefonos'
    | 'catalogos'
    | 'diccionarios'
    | 'extensiones'
    | 'indice'
    | 'biblioteca'
    | 'acerca';
  /** Los tres alcances de borrado que ofrece Ajustes. */
  export type AlcanceDeBorrado = 'historial' | 'anotaciones' | 'biblioteca';
</script>

<script lang="ts">
  import { IDIOMAS, idioma, NOMBRE_DEL_IDIOMA, textos } from '$lib/i18n';
  import { traducirError } from '$lib/i18n/errores';
  import { TEXTOS_DE_AJUSTES } from '$lib/i18n/locales/ajustes';
  import { TEXTOS_DE_IA } from '$lib/i18n/locales/ia';
  import type {
    AiInstallProgress,
    AiModels,
    AvailabilityReport,
    Catalog,
    ComponentStatus,
    CredentialStatus,
    HealthReport,
    IndexingReport,
    DiagnosticsPackage,
    DictionaryManifest,
    ExtensionApi,
    ExtensionPending,
    ExtensionTheme,
    ExtensionView,
    LegalNotices,
    LibraryRootView,
    NetworkReport,
    NeuralVoice,
    Withdrawal
  } from '$lib/bindings';
  import {
    clearAnnotationsAndNotes,
    clearLibrary,
    clearReadingHistory,
    legalNotices
  } from '$core/ipc/datos';
  import { neuralVoice } from '$core/ipc/voz';
  import type { Setting, SettingValue } from '$lib/bindings';

  import Field from '../Field.svelte';
  import Icon from '../Icon.svelte';
  import Spinner from '../Spinner.svelte';
  import type { NombreDeIcono } from '../iconos';
  import Logo from '../Logo.svelte';
  import Range from '../Range.svelte';
  import Select from '../Select.svelte';
  import Button from '../Button.svelte';
  import Switch from '../Switch.svelte';
  import {
    ESCALA_MAXIMA,
    ESCALA_MINIMA,
    esquemaDe,
    resolverTema,
    type Tema,
    TEMAS,
    type TemaResuelto
  } from '../tema';
  import Seccion from './Seccion.svelte';
  import ActualizacionPanel from './ActualizacionPanel.svelte';
  import TelefonosPanel from './TelefonosPanel.svelte';

  interface Props {
    preferencias: Setting[];
    alCambiar: (clave: string, valor: SettingValue) => void;
    /**
     * Si el sistema pide esquema oscuro. Entra por propiedad y no se consulta
     * aquí porque tiene que ser reactivo: quien lo escucha es la página, y
     * preguntarlo al montar dejaría la muestra del tema del sistema congelada.
     */
    esquemaOscuro?: boolean;
    etiquetas?: Readonly<Record<string, string>>;
    /**
     * Diagnóstico ya preparado, o nulo si todavía no se ha pedido.
     *
     * Se pide, no se prepara solo: montarlo lee el registro del disco, y quien
     * abre los ajustes para cambiar el tema no está pidiendo eso.
     */
    diagnostico?: DiagnosticsPackage | null;
    diagnosticoGuardado?: boolean;
    alPrepararDiagnostico?: () => void;
    alGuardarDiagnostico?: () => void;
    /**
     * Diccionarios instalados.
     *
     * Viven aquí y no en el panel de consulta del lector porque instalar es una
     * acción de configuración, no de lectura: quien está leyendo y encuentra una
     * palabra que no conoce no quiere que le pidan elegir un archivo. El panel
     * del lector solo dice que faltan y dónde se ponen.
     */
    diccionarios?: DictionaryManifest[];
    alInstalarDiccionario?: () => void;
    alDesinstalarDiccionario?: (id: string) => void;
    /** Lo instalado, con su estado y sus permisos. */
    extensiones?: ExtensionView[];
    /**
     * Qué versión de la API de extensiones habla la aplicación.
     *
     * Está aquí porque la sección 2.2 de `versioning.md` promete que la
     * aplicación declara la versión de la API, y el sitio donde eso le importa
     * a alguien es este, delante de la lista de lo
     * instalado. El número viene del anfitrión y no se escribe en el frontend.
     */
    api?: ExtensionApi | null;
    /** Los temas que aportan las extensiones instaladas. */
    temasDeExtension?: ExtensionTheme[];
    /** Cuál está puesto, o nulo si manda el del núcleo. */
    temaDeExtensionActivo?: string | null;
    alElegirTemaDeExtension?: (extension: string | null) => void;
    /**
     * La que está esperando permiso, si hay alguna.
     *
     * Va aparte de la lista porque **todavía no está instalada**: mezclarla
     * daría a entender que ya entró, que es justo lo que no ha pasado.
     */
    pendiente?: ExtensionPending | null;
    /** Las generaciones a las que se puede volver. */
    generacionesDeExtensiones?: number[];
    /**
     * Lo que el registro ha retirado y está instalado aquí.
     *
     * Sale de lo guardado en la última comprobación, así que se ve aunque hoy
     * no haya red: una extensión apagada por maliciosa sin el motivo delante
     * sería una desaparición sin explicación.
     */
    retiradas?: Withdrawal[];
    alComprobarRetiradas?: () => void;
    alInstalarExtension?: () => void;
    alConcederPendiente?: (anfitrion: string) => void;
    alHabilitarExtension?: (id: string, activa: boolean) => void;
    alDesinstalarExtension?: (id: string) => void;
    alRevocarExtension?: (id: string, anfitrion: string) => void;
    alVolverAtrasExtensiones?: () => void;
    /**
     * Lo que sale de este equipo: permisos concedidos y registro de salidas.
     *
     * Es la mitad del innegociable de red que no se cumple impidiendo la salida:
     * un permiso concedido hace seis meses y olvidado es indistinguible de uno
     * que nunca se pidió si no hay dónde mirarlo.
     */
    red?: NetworkReport | null;
    alRevocar?: (host: string, purpose: string) => void;
    /** Concede la salida a un destino que se conoce y aún no la tiene. */
    alAutorizar?: (host: string, cleartext: boolean) => void;
    /**
     * Concede la salida al servicio de síntesis configurado.
     *
     * Sin anfitrión: lo pone el runtime desde lo que se escribió arriba. Un
     * comando que aceptara cualquiera dejaría que la interfaz decidiera a dónde
     * sale el texto de los libros.
     */
    alAutorizarVoz?: () => void;
    /**
     * Si cabe guardar una clave aquí y si el servidor ya tiene una.
     *
     * Nunca trae la clave: ver `CredentialStatus`. Con `keyringAvailable` en
     * falso no se ofrece guardar nada, porque el repliegue a texto plano está
     * prohibido y ofrecer un campo que no puede cumplir sería mentir.
     */
    credencial?: CredentialStatus | null;
    /**
     * La dirección exacta a la que saldría el texto, tal y como la entiende el
     * runtime. Se enseña debajo del servidor: se puede escribir de muchas formas,
     * y lo que importa es la que resulta.
     */
    puntoDeVoz?: string | null;
    /** Guarda la clave del servidor configurado en el llavero del sistema. */
    alGuardarCredencial?: (secreto: string) => void;
    /** Borra del llavero la clave del servidor configurado. */
    alBorrarCredencial?: () => void;
    /**
     * Catálogos OPDS añadidos.
     *
     * Se añaden aquí y no en Descubrir porque añadir uno es **configurar**, no
     * descubrir: se hace una vez y no se vuelve. Y porque Descubrir no existe
     * hasta que hay uno, así que un formulario allí no lo vería nadie.
     */
    catalogos?: Catalog[];
    alAnadirCatalogo?: (titulo: string, url: string) => void;
    alQuitarCatalogo?: (id: string) => void;
    /**
     * Dónde viven los archivos de la biblioteca, y cuántos no están ahí.
     *
     * Está aquí porque mudar los libros de disco es lo que convierte una
     * biblioteca en una lista de documentos que no abren, y reapuntar una raíz
     * es la operación entera que lo deshace. Sin esta sección la capacidad
     * existía en el runtime y no había forma de usarla.
     */
    /**
     * Avance de la indexación de la biblioteca entera.
     *
     * Está en Ajustes y no en el panel de búsqueda porque es un dato de la
     * biblioteca: en el panel importa si **este** libro se puede buscar ya, que
     * es otra pregunta. Rehacer el índice, además, son horas sobre un catálogo
     * real, y una acción así no va donde se busca.
     */
    indice?: IndexingReport | null;
    indicePausado?: boolean;
    alPausarIndice?: (pausado: boolean) => void;
    alRehacerIndice?: () => void;
    raices?: LibraryRootView[];
    revisandoRaices?: boolean;
    /** Recuento de la última revisión, o nulo si todavía no se ha hecho una. */
    revision?: AvailabilityReport | null;
    alRevisarRaices?: () => void;
    alReapuntarRaiz?: (rootId: string, ruta: string) => void;
    /**
     * Exportar la biblioteca entera. Va con las carpetas porque responde a la misma
     * pregunta, dónde está lo tuyo, y es lo que se hace antes de mudarse.
     */
    alExportarBiblioteca?: () => void;
    exportandoBiblioteca?: boolean;
    /**
     * La sección abierta.
     *
     * Se ata desde fuera porque hay otros sitios que llevan a una en concreto: la
     * tarjeta del índice de la barra lateral abre la suya, y la de "algo necesita
     * atención" abre Acerca de.
     */
    seccion?: SeccionDeAjustes;
    /** Versión y esquema, para Acerca de. */
    informe?: HealthReport | null;
    /** Cómo está cada componente del runtime, para Acerca de. */
    componentes?: ComponentStatus[];
    /** El último fallo que se avisó, si lo hubo: es donde se mira cuando algo va mal. */
    ultimoFallo?: string | null;
    /**
     * Los modelos de IA del catálogo. La sección que
     * [ADR-013](../../../../docs/adr/decisions/ADR-013-information-architecture.md)
     * llama "IA y modelos" y que no existía.
     */
    modelos?: AiModels | null;

    /** El avance de la descarga en curso, que llega por evento. */
    avanceDeModelo?: AiInstallProgress | null;
    /**
     * El anfitrión al que la descarga necesita salir y aún no tiene permiso: la
     * red de distribución a la que redirige el del catálogo.
     */
    modeloPideAutorizar?: string | null;
    /** El código del último fallo al instalar, para decirlo en contexto. */
    falloDeModelo?: string | null;
    /** Autorizar el anfitrión del catálogo y empezar a descargar. */
    alDescargarModelo?: (id: string, host: string) => void;
    /** Autorizar el anfitrión que pidió la descarga y seguir. */
    alAutorizarModelo?: (host: string) => void;
    alCancelarModelo?: () => void;
    alQuitarModelo?: (id: string) => void;
    /** El proveedor de IA externo. */
    ia?: EstadoDeIa;
    /** Lo que se pide sobre él. `secreto` solo acompaña a `guardarClave`. */
    alIa?: (accion: AccionDeIa, secreto?: string) => void;
    /** Quitar un aviso suelto. Va aparte porque lleva identificador. */
    alQuitarReporte?: (id: number) => void;
    /** Avisa a la página cuando uno de los tres alcances de borrado se completó. */
    alBorrarDatos?: (alcance: AlcanceDeBorrado) => void;
    /**
     * Textos legales (`LICENSE`, `NOTICE` y `THIRD-PARTY-NOTICES.md`) ya cargados,
     * o nulo para que se pidan al runtime desde `Acerca de`.
     */
    avisosLegales?: LegalNotices | null;
  }

  let {
    preferencias,
    alCambiar,
    esquemaOscuro = false,
    etiquetas,
    diagnostico = null,
    diagnosticoGuardado = false,
    alPrepararDiagnostico,
    alGuardarDiagnostico,
    diccionarios = [],
    alInstalarDiccionario,
    alDesinstalarDiccionario,
    extensiones = [],
    api = null,
    temasDeExtension = [],
    temaDeExtensionActivo = null,
    alElegirTemaDeExtension,
    pendiente = null,
    generacionesDeExtensiones = [],
    retiradas = [],
    alComprobarRetiradas,
    alInstalarExtension,
    alConcederPendiente,
    alHabilitarExtension,
    alDesinstalarExtension,
    alRevocarExtension,
    alVolverAtrasExtensiones,
    red = null,
    alAutorizar,
    alAutorizarVoz,
    credencial = null,
    puntoDeVoz = null,
    alGuardarCredencial,
    alBorrarCredencial,
    alRevocar,
    catalogos = [],
    alAnadirCatalogo,
    alQuitarCatalogo,
    indice = null,
    indicePausado = false,
    alPausarIndice,
    alRehacerIndice,
    raices = [],
    revisandoRaices = false,
    revision = null,
    alRevisarRaices,
    alReapuntarRaiz,
    alExportarBiblioteca,
    exportandoBiblioteca = false,
    seccion = $bindable('apariencia'),
    informe = null,
    componentes = [],
    ultimoFallo = null,
    modelos = null,
    avanceDeModelo = null,
    modeloPideAutorizar = null,
    falloDeModelo = null,
    alDescargarModelo,
    alAutorizarModelo,
    alCancelarModelo,
    alQuitarModelo,
    ia = { proveedor: null, prueba: null, probando: false, envios: [], reportes: [] },
    alIa,
    alQuitarReporte,
    alBorrarDatos,
    avisosLegales = null
  }: Props = $props();

  const proveedorDeIa = $derived(ia.proveedor);
  const pruebaDeIa = $derived(ia.prueba);
  const enviosDeIa = $derived(ia.envios);
  const reportesDeIa = $derived(ia.reportes);

  /**
   * El modelo cuya descarga se está confirmando.
   *
   * Descargar no es un botón que descarga: es un botón que enseña **antes** qué
   * se va a traer, de quién es, bajo qué licencia y a dónde se sale, y solo
   * entonces ofrece hacerlo. Es el criterio —el usuario ve la licencia
   * antes de descargar— y el de privacidad —el consentimiento se da con el
   * anfitrión y lo que se envía delante—, en el mismo paso.
   */
  let confirmandoModelo = $state<string | null>(null);
  /**
   * El modelo que se va a usar **aunque probablemente cierre la aplicación**, a la
   * espera de la confirmación fuerte (2026-09-22).
   */
  let confirmandoUso = $state<string | null>(null);

  /** Alcance de borrado que espera confirmación del usuario. */
  let confirmandoBorrado = $state<AlcanceDeBorrado | null>(null);
  let borrandoDatos = $state(false);
  let borradoReciente = $state<AlcanceDeBorrado | null>(null);
  let falloDeBorrado = $state<string | null>(null);

  /** Avisos legales cargados bajo demanda en `Acerca de`. */
  let avisosCargados = $state<LegalNotices | null>(null);
  const avisosMostrados = $derived(avisosLegales ?? avisosCargados);

  async function cargarAvisosLegales() {
    if (avisosMostrados) return;
    try {
      avisosCargados = await legalNotices();
    } catch {
      // No falla en el runtime real; si en un entorno simulado no está, no rompe la vista.
    }
  }

  $effect(() => {
    if (seccion === 'acerca' && !avisosMostrados) {
      void cargarAvisosLegales();
    }
  });

  async function ejecutarBorrado(alcance: AlcanceDeBorrado) {
    if (borrandoDatos) return;
    borrandoDatos = true;
    borradoReciente = null;
    falloDeBorrado = null;
    try {
      if (alcance === 'historial') await clearReadingHistory();
      else if (alcance === 'anotaciones') await clearAnnotationsAndNotes();
      else await clearLibrary();
      confirmandoBorrado = null;
      borradoReciente = alcance;
      alBorrarDatos?.(alcance);
    } catch (e) {
      falloDeBorrado = traducirError(e);
    } finally {
      borrandoDatos = false;
    }
  }

  /** Una espera por función, dicha como se dice. */
  function minutosDeModelo(segundos: number): string {
    if (segundos < 60) return texto('modeloMenosDeUnMinuto');
    return `${texto('modeloUnos')} ${Math.round(segundos / 60)} ${texto('modeloMinutos')}`;
  }

  /**
   * La voz neuronal, **pedida desde aquí y no recibida como propiedad**.
   *
   * Preguntar carga el modelo y mide la máquina, así que se hace cuando su tarjeta
   * se va a ver y no al arrancar; y se pide aquí porque esta vista se carga al
   * abrir Ajustes, mientras que la pantalla principal entra en el paquete de
   * arranque, que tiene presupuesto.
   */
  let vozNeuronal = $state<NeuralVoice | null>(null);
  let cargandoVoz = $state(false);

  $effect(() => {
    if (seccion !== 'voz' || cadena('voice_provider', 'local') !== 'neural') return;
    // Aquí sí se mide: es donde se enseña el factor, y quien abre esta sección
    // quiere saberlo. El narrador pregunta sin medir, para no cargar 600 MB al
    // arrancar.
    cargandoVoz = true;
    void neuralVoice(true)
      .then((vista) => {
        vozNeuronal = vista;
      })
      .catch(() => {
        vozNeuronal = null;
      })
      .finally(() => {
        cargandoVoz = false;
      });
  });

  /** Megabytes, con un decimal: un modelo pesa cientos y un giga se lee peor. */
  const megas = (bytes: number) => `${unDecimal.format(bytes / 1024 ** 2)} MB`;

  /**
   * Las secciones, en el orden en que se ofrecen: primero cómo se ve y se usa,
   * después qué sale del equipo y de dónde viene lo que se lee, y al final lo que
   * se mira cuando algo va mal.
   */
  const SECCIONES: ReadonlyArray<readonly [SeccionDeAjustes, NombreDeIcono, string]> = [
    ['apariencia', 'apariencia', 'apariencia'],
    ['accesibilidad', 'accesibilidad', 'accesibilidad'],
    ['idioma', 'idioma', 'idioma'],
    ['voz', 'voz', 'voz'],
    ['modelos', 'ia', 'modelos'],
    ['red', 'privacidad', 'red'],
    ['telefonos', 'telefono', 'movilSeccion'],
    ['catalogos', 'descubrir', 'catalogos'],
    ['diccionarios', 'diccionario', 'diccionarios'],
    ['extensiones', 'extensiones', 'extensiones'],
    ['indice', 'buscar', 'indice'],
    ['biblioteca', 'almacenamiento', 'biblioteca'],
    ['acerca', 'informacion', 'acercaDe']
  ];

  let raiz: HTMLElement | undefined = $state();

  // Cambiar de sección vuelve arriba: abrir una y aparecer a mitad de ella, donde
  // se había dejado la anterior, se lee como que no ha cambiado nada.
  $effect(() => {
    void seccion;
    raiz?.closest('main')?.scrollTo?.({ top: 0 });
  });

  /**
   * La clave mientras se escribe, y solo mientras.
   *
   * No se precarga con la que hubiera guardada porque el runtime no la devuelve:
   * lo que llega es si existe, no cuál es. Se vacía en cuanto se guarda.
   */
  let claveEscrita = $state('');
  /** La clave del proveedor de IA, de paso: no se queda en el DOM tras guardarla. */
  let claveDeIaEscrita = $state('');

  /** Los protocolos que se ofrecen: los mismos que admite el runtime. */
  const PROTOCOLOS_DE_IA = ['none', 'openai', 'anthropic'] as const;
  const GENERADORES_DE_IA = ['local', 'external'] as const;

  /**
   * De dónde sale la voz. Los mismos tres que admite el runtime, y en este orden:
   * primero la que ya está en la máquina, después la que se descarga y funciona
   * sin red, y al final la que manda el texto del libro a otro ordenador.
   */
  const ORIGENES_DE_VOZ = ['local', 'neural', 'remote'] as const;

  /** Qué suele querer decir un rechazo, para no dejar solo un número. */
  function motivoDeRechazo(status: number): string {
    if (status === 401 || status === 403) return texto('iaRechazoClave');
    if (status === 404) return texto('iaRechazoModelo');
    if (status === 429) return texto('iaRechazoLimite');
    return texto('iaRechazoOtro');
  }

  const fechaDeEnvio = $derived(
    new Intl.DateTimeFormat(idioma(), { dateStyle: 'medium', timeStyle: 'short' })
  );

  // El diccionario del idioma en uso, con las anulaciones de quien monta el
  // componente encima. `$derived` y no una constante: cambiar de idioma tiene
  // que repintar, y una constante capturaría el diccionario de un solo momento.
  // Los textos de la IA viajan con Ajustes y no en el diccionario del arranque:
  // ver `locales/ia.ts`.
  const t = $derived({
    // El grupo entero viaja con esta vista y no en el diccionario del arranque:
    // son once kilobytes que solo se usan aquí. Ver `locales/ajustes.ts`.
    ...TEXTOS_DE_AJUSTES[idioma()],
    ...TEXTOS_DE_IA[idioma()],
    ...etiquetas
  } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  /** Solo el plano de la aplicación: el del libro se ajusta leyendo. */
  const valores = $derived(
    new Map(preferencias.filter((p) => p.plane === 'application').map((p) => [p.key, p.value]))
  );

  const cadena = (clave: string, defecto: string): string => {
    const valor = valores.get(clave);
    return typeof valor === 'string' ? valor : defecto;
  };

  /**
   * El aviso de retirada de una extensión, si lo hay.
   *
   * Va en la fila de esa extensión y no en una lista aparte, porque lo que hace
   * falta saber es **cuál de las que tengo** está retirada: una lista al margen
   * obligaría a cruzar dos sitios para responder a eso.
   */
  const retiradaDe = (id: string): Withdrawal | undefined =>
    retiradas.find((retirada) => retirada.extension === id);

  /*
   * Si el servicio de síntesis ya tiene permiso.
   *
   * Sale del informe de red y no de una bandera propia: quien sabe qué está
   * autorizado es la puerta, y una segunda copia aquí acabaría diciendo que sí
   * cuando el permiso se hubiera revocado desde la lista de abajo.
   */
  /**
   * Si el servidor de voz está en la red local, en claro, y no en este equipo.
   *
   * **Esto no decide nada**: quien admite o rechaza una dirección es `net.rs`,
   * y esta comprobación solo elige si se enseña el aviso. Por eso puede ser
   * aproximada sin que eso abra nada; si se equivocara, lo peor que pasa es que
   * se avise de más.
   *
   * Se avisa porque hacia este mismo equipo no hay red que escuche y hacia
   * `192.168.x.x` sí la hay: el texto del libro va sin cifrar por ella. Un
   * permiso que no dice eso no es informado.
   */
  const vozEnClaroPorLaRed = $derived.by(() => {
    // Se mira la dirección que resulta y no la escrita: sin esquema, a la red
    // propia se habla en claro, y el aviso tiene que salir igual.
    const escrito = (puntoDeVoz ?? cadena('speech_host', '')).trim();
    if (!escrito.startsWith('http://')) return false;

    const autoridad = escrito.slice('http://'.length).split(/[/?#]/)[0] ?? '';
    const anfitrion = autoridad.split('@').pop() ?? '';
    const host = (
      anfitrion.startsWith('[')
        ? anfitrion.slice(1, anfitrion.indexOf(']'))
        : (anfitrion.split(':')[0] ?? '')
    ).toLowerCase();
    if (host === 'localhost' || host.startsWith('127.') || host === '::1') return false;
    return host.length > 0;
  });

  const vozAutorizada = $derived(
    (red?.consents ?? []).some(
      (permiso) =>
        permiso.purpose === 'speech' && permiso.host === anfitrionDe(cadena('speech_host', ''))
    )
  );

  const ORDEN_PROPOSITOS: readonly string[] = [
    'catalog',
    'catalog-cleartext',
    'metadata',
    'lookup',
    'update',
    'extension',
    'registry',
    'speech',
    'model',
    'ai'
  ];

  /**
   * Permisos concedidos agrupados por propósito.
   *
   * Que muchos permisos no pesen sin que conceder se vuelva "permitir todo":
   * Ajustes agrupa los permisos por propósito con los anfitriones dentro, y se
   * concede y se revoca por anfitrión y propósito individualmente.
   */
  const permisosPorProposito = $derived.by(() => {
    if (!red || red.consents.length === 0) return [];
    const mapa = new Map<string, typeof red.consents>();
    for (const permiso of red.consents) {
      const lista = mapa.get(permiso.purpose) ?? [];
      lista.push(permiso);
      mapa.set(permiso.purpose, lista);
    }
    return Array.from(mapa.entries())
      .map(([purpose, consents]) => ({
        purpose,
        consents: [...consents].sort(
          (a, b) => a.host.localeCompare(b.host) || a.origin.localeCompare(b.origin)
        )
      }))
      .sort((a, b) => {
        const ordenA = ORDEN_PROPOSITOS.indexOf(a.purpose);
        const ordenB = ORDEN_PROPOSITOS.indexOf(b.purpose);
        const posA = ordenA === -1 ? 999 : ordenA;
        const posB = ordenB === -1 ? 999 : ordenB;
        if (posA !== posB) return posA - posB;
        return a.purpose.localeCompare(b.purpose);
      });
  });

  /** El anfitrión de lo que se escribió, sin esquema, puerto ni ruta. */
  function anfitrionDe(escrito: string): string {
    const limpio = escrito.trim().replace(/^https?:\/\//, '');
    const autoridad = limpio.split(/[/?#]/)[0] ?? '';
    return (autoridad.split('@').pop() ?? '').replace(/:\d+$/, '').toLowerCase();
  }

  const numero = (clave: string, defecto: number): number => {
    const valor = valores.get(clave);
    return typeof valor === 'number' ? valor : defecto;
  };
  const bandera = (clave: string): boolean => valores.get(clave) === true;

  const temaElegido = $derived(cadena('theme', 'system'));
  const acentoElegido = $derived(cadena('accent_color', 'theme'));

  /** Tema real detrás del comodín, para vestir con él lo que no puede heredarlo. */
  const temaResuelto = $derived(resolverTema(temaElegido as Tema, esquemaOscuro));

  /**
   * Acento a pintar en las miniaturas.
   *
   * Nulo cuando manda el tema —cada miniatura enseña entonces el suyo, que es
   * parte de lo que distingue un tema de otro— y nulo también con alto
   * contraste, que trae el suyo medido y gana a cualquiera.
   *
   * `theme` sigue siendo un valor válido del ajuste: es lo que hay guardado
   * mientras nadie elija color, y lo que deja el acento en manos del tema. Lo
   * que se retiró es su **casilla**: con seis colores fijos al lado, una
   * séptima que a veces coincide con uno de ellos solo se lee como repetida.
   */
  const acentoDeLaMuestra = $derived(
    acentoElegido !== 'theme' && !bandera('high_contrast') ? acentoElegido : null
  );

  /**
   * El tema de extensión que manda sobre el acento, si hay uno puesto, con el
   * color que trae. Ver la sección del acento.
   */
  const temaQueManda = $derived.by(() => {
    const tema = temasDeExtension.find((t) => t.extension === temaDeExtensionActivo);
    const color = tema?.tokens['--nb-accent'];
    return tema && color ? { nombre: tema.name, color } : null;
  });

  /**
   * Lo que se dice del estado de un componente: su motivo cuando lo trae y hay
   * palabras para él, y el estado a secas cuando no.
   *
   * Antes se decía solo el estado, y `STOPPED` es "Detenido" tanto
   * para lo que se paró como para lo que no existe todavía: Acerca de enseñaba
   * "Detenido" junto a la sincronización y la institución, que ninguna versión
   * ha traído nunca. El motivo es lo que distingue una cosa de la otra, y ya
   * viajaba con el estado.
   */
  function etiquetaDeEstado(componente: ComponentStatus): string {
    const porMotivo = componente.reasonCode ? t[`motivo-${componente.reasonCode}`] : undefined;
    return porMotivo ?? texto(`estado-${componente.state}`);
  }

  /**
   * Los topes de memoria para la IA que se ofrecen, en MiB. Los mismos que admite
   * el runtime (`settings::MEMORIAS_DE_IA_ADMITIDAS`), que es quien los hace
   * cumplir.
   */
  const MEMORIAS_DE_IA = [512, 1024, 2048, 4096] as const;

  /**
   * Los hilos para la IA que se ofrecen: automático y las potencias de dos que
   * dejan al menos un núcleo libre. El runtime admite cualquier entero hasta los
   * núcleos menos uno; ofrecerlos todos serían dieciséis botones en un equipo de
   * dieciséis núcleos para una diferencia que no se nota.
   */
  function hilosOfrecidos(nucleos: number): number[] {
    return [0, ...[1, 2, 4, 8, 16, 32].filter((n) => n <= nucleos - 1)];
  }

  /**
   * Los topes de lo derivado que se ofrecen. Los mismos que admite el runtime
   * (`settings::PRESUPUESTOS_ADMITIDOS`), que es quien los hace cumplir.
   */
  const TOPES = [20, 30, 40] as const;

  const cifra = $derived(new Intl.NumberFormat(idioma()));
  const unDecimal = $derived(new Intl.NumberFormat(idioma(), { maximumFractionDigits: 1 }));
  const gigas = (bytes: number) => `${unDecimal.format(bytes / 1024 ** 3)} GB`;

  /** Todos los temas más el del sistema, que va el último por ser el comodín. */
  const opciones = $derived<Array<TemaResuelto | 'system'>>([...TEMAS, 'system']);

  /**
   * Acentos ofrecidos.
   *
   * Seis y no una rueda de color: una rueda invita a elegir un color que no
   * contrasta, y entonces el acento deja de verse en la mitad de los temas.
   * Estos seis están medidos contra los siete fondos.
   */
  /** Una marca de tiempo de la base, en la hora y el idioma de quien mira. */
  function fechaYHora(iso: string): string {
    const f = new Date(iso);
    if (Number.isNaN(f.getTime())) return iso;
    return new Intl.DateTimeFormat(idioma(), { dateStyle: 'medium', timeStyle: 'short' }).format(f);
  }

  const ACENTOS = ['#d9a441', '#c8674a', '#7fa88b', '#8a9ac9', '#b07fb0', '#6f9ec9'] as const;

  /**
   * El nombre de cada acento, para quien no lo ve: el nombre accesible era
   * el código, y un lector de pantalla leía "almohadilla d nueve a cuatro…".
   */
  const NOMBRE_DEL_ACENTO: Record<(typeof ACENTOS)[number], string> = {
    '#d9a441': 'acentoAmbar',
    '#c8674a': 'acentoTeja',
    '#7fa88b': 'acentoSalvia',
    '#8a9ac9': 'acentoLavanda',
    '#b07fb0': 'acentoMalva',
    '#6f9ec9': 'acentoCielo'
  };

  // Cada idioma se ofrece **en ese idioma**: quien busca el suyo lo reconoce por
  // su nombre, no por cómo lo llamamos nosotros. Sale del catálogo de i18n para
  // que añadir un idioma no obligue a acordarse de esta lista.
  const OPCIONES_DE_IDIOMA = IDIOMAS.map((codigo) => [codigo, NOMBRE_DEL_IDIOMA[codigo]] as const);

  // El catálogo que se está añadiendo. Vive aquí y no fuera porque es un
  // formulario, no un dato del sistema: al enviarlo se vacía y no queda nada.
  let nombreDelCatalogo = $state('');
  let direccionDelCatalogo = $state('');

  function anadirCatalogo() {
    const nombre = nombreDelCatalogo.trim();
    const direccion = direccionDelCatalogo.trim();
    if (!nombre || !direccion) return;
    alAnadirCatalogo?.(nombre, direccion);
    nombreDelCatalogo = '';
    direccionDelCatalogo = '';
  }
</script>

<div class="nb-ajustes" bind:this={raiz}>
  <nav class="nb-ajustes-nav" aria-label={texto('secciones')}>
    <!--
      El nombre de la vista ya es el encabezado de la barra superior. Aquí se ve,
      y no se repite para quien lo oye.
    -->
    <p class="nb-ajustes-titulo" aria-hidden="true">{texto('region')}</p>
    {#each SECCIONES as [clave, icono, etiqueta] (clave)}
      <button
        class="nb-ajustes-destino"
        type="button"
        aria-current={seccion === clave ? 'true' : undefined}
        onclick={() => (seccion = clave)}
      >
        <Icon nombre={icono} />
        <span class="nb-ajustes-destino-texto">{texto(etiqueta)}</span>
      </button>
    {/each}
  </nav>

  <div class="nb-ajustes-contenido" aria-label={texto('region')}>
    {#if seccion === 'apariencia'}
      <Seccion titulo={texto('apariencia')}>
        <p class="nb-ayuda">{texto('temaAyuda')}</p>

        <div class="nb-temas" role="radiogroup" aria-label={texto('tema')}>
          {#each opciones as tema (tema)}
            <button
              class="nb-tema"
              type="button"
              role="radio"
              aria-checked={temaDeExtensionActivo === null && temaElegido === tema}
              data-theme={tema === 'system' ? temaResuelto : tema}
              data-resolved-scheme={esquemaDe(tema === 'system' ? temaResuelto : tema)}
              data-contrast={bandera('high_contrast') ? 'high' : undefined}
              style:--nb-accent={acentoDeLaMuestra}
              onclick={() => {
                /*
                 * Elegir un tema del núcleo **retira el de la extensión**, y ese es
                 * el arreglo: los dos vivían en el mismo grupo de radio y los dos se
                 * marcaban a la vez, así que la pantalla decía que había dos temas
                 * puestos. Un tema es un tema, y de esos hay uno.
                 */
                if (temaDeExtensionActivo !== null) alElegirTemaDeExtension?.(null);
                alCambiar('theme', tema);
              }}
            >
              <!--
            La miniatura usa los tokens del tema que representa, así que no hay
            un solo color escrito aquí: el atributo `data-tema` los trae.
          -->
              <span class="nb-tema-muestra" aria-hidden="true">
                <span class="nb-tema-barra"></span>
                <span class="nb-tema-cuerpo">
                  <!-- Muestra de la letra del tema. check-i18n: no es texto -->
                  <span class="nb-tema-titulo">Aa</span>
                  <span class="nb-tema-linea"></span>
                  <span class="nb-tema-linea nb-tema-linea-corta"></span>
                  <span class="nb-tema-boton"></span>
                </span>
              </span>
              <span class="nb-tema-nombre">
                {texto(tema)}
                {#if temaDeExtensionActivo === null && temaElegido === tema}
                  <Icon nombre="comprobado" tamano={15} />
                {/if}
              </span>
            </button>
          {/each}

          <!--
        Y los que aportan las extensiones. Van en el mismo sitio que
        los del núcleo porque un tema es un tema, y con la misma miniatura: sus
        colores se aplican en línea, así que la muestra enseña de verdad lo que
        se va a ver y no un cuadrado de color.
        
        Se marcan como de extensión: se dice de dónde vienen, no se esconde.
      -->
          {#each temasDeExtension as tema (tema.extension)}
            <button
              class="nb-tema"
              type="button"
              role="radio"
              aria-checked={temaDeExtensionActivo === tema.extension}
              data-resolved-scheme="dark"
              style={Object.entries(tema.tokens)
                .map(([token, valor]) => `${token}: ${valor}`)
                .join('; ')}
              onclick={() =>
                alElegirTemaDeExtension?.(
                  temaDeExtensionActivo === tema.extension ? null : tema.extension
                )}
            >
              <span class="nb-tema-muestra" aria-hidden="true">
                <span class="nb-tema-barra"></span>
                <span class="nb-tema-cuerpo">
                  <!-- Muestra de la letra del tema. check-i18n: no es texto -->
                  <span class="nb-tema-titulo">Aa</span>
                  <span class="nb-tema-linea"></span>
                  <span class="nb-tema-linea nb-tema-linea-corta"></span>
                  <span class="nb-tema-boton"></span>
                </span>
              </span>
              <span class="nb-tema-nombre">
                {tema.name}
                {#if temaDeExtensionActivo === tema.extension}
                  <Icon nombre="comprobado" tamano={15} />
                {/if}
              </span>
            </button>
          {/each}
        </div>
        {#if temasDeExtension.length > 0}
          <p class="nb-ayuda">{texto('temasDeExtensionAyuda')}</p>
        {/if}
      </Seccion>

      <Seccion titulo={texto('acento')}>
        {#if temaQueManda}
          <!--
            Con un tema de extensión puesto, **el acento es suyo**: la paleta sale
            de su color, y el del usuario se queda guardado esperando a que se
            quite el tema. Antes se ofrecían igual los seis colores,
            se marcaba el elegido y no cambiaba nada al pulsarlos, que es un
            control que miente. Ahora se enseña el que manda, marcado y sin
            poderse cambiar, y se dice de quién es.
          -->
          <div class="nb-acentos" role="radiogroup" aria-label={texto('acento')}>
            <button
              class="nb-acento"
              type="button"
              role="radio"
              aria-checked="true"
              aria-disabled="true"
              aria-label={temaQueManda.color}
              style:background={temaQueManda.color}
              disabled
            >
              <Icon nombre="comprobado" tamano={15} />
            </button>
          </div>
          <p class="nb-ayuda">{texto('acentoDelTema')} {temaQueManda.nombre}</p>
        {:else}
          <div class="nb-acentos" role="radiogroup" aria-label={texto('acento')}>
            {#each ACENTOS as color (color)}
              <button
                class="nb-acento"
                type="button"
                role="radio"
                aria-checked={acentoElegido === color}
                aria-label={texto(NOMBRE_DEL_ACENTO[color])}
                style:background={color}
                onclick={() => alCambiar('accent_color', color)}
              >
                {#if acentoElegido === color}<Icon nombre="comprobado" tamano={15} />{/if}
              </button>
            {/each}
          </div>
        {/if}
        <!--
          Encendido de fábrica, y distinto de una bandera: apagarlo es la segunda
          mitigación de ADR-013 contra un Inicio que empuja, así que se lee como
          "no está apagado" y no como "está encendido".
        -->
        <div class="nb-fila">
          <span class="nb-fila-texto">
            {texto('sugerenciasEnInicio')}
            <span class="nb-ayuda">{texto('sugerenciasEnInicioAyuda')}</span>
          </span>
          <Switch
            etiqueta={texto('sugerenciasEnInicio')}
            activo={valores.get('home_suggestions') !== false}
            alCambiar={(v) => alCambiar('home_suggestions', v)}
          />
        </div>
      </Seccion>
    {:else if seccion === 'accesibilidad'}
      <Seccion titulo={texto('accesibilidad')}>
        <div class="nb-fila">
          <span class="nb-fila-texto">
            {texto('altoContraste')}
            <span class="nb-ayuda">{texto('altoContrasteAyuda')}</span>
          </span>
          <Switch
            etiqueta={texto('altoContraste')}
            activo={bandera('high_contrast')}
            alCambiar={(v) => alCambiar('high_contrast', v)}
          />
        </div>
        <div class="nb-fila">
          <span class="nb-fila-texto">
            {texto('movimientoReducido')}
            <span class="nb-ayuda">{texto('movimientoAyuda')}</span>
          </span>
          <Switch
            etiqueta={texto('movimientoReducido')}
            activo={bandera('reduced_motion')}
            alCambiar={(v) => alCambiar('reduced_motion', v)}
          />
        </div>
        <Field
          etiqueta="{texto('escala')}: {Math.round(numero('font_scale', 1) * 100)} %"
          descripcion={texto('escalaAyuda')}
        >
          {#snippet children({ id, 'aria-describedby': describedby })}
            <Range
              {id}
              etiqueta={texto('escala')}
              aria-describedby={describedby}
              valor={numero('font_scale', 1)}
              minimo={ESCALA_MINIMA}
              maximo={ESCALA_MAXIMA}
              paso={0.05}
              alCambiar={(v) => alCambiar('font_scale', v)}
            />
          {/snippet}
        </Field>
      </Seccion>
    {:else if seccion === 'idioma'}
      <Seccion titulo={texto('idioma')}>
        <Field etiqueta={texto('idiomaTexto')}>
          {#snippet children({ id })}
            <Select
              {id}
              etiqueta={texto('idiomaTexto')}
              valor={cadena('language', 'es')}
              opciones={OPCIONES_DE_IDIOMA}
              alCambiar={(v) => alCambiar('language', v)}
            />
          {/snippet}
        </Field>
      </Seccion>
    {:else if seccion === 'voz'}
      <!--
    La voz, y de dónde sale.

    Va en su propia sección y no dentro de Red porque la decisión no es de red:
    es de con qué se lee. Que una de las dos opciones salga del dispositivo es
    una consecuencia, y se dice con todas las letras justo donde se elige.
  -->
      <Seccion titulo={texto('voz')}>
        <!--
          De dónde sale la voz. Son tres, y por eso esto no es un
          interruptor: la de en medio es un modelo que corre aquí dentro, así que
          "remota sí o no" ya no describe la elección.
        -->
        <h3 class="nb-subtitulo">{texto('vozOrigen')}</h3>
        <p class="nb-ayuda">{texto('vozOrigenAyuda')}</p>
        <div class="nb-topes" role="radiogroup" aria-label={texto('vozOrigen')}>
          {#each ORIGENES_DE_VOZ as origen (origen)}
            <button
              type="button"
              role="radio"
              class="nb-tope"
              aria-checked={cadena('voice_provider', 'local') === origen}
              onclick={() => alCambiar('voice_provider', origen)}
            >
              {texto(`vozOrigen-${origen}`)}
            </button>
          {/each}
        </div>

        {#if cadena('voice_provider', 'local') === 'neural' && cargandoVoz && !vozNeuronal}
          <!--
            Cargar y medir la voz tarda unos segundos, y hasta ahora no cambiaba
            nada en pantalla al elegirla.
          -->
          <p class="nb-ayuda" role="status">
            <Spinner etiqueta={texto('vozCargando')} enLinea />
            {texto('vozCargando')}
          </p>
        {/if}
        {#if cadena('voice_provider', 'local') === 'neural' && vozNeuronal}
          {@const voz = vozNeuronal}
          <article class="nb-modelo" aria-labelledby="nb-voz-neuronal">
            <header class="nb-modelo-cabecera">
              <h3 id="nb-voz-neuronal" class="nb-modelo-nombre">{voz.name}</h3>
              <span class="nb-ayuda">{voz.author}</span>
            </header>

            <dl class="nb-modelo-datos">
              <div>
                <dt>{texto('modeloLicencia')}</dt>
                <dd>{voz.licence}</dd>
              </div>
              <div>
                <dt>{texto('modeloDescarga')}</dt>
                <dd>{megas(voz.downloadBytes)}</dd>
              </div>
              <div>
                <dt>{texto('modeloMemoria')}</dt>
                <dd>{megas(voz.memoryBytes)}</dd>
              </div>
              <div>
                <dt>{texto('vozVoces')}</dt>
                <dd>{voz.voices.map((v) => v.name).join(', ')}</dd>
              </div>
            </dl>

            <!--
              El factor de tiempo real **medido en esta máquina**. Se enseña
              siempre que exista, y no solo cuando impide narrar: es la diferencia
              entre "aquí no se ofrece" y "aquí tarda 1,7 veces lo que dura".
            -->
            {#if voz.factor !== null}
              <p class="nb-ayuda" role="status">
                {texto('vozFactor')}
                {unDecimal.format(voz.factor)}
              </p>
            {/if}

            {#if voz.installed}
              {#if voz.ready}
                <p class="nb-ayuda" role="status">{texto('vozListaParaNarrar')}</p>
              {:else}
                <p class="nb-ayuda" role="status">
                  {texto(`vozMotivo-${voz.reason ?? 'NO_SE_CARGA'}`)}
                </p>
              {/if}
              <div class="nb-acciones-diagnostico">
                <Button onclick={() => alQuitarModelo?.(voz.id)}>
                  {texto('modeloQuitar')}
                </Button>
              </div>
            {:else if avanceDeModelo?.modelId === voz.id}
              <p class="nb-cobertura" role="status">
                <strong>{megas(avanceDeModelo.done)}</strong>
                {texto('indiceDe')}
                {megas(avanceDeModelo.total)}
              </p>
              <div class="nb-acciones-diagnostico">
                <Button onclick={() => alCancelarModelo?.()}>{texto('modeloCancelar')}</Button>
              </div>
            {:else if confirmandoModelo === voz.id}
              <div class="nb-modelo-confirmar">
                <h4 class="nb-subtitulo">{texto('modeloAntesDeDescargar')}</h4>
                <p class="nb-ayuda">
                  {texto('modeloDeQuien')}
                  <strong>{voz.author}</strong>{texto('modeloBajoLicencia')}
                  <strong>{voz.licence}</strong>.
                </p>
                <p class="nb-ayuda">
                  {texto('modeloSeDescargaDe')} <strong>{voz.host}</strong>. {texto('vozQueNoSale')}
                </p>
                <div class="nb-acciones-diagnostico">
                  <Button
                    onclick={() => {
                      confirmandoModelo = null;
                      alDescargarModelo?.(voz.id, voz.host);
                    }}
                  >
                    {texto('modeloAutorizarYDescargar')}
                  </Button>
                  <Button onclick={() => (confirmandoModelo = null)}>
                    {texto('modeloNoDescargar')}
                  </Button>
                </div>
              </div>
            {:else}
              <p class="nb-ayuda">{texto('vozNoInstalada')}</p>
              <div class="nb-acciones-diagnostico">
                <Button onclick={() => (confirmandoModelo = voz.id)}>
                  {texto('modeloDescargar')}
                </Button>
              </div>
            {/if}
          </article>
        {/if}

        {#if cadena('voice_provider', 'local') === 'remote'}
          <Field etiqueta={texto('vozServidor')}>
            {#snippet children({ id })}
              <input
                {id}
                class="nb-entrada"
                type="text"
                inputmode="url"
                autocomplete="off"
                spellcheck="false"
                placeholder={texto('vozServidorEjemplo')}
                value={cadena('speech_host', '')}
                onchange={(evento) => alCambiar('speech_host', evento.currentTarget.value.trim())}
              />
            {/snippet}
          </Field>
          <p class="nb-ayuda">{texto('vozServidorAyuda')}</p>
          {#if puntoDeVoz}
            <p class="nb-ayuda" role="status">
              {texto('vozSeEnviaA')} <span class="nb-selectable nb-direccion">{puntoDeVoz}</span>
            </p>
          {/if}
          {#if vozEnClaroPorLaRed}
            <div class="nb-fila">
              <span class="nb-fila-texto">
                {texto('vozEnClaro')}
                <span class="nb-ayuda">{texto('vozEnClaroAyuda')}</span>
              </span>
            </div>
          {/if}

          <Field etiqueta={texto('vozModelo')}>
            {#snippet children({ id })}
              <input
                {id}
                class="nb-entrada"
                type="text"
                autocomplete="off"
                spellcheck="false"
                value={cadena('speech_model', '')}
                onchange={(evento) => alCambiar('speech_model', evento.currentTarget.value.trim())}
              />
            {/snippet}
          </Field>

          <Field etiqueta={texto('vozNombre')}>
            {#snippet children({ id })}
              <input
                {id}
                class="nb-entrada"
                type="text"
                autocomplete="off"
                spellcheck="false"
                value={cadena('speech_voice', '')}
                onchange={(evento) => alCambiar('speech_voice', evento.currentTarget.value.trim())}
              />
            {/snippet}
          </Field>
          <p class="nb-ayuda">{texto('vozNombreAyuda')}</p>

          <!--
        Escribir el servidor **no autoriza a salir a él**. Es la misma regla que
        los catálogos: anotar una dirección es una cosa y conceder el permiso es
        otra, y aquí pesa más que en ningún otro sitio porque lo que sale es el
        texto del libro. El botón desaparece cuando ya está concedido.
      -->
          {#if cadena('speech_host', '').trim().length > 0 && !vozAutorizada}
            <div class="nb-fila">
              <span class="nb-fila-texto">
                {texto('vozAutorizar')}
                <span class="nb-ayuda">{texto('vozAutorizarAyuda')}</span>
              </span>
              <Button onclick={() => alAutorizarVoz?.()}>{texto('autorizar')}</Button>
            </div>
          {/if}

          <!--
        La clave del servicio. Va aquí y no en la base porque los ajustes se
        exportan con la biblioteca, y un secreto que viaja en la exportación es
        un secreto filtrado. Sin llavero no se ofrece el campo: no hay repliegue
        a texto plano.
      -->
          {#if cadena('speech_host', '').trim().length > 0}
            {#if credencial && !credencial.keyringAvailable}
              <div class="nb-fila">
                <span class="nb-fila-texto">
                  {texto('vozSinLlavero')}
                  <span class="nb-ayuda">{texto('vozSinLlaveroAyuda')}</span>
                </span>
              </div>
            {:else}
              <Field etiqueta={texto('vozClave')}>
                {#snippet children({ id })}
                  <input
                    {id}
                    class="nb-entrada"
                    type="password"
                    autocomplete="off"
                    spellcheck="false"
                    placeholder={texto('vozClaveEjemplo')}
                    bind:value={claveEscrita}
                  />
                {/snippet}
              </Field>
              <p class="nb-ayuda">{texto('vozClaveAyuda')}</p>
              <div class="nb-fila">
                <span class="nb-fila-texto">
                  {#if credencial?.stored}{texto('vozClaveGuardada')}{/if}
                </span>
                {#if credencial?.stored}
                  <Button onclick={() => alBorrarCredencial?.()}>{texto('vozClaveBorrar')}</Button>
                {/if}
                <Button
                  onclick={() => {
                    alGuardarCredencial?.(claveEscrita);
                    // No se queda en el DOM después de guardarla: el campo es de
                    // paso, no un sitio donde la clave viva.
                    claveEscrita = '';
                  }}
                >
                  {texto('vozClaveGuardar')}
                </Button>
              </div>
            {/if}
          {/if}
        {/if}
      </Seccion>
    {:else if seccion === 'modelos'}
      <!--
        La IA es opcional y local, y esta es la única puerta para traerla: un
        modelo que se descarga una vez. Aquí y no en un destino propio, que es la
        regla 4 de ADR-013 —la IA no es un destino—, y sin tarjeta de "algo
        necesita atención" para quien no lo descargue.
      -->
      <Seccion titulo={texto('modelos')}>
        <p class="nb-ayuda">{texto('modelosAyuda')}</p>
        <p class="nb-ayuda">{texto('modelosAvisoDeRendimiento')}</p>

        <!--
          El proveedor activo, arriba y siempre: quien abre esta sección
          tiene que saber antes que nada si lo que pida la IA sale del equipo. Es la
          regla de ai/privacy.md sección 1.
        -->
        <p class="nb-proveedor-activo" role="status">
          <strong>{texto('iaActivo')}</strong>
          {#if proveedorDeIa?.configured}
            {proveedorDeIa.model}
            {texto('iaEn')}
            <span class="nb-direccion">{proveedorDeIa.host}</span>. {texto('iaActivoSale')}
          {:else}
            {texto('iaActivoLocal')}
          {/if}
        </p>

        {#if modelos}
          <!--
            Dónde calcula y con qué edición: de ahí sale casi toda la diferencia de
            velocidad entre equipos. Si hay una edición mejor para este, se dice cuál;
            elegirla es de quien tiene la máquina.
          -->
          <p class="nb-ayuda">
            {texto('iaCalculaEn')}
            <strong>{texto(`iaAcelerador-${modelos.accelerator}`)}</strong>.
            {#if !modelos.fastBuild}{texto('iaEdicionRapida')}{/if}
          </p>
        {/if}

        {#if modelos && !modelos.machineSupported}
          <div class="nb-aviso-indice" role="status">
            <Icon nombre="informacion" tamano={18} />
            <p class="nb-ayuda">{texto('modelosSinMaquina')}</p>
          </div>
        {/if}

        {#each modelos?.models ?? [] as modelo (modelo.id)}
          {@const avance =
            avanceDeModelo?.modelId === modelo.id ? avanceDeModelo : modelos?.installing}
          <article class="nb-modelo" aria-labelledby="nb-modelo-{modelo.id}">
            <header class="nb-modelo-cabecera">
              <h3 id="nb-modelo-{modelo.id}" class="nb-modelo-nombre">{modelo.name}</h3>
              <span class="nb-ayuda">{modelo.author}</span>
            </header>

            <!--
              La licencia va **siempre** a la vista, esté instalado o no. Un dato
              que solo aparece al pulsar "descargar" llega cuando ya se ha
              decidido.
            -->
            <dl class="nb-modelo-datos">
              <div>
                <dt>{texto('modeloPara')}</dt>
                <dd>{modelo.generates ? texto('modeloParaGenerar') : texto('modeloParaBuscar')}</dd>
              </div>
              {#if modelo.generates}
                <div>
                  <dt>{texto('modeloComoEscribe')}</dt>
                  <dd>{texto(`modeloRasgo-${modelo.traitMeasured}`)}</dd>
                </div>
                <div>
                  <dt>{texto('modeloEquipo')}</dt>
                  <dd>
                    {texto('modeloEquipoDesde')}
                    {megas(modelo.needsMemoryBytes)}
                    {texto('modeloEquipoY')}
                    {modelo.needsCores}
                    {texto('modeloEquipoNucleos')}
                  </dd>
                </div>
              {/if}
              <div>
                <dt>{texto('modeloLicencia')}</dt>
                <dd>{modelo.license}</dd>
              </div>
              <div>
                <dt>{texto('modeloDescarga')}</dt>
                <dd>{megas(modelo.diskBytes)}</dd>
              </div>
              <div>
                <dt>{texto('modeloMemoria')}</dt>
                <dd>{megas(modelo.memoryBytes)}</dd>
              </div>
              <div>
                <dt>{texto('modeloIdiomas')}</dt>
                <dd>{modelo.languages.length}: {modelo.languages.join(', ')}</dd>
              </div>
            </dl>

            {#if modelo.generates && (modelo.recommended || modelo.inUse)}
              <p class="nb-ayuda" role="status">
                {modelo.inUse ? texto('modeloEnUso') : texto('modeloRecomendado')}
                {#if modelo.recommended}{texto('modeloPorQueRecomendado')}{/if}
              </p>
            {/if}

            <!--
              **Lo que es probable que pase con este modelo en este equipo.** Recomendar
              no es obligar (2026-09-22): todos se pueden elegir, y cada uno
              dice lo que se espera de él con los datos medidos. La cifra es una
              estimación a partir de otra máquina, y lo pone.
            -->
            {#if modelo.generates && modelo.estimatedSeconds !== null}
              <p class="nb-ayuda">
                {texto('modeloPrevision')}
                {minutosDeModelo(modelo.estimatedSeconds)}
                {texto('modeloPrevisionEstimada')}
              </p>
            {/if}
            {#each modelo.warnings as aviso (aviso)}
              <p class={aviso === 'EXCEEDS_MACHINE_MEMORY' ? 'nb-fallo' : 'nb-ayuda'} role="status">
                {texto(`modeloAviso-${aviso}`)}
              </p>
            {/each}

            {#if modelo.status === 'installing' && avance}
              <p class="nb-cobertura" role="status">
                <strong>{megas(avance.done)}</strong>
                {texto('indiceDe')}
                {megas(avance.total)}
                ({Math.floor((avance.done / Math.max(avance.total, 1)) * 100)} %)
              </p>
              <div
                class="nb-barra-cobertura"
                role="progressbar"
                aria-valuemin="0"
                aria-valuemax={avance.total}
                aria-valuenow={avance.done}
                aria-label={texto('modeloDescargando')}
              >
                <span style:width="{(avance.done / Math.max(avance.total, 1)) * 100}%"></span>
              </div>
              <div class="nb-acciones-diagnostico">
                <Button onclick={() => alCancelarModelo?.()}>{texto('modeloCancelar')}</Button>
              </div>
            {:else if modelo.status === 'installed'}
              <p class="nb-ayuda" role="status">{texto('modeloInstalado')}</p>
              <div class="nb-acciones-diagnostico">
                <!--
                  Uno a la vez: elegir otro suelta el que estuviera cargado, que son
                  gigabytes. Se decidió el 2026-09-17, y por eso esto es un
                  botón por modelo y no una lista aparte.
                -->
                {#if modelo.generates && !modelo.inUse}
                  {#if confirmandoUso === modelo.id}
                    <p class="nb-fallo" role="alert">{texto('modeloConfirmarMemoria')}</p>
                    <Button
                      onclick={() => {
                        confirmandoUso = null;
                        alCambiar('ai_model_local', modelo.id);
                      }}
                    >
                      {texto('modeloUsarIgualmente')}
                    </Button>
                    <Button onclick={() => (confirmandoUso = null)}
                      >{texto('modeloNoDescargar')}</Button
                    >
                  {:else}
                    <Button
                      onclick={() => {
                        if (modelo.warnings.includes('EXCEEDS_MACHINE_MEMORY')) {
                          confirmandoUso = modelo.id;
                        } else {
                          alCambiar('ai_model_local', modelo.id);
                        }
                      }}
                    >
                      {texto('modeloUsarEste')}
                    </Button>
                  {/if}
                {/if}
                <Button onclick={() => alQuitarModelo?.(modelo.id)}>{texto('modeloQuitar')}</Button>
              </div>
            {:else if confirmandoModelo === modelo.id}
              <div class="nb-modelo-confirmar">
                <h4 class="nb-subtitulo">{texto('modeloAntesDeDescargar')}</h4>
                <p class="nb-ayuda">
                  {texto('modeloDeQuien')}
                  <strong>{modelo.author}</strong>{texto('modeloBajoLicencia')}
                  <strong>{modelo.license}</strong>.
                </p>
                <p class="nb-ayuda">
                  {texto('modeloSeDescargaDe')} <strong>{modelo.host}</strong>. {texto(
                    'modeloQueSale'
                  )}
                  {modelo.whatIsSent}.
                </p>
                {#if modelo.warnings.includes('EXCEEDS_MACHINE_MEMORY')}
                  <!--
                    La confirmación fuerte que se decidió el 2026-09-22: se
                    ofrece igual, pero nadie se lo encuentra por sorpresa.
                  -->
                  <p class="nb-fallo" role="alert">{texto('modeloConfirmarMemoria')}</p>
                {/if}
                {#if modelo.fits === false}
                  <p class="nb-fallo" role="alert">
                    {texto('modeloNoCabe')}
                    {megas(modelo.diskBytes)}{texto('modeloNoCabeLibres')}
                    {megas(modelo.freeBytes ?? 0)}.
                  </p>
                {/if}
                <div class="nb-acciones-diagnostico">
                  {#if modelo.fits !== false}
                    <Button
                      onclick={() => {
                        confirmandoModelo = null;
                        alDescargarModelo?.(modelo.id, modelo.host);
                      }}
                    >
                      {texto('modeloAutorizarYDescargar')}
                    </Button>
                  {/if}
                  <Button onclick={() => (confirmandoModelo = null)}
                    >{texto('modeloNoDescargar')}</Button
                  >
                </div>
              </div>
            {:else}
              {#if modelo.status === 'broken'}
                <p class="nb-ayuda" role="status">{texto('modeloRoto')}</p>
              {/if}
              <div class="nb-acciones-diagnostico">
                <Button onclick={() => (confirmandoModelo = modelo.id)}>
                  {modelo.status === 'broken'
                    ? texto('modeloReinstalar')
                    : texto('modeloDescargar')}
                </Button>
              </div>
            {/if}

            {#if modeloPideAutorizar}
              <!--
                La descarga redirige a la red de distribución, que tiene otro
                nombre y necesita su propio permiso. Se dice cuál, y qué se envía,
                antes de pedirlo: autorizar el primero no autoriza la segunda.
              -->
              <div class="nb-modelo-confirmar" role="status">
                <p class="nb-ayuda">
                  {texto('modeloSigueDesde')} <strong>{modeloPideAutorizar}</strong>{texto(
                    'modeloRedDeDistribucion'
                  )}
                  {texto('modeloQueSale')}
                  {modelo.whatIsSent}.
                </p>
                <div class="nb-acciones-diagnostico">
                  <Button onclick={() => alAutorizarModelo?.(modeloPideAutorizar ?? '')}>
                    {texto('modeloAutorizarYSeguir')}
                  </Button>
                </div>
              </div>
            {/if}

            {#if falloDeModelo}
              <p class="nb-fallo" role="alert">
                {falloDeModelo === 'DOCUMENT_CORRUPTED'
                  ? texto('modeloNoCoincide')
                  : traducirError(falloDeModelo)}
              </p>
            {/if}

            {#if modelo.exceedsMemoryLimit}
              <p class="nb-ayuda" role="status">
                {texto('modeloNoCabeEnElTope')}
                {megas(modelo.memoryBytes)}.
              </p>
              {#if modelo.suggestedLimitMb !== null}
                <!--
                  El tope lo puso la propia persona, y por eso se respeta; pero
                  cambiarlo no puede exigir ir a buscar dónde se cambia.
                -->
                <div class="nb-acciones-diagnostico">
                  <Button
                    onclick={() => {
                      alCambiar('ai_memory_limit_mb', modelo.suggestedLimitMb ?? 0);
                      if (modelo.generates && modelo.status === 'installed') {
                        alCambiar('ai_model_local', modelo.id);
                      }
                    }}
                  >
                    {texto('modeloSubirTope')}
                    {megas((modelo.suggestedLimitMb ?? 0) * 1024 ** 2)}
                  </Button>
                </div>
              {/if}
            {/if}
          </article>
        {/each}

        {#if modelos}
          <!--
            Lo que la IA puede gastar. Los topes los hace cumplir el
            runtime —nunca todos los núcleos, y solo las memorias de la lista—, y
            lo que se ofrece aquí es un subconjunto de lo que admite.
          -->
          <h3 class="nb-subtitulo">{texto('modelosRecursos')}</h3>
          <p class="nb-ayuda">{texto('modelosRecursosAyuda')}</p>

          {#if modelos.backgroundPause}
            <div class="nb-aviso-indice" role="status">
              <Icon nombre="informacion" tamano={18} />
              <p class="nb-ayuda">
                {texto(
                  modelos.backgroundPause === 'AI_SUSPENDED_HEAT'
                    ? 'modelosEnPausaCalor'
                    : 'modelosEnPausaBateria'
                )}
              </p>
            </div>
          {/if}

          <Field etiqueta={texto('modelosHilos')} descripcion={texto('modelosHilosAyuda')}>
            {#snippet children({ id, 'aria-describedby': describedby })}
              <div
                class="nb-topes"
                role="radiogroup"
                {id}
                aria-label={texto('modelosHilos')}
                aria-describedby={describedby}
              >
                {#each hilosOfrecidos(modelos.cores) as hilos (hilos)}
                  <button
                    type="button"
                    role="radio"
                    class="nb-tope"
                    aria-checked={numero('ai_threads', 0) === hilos}
                    onclick={() => alCambiar('ai_threads', hilos)}
                  >
                    {hilos === 0 ? texto('modelosHilosAutomatico') : hilos}
                    <span class="nb-tope-detalle">
                      {hilos === 0
                        ? `${modelos.automaticThreads} ${texto('modelosDe')} ${modelos.cores}`
                        : `${texto('modelosDe')} ${modelos.cores}`}
                    </span>
                  </button>
                {/each}
              </div>
            {/snippet}
          </Field>

          <Field etiqueta={texto('modelosMemoriaTope')} descripcion={texto('modelosMemoriaAyuda')}>
            {#snippet children({ id, 'aria-describedby': describedby })}
              <div
                class="nb-topes"
                role="radiogroup"
                {id}
                aria-label={texto('modelosMemoriaTope')}
                aria-describedby={describedby}
              >
                {#each MEMORIAS_DE_IA as memoria (memoria)}
                  <button
                    type="button"
                    role="radio"
                    class="nb-tope"
                    aria-checked={numero('ai_memory_limit_mb', 1024) === memoria}
                    onclick={() => alCambiar('ai_memory_limit_mb', memoria)}
                  >
                    {memoria < 1024 ? `${memoria} MB` : `${memoria / 1024} GB`}
                  </button>
                {/each}
              </div>
            {/snippet}
          </Field>
        {/if}

        <!--
          El proveedor externo. Configurarlo es la elección consciente que
          pide la regla 2 de ai/philosophy.md: sin él, nada sale. Escribir la
          dirección no autoriza a salir, y el primer envío enseña entero qué saldría.
        -->
        <!--
          Quién genera las funciones de estudio. Se decidió el
          2026-09-17: las dos posibilidades existen y la elige la persona. Sin
          repliegue: si el elegido no está, la función lo dice.
        -->
        <h3 class="nb-subtitulo">{texto('iaGenerador')}</h3>
        <p class="nb-ayuda">{texto('iaGeneradorAyuda')}</p>
        <div class="nb-topes" role="radiogroup" aria-label={texto('iaGenerador')}>
          {#each GENERADORES_DE_IA as generador (generador)}
            <button
              type="button"
              role="radio"
              class="nb-tope"
              aria-checked={cadena('ai_generator', 'local') === generador}
              onclick={() => alCambiar('ai_generator', generador)}
            >
              {texto(`iaGenerador-${generador}`)}
            </button>
          {/each}
        </div>

        <h3 class="nb-subtitulo">{texto('iaExterna')}</h3>
        <p class="nb-ayuda">{texto('iaExternaAyuda')}</p>

        <Field etiqueta={texto('iaProtocolo')}>
          {#snippet children({ id })}
            <div class="nb-topes" role="radiogroup" {id} aria-label={texto('iaProtocolo')}>
              {#each PROTOCOLOS_DE_IA as protocolo (protocolo)}
                <button
                  type="button"
                  role="radio"
                  class="nb-tope"
                  aria-checked={cadena('ai_provider', 'none') === protocolo}
                  onclick={() => alCambiar('ai_provider', protocolo)}
                >
                  {texto(`iaProtocolo-${protocolo}`)}
                </button>
              {/each}
            </div>
          {/snippet}
        </Field>

        {#if cadena('ai_provider', 'none') !== 'none'}
          <Field etiqueta={texto('iaServidor')}>
            {#snippet children({ id })}
              <input
                {id}
                class="nb-entrada"
                type="text"
                inputmode="url"
                autocomplete="off"
                spellcheck="false"
                placeholder={cadena('ai_provider', 'none') === 'anthropic'
                  ? 'api.anthropic.com'
                  : texto('iaServidorEjemplo')}
                value={cadena('ai_host', '')}
                onchange={(evento) => alCambiar('ai_host', evento.currentTarget.value.trim())}
              />
            {/snippet}
          </Field>
          {#if proveedorDeIa?.invalidAddress}
            <p class="nb-fallo" role="alert">{texto('iaDireccionNoVale')}</p>
          {:else if proveedorDeIa?.endpoint}
            <p class="nb-ayuda" role="status">
              {texto('vozSeEnviaA')}
              <span class="nb-selectable nb-direccion">{proveedorDeIa.endpoint}</span>
            </p>
          {/if}

          <Field etiqueta={texto('iaModelo')}>
            {#snippet children({ id })}
              <input
                {id}
                class="nb-entrada"
                type="text"
                autocomplete="off"
                spellcheck="false"
                value={cadena('ai_model', '')}
                onchange={(evento) => alCambiar('ai_model', evento.currentTarget.value.trim())}
              />
            {/snippet}
          </Field>

          {#if proveedorDeIa?.host}
            {#if !proveedorDeIa.keyringAvailable}
              <p class="nb-ayuda">{texto('vozSinLlaveroAyuda')}</p>
            {:else}
              <Field etiqueta={texto('iaClave')}>
                {#snippet children({ id })}
                  <input
                    {id}
                    class="nb-entrada"
                    type="password"
                    autocomplete="off"
                    spellcheck="false"
                    bind:value={claveDeIaEscrita}
                  />
                {/snippet}
              </Field>
              <p class="nb-ayuda">{texto('iaClaveAyuda')}</p>
              <div class="nb-fila">
                <span class="nb-fila-texto">
                  {#if proveedorDeIa.credentialStored}{texto('vozClaveGuardada')}{/if}
                </span>
                {#if proveedorDeIa.credentialStored}
                  <Button onclick={() => alIa?.('borrarClave')}>{texto('vozClaveBorrar')}</Button>
                {/if}
                <Button
                  onclick={() => {
                    alIa?.('guardarClave', claveDeIaEscrita);
                    claveDeIaEscrita = '';
                  }}
                >
                  {texto('vozClaveGuardar')}
                </Button>
              </div>
            {/if}
          {/if}

          {#if proveedorDeIa?.configured}
            <div class="nb-acciones-diagnostico">
              <Button deshabilitado={ia.probando} onclick={() => alIa?.('probar')}
                >{texto('iaProbar')}</Button
              >
            </div>
          {/if}

          {#if pruebaDeIa?.outcome === 'needsConsent'}
            <!--
              El criterio: antes del primer envío a este proveedor, el
              mensaje entero, a dónde va y qué hace con él quien lo recibe. No se ha
              enviado nada todavía.
            -->
            <div
              class="nb-modelo-confirmar"
              role="region"
              aria-label={texto('iaAntesDelPrimerEnvio')}
            >
              <h4 class="nb-subtitulo">{texto('iaAntesDelPrimerEnvio')}</h4>
              <p class="nb-ayuda">
                {texto('iaSeEnviaraA')} <strong>{pruebaDeIa.host}</strong>. {texto('modeloQueSale')}
                {pruebaDeIa.whatIsSent}.
              </p>
              <p class="nb-ayuda" data-testid="ia-tamano-envio-prueba">
                {texto('iaTamanoDelEnvio')
                  .replace('{caracteres}', String(pruebaDeIa.body.length))
                  .replace('{tokens}', String(Math.ceil(pruebaDeIa.body.length / 4)))}
              </p>
              <p class="nb-ayuda">{texto('iaRetencion')}</p>
              <pre class="nb-cuerpo-de-envio nb-selectable">{pruebaDeIa.body}</pre>
              <div class="nb-acciones-diagnostico">
                <Button onclick={() => alIa?.('autorizarYProbar')}
                  >{texto('iaAutorizarYEnviar')}</Button
                >
                <Button onclick={() => alIa?.('descartarPrueba')}
                  >{texto('modeloNoDescargar')}</Button
                >
              </div>
            </div>
          {:else if pruebaDeIa?.outcome === 'answered'}
            <p class="nb-ayuda" role="status">
              {texto('iaRespondio')} <span class="nb-selectable">{pruebaDeIa.text}</span>
            </p>
          {:else if pruebaDeIa?.outcome === 'rejected'}
            <p class="nb-fallo" role="alert">
              {texto('iaRechazo')}
              {pruebaDeIa.status}. {motivoDeRechazo(pruebaDeIa.status)}
            </p>
          {:else if pruebaDeIa?.outcome === 'notConfigured'}
            <p class="nb-fallo" role="alert">{texto('iaSinConfigurar')}</p>
          {/if}
        {/if}

        <!--
          Lo que salió, entero y tal como salió (ai/privacy.md, regla 4). Local y
          borrable, que es lo que dice su sección 3.
        -->
        <h3 class="nb-subtitulo">{texto('iaEnvios')}</h3>
        <p class="nb-ayuda">{texto('iaEnviosAyuda')}</p>
        {#if enviosDeIa.length === 0}
          <p class="nb-ayuda">{texto('iaSinEnvios')}</p>
        {:else}
          <ul class="nb-envios">
            {#each enviosDeIa as envio (envio.id)}
              <li>
                <details class="nb-envio">
                  <summary>
                    {fechaDeEnvio.format(new Date(envio.sentAt))} · {envio.model}
                    {texto('iaEn')}
                    {envio.host} · {cifra.format(envio.bytes)} B · {envio.status === null
                      ? texto('iaSinRespuesta')
                      : envio.status}
                  </summary>
                  <pre class="nb-cuerpo-de-envio nb-selectable">{envio.body}</pre>
                </details>
              </li>
            {/each}
          </ul>
          <div class="nb-acciones-diagnostico">
            <Button onclick={() => alIa?.('borrarEnvios')}>{texto('iaBorrarEnvios')}</Button>
          </div>
        {/if}

        <!--
          Lo marcado como incorrecto. Local: no se manda a ninguna parte,
          y se dice, porque un registro que el usuario cree que viaja es peor que
          uno que sabe que no.
        -->
        <h3 class="nb-subtitulo">{texto('iaReportes')}</h3>
        <p class="nb-ayuda">{texto('iaReportesAyuda')}</p>
        {#if reportesDeIa.length === 0}
          <p class="nb-ayuda">{texto('iaSinReportes')}</p>
        {:else}
          <ul class="nb-envios">
            {#each reportesDeIa as reporte (reporte.id)}
              <li>
                <details class="nb-envio">
                  <summary>
                    {fechaDeEnvio.format(new Date(reporte.createdAt))} · {reporte.function} · {texto(
                      `iaMotivo-${reporte.reason}`
                    )}
                  </summary>
                  <p class="nb-ayuda">
                    {reporte.templateId}@{reporte.templateVersion} · {reporte.providerLabel}
                  </p>
                  {#if reporte.note}
                    <p class="nb-ayuda nb-selectable">{reporte.note}</p>
                  {/if}
                  <pre class="nb-cuerpo-de-envio nb-selectable">{reporte.output}</pre>
                  <div class="nb-acciones-diagnostico">
                    <Button onclick={() => alQuitarReporte?.(reporte.id)}>
                      {texto('iaQuitarReporte')}
                    </Button>
                  </div>
                </details>
              </li>
            {/each}
          </ul>
          <div class="nb-acciones-diagnostico">
            <Button onclick={() => alIa?.('exportarReportes')}>{texto('iaExportarReportes')}</Button
            >
            <Button onclick={() => alIa?.('borrarReportes')}>{texto('iaBorrarReportes')}</Button>
          </div>
        {/if}
      </Seccion>
    {:else if seccion === 'red'}
      <Seccion titulo={texto('red')}>
        <div class="nb-fila">
          <span class="nb-fila-texto">
            {texto('sinRed')}
            <span class="nb-ayuda">{texto('sinRedAyuda')}</span>
          </span>
          <Switch
            etiqueta={texto('sinRed')}
            activo={bandera('offline_mode')}
            alCambiar={(v) => alCambiar('offline_mode', v)}
          />
        </div>

        <!--
      El traductor no trae ningún servidor puesto, y no es un hueco por rellenar:
      traducir manda el texto que estás leyendo a un tercero, y elegir por el
      usuario a qué empresa se lo manda no le corresponde al programa.
    -->
        <Field etiqueta={texto('traductor')}>
          {#snippet children({ id })}
            <input
              {id}
              class="nb-entrada"
              type="text"
              inputmode="url"
              autocomplete="off"
              spellcheck="false"
              placeholder={texto('traductorEjemplo')}
              value={cadena('translation_host', '')}
              onchange={(evento) =>
                alCambiar('translation_host', evento.currentTarget.value.trim())}
            />
          {/snippet}
        </Field>
        <p class="nb-ayuda">{texto('traductorAyuda')}</p>

        <!--
      Lo que falta por autorizar va **antes** que lo ya autorizado.

      Anotar un catálogo no concede la salida hacia él, y eso está bien: sería
      convertir un formulario en un permiso. Lo que estaba mal es que el único
      sitio donde se podía conceder fuera el aviso de error de Descubrir, así
      que había que tropezar con el fallo para poder arreglarlo. Aquí se ve
      antes de tropezar, con el nombre que el usuario le puso al catálogo y con
      qué se enviaría, que es lo que hace falta para poder decidir.
    -->
        {#if red && red.pending.length > 0}
          <h3 class="nb-subtitulo">{texto('pendientes')}</h3>
          <ul class="nb-diccionarios">
            {#each red.pending as destino (destino.host)}
              <li class="nb-fila">
                <span class="nb-fila-texto">
                  {destino.host}
                  <span class="nb-ayuda">{destino.label} &middot; {destino.whatIsSent}</span>
                </span>
                <Button onclick={() => alAutorizar?.(destino.host, destino.cleartext)}>
                  {destino.cleartext ? texto('autorizarEnClaro') : texto('autorizar')}
                </Button>
              </li>
            {/each}
          </ul>
        {/if}

        <h3 class="nb-subtitulo">{texto('permisos')}</h3>
        {#if !red || red.consents.length === 0}
          <p class="nb-ayuda">{texto('sinPermisos')}</p>
        {:else}
          {#each permisosPorProposito as grupo (grupo.purpose)}
            <div class="nb-grupo-permisos">
              <h4 class="nb-permiso-proposito">{texto(`proposito-${grupo.purpose}`)}</h4>
              <ul class="nb-diccionarios">
                {#each grupo.consents as permiso (`${permiso.origin}-${permiso.host}-${permiso.purpose}`)}
                  <li class="nb-fila">
                    <span class="nb-fila-texto">
                      {permiso.host}
                      {#if permiso.origin}
                        <span class="nb-ayuda">{permiso.origin}</span>
                      {/if}
                    </span>
                    <Button
                      variante="peligro"
                      onclick={() => alRevocar?.(permiso.host, permiso.purpose)}
                    >
                      {texto('revocar')}
                    </Button>
                  </li>
                {/each}
              </ul>
            </div>
          {/each}
        {/if}

        <h3 class="nb-subtitulo">{texto('registroDeSalidas')}</h3>
        {#if !red || red.log.length === 0}
          <p class="nb-ayuda">{texto('sinSalidas')}</p>
        {:else}
          <!--
        Anfitrión y propósito, nunca la ruta ni lo que se buscó: la ruta puede
        llevar dentro el título de un documento o el texto que se seleccionó.
        Es la misma regla de redacción que rige los registros de la aplicación.
      -->
          <ul class="nb-salidas">
            {#each red.log as salida, indice (`${salida.at}-${indice}`)}
              <li>
                <span class="nb-salida-destino">{salida.host}</span>
                <span class="nb-ayuda">
                  {texto(`proposito-${salida.purpose}`)}
                  &middot; {texto(`salida-${salida.outcome}`)}
                  &middot; {fechaYHora(salida.at)}
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      </Seccion>
    {:else if seccion === 'telefonos'}
      <Seccion titulo={texto('movilSeccion')}>
        <TelefonosPanel {texto} />
      </Seccion>
    {:else if seccion === 'extensiones'}
      <!--
    Extensiones. Va después de red y privacidad porque comparte con ella lo que
    de verdad se decide aquí: qué puede salir del dispositivo y quién lo manda.
    El orden lo fija ui-components.md, sección 4.
  -->
      <Seccion titulo={texto('extensiones')}>
        <p class="nb-ayuda">{texto('extensionesAyuda')}</p>

        <!--
      Que la API todavía no es estable se dice aquí y no en una nota de la
      versión que nadie lee. Solo aparece mientras no lo sea: cuando lo sea, el
      aviso desaparece solo, porque sale del número y no de una cadena escrita.
    -->
        {#if api && !api.stable}
          <p class="nb-ayuda">
            {texto('apiDeExtensiones')}
            {api.current}.x. {texto('apiEnDesarrollo')}
          </p>
        {/if}

        <!--
      Lo que está esperando permiso va **antes** que lo instalado, por el mismo
      motivo que en la sección de red: la decisión pendiente es la que trae aquí
      a quien abre esto, y enterrarla debajo de una lista la esconde.
    -->
        {#if pendiente}
          <h3 class="nb-subtitulo">{pendiente.name} {pendiente.version}</h3>
          <!--
        La firma va aquí arriba y no en la lista de instaladas, porque el
        momento de decidir es este: después de instalar, saber que no venía
        firmada ya no sirve para no instalarla.
      -->
          <p class="nb-ayuda">
            {#if pendiente.signature.estado === 'valida'}
              {texto('extensionFirmadaPor')}
              {pendiente.signature.clave.slice(0, 16)}
            {:else if pendiente.signature.estado === 'invalida'}
              {texto('extensionFirmaNoVale')}
            {:else}
              {texto('extensionSinFirma')}
            {/if}
          </p>
          <p class="nb-ayuda">{texto('extensionPideConsentimiento')}</p>
          <ul class="nb-diccionarios">
            {#each pendiente.permissions as permiso (permiso.host)}
              <li class="nb-fila">
                <span class="nb-fila-texto">
                  {permiso.host}
                  <span class="nb-ayuda">{permiso.implies} {permiso.whatIsSent}</span>
                </span>
                {#if permiso.granted}
                  <span class="nb-ayuda">{texto('extensionConcedido')}</span>
                {:else}
                  <Button onclick={() => alConcederPendiente?.(permiso.host)}>
                    {texto('extensionConceder')}
                  </Button>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}

        <!--
      El registro no trae dirección puesta, y no es un hueco por rellenar: el
      registro oficial todavía no existe. Poner una que no responde sería peor
      que no poner ninguna, y es la misma regla que ya siguen el traductor y los
      catálogos.
    -->
        <Field etiqueta={texto('registroDeExtensiones')}>
          {#snippet children({ id })}
            <input
              {id}
              class="nb-entrada"
              type="text"
              inputmode="url"
              autocomplete="off"
              spellcheck="false"
              placeholder={texto('registroEjemplo')}
              value={cadena('registry_host', '')}
              onchange={(evento) => alCambiar('registry_host', evento.currentTarget.value.trim())}
            />
          {/snippet}
        </Field>
        <p class="nb-ayuda">{texto('registroAyuda')}</p>
        <!-- Sin registro configurado no hay a quién preguntar. -->
        <Button
          deshabilitado={cadena('registry_host', '') === ''}
          onclick={() => alComprobarRetiradas?.()}
        >
          {texto('comprobarRetiradas')}
        </Button>

        {#if extensiones.length === 0}
          <p class="nb-ayuda">{texto('sinExtensiones')}</p>
        {:else}
          <ul class="nb-diccionarios">
            {#each extensiones as extension (extension.id)}
              <li class="nb-fila">
                <span class="nb-fila-texto">
                  {extension.name}
                  <span class="nb-ayuda">
                    {extension.version}
                    {#if !extension.enabled}
                      &middot; {texto('extensionApagada')}
                    {/if}
                    {#if retiradaDe(extension.id)}
                      &middot; {texto(
                        retiradaDe(extension.id)?.disabled
                          ? 'extensionRetiradaYApagada'
                          : 'extensionRetirada'
                      )}
                    {/if}
                    {#if extension.key}
                      &middot; {texto('extensionFirmadaPor')}
                      {extension.key.slice(0, 16)}
                    {:else}
                      &middot; {texto('extensionSinFirma')}
                    {/if}
                    {#if !extension.readable}
                      &middot; {texto('extensionIlegible')}
                    {:else if extension.missingModules.length > 0}
                      &middot; {texto('extensionIncompleta')}
                      {extension.missingModules.join(', ')}
                    {:else if extension.commands > 0}
                      &middot; {extension.commands}
                      {texto('extensionComandos')}
                    {/if}
                  </span>
                </span>
                <Switch
                  etiqueta={extension.name}
                  activo={extension.enabled}
                  alCambiar={(activo: boolean) => alHabilitarExtension?.(extension.id, activo)}
                />
                <Button onclick={() => alDesinstalarExtension?.(extension.id)}>
                  {texto('desinstalar')}
                </Button>
              </li>
              {#each extension.permissions as permiso (permiso.host)}
                <li class="nb-fila">
                  <span class="nb-fila-texto">
                    {permiso.host}
                    <span class="nb-ayuda">
                      {permiso.granted ? texto('extensionConcedido') : texto('extensionPendiente')}
                      &middot; {permiso.whatIsSent}
                    </span>
                  </span>
                  {#if permiso.granted}
                    <Button onclick={() => alRevocarExtension?.(extension.id, permiso.host)}>
                      {texto('extensionRevocar')}
                    </Button>
                  {/if}
                </li>
              {/each}
            {/each}
          </ul>
        {/if}

        <h3 class="nb-subtitulo">{texto('extensionGeneraciones')}</h3>
        {#if generacionesDeExtensiones.length < 2}
          <p class="nb-ayuda">{texto('extensionSinVuelta')}</p>
        {:else}
          <div class="nb-acciones-diagnostico">
            <Button onclick={() => alVolverAtrasExtensiones?.()}>
              {texto('extensionVolverAtras')}
            </Button>
          </div>
        {/if}

        <div class="nb-acciones-diagnostico">
          <Button onclick={() => alInstalarExtension?.()}>{texto('instalarExtension')}</Button>
        </div>
      </Seccion>
    {:else if seccion === 'diccionarios'}
      <Seccion titulo={texto('diccionarios')}>
        <p class="nb-ayuda">{texto('diccionariosAyuda')}</p>

        {#if diccionarios.length === 0}
          <p class="nb-ayuda">{texto('sinDiccionarios')}</p>
        {:else}
          <ul class="nb-diccionarios">
            {#each diccionarios as diccionario (diccionario.id)}
              <li class="nb-fila">
                <span class="nb-fila-texto">
                  {diccionario.name}
                  <span class="nb-ayuda">
                    {diccionario.language}
                    {#if diccionario.wordCount > 0}
                      &middot; {diccionario.wordCount}
                      {texto('palabras')}
                    {/if}
                  </span>
                </span>
                <Button onclick={() => alDesinstalarDiccionario?.(diccionario.id)}>
                  {texto('desinstalar')}
                </Button>
              </li>
            {/each}
          </ul>
        {/if}

        <div class="nb-acciones-diagnostico">
          <Button onclick={() => alInstalarDiccionario?.()}>{texto('instalarDiccionario')}</Button>
        </div>
      </Seccion>
    {:else if seccion === 'catalogos'}
      <Seccion titulo={texto('catalogos')}>
        <p class="nb-ayuda">{texto('catalogosAyuda')}</p>

        {#if catalogos.length === 0}
          <p class="nb-ayuda">{texto('sinCatalogos')}</p>
        {:else}
          <ul class="nb-diccionarios">
            {#each catalogos as catalogo (catalogo.id)}
              <li class="nb-fila">
                <span class="nb-fila-texto">
                  {catalogo.title}
                  <span class="nb-ayuda">{catalogo.url}</span>
                </span>
                <Button variante="peligro" onclick={() => alQuitarCatalogo?.(catalogo.id)}>
                  {texto('quitar')}
                </Button>
              </li>
            {/each}
          </ul>
        {/if}

        <Field etiqueta={texto('nombreDelCatalogo')}>
          {#snippet children({ id })}
            <input {id} class="nb-entrada" type="text" bind:value={nombreDelCatalogo} />
          {/snippet}
        </Field>
        <Field etiqueta={texto('direccionDelCatalogo')}>
          {#snippet children({ id })}
            <input
              {id}
              class="nb-entrada"
              type="url"
              inputmode="url"
              autocomplete="off"
              spellcheck="false"
              placeholder={texto('direccionEjemplo')}
              bind:value={direccionDelCatalogo}
            />
          {/snippet}
        </Field>
        {#if direccionDelCatalogo.trim().toLowerCase().startsWith('http://')}
          <!--
            Se dice **antes** de añadirlo: un catálogo en claro se admite, pero
            deja a la vista qué se mira y qué se descarga, y eso se consiente
            aparte al abrirlo.
          -->
          <p class="nb-ayuda" role="status">{texto('catalogoEnClaro')}</p>
        {/if}
        <div class="nb-acciones-diagnostico">
          <Button onclick={anadirCatalogo}>{texto('anadirCatalogo')}</Button>
        </div>
      </Seccion>
    {:else if seccion === 'indice'}
      <!--
    La búsqueda dentro de los libros. Va junto a dónde viven porque es lo mismo:
    el estado de la biblioteca como conjunto, no una preferencia de aspecto.
  -->
      <Seccion titulo={texto('indice')}>
        <p class="nb-ayuda">{texto('indiceAyuda')}</p>

        {#if indice && indice.total > 0}
          <!--
            Lo que importa es **cuánto de la biblioteca se puede buscar**, contado
            en la base. Antes se enseñaba lo indexado en esta sesión
            y lo que quedaba en la cola, que sobre una biblioteca indexada a medias
            en sesiones anteriores no decía nada.
          -->
          <p class="nb-cobertura" role="status">
            <strong>{cifra.format(indice.searchable)}</strong>
            {texto('indiceDe')}
            {cifra.format(indice.total)}
            {texto('indiceCobertura')}
            ({Math.floor((indice.searchable / indice.total) * 100)} %)
          </p>
          <span class="nb-barra-cobertura" aria-hidden="true">
            <span style:width="{(indice.searchable / indice.total) * 100}%"></span>
          </span>
        {/if}

        {#if indice?.pauseReason === 'STORAGE_BUDGET_REACHED' && indice.storage}
          <!--
            Por qué se paró, con las cifras, y qué se puede hacer. Es lo que se le
            dice al usuario: una biblioteca donde la
            mayoría de las obras no se pueden buscar por dentro y nada lo explica
            es peor que una que lo dice.
          -->
          <div class="nb-aviso-indice" role="status">
            <Icon nombre="almacenamiento" tamano={18} />
            <p class="nb-ayuda">
              {texto('indiceSinSitio')}
              <strong>{gigas(indice.storage.derivedBytes)}</strong>,
              {Math.round(indice.storage.percentOfDocuments)} %
              {texto('indiceSinSitioDe')}
              <strong>{gigas(indice.storage.documentsBytes)}</strong>.
              {texto('indiceSinSitioTope')}
              {indice.storage.budgetPercent} %. {texto('indiceSinSitioAbiertos')}
            </p>
          </div>
        {/if}

        <Field etiqueta={texto('indiceTope')} descripcion={texto('indiceTopeAyuda')}>
          {#snippet children({ id, 'aria-describedby': describedby })}
            <div
              class="nb-topes"
              role="radiogroup"
              {id}
              aria-label={texto('indiceTope')}
              aria-describedby={describedby}
            >
              {#each TOPES as tope (tope)}
                <button
                  type="button"
                  role="radio"
                  class="nb-tope"
                  aria-checked={numero('derived_budget', 20) === tope}
                  onclick={() => alCambiar('derived_budget', tope)}
                >
                  {tope} %
                  {#if indice?.storage && indice.storage.documentsBytes > 0}
                    <span class="nb-tope-detalle">
                      {texto('indiceHasta')}
                      {gigas((indice.storage.documentsBytes * tope) / 100)}
                    </span>
                  {/if}
                </button>
              {/each}
            </div>
          {/snippet}
        </Field>

        <div class="nb-fila">
          <span class="nb-fila-texto">
            {texto('indicePausar')}
            <span class="nb-ayuda">{texto('indicePausarAyuda')}</span>
          </span>
          <Switch
            etiqueta={texto('indicePausar')}
            activo={indice?.pauseReason === 'USER_PAUSED' || indicePausado}
            alCambiar={(valor) => alPausarIndice?.(valor)}
          />
        </div>

        <p class="nb-ayuda">{texto('indiceRehacerAyuda')}</p>
        <div class="nb-acciones-diagnostico">
          <Button onclick={() => alRehacerIndice?.()}>{texto('indiceRehacer')}</Button>
        </div>
      </Seccion>
    {:else if seccion === 'biblioteca'}
      <!--
    Dónde viven los libros. Va después de los catálogos y antes del diagnóstico
    porque es lo mismo que ellos —declarar de dónde sale el contenido— y no una
    herramienta de soporte.
  -->
      <Seccion titulo={texto('biblioteca')}>
        <p class="nb-ayuda">{texto('bibliotecaAyuda')}</p>

        {#if raices.length === 0}
          <p class="nb-ayuda">{texto('sinRaices')}</p>
        {:else}
          <ul class="nb-diccionarios">
            <!--
              La biblioteca administrada vacía no se enseña: "0 archivos, todos
              donde deben" en una carpeta que el usuario nunca usó confundía.
            -->
            {#each raices.filter((r) => r.kind !== 'managed' || r.files > 0) as raiz (raiz.id)}
              <li class="nb-fila">
                <span class="nb-fila-texto">
                  {raiz.name}
                  <span class="nb-ayuda">
                    {raiz.path ?? texto('raizSinLocalizar')}
                  </span>
                  <span class="nb-ayuda">
                    {#if raiz.missing > 0}
                      <Icon nombre="noDisponible" tamano={13} />
                      {raiz.missing}
                      {texto('deAusentes')}
                      {raiz.files}
                    {:else}
                      {new Intl.NumberFormat(idioma()).format(raiz.files)}
                      {texto('archivosAqui')}
                    {/if}
                  </span>
                </span>
                {#if raiz.kind === 'referenced'}
                  <Button
                    onclick={() => alReapuntarRaiz?.(raiz.id, '')}
                    deshabilitado={revisandoRaices}
                  >
                    {texto('reapuntar')}
                  </Button>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}

        <div class="nb-acciones-diagnostico">
          <Button onclick={() => alRevisarRaices?.()} deshabilitado={revisandoRaices}>
            {texto('revisar')}
          </Button>
          {#if revisandoRaices}
            <!-- Son segundos de disco sobre una biblioteca grande: se dice. -->
            <span class="nb-ayuda" role="status">
              <Spinner etiqueta={texto('revisando')} enLinea />
              {texto('revisando')}
            </span>
          {/if}
        </div>

        {#if revision}
          <p class="nb-ayuda" role="status">
            {revision.available}
            {texto('deDisponibles')}
            {revision.checked}{revision.missing > 0
              ? `. ${revision.missing} ${texto('ausentes')}`
              : ''}
          </p>
        {/if}
      </Seccion>

      <Seccion titulo={texto('exportar')}>
        <p class="nb-ayuda">{texto('exportarAyuda')}</p>
        <p class="nb-ayuda">{texto('exportarAudioAviso')}</p>
        <div class="nb-acciones-diagnostico">
          <Button
            variante="primario"
            onclick={() => alExportarBiblioteca?.()}
            deshabilitado={exportandoBiblioteca}
          >
            {exportandoBiblioteca ? texto('exportando') : texto('exportarBoton')}
          </Button>
        </div>
      </Seccion>

      <Seccion titulo={texto('borrarDatos')}>
        <p class="nb-ayuda">{texto('borrarDatosAyuda')}</p>

        {#each [{ alcance: 'historial' as const, titulo: texto('borrarHistorialTitulo'), explicacion: texto('borrarHistorialExplicacion'), boton: texto('borrarHistorialBoton'), hecho: texto('borrarHistorialHecho') }, { alcance: 'anotaciones' as const, titulo: texto('borrarAnotacionesTitulo'), explicacion: texto('borrarAnotacionesExplicacion'), boton: texto('borrarAnotacionesBoton'), hecho: texto('borrarAnotacionesHecho') }, { alcance: 'biblioteca' as const, titulo: texto('borrarBibliotecaTitulo'), explicacion: texto('borrarBibliotecaExplicacion'), boton: texto('borrarBibliotecaBoton'), hecho: texto('borrarBibliotecaHecho') }] as opcion (opcion.alcance)}
          <div class="nb-modelo">
            <div class="nb-modelo-cabecera">
              <p class="nb-modelo-nombre">{opcion.titulo}</p>
            </div>
            <p class="nb-ayuda">{opcion.explicacion}</p>
            {#if confirmandoBorrado === opcion.alcance}
              <div class="nb-modelo-confirmar" role="region" aria-label={opcion.boton}>
                <p class="nb-ayuda">{texto('borrarConfirmarPregunta')}</p>
                <div class="nb-acciones-diagnostico">
                  <Button
                    variante="peligro"
                    deshabilitado={borrandoDatos}
                    onclick={() => void ejecutarBorrado(opcion.alcance)}
                  >
                    {texto('borrarConfirmarBoton')}
                  </Button>
                  <Button
                    deshabilitado={borrandoDatos}
                    onclick={() => {
                      confirmandoBorrado = null;
                    }}
                  >
                    {texto('borrarCancelarBoton')}
                  </Button>
                </div>
              </div>
            {:else}
              <div class="nb-acciones-diagnostico">
                <Button
                  variante="peligro"
                  deshabilitado={borrandoDatos}
                  onclick={() => {
                    borradoReciente = null;
                    falloDeBorrado = null;
                    confirmandoBorrado = opcion.alcance;
                  }}
                >
                  {opcion.boton}
                </Button>
              </div>
            {/if}
            {#if borradoReciente === opcion.alcance}
              <p class="nb-ayuda" role="status">{opcion.hecho}</p>
            {/if}
          </div>
        {/each}

        {#if falloDeBorrado}
          <p class="nb-ayuda" role="alert">{falloDeBorrado}</p>
        {/if}
      </Seccion>
    {:else if seccion === 'acerca'}
      <Seccion titulo={texto('acercaDe')}>
        <div class="nb-acerca">
          <Logo tamano="4.5rem" />
          <div class="nb-acerca-texto">
            <p class="nb-acerca-nombre">{texto('aplicacion')}</p>
            {#if informe}
              <p class="nb-ayuda nb-selectable">
                {texto('version')}
                {informe.version} &middot; {texto('esquema')}
                {informe.schemaVersion}
              </p>
            {/if}
          </div>
        </div>
      </Seccion>

      <Seccion titulo={texto('actualizaciones')}>
        <ActualizacionPanel {texto} />
      </Seccion>

      <!--
    El estado de cada componente, **siempre** y no solo cuando algo va mal.

    Antes vivía en un botón de la barra superior que aparecía únicamente si algún
    componente fallaba. Era coherente con esconder lo que no se puede usar, pero
    dejaba sin sitio donde comprobar que todo está bien, que es lo que se busca
    cuando se sospecha que no lo está.
  -->
      <Seccion titulo={texto('estadoDelRuntime')}>
        <p class="nb-ayuda">{texto('estadoAyuda')}</p>
        <ul class="nb-diccionarios">
          {#each componentes as componente (componente.component)}
            <li class="nb-fila">
              <span class="nb-fila-texto">{texto(`componente-${componente.component}`)}</span>
              <span class="nb-estado" data-state={componente.state}>
                {etiquetaDeEstado(componente)}
              </span>
            </li>
          {/each}
        </ul>
        {#if ultimoFallo}
          <p class="nb-fallo nb-selectable" role="alert">{ultimoFallo}</p>
        {/if}
      </Seccion>

      <!--
    El diagnóstico va junto al estado y no en una sección aparte al final: un
    diagnóstico se exporta para contar un problema, y es aquí donde se mira cuando
    se sospecha que hay uno.
  -->
      <Seccion titulo={texto('diagnostico')}>
        <p class="nb-ayuda">{texto('queLleva')}</p>

        <div class="nb-acciones-diagnostico">
          <Button onclick={() => alPrepararDiagnostico?.()}>{texto('verDiagnostico')}</Button>
          {#if diagnostico}
            <Button variante="primario" onclick={() => alGuardarDiagnostico?.()}>
              {texto('guardarDiagnostico')}
            </Button>
          {/if}
        </div>

        {#if diagnosticoGuardado}
          <p class="nb-ayuda" role="status">{texto('guardado')}</p>
        {/if}

        {#if diagnostico}
          {#if diagnostico.log.length === 0}
            <p class="nb-ayuda">{texto('registroVacio')}</p>
          {:else}
            {#if diagnostico.omittedLines > 0}
              <p class="nb-ayuda">
                {diagnostico.omittedLines}
                {texto('lineasOmitidas')}
              </p>
            {/if}
            <!--
          Se enseña tal cual, con su fuente monoespaciada: es lo que se va a
          enviar, y enseñarlo reformateado sería enseñar otra cosa.
        -->
            <pre class="nb-registro nb-selectable">{diagnostico.log.join('\n')}</pre>
          {/if}
        {/if}
      </Seccion>

      <!--
        Los avisos de terceros y la licencia del proyecto.
        Se cargan desde el runtime y se pintan en bloques `<pre>` para no meter
        67 KB en el paquete de arranque ni cientos de filas en el DOM.
      -->
      <Seccion titulo={texto('avisosLegales')}>
        <p class="nb-ayuda">{texto('avisosLegalesAyuda')}</p>

        {#if !avisosMostrados}
          <div class="nb-acciones-diagnostico">
            <Button onclick={cargarAvisosLegales}>{texto('verAvisosLegales')}</Button>
          </div>
        {:else}
          <p class="nb-subtitulo">{texto('avisoApache')}</p>
          <pre class="nb-registro nb-selectable">{avisosMostrados.notice}</pre>

          <p class="nb-subtitulo">{texto('licenciaDelProyecto')}</p>
          <pre class="nb-registro nb-selectable">{avisosMostrados.license}</pre>

          <p class="nb-subtitulo">{texto('avisosDeTerceros')}</p>
          <pre class="nb-registro nb-selectable">{avisosMostrados.thirdParty}</pre>
        {/if}
      </Seccion>
    {/if}
  </div>
</div>

<style>
  /*
   * Dos columnas que se apilan cuando no caben: las secciones y la abierta. Sin
   * punto de corte propio, porque los únicos son los de `anchos.ts`: el reparto
   * sale de cuánto pide cada una.
   */
  .nb-ajustes {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--nb-space-4);
    min-height: 100%;
    padding: var(--nb-space-4);
    box-sizing: border-box;
  }

  .nb-ajustes-nav {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1 1 14rem;
    max-width: 18rem;
    padding: var(--nb-space-2) 0;
  }

  .nb-ajustes-titulo {
    margin: var(--nb-space-2) 0 var(--nb-space-4) var(--nb-space-4);
    font-family: var(--nb-font-display);
    font-size: var(--nb-text-xl);
    font-weight: 600;
    line-height: 1.2;
    color: var(--m3c-on-surface);
  }

  .nb-ajustes-destino {
    display: flex;
    align-items: center;
    gap: var(--nb-space-3);
    width: 100%;
    min-height: 3rem;
    padding: 0 var(--nb-space-4);
    border: none;
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    font: inherit;
    font-size: var(--nb-text-sm);
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition:
      background var(--nb-motion-fast) var(--nb-easing),
      color var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-ajustes-destino:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
    color: var(--m3c-on-surface);
  }

  .nb-ajustes-destino[aria-current='true'] {
    background: var(--m3c-primary-container);
    color: var(--m3c-on-primary-container);
    font-weight: 600;
  }

  .nb-ajustes-destino-texto {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .nb-ajustes-contenido {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-4);
    flex: 999 1 28rem;
    min-width: 0;
    max-width: 56rem;
    padding: var(--nb-space-2) 0 var(--nb-space-6);
  }

  /* Cada sección es una tarjeta de la capa de encima: sin borde ni sombra. */
  :global(.nb-ajustes-contenido > .nb-seccion) {
    padding: var(--nb-space-5);
    border-radius: var(--nb-radius-lg);
    background: var(--m3c-surface-container);
  }

  .nb-acerca {
    display: flex;
    align-items: center;
    gap: var(--nb-space-5);
  }

  .nb-acerca-texto {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    min-width: 0;
  }

  .nb-acerca-nombre {
    margin: 0;
    font-family: var(--nb-font-display);
    font-size: var(--nb-text-xl);
    font-weight: 600;
    line-height: 1.1;
    color: var(--m3c-on-surface);
  }

  /*
   * El estado se dice con la palabra, y el color la acompaña. Los tres salen de
   * los contenedores de Material, que están medidos con su texto encima.
   */
  .nb-estado {
    flex: 0 0 auto;
    padding: var(--nb-space-1) var(--nb-space-3);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
    font-weight: 600;
  }

  .nb-estado[data-state='READY'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }

  .nb-estado[data-state='DEGRADED'],
  .nb-estado[data-state='STARTING'] {
    background: var(--m3c-tertiary-container);
    color: var(--m3c-on-tertiary-container);
  }

  .nb-estado[data-state='FAILED'] {
    background: var(--m3c-error-container);
    color: var(--m3c-on-error-container);
  }

  .nb-fallo {
    margin: 0;
    color: var(--m3c-error);
    font-size: var(--nb-text-sm);
  }

  .nb-acciones-diagnostico {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
    margin-top: var(--nb-space-2);
  }

  .nb-subtitulo {
    margin: var(--nb-space-3) 0 var(--nb-space-2);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    font-weight: 600;
  }

  .nb-grupo-permisos {
    margin-bottom: var(--nb-space-3);
  }

  .nb-permiso-proposito {
    margin: var(--nb-space-3) 0 var(--nb-space-2);
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
    font-weight: 600;
  }

  .nb-salidas {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    max-height: 18rem;
    margin: 0;
    padding: var(--nb-space-3);
    overflow-y: auto;
    list-style: none;
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-highest);
  }

  .nb-direccion {
    color: var(--m3c-on-surface);
    font-family: monospace;
    word-break: break-all;
  }

  .nb-salida-destino {
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
    font-family: monospace;
    word-break: break-all;
  }

  .nb-entrada {
    box-sizing: border-box;
    width: 100%;
    min-height: 2.5rem;
    padding: 0 var(--nb-space-3);
    border: 1px solid var(--m3c-outline);
    border-radius: var(--nb-radius-sm);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-sm);
    transition: none;
  }

  /*
   * El anillo acompaña al contorno del foco, no lo sustituye: apagarlo dejaba a
   * quien navega con el teclado distinguiendo el campo activo por un cambio de
   * borde de tres píxeles.
   */
  .nb-entrada:focus-visible {
    border-color: var(--m3c-primary);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--m3c-primary) 20%, transparent);
  }

  .nb-diccionarios {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    margin: 0 0 var(--nb-space-4) 0;
    padding: 0;
    list-style: none;
  }

  .nb-registro {
    max-height: 18rem;
    margin: var(--nb-space-3) 0 0 0;
    padding: var(--nb-space-3);
    overflow: auto;
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface-variant);
    font-family: monospace;
    font-size: var(--nb-text-xs);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .nb-temas {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
    gap: var(--nb-space-3);
    margin-top: var(--nb-space-2);
  }

  .nb-tema {
    display: flex;
    flex-direction: column;
    padding: 0;
    border: 2px solid transparent;
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font: inherit;
    overflow: hidden;
    cursor: pointer;
    transition: border-color var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-tema:hover {
    border-color: var(--m3c-outline-variant);
  }

  .nb-tema[aria-checked='true'] {
    border-color: var(--m3c-primary);
  }

  .nb-tema-muestra {
    display: flex;
    gap: var(--nb-space-2);
    height: 6rem;
    padding: var(--nb-space-3);
    background: var(--nb-surface);
  }

  .nb-tema-barra {
    width: 2rem;
    border-radius: var(--nb-radius-xs);
    background: var(--nb-surface-raised);
  }

  .nb-tema-cuerpo {
    flex: 1 1 auto;
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    min-width: 0;
  }

  .nb-tema-titulo {
    font-family: var(--nb-font-display);
    font-size: var(--nb-text-md);
    font-weight: 700;
    line-height: 1.2;
    color: var(--nb-text);
  }

  .nb-tema-linea {
    height: 4px;
    border-radius: 2px;
    background: var(--nb-border-strong);
  }

  .nb-tema-linea-corta {
    width: 70%;
  }

  .nb-tema-boton {
    width: 3rem;
    height: 1rem;
    margin-top: auto;
    border-radius: var(--nb-radius-sm);
    background: var(--nb-accent);
  }

  .nb-tema-nombre {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-2);
    padding: var(--nb-space-2) var(--nb-space-3);
    font-size: var(--nb-text-sm);
    font-weight: 500;
    color: var(--m3c-on-surface);
  }

  .nb-cobertura {
    margin: var(--nb-space-2) 0 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    font-variant-numeric: tabular-nums;
  }

  .nb-cobertura strong {
    color: var(--m3c-on-surface);
    font-weight: 600;
  }

  .nb-barra-cobertura {
    display: block;
    height: 6px;
    margin: var(--nb-space-2) 0 var(--nb-space-3);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-highest);
    overflow: hidden;
  }

  .nb-barra-cobertura > span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--m3c-primary);
  }

  .nb-aviso-indice {
    display: flex;
    align-items: flex-start;
    gap: var(--nb-space-3);
    margin: var(--nb-space-2) 0 var(--nb-space-3);
    padding: var(--nb-space-3) var(--nb-space-4);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-tertiary-container);
    color: var(--m3c-on-tertiary-container);
  }

  .nb-aviso-indice .nb-ayuda {
    color: inherit;
  }

  .nb-topes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
  }

  .nb-tope {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    min-width: 7rem;
    padding: var(--nb-space-2) var(--nb-space-4);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-md);
    background: transparent;
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-sm);
    font-weight: 600;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-tope:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-tope[aria-checked='true'] {
    border-color: transparent;
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }

  .nb-proveedor-activo {
    margin: 0;
    padding: var(--nb-space-2) var(--nb-space-3);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
    line-height: 1.5;
  }

  .nb-cuerpo-de-envio {
    max-height: 18rem;
    margin: 0;
    padding: var(--nb-space-2) var(--nb-space-3);
    overflow: auto;
    border-radius: var(--nb-radius-sm);
    background: var(--m3c-surface-container);
    color: var(--m3c-on-surface);
    font-family: monospace;
    font-size: var(--nb-text-xs);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .nb-envios {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-envio summary {
    cursor: pointer;
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
  }

  .nb-tope-detalle {
    font-size: var(--nb-text-xs);
    font-weight: 500;
    opacity: 0.8;
  }

  .nb-acentos {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-3);
    margin-top: var(--nb-space-2);
  }

  .nb-acento {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 2.5rem;
    height: 2.5rem;
    border: 2px solid transparent;
    border-radius: 50%;
    color: var(--nb-on-accent);
    cursor: pointer;
    transition: box-shadow var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-acento:disabled {
    cursor: default;
  }

  .nb-acento[aria-checked='true'] {
    box-shadow:
      0 0 0 3px var(--m3c-surface-container),
      0 0 0 5px var(--m3c-on-surface);
  }

  .nb-fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-4);
    min-height: 3rem;
    padding: var(--nb-space-3) var(--nb-space-4);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    font-size: var(--nb-text-sm);
  }

  .nb-fila-texto {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    font-weight: 500;
  }

  .nb-ayuda {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    line-height: 1.5;
  }

  /*
   * Un modelo es una ficha y no una fila: lleva cuatro datos que hay que poder
   * leer antes de decidir, y apretados en una fila la licencia se pierde entre
   * los megabytes.
   */
  .nb-modelo {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
    margin-top: var(--nb-space-3);
    padding: var(--nb-space-4);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
  }

  .nb-modelo-cabecera {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--nb-space-2);
  }

  .nb-modelo-nombre {
    margin: 0;
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-md);
    font-weight: 600;
  }

  .nb-modelo-datos {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));
    gap: var(--nb-space-2) var(--nb-space-4);
    margin: 0;
    font-size: var(--nb-text-sm);
  }

  .nb-modelo-datos dt {
    color: var(--m3c-on-surface-variant);
  }

  .nb-modelo-datos dd {
    margin: 0;
    color: var(--m3c-on-surface);
    font-weight: 500;
  }

  .nb-modelo-confirmar {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding: var(--nb-space-3);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-highest);
  }
</style>
