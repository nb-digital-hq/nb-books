<!--
  @component
  Estudio: lo que el usuario escribe, sin abrir el libro donde lo escribió.

  Contrato: ADR-013.

  **Frontera con el Lector, para que no se solapen:** en el Lector se ven las
  anotaciones del documento abierto; aquí, las de todos. Es el mismo dato con dos
  alcances, exactamente como la búsqueda de catálogo y la de corpus.

  Una nota **sí se edita**, a diferencia de una anotación: es del usuario y no
  ancla a nada de nadie, así que no hay ancla que romper.

  **Ahora la disposición de Ajustes: navegación, lista y detalle.** el equipo,
  el 2026-09-12: los filtros de anotaciones "mal distribuidos", y pidió la forma
  de Ajustes. Las vistas eran pestañas encima de dos columnas, y los filtros una
  rejilla de botones encima de la lista que empujaba la lista hacia abajo. Ahora
  la navegación es una columna a la izquierda, y lo que filtra una vista -las
  clases de anotación, los tableros- cuelga de ella como subdestinos: un filtro
  es a dónde se va dentro de la vista, no un control encima de su contenido. Los
  filtros sin nada dentro no se ofrecen. El tablero y las vistas de extensión
  ocupan la lista y el detalle juntos, porque no tienen lista.
-->
<script lang="ts">
  import { idioma, textos } from '$lib/i18n';
  import type {
    Annotation,
    AnnotationKind,
    AnnotationCounts,
    Board,
    BoardColumn,
    BoardLimits,
    ExtensionRenderedView,
    LibraryAnnotation,
    Locator,
    Note,
    VocabularyItem,
    LookupOutcome
  } from '$lib/bindings';
  import { lookupSelection } from '$core/ipc';
  import { guardarVocabulario, vocabularyToCards } from '$core/ipc/lector';
  import { mensajeDeError } from '$lib/i18n/errores';
  import { TEXTOS_DE_ESTUDIO } from '$lib/i18n/locales/estudio';

  import { notificar } from '../avisos.svelte';
  import { onDestroy, untrack } from 'svelte';

  import BoardView from './BoardView.svelte';
  import CuadernoDeEstudio from './CuadernoDeEstudio.svelte';
  import Button from '../Button.svelte';
  import Field from '../Field.svelte';
  import Icon from '../Icon.svelte';
  import IconButton from '../IconButton.svelte';
  import type { NombreDeIcono } from '../iconos';
  import Select from '../Select.svelte';
  import EmptyState from './EmptyState.svelte';
  import Seccion from './Seccion.svelte';

  /**
   * Una vista de Estudio.
   *
   * Es una lista abierta a propósito: notas, anotaciones y vocabulario son las
   * nativas, y una extensión añade la suya aquí con el mismo aspecto. Sin este
   * sitio reservado desde el principio, el lienzo llegaría como un
   * injerto y se notaría que se añadió después.
   */
  export interface VistaDeEstudio {
    id: string;
    /** En formato de título. Se traduce. */
    etiqueta: string;
    /** Cuántos elementos tiene, si la cuenta significa algo. */
    cuenta?: number;
    /** La aporta una extensión y no el núcleo. Se marca, no se esconde. */
    deExtension?: boolean;
  }

  interface Props {
    notas: Note[];
    /**
     * Anotaciones de **todos** los documentos, de la más reciente a la más
     * antigua.
     *
     * Es la frontera con el Lector: allí se ven las del documento abierto, en
     * orden de lectura; aquí las de todos, por fecha. La progresión de un libro
     * no es comparable con la de otro, así que el orden de lectura no significa
     * nada cuando hay sesenta.
     */
    anotaciones?: LibraryAnnotation[];
    /** Clase por la que se filtra, o nula para todas. */
    clase?: AnnotationKind | null;
    /**
     * Cuántas hay de cada clase, para no ofrecer un filtro que no lleva a nada.
     * Nulo mientras no se sabe: entonces se ofrecen todos.
     */
    cuentasDeAnotaciones?: AnnotationCounts | null;
    alFiltrar?: (clase: AnnotationKind | null) => void;
    /** Abrir el documento por la anotación. No se ofrece si no hay archivo. */
    alSaltar?: (anotacion: LibraryAnnotation) => void;
    /** Las vistas disponibles. La primera es la que se muestra. */
    vistas?: VistaDeEstudio[];
    /**
     * Lo que compone la extensión cuya vista está puesta, o nulo.
     *
     * Llega compuesto y no se compone aquí: pedirlo es lo que carga el código
     * de la extensión, y eso pasa en el runtime y no en el hilo de la interfaz.
     */
    vistaDeExtension?: ExtensionRenderedView | null;
    vistaActiva?: string;
    alCambiarVista?: (id: string) => void;
    /** Los tableros que hay, para la vista de tableros. */
    tableros?: readonly Board[];
    tableroActivo?: string | null;
    columnasDelTablero?: readonly BoardColumn[];
    limitesDelTablero?: BoardLimits;
    alAbrirTablero?: (id: string) => void;
    /** Vuelve a la lista de tableros, sin ninguno abierto. */
    alCerrarTablero?: () => void;
    /** Crea un tablero con sus primeras columnas, ya nombradas en el idioma en uso. */
    alCrearTablero?: (titulo: string, columnas: string[]) => void;
    alBorrarTablero?: (id: string) => void;
    alAnadirColumna?: (titulo: string) => void;
    alRenombrarColumna?: (columna: string, titulo: string) => void;
    alBorrarColumna?: (columna: string) => void;
    alMoverColumna?: (columna: string, lugar: number) => void;
    alPonerNotaEnColumna?: (columna: string, nota: string) => void;
    alEnlazarEnColumna?: (columna: string, tablero: string) => void;
    alCrearNotaEnColumna?: (columna: string, titulo: string) => void;
    alMoverTarjeta?: (tarjeta: string, columna: string, lugar: number) => void;
    alQuitarTarjeta?: (tarjeta: string) => void;
    /** Abre una nota por su identificador: es lo que hace pulsar una tarjeta. */
    alAbrirNotaPorId?: (id: string) => void;
    /** Notas que apuntan a la abierta. No se guardan: se consultan. */
    retroenlaces: Note[];
    /** Nota abierta, o `null` si no hay ninguna. */
    abierta: Note | null;
    alAbrir: (nota: Note) => void;
    /** `silencioso`: lo guarda el autoguardado, y no se avisa de cada pulsación. */
    alCrear: (titulo: string, cuerpo: string, silencioso?: boolean) => void;
    alGuardar: (id: string, titulo: string, cuerpo: string, silencioso?: boolean) => void;
    alBorrar: (id: string) => void;
    alBorrarAnotacion?: (id: string) => void;
    /**
     * Versiones anteriores de la anotación abierta, si se han pedido.
     *
     * Vacío significa "no se han pedido", no "no hay": una anotación con
     * `supersedes` tiene historial por definición.
     */
    historial?: Annotation[];
    alVerHistorial?: (id: string) => void;
    /**
     * Pasajes que la nota abierta cita, con el libro del que salió cada uno.
     *
     * Es la otra clase de enlace de una nota, y no se declara en el cuerpo: una
     * anotación no tiene nombre que escribir entre corchetes. Ver `citedAnnotations`.
     */
    citadas?: LibraryAnnotation[];
    alQuitarPasaje?: (noteId: string, annotationId: string) => void;
    /** Notas que citan la anotación abierta. No se guardan: se consultan. */
    notasQueCitan?: Note[];
    /** Las notas entre las que se elige al citar. Son las propias del usuario. */
    alCitarEnNota?: (noteId: string, annotationId: string) => void;
    /**
     * Vocabulario consultado, de lo más reciente a lo más antiguo.
     *
     * Es lo que ya ocurrió mientras alguien leía: no hay nada que el usuario
     * tenga que hacer para llenarlo. De cada palabra se puede volver al punto
     * exacto donde apareció, que es lo que la distingue de una lista de palabras
     * sueltas.
     */
    vocabulario?: VocabularyItem[];
    alSaltarAPalabra?: (palabra: VocabularyItem) => void;
    alBorrarPalabra?: (id: string) => void;
    alBorrarVocabulario?: () => void;
    /** Abre el libro en la fuente de una tarjeta o de un concepto. */
    alAbrirFuente?: (fileId: string, locator: Locator) => void;
    etiquetas?: Readonly<Record<string, string>>;
    anotacionActiva?: LibraryAnnotation | null;
    palabraActiva?: VocabularyItem | null;
    /**
     * Qué listas tienen más de lo que se está enseñando, y cómo pedir el resto.
     *
     * Que la función esté **es** la respuesta a "¿hay más?". Con dos formas de
     * decirlo -un booleano y una función- podrían discrepar, y la discrepancia
     * sería un botón que no trae nada o una lista acotada que no dice cómo
     * seguir. Lo segundo esconde datos del usuario. Ver la regla 1 del contrato
     * de carga en `optimization-catalog.md`, sección 1.3.
     */
    mas?: {
      notas?: () => void;
      anotaciones?: () => void;
      vocabulario?: () => void;
      tableros?: () => void;
    };
  }

  let {
    notas,
    anotaciones = [],
    clase = null,
    cuentasDeAnotaciones = null,
    alFiltrar,
    alSaltar,
    alBorrarAnotacion,
    historial = [],
    alVerHistorial,
    citadas = [],
    alQuitarPasaje,
    notasQueCitan = [],
    alCitarEnNota,
    vistas = [{ id: 'notas', etiqueta: 'Notas' }],
    tableros = [],
    tableroActivo = null,
    columnasDelTablero = [],
    limitesDelTablero = { columns: 8, cards: 120 },
    alAbrirTablero,
    alCerrarTablero,
    alCrearTablero,
    alBorrarTablero,
    alAnadirColumna,
    alRenombrarColumna,
    alBorrarColumna,
    alMoverColumna,
    alPonerNotaEnColumna,
    alEnlazarEnColumna,
    alCrearNotaEnColumna,
    alMoverTarjeta,
    alQuitarTarjeta,
    alAbrirNotaPorId,
    vistaDeExtension = null,
    vistaActiva = 'notas',
    alCambiarVista,
    retroenlaces,
    abierta,
    alAbrir,
    alCrear,
    alGuardar,
    alBorrar,
    vocabulario = [],
    alSaltarAPalabra,
    alBorrarPalabra,
    alBorrarVocabulario,
    alAbrirFuente,
    etiquetas,
    anotacionActiva = $bindable(null),
    palabraActiva = $bindable(null),
    mas
  }: Props = $props();

  // El diccionario del idioma en uso, con las anulaciones de quien monta el
  // componente encima. `$derived` y no una constante: cambiar de idioma tiene
  // que repintar, y una constante capturaría el diccionario de un solo momento.
  const t = $derived({ ...textos().estudio, ...etiquetas } as Record<string, string>);

  /**
   * La fecha de una versión, **en el idioma de la interfaz**.
   *
   * No en el del sistema, que es lo que hacía `toLocaleDateString()` sin
   * argumento: en un escritorio en inglés escribía `9/2/2026` dentro de una
   * aplicación en español, donde ese mismo texto significa el 9 de febrero. Es
   * el mismo criterio que ya seguían las cifras de la biblioteca, y aquí faltaba.
   *
   * Sin hora: lo que dice una versión anterior es "esto lo escribiste antes", no
   * a qué minuto, y el minuto ocuparía la mitad de la línea.
   */
  const formatoDeFecha = $derived(new Intl.DateTimeFormat(idioma(), { dateStyle: 'medium' }));

  function fecha(iso: string): string {
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? iso : formatoDeFecha.format(d);
  }
  const texto = (clave: string): string => t[clave] ?? clave;

  /*
   * Lo que se consultó queda: el vocabulario se exporta y se convierte en
   * tarjetas desde aquí. Van con sus textos diferidos y llamando al runtime desde
   * esta vista, que se carga al entrar en Estudio, y no desde la carcasa, que va
   * en el paquete de arranque.
   */
  const te = $derived(TEXTOS_DE_ESTUDIO[idioma()] as Record<string, string>);
  let haciendoTarjetas = $state(false);

  async function exportarVocabulario() {
    try {
      const cuantas = await guardarVocabulario(
        'vocabulario.csv',
        te.vocabularioExportarTitulo ?? ''
      );
      if (cuantas !== null) {
        notificar.exito((te.vocabularioExportado ?? '').replace('{n}', `${cuantas}`));
      }
    } catch (e) {
      notificar.error(mensajeDeError(e));
    }
  }

  async function hacerTarjetas() {
    haciendoTarjetas = true;
    try {
      const hecho = await vocabularyToCards();
      notificar.exito(
        (te.vocabularioTarjetasHechas ?? '')
          .replace('{n}', `${hecho.created}`)
          .replace('{ya}', `${hecho.already}`)
          .replace('{sin}', `${hecho.withoutDefinition}`)
      );
    } catch (e) {
      notificar.error(mensajeDeError(e));
    } finally {
      haciendoTarjetas = false;
    }
  }

  /** Los textos del tablero viven en su propio grupo, como los de los modos. */
  const tt = $derived(textos().tableros as Record<string, string>);
  const textoDeTablero = (clave: string): string => tt[clave] ?? clave;

  let tituloDeTableroNuevo = $state('');

  function crearTablero(evento: SubmitEvent) {
    evento.preventDefault();
    const titulo = tituloDeTableroNuevo.trim();
    if (titulo === '') return;
    // Tres columnas para empezar: un tablero sin columnas no se puede usar, y
    // tres es lo mínimo que ya ordena algo. Se renombran o se borran después.
    alCrearTablero?.(titulo, [
      textoDeTablero('columnaInicial1'),
      textoDeTablero('columnaInicial2'),
      textoDeTablero('columnaInicial3')
    ]);
    tituloDeTableroNuevo = '';
  }

  /** El icono de cada vista en la navegación. Las de extensión llevan el suyo. */
  const ICONO_DE_VISTA: Record<string, NombreDeIcono> = {
    notas: 'nota',
    anotaciones: 'anotaciones',
    vocabulario: 'vocabulario',
    tablero: 'tablero',
    tarjetas: 'ia',
    conceptos: 'enlace'
  };

  /** Las vistas que no tienen lista, y ocupan la lista y el detalle juntos. */
  const vistaAncha = $derived(
    vistaActiva === 'tablero' || !['notas', 'anotaciones', 'vocabulario'].includes(vistaActiva)
  );

  let titulo = $state('');
  let cuerpo = $state('');
  let creando = $state(false);

  // Abrir otra nota trae su contenido al editor. Se hace con una dependencia
  // explícita en el identificador: con el objeto entero, escribir una letra
  // reiniciaría el editor con lo que acaba de escribirse.
  $effect(() => {
    const id = abierta?.id;
    if (id) {
      titulo = abierta?.title ?? '';
      cuerpo = abierta?.body ?? '';
      creando = false;
    }
  });

  /**
   * La definición de la palabra abierta, con la palabra a la que responde.
   *
   * Con la palabra dentro, una respuesta que llega tarde -se consultó una, se
   * abrió otra antes de que respondiera- no pinta la definición de otra palabra.
   * Antes eso se daba por imposible "porque es local", y no lo es: el
   * diccionario puede ser una extensión, y la consulta, un servicio en la red.
   */
  let definicion = $state<{
    id: string;
    estado: 'consultando' | 'lista' | 'fallo';
    resultado: LookupOutcome | null;
  } | null>(null);

  $effect(() => {
    const palabra = palabraActiva;
    if (!palabra) {
      definicion = null;
      return;
    }
    const id = palabra.entry.id;
    definicion = { id, estado: 'consultando', resultado: null };
    lookupSelection(
      palabra.entry.headword,
      palabra.entry.term,
      null,
      palabra.entry.editionId,
      palabra.entry.locator
    )
      .then((resultado) => {
        if (definicion?.id === id) definicion = { id, estado: 'lista', resultado };
      })
      .catch(() => {
        if (definicion?.id === id) definicion = { id, estado: 'fallo', resultado: null };
      });
  });

  const CLASES = $derived<
    Array<{ id: AnnotationKind | null; etiqueta: string; cuenta: number | undefined }>
  >([
    { id: null, etiqueta: texto('todas'), cuenta: cuentasDeAnotaciones?.all },
    { id: 'note', etiqueta: texto('notaClase'), cuenta: cuentasDeAnotaciones?.note },
    { id: 'highlight', etiqueta: texto('resaltados'), cuenta: cuentasDeAnotaciones?.highlight },
    { id: 'bookmark', etiqueta: texto('marcadores'), cuenta: cuentasDeAnotaciones?.bookmark },
    { id: 'underline', etiqueta: texto('subrayados'), cuenta: cuentasDeAnotaciones?.underline },
    { id: 'drawing', etiqueta: texto('trazos'), cuenta: cuentasDeAnotaciones?.drawing }
  ]);

  /**
   * Los filtros que llevan a algo. Observación de uso del 2026-09-12: "si no tiene ítems no
   * se muestra". Sin cuentas todavía se ofrecen todos, y el que está puesto se
   * ofrece siempre: esconder el filtro en uso dejaría la lista filtrada sin decir
   * por qué.
   */
  const clasesVisibles = $derived(
    CLASES.filter(
      (opcion) => opcion.cuenta === undefined || opcion.cuenta > 0 || opcion.id === clase
    )
  );

  const ICONO_DE_FILTRO: Record<string, NombreDeIcono> = {
    todas: 'anotaciones',
    note: 'nota',
    highlight: 'resaltar',
    bookmark: 'marcador',
    underline: 'subrayar',
    drawing: 'trazoLibre'
  };

  const ICONO_DE_CLASE: Record<
    AnnotationKind,
    'nota' | 'resaltar' | 'marcador' | 'subrayar' | 'trazoLibre'
  > = {
    note: 'nota',
    highlight: 'resaltar',
    bookmark: 'marcador',
    underline: 'subrayar',
    drawing: 'trazoLibre'
  };

  /** La cita, y el título del punto cuando no hay cita: un marcador no la trae. */
  function citaDe(anotacion: LibraryAnnotation): string {
    const cita = anotacion.annotation.target.text?.highlight?.trim();
    return cita || anotacion.annotation.target.title?.trim() || anotacion.title;
  }

  // La lista cambia al filtrar y al borrar. Lo abierto tiene que seguir estando
  // en ella: si no, el detalle enseñaría algo que ya no aparece a su izquierda.
  $effect(() => {
    const vivas = anotaciones;
    const abiertaId = anotacionActiva?.annotation.id;
    if (abiertaId && !vivas.some((a) => a.annotation.id === abiertaId)) {
      anotacionActiva = null;
    }
  });

  /**
   * Si la vista en curso está vacía del todo.
   *
   * Con la lista vacía no hay nada que elegir, así que la columna de la lista no
   * pinta nada y el estado vacío ocupa el ancho entero. Antes quedaba una
   * columna de trescientos veinte píxeles en blanco al lado de un mensaje
   * centrado en la otra mitad, y el mensaje aparecía descentrado respecto a la
   * vista sin que nada explicara por qué.
   */
  function empezarNueva() {
    guardarPendiente();
    creando = true;
    titulo = '';
    cuerpo = '';
  }

  function guardar(silencioso = false) {
    clearTimeout(autoguardado);
    // Una nota con texto y sin título no se tira: toma de título su primera
    // línea. Antes no se guardaba, y al salir de Estudio se perdía.
    const nombre = titulo.trim() || primeraLinea(cuerpo);
    if (nombre.length === 0) return;
    if (creando || !abierta) {
      alCrear(nombre, cuerpo, silencioso);
      creando = false;
    } else {
      alGuardar(abierta.id, nombre, cuerpo, silencioso);
    }
  }

  /** Los `[[Título]]` del cuerpo que no son el título de ninguna nota. */
  const enlacesSinNota = $derived.by(() => {
    const existentes = new Set(notas.map((n) => n.title.trim().toLowerCase()));
    const vistos = new Set<string>();
    for (const m of cuerpo.matchAll(/\[\[([^\]]+)\]\]/g)) {
      const t = (m[1] ?? '').trim();
      if (t && !existentes.has(t.toLowerCase())) vistos.add(t);
    }
    return [...vistos].slice(0, 8);
  });

  function primeraLinea(texto: string): string {
    const linea = texto.trim().split('\n')[0]?.trim() ?? '';
    return linea.length > 60 ? `${linea.slice(0, 60)}…` : linea;
  }

  /** Si lo escrito difiere de lo guardado. */
  function hayCambios(): boolean {
    if (creando || !abierta) return titulo.trim() !== '' || cuerpo.trim() !== '';
    return titulo !== (abierta.title ?? '') || cuerpo !== (abierta.body ?? '');
  }

  /*
   * Autoguardado. Antes una nota solo se guardaba con el
   * botón, y cambiar de destino o de nota la tiraba sin avisar. Ahora se guarda
   * sola un segundo después de dejar de escribir, y al irse se guarda lo que
   * quedara pendiente. El botón sigue: es la forma de decir "ya".
   */
  let autoguardado: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    void titulo;
    void cuerpo;
    if (!untrack(hayCambios)) return;
    clearTimeout(autoguardado);
    autoguardado = setTimeout(() => guardar(true), 1000);
  });

  function guardarPendiente() {
    if (hayCambios()) guardar(true);
  }

  function abrirOtra(nota: Note) {
    guardarPendiente();
    alAbrir(nota);
  }

  onDestroy(guardarPendiente);
</script>

<div class="nb-estudio">
  <div class="nb-estudio-marco">
    <nav class="nb-estudio-nav" aria-label={texto('secciones')}>
      <!-- El nombre del destino ya es el encabezado de la barra superior. -->
      <p class="nb-estudio-titulo" aria-hidden="true">{texto('region')}</p>
      {#each vistas as vista (vista.id)}
        <button
          class="nb-estudio-destino"
          type="button"
          aria-current={vistaActiva === vista.id ? 'true' : undefined}
          onclick={() => {
            // Volver a pulsar Tableros con uno abierto vuelve a la lista de tableros:
            // es la salida del tablero, y está donde se entró.
            if (vista.id === 'tablero' && vistaActiva === 'tablero') alCerrarTablero?.();
            else alCambiarVista?.(vista.id);
          }}
        >
          <Icon nombre={ICONO_DE_VISTA[vista.id] ?? 'extensiones'} />
          <span class="nb-estudio-destino-texto">{vista.etiqueta}</span>
          {#if vista.cuenta !== undefined}
            <span class="nb-estudio-cuenta">{vista.cuenta}</span>
          {/if}
          {#if vista.deExtension}
            <!-- De una extensión: se marca, porque quien la instaló querrá saberlo. -->
            <span class="nb-vista-marca" aria-hidden="true"></span>
          {/if}
        </button>

        {#if vista.id === 'anotaciones' && vistaActiva === 'anotaciones' && clasesVisibles.length > 1}
          <div class="nb-estudio-subdestinos" role="group" aria-label={texto('filtro')}>
            {#each clasesVisibles as opcion (opcion.etiqueta)}
              <button
                type="button"
                class="nb-estudio-subdestino"
                aria-pressed={clase === opcion.id}
                onclick={() => alFiltrar?.(opcion.id)}
              >
                <Icon nombre={ICONO_DE_FILTRO[opcion.id ?? 'todas'] ?? 'anotaciones'} tamano={18} />
                <span class="nb-estudio-destino-texto">{opcion.etiqueta}</span>
                {#if opcion.cuenta !== undefined}
                  <span class="nb-estudio-cuenta">{opcion.cuenta}</span>
                {/if}
              </button>
            {/each}
          </div>
        {/if}

        {#if vista.id === 'tablero' && vistaActiva === 'tablero' && tableros.length > 0}
          <div class="nb-estudio-subdestinos" role="group" aria-label={textoDeTablero('tableros')}>
            {#each tableros as tablero (tablero.id)}
              <button
                type="button"
                class="nb-estudio-subdestino"
                aria-current={tableroActivo === tablero.id ? 'true' : undefined}
                onclick={() => alAbrirTablero?.(tablero.id)}
              >
                <span class="nb-estudio-destino-texto">{tablero.title}</span>
                <span class="nb-estudio-cuenta">{tablero.filled}</span>
              </button>
            {/each}
            {#if mas?.tableros}
              <button type="button" class="nb-estudio-subdestino" onclick={mas.tableros}>
                <span class="nb-estudio-destino-texto">{texto('cargarMas')}</span>
              </button>
            {/if}
          </div>
        {/if}
      {/each}
    </nav>

    {#if !vistaAncha}
      <div class="nb-lista">
        {#if vistaActiva === 'anotaciones'}
          <Seccion titulo={texto('anotaciones')}>
            <!--
          La lista vacía no dice nada: lo dice el detalle, una sola vez.

          Antes las dos columnas enseñaban **la misma frase a la vez** y,
          en Notas, además los dos el mismo botón: "Aún no has escrito ninguna
          nota" aparecía en la lista y otra vez en el centro de la pantalla, con
          un "Nota Nueva" en cada sitio. Repetir un mensaje no lo hace más claro;
          lo que hace es que el usuario busque la diferencia entre los dos.
        -->
            {#if anotaciones.length > 0}
              <ul>
                {#each anotaciones as anotacion (anotacion.annotation.id)}
                  <li>
                    <button
                      type="button"
                      class="nb-item"
                      aria-current={anotacionActiva?.annotation.id === anotacion.annotation.id
                        ? 'true'
                        : undefined}
                      onclick={() => (anotacionActiva = anotacion)}
                    >
                      <Icon nombre={ICONO_DE_CLASE[anotacion.annotation.kind]} tamano={16} />
                      <span class="nb-item-titulo">
                        {citaDe(anotacion)}
                        <!-- De qué libro es: sin esto, una lista de citas sueltas
                         no dice de dónde salió ninguna. -->
                        <span class="nb-item-libro">{anotacion.title}</span>
                      </span>
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
            {#if mas?.anotaciones}
              <Button onclick={mas.anotaciones}>{texto('cargarMas')}</Button>
            {/if}
          </Seccion>
        {:else if vistaActiva === 'vocabulario'}
          <Seccion titulo={texto('vocabulario')}>
            {#snippet accion()}
              <!-- La acción se declara siempre y se pinta cuando hay algo que
               borrar: un fragmento declarado dentro de un `if` no llega como
               propiedad del componente, y el botón no aparecería nunca. -->
              {#if vocabulario.length > 0}
                <Button onclick={() => void exportarVocabulario()}>
                  {te.vocabularioExportar}
                </Button>
                <Button deshabilitado={haciendoTarjetas} onclick={() => void hacerTarjetas()}>
                  {te.vocabularioTarjetas}
                </Button>
                <Button variante="peligro" onclick={() => alBorrarVocabulario?.()}>
                  {texto('borrarVocabulario')}
                </Button>
              {/if}
            {/snippet}
            {#if vocabulario.length > 0}
              <p class="nb-ayuda-vocabulario">{te.vocabularioTarjetasAyuda}</p>
              <ul>
                {#each vocabulario as palabra (palabra.entry.id)}
                  <li>
                    <button
                      type="button"
                      class="nb-item"
                      aria-current={palabraActiva?.entry.id === palabra.entry.id
                        ? 'true'
                        : undefined}
                      onclick={() => (palabraActiva = palabra)}
                    >
                      <Icon nombre="vocabulario" tamano={16} />
                      <span class="nb-item-titulo">
                        {palabra.entry.headword}
                        <!-- De qué libro salió: una lista de palabras sueltas no
                         dice dónde se aprendió ninguna. -->
                        <span class="nb-item-libro">{palabra.title}</span>
                      </span>
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
            {#if mas?.vocabulario}
              <Button onclick={mas.vocabulario}>{texto('cargarMas')}</Button>
            {/if}
          </Seccion>
        {:else}
          <Seccion titulo={texto('notas')}>
            {#snippet accion()}
              <Button onclick={empezarNueva}>{texto('nueva')}</Button>
            {/snippet}
            {#if notas.length > 0}
              <ul>
                {#each notas as nota (nota.id)}
                  <li>
                    <button
                      type="button"
                      class="nb-item"
                      aria-current={abierta?.id === nota.id ? 'true' : undefined}
                      onclick={() => abrirOtra(nota)}
                    >
                      <Icon nombre="nota" tamano={16} />
                      <span class="nb-item-titulo">{nota.title}</span>
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
            {#if mas?.notas}
              <Button onclick={mas.notas}>{texto('cargarMas')}</Button>
            {/if}
          </Seccion>
        {/if}
      </div>
    {/if}

    <div class="nb-editor" class:nb-editor-ancho={vistaAncha}>
      {#if vistaActiva === 'tablero'}
        {#if tableroActivo}
          {@const abierto = tableroActivo}
          <BoardView
            titulo={tableros.find((t) => t.id === abierto)?.title ?? ''}
            columnas={columnasDelTablero}
            limites={limitesDelTablero}
            {notas}
            otrosTableros={tableros.filter((t) => t.id !== abierto)}
            {alAnadirColumna}
            {alRenombrarColumna}
            {alBorrarColumna}
            {alMoverColumna}
            alPonerNota={alPonerNotaEnColumna}
            alEnlazar={alEnlazarEnColumna}
            alCrearNota={alCrearNotaEnColumna}
            {alMoverTarjeta}
            {alQuitarTarjeta}
            alAbrirNota={alAbrirNotaPorId}
            alAbrirTablero={(id) => alAbrirTablero?.(id)}
            alBorrarTablero={() => alBorrarTablero?.(abierto)}
          />
        {:else}
          <Seccion titulo={textoDeTablero('tableros')}>
            <p class="nb-ayuda">{textoDeTablero('sinTableros')}</p>
            <form class="nb-tablero-nuevo" onsubmit={crearTablero}>
              <Field etiqueta={textoDeTablero('nombreDelTablero')}>
                {#snippet children({ id })}
                  <input {id} class="nb-campo" type="text" bind:value={tituloDeTableroNuevo} />
                {/snippet}
              </Field>
              <Button
                tipo="submit"
                variante="primario"
                deshabilitado={tituloDeTableroNuevo.trim() === ''}
              >
                {textoDeTablero('crear')}
              </Button>
            </form>
            {#if tableros.length > 0}
              <p class="nb-ayuda">{textoDeTablero('eligeTablero')}</p>
            {/if}
          </Seccion>
        {/if}
      {:else if vistaActiva === 'anotaciones'}
        {#if anotacionActiva}
          {@const activa = anotacionActiva}
          <Seccion titulo={activa.title}>
            <p class="nb-cita-larga">{citaDe(activa)}</p>
            {#if activa.annotation.body}
              <p class="nb-cuerpo-anotacion">{activa.annotation.body}</p>
            {/if}
            {#if activa.authors}
              <p class="nb-autores">{activa.authors}</p>
            {/if}

            <div class="nb-acciones">
              {#if activa.fileId}
                <Button variante="primario" onclick={() => alSaltar?.(activa)}>
                  {texto('abrir')}
                </Button>
              {/if}
              <Button variante="peligro" onclick={() => alBorrarAnotacion?.(activa.annotation.id)}>
                {texto('borrar')}
              </Button>
            </div>

            {#if !activa.fileId}
              <!-- Sin archivo no se ofrece el salto, en lugar de ofrecerlo y
                 fallar al pulsarlo. Se dice por qué. -->
              <p class="nb-nota-vacia">{texto('sinArchivo')}</p>
            {/if}

            <!--
            Las versiones anteriores, cuando las hay.
            
            Una anotación no se reescribe: editarla crea una sucesora y la
            anterior se queda. Esa decisión tiene que poder verse, así que lo que el usuario escribió antes existía en la
            base y en ninguna pantalla. Se ofrecen aquí y no en el panel del
            lector porque leer no es revisar, y porque el panel del lector se
            recorre con el teclado por un contrato que esto no tiene por qué
            tocar.

            Se piden al pulsar, no al seleccionar: son otra consulta, y la
            mayoría de las anotaciones nunca se editaron.
          -->
            <!--
            Citar el pasaje en una nota.
            
            El gesto vive aquí y no en la nota porque aquí la elección es corta:
            las notas del usuario son unas decenas, las anotaciones de la
            biblioteca son miles. Y porque es el momento en que se piensa: quien
            repasa sus anotaciones va decidiendo a qué idea pertenece cada una.
          -->
            {#if notas.length > 0}
              <div class="nb-citar">
                <Select
                  etiqueta={texto('citarEn')}
                  prefijo={texto('citarEn')}
                  valor=""
                  opciones={[
                    ['', texto('eligeNota')],
                    ...notas.map((n) => [n.id, n.title] as [string, string])
                  ]}
                  alCambiar={(noteId) => {
                    if (noteId) alCitarEnNota?.(noteId, activa.annotation.id);
                  }}
                />
              </div>
            {/if}

            {#if notasQueCitan.length > 0}
              <!-- El retroenlace, igual que entre notas: no se guarda, se consulta. -->
              <Seccion titulo={texto('notasQueCitan')}>
                <ul>
                  {#each notasQueCitan as nota (nota.id)}
                    <li>
                      <button type="button" class="nb-item" onclick={() => abrirOtra(nota)}>
                        <Icon nombre="retroenlace" tamano={16} />
                        <span class="nb-item-titulo">{nota.title}</span>
                      </button>
                    </li>
                  {/each}
                </ul>
              </Seccion>
            {/if}

            {#if activa.annotation.supersedes}
              {#if historial.length > 0}
                <ol class="nb-historial">
                  {#each historial as version (version.id)}
                    <li class="nb-version">
                      <span class="nb-version-fecha">{fecha(version.createdAt)}</span>
                      <span class="nb-version-cuerpo">{version.body ?? texto('sinCuerpo')}</span>
                    </li>
                  {/each}
                </ol>
              {:else}
                <div class="nb-acciones">
                  <Button onclick={() => alVerHistorial?.(activa.annotation.id)}>
                    {texto('verHistorial')}
                  </Button>
                </div>
              {/if}
            {/if}
          </Seccion>
        {:else if anotaciones.length > 0}
          <EmptyState icono="anotaciones" mensaje={texto('seleccionaAnotacion')} />
        {:else}
          <EmptyState icono="anotaciones" mensaje={texto('sinAnotaciones')} />
        {/if}
      {:else if vistaActiva === 'vocabulario'}
        {#if palabraActiva}
          {@const activa = palabraActiva}
          <Seccion titulo={activa.entry.headword}>
            <!-- La forma seleccionada y el lema casi nunca coinciden: se
               seleccionó "corriendo" y el diccionario respondió "correr". Las
               dos hacen falta, y por eso se enseñan las dos. -->
            <div class="nb-vocabulario-tarjeta">
              {#if activa.entry.term.toLocaleLowerCase() !== activa.entry.headword.toLocaleLowerCase()}
                <p class="nb-vocabulario-dato">
                  <strong>{texto('consultadaComo')}:</strong> <span>{activa.entry.term}</span>
                </p>
              {/if}
              <p class="nb-vocabulario-dato">
                <strong>{texto('deQueLibro')}:</strong>
                <span class="nb-vocabulario-titulo">{activa.title}</span>
              </p>

              {#if definicion?.estado === 'consultando'}
                <p class="nb-nota-vacia">{texto('consultando')}</p>
              {:else if definicion?.resultado && definicion.resultado.entries.length > 0}
                <div class="nb-vocabulario-definiciones">
                  {#each definicion.resultado.entries as entrada, n (n)}
                    <div class="nb-vocabulario-definicion">
                      <p class="nb-vocabulario-cuerpo">{entrada.body}</p>
                      {#if entrada.source}
                        <p class="nb-vocabulario-fuente">{entrada.source}</p>
                      {/if}
                    </div>
                  {/each}
                </div>
              {/if}
            </div>

            <div class="nb-acciones">
              {#if activa.fileId}
                <Button variante="primario" onclick={() => alSaltarAPalabra?.(activa)}>
                  {texto('dondeAparecio')}
                </Button>
              {/if}
              <Button variante="peligro" onclick={() => alBorrarPalabra?.(activa.entry.id)}>
                {texto('borrar')}
              </Button>
            </div>

            {#if !activa.fileId}
              <p class="nb-nota-vacia">{texto('sinArchivo')}</p>
            {/if}
          </Seccion>
        {:else if vocabulario.length > 0}
          <EmptyState icono="vocabulario" mensaje={texto('seleccionaPalabra')} />
        {:else}
          <EmptyState icono="vocabulario" mensaje={texto('sinVocabulario')} />
        {/if}
      {:else if vistaActiva === 'tarjetas' || vistaActiva === 'conceptos'}
        <CuadernoDeEstudio vista={vistaActiva} alAbrirFuente={(f, l) => alAbrirFuente?.(f, l)} />
      {:else if vistaDeExtension}
        <!--
        Lo que compone una extensión. Se pinta con los componentes del núcleo a
        partir de bloques tipados: una extensión aporta contenido, no marcado.
        Si trajera marcado, el sistema de diseño, la accesibilidad y el
        presupuesto de fotograma dejarían de ser nuestros.
      -->
        <Seccion titulo={vistaDeExtension.title}>
          {#each vistaDeExtension.blocks as bloque, indice (indice)}
            {#if bloque.clase === 'heading'}
              <h3 class="nb-subtitulo">{bloque.valor}</h3>
            {:else if bloque.clase === 'paragraph'}
              <p class="nb-cuerpo-anotacion">{bloque.valor}</p>
            {:else if bloque.clase === 'items'}
              <ul>
                {#each bloque.valor as linea, n (n)}
                  <li class="nb-cita-fila">{linea}</li>
                {/each}
              </ul>
            {:else}
              <p class="nb-vocabulario-dato">
                <strong>{bloque.valor.label}:</strong>
                <span>{bloque.valor.value}</span>
              </p>
            {/if}
          {/each}
        </Seccion>
      {:else if abierta || creando}
        <Field etiqueta={texto('titulo')}>
          {#snippet children({ id })}
            <input {id} class="nb-campo" type="text" bind:value={titulo} />
          {/snippet}
        </Field>

        <Field etiqueta={texto('cuerpo')} descripcion={texto('ayudaEnlace')}>
          {#snippet children({ id, 'aria-describedby': describedby })}
            <textarea
              {id}
              class="nb-campo nb-cuerpo"
              aria-describedby={describedby}
              bind:value={cuerpo}></textarea>
          {/snippet}
        </Field>

        {#if enlacesSinNota.length > 0}
          <!--
            Un [[Título]] que no existe no enlaza con nada, y nada lo decía:
            se ofrece crearla con ese título.
          -->
          <p class="nb-ayuda nb-enlaces-sueltos">
            {texto('enlacesSinNota')}
            {#each enlacesSinNota as falta (falta)}
              <Button
                onclick={() => {
                  guardarPendiente();
                  alCrear(falta, '', true);
                }}>{falta}</Button
              >
            {/each}
          </p>
        {/if}

        <div class="nb-acciones">
          <Button variante="primario" onclick={() => guardar()}>{texto('guardar')}</Button>
          {#if abierta && !creando}
            <Button variante="peligro" onclick={() => alBorrar(abierta.id)}
              >{texto('borrar')}</Button
            >
          {/if}
        </div>

        {#if abierta && !creando}
          <!--
          Los pasajes que la nota cita.
          
          Aquí solo se **ven** y se quitan: citar uno se hace desde la anotación,
          en la otra vista. El motivo es de tamaño, no de gusto. Elegir una nota
          entre las propias es elegir entre unas decenas; elegir una anotación
          entre las de la biblioteca entera es elegir entre miles, y un selector
          de miles de elementos no es un gesto, es una búsqueda. Se enlaza donde
          la elección es la corta.
        -->
          <Seccion titulo={texto('pasajes')}>
            {#if citadas.length === 0}
              <p class="nb-nota-vacia">{texto('sinPasajes')}</p>
            {:else}
              <ul>
                {#each citadas as cita (cita.annotation.id)}
                  <li class="nb-cita-fila">
                    <button
                      type="button"
                      class="nb-item"
                      disabled={!cita.fileId}
                      onclick={() => alSaltar?.(cita)}
                    >
                      <Icon nombre="anotaciones" tamano={16} />
                      <span class="nb-item-titulo">
                        <span class="nb-item-cita">{citaDe(cita)}</span>
                        <span class="nb-item-libro">{cita.title}</span>
                      </span>
                    </button>
                    <IconButton
                      etiqueta={texto('quitarPasaje')}
                      onclick={() => alQuitarPasaje?.(abierta.id, cita.annotation.id)}
                    >
                      <Icon nombre="borrar" tamano={15} />
                    </IconButton>
                  </li>
                {/each}
              </ul>
            {/if}
          </Seccion>

          <Seccion titulo={texto('retroenlaces')}>
            {#if retroenlaces.length === 0}
              <p class="nb-nota-vacia">{texto('sinRetroenlaces')}</p>
            {:else}
              <ul>
                {#each retroenlaces as nota (nota.id)}
                  <li>
                    <button type="button" class="nb-item" onclick={() => abrirOtra(nota)}>
                      <Icon nombre="retroenlace" tamano={16} />
                      <span class="nb-item-titulo">{nota.title}</span>
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          </Seccion>
        {/if}
      {:else if notas.length > 0}
        <!-- Sin acción propia: la lista de al lado ya ofrece "Nota Nueva", y dos
         botones iguales en la misma pantalla hacen dudar de si son lo mismo. -->
        <EmptyState icono="estudio" mensaje={texto('seleccionaNota')} />
      {:else}
        <!-- Tampoco aquí: con la lista vacía, su "Nota Nueva" sigue al lado. -->
        <EmptyState icono="estudio" mensaje={texto('vacio')} />
      {/if}
    </div>
  </div>
</div>

<style>
  .nb-ayuda {
    margin: 0;
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }

  .nb-tablero-nuevo {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: var(--nb-space-3);
    margin-block: var(--nb-space-3);
  }

  /* Una fila de pasaje citado: el salto ocupa lo que puede, quitar no crece. */
  .nb-cita-fila {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
  }

  .nb-cita-fila .nb-item {
    flex: 1 1 auto;
    min-width: 0;
  }

  .nb-item-cita {
    display: block;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .nb-citar {
    margin-top: var(--nb-space-3);
  }

  /* Las versiones anteriores, en orden y sin competir con la vigente. */
  .nb-historial {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    margin: var(--nb-space-4) 0 0;
    padding: 0;
    list-style: none;
  }

  .nb-version {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }

  .nb-version-fecha {
    font-size: var(--nb-text-xs);
    font-variant-numeric: tabular-nums;
    color: var(--m3c-on-surface-variant);
  }

  /* Lo que decía antes. Se respetan sus saltos de línea: es texto escrito. */
  .nb-version-cuerpo {
    white-space: pre-wrap;
  }

  /*
   * La disposición de Ajustes: navegación, lista y detalle, cada una con su propio
   * desplazamiento, para que abrir algo del final de una lista larga no deje el
   * detalle fuera de la vista.
   *
   * Se decide con una consulta de contenedor y no de medios: lo que cuenta es el
   * sitio que le deja la carcasa a Estudio -con la barra lateral desplegada o
   * plegada, con un panel o sin él-, no el ancho de la ventana. Por eso no
   * duplica los puntos de corte de `anchos.ts`, que son de la ventana.
   */
  .nb-estudio {
    container: estudio / inline-size;
    height: 100%;
    min-height: 0;
  }

  .nb-estudio-marco {
    display: grid;
    grid-template-columns: minmax(11rem, 14rem) minmax(13rem, 20rem) minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    gap: var(--nb-space-4);
    box-sizing: border-box;
    height: 100%;
    padding: var(--nb-space-4);
  }

  .nb-estudio-nav,
  .nb-lista,
  .nb-editor {
    min-height: 0;
    overflow: auto;
  }

  .nb-estudio-nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--nb-space-2) 0;
  }

  /* Doble clase: tiene que ganarle al ancho de lectura de `.nb-editor`, que va después. */
  .nb-editor.nb-editor-ancho {
    grid-column: 2 / -1;
    max-width: none;
    margin: 0;
  }

  .nb-estudio-titulo {
    margin: var(--nb-space-2) 0 var(--nb-space-4) var(--nb-space-4);
    font-family: var(--nb-font-display);
    font-size: var(--nb-text-xl);
    font-weight: 600;
    line-height: 1.2;
    color: var(--m3c-on-surface);
  }

  .nb-estudio-destino,
  .nb-estudio-subdestino {
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

  .nb-estudio-subdestino {
    min-height: 2.5rem;
    padding-inline-start: var(--nb-space-6);
    font-size: var(--nb-text-xs);
  }

  .nb-estudio-subdestinos {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: var(--nb-space-2);
  }

  .nb-estudio-destino:hover,
  .nb-estudio-subdestino:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
    color: var(--m3c-on-surface);
  }

  .nb-estudio-destino[aria-current='true'] {
    background: var(--m3c-primary-container);
    color: var(--m3c-on-primary-container);
    font-weight: 600;
  }

  .nb-estudio-subdestino[aria-pressed='true'],
  .nb-estudio-subdestino[aria-current='true'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font-weight: 600;
  }

  .nb-estudio-destino-texto {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .nb-estudio-cuenta {
    flex: 0 0 auto;
    font-size: var(--nb-text-xs);
    font-variant-numeric: tabular-nums;
  }

  /* El punto que marca que una vista la aporta una extensión. Sin regla no se pinta. */
  .nb-vista-marca {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--m3c-on-surface-variant);
  }

  /*
   * Donde no caben las tres columnas, una debajo de otra, y la navegación en una
   * fila que envuelve: una columna de destinos encima del contenido lo empujaría media
   * pantalla hacia abajo.
   */
  @container estudio (max-width: 52rem) {
    .nb-estudio-marco {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: none;
      height: auto;
    }

    .nb-estudio-nav,
    .nb-lista,
    .nb-editor {
      overflow: visible;
    }

    /* Envuelve, y no se desplaza: una barra horizontal escondida es lo que la
       suite prohíbe en cualquier destino. Dos filas a 1100 px es el precio. */
    .nb-estudio-nav {
      flex-direction: row;
      flex-wrap: wrap;
      padding: 0;
    }

    .nb-estudio-titulo {
      display: none;
    }

    .nb-estudio-destino,
    .nb-estudio-subdestino {
      width: auto;
    }

    .nb-estudio-subdestinos {
      flex-direction: row;
      flex-wrap: wrap;
      flex-basis: 100%;
      margin: 0;
    }

    .nb-estudio-subdestino {
      padding-inline-start: var(--nb-space-4);
    }

    .nb-editor.nb-editor-ancho {
      grid-column: auto;
    }
  }

  .nb-ayuda-vocabulario {
    margin: 0 0 var(--nb-space-2);
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
  }

  .nb-item-libro {
    display: block;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }

  /*
   * La cita se enseña entera y no recortada: aquí no acompaña a la lectura, es
   * lo que se ha venido a leer. En la lista sí se recorta, que es donde estorba.
   */
  .nb-cita-larga {
    margin: 0;
    padding: var(--nb-space-4);
    border-left: 3px solid var(--m3c-primary);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-md);
    line-height: 1.6;
  }

  .nb-cuerpo-anotacion {
    margin: var(--nb-space-3) 0 0;
    padding: var(--nb-space-3) var(--nb-space-4);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    white-space: pre-wrap;
  }

  .nb-autores {
    margin: var(--nb-space-1) 0 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }

  .nb-editor {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-4);
    min-width: 0;
    width: 100%;
    max-width: 46rem;
    height: 100%;
    margin: 0 auto;
    overflow: auto;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nb-item {
    display: flex;
    align-items: center;
    gap: var(--nb-space-3);
    width: 100%;
    min-height: 2.5rem;
    padding: 0 var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-md);
    background: transparent;
    color: var(--m3c-on-surface);
    font-family: inherit;
    font-size: var(--nb-text-sm);
    text-align: left;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-item:hover,
  .nb-item:focus-visible {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-item:active {
    background: color-mix(in srgb, var(--m3c-on-surface) 12%, transparent);
  }

  .nb-item[aria-current='true'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font-weight: 600;
  }

  .nb-item-titulo {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .nb-campo {
    box-sizing: border-box;
    width: 100%;
    padding: var(--nb-space-3) var(--nb-space-4);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font-family: inherit;
    font-size: var(--nb-text-md);
    transition:
      border-color var(--nb-motion-fast) var(--nb-easing),
      box-shadow var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-campo:hover {
    border-color: var(--m3c-outline);
  }

  .nb-campo:focus {
    border-color: var(--m3c-primary);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--m3c-primary) 20%, transparent);
  }

  .nb-cuerpo {
    min-height: 14rem;
    resize: vertical;
    line-height: 1.6;
  }

  .nb-acciones {
    display: flex;
    gap: var(--nb-space-2);
  }

  .nb-nota-vacia {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }

  .nb-vocabulario-tarjeta {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    margin: var(--nb-space-2) 0 var(--nb-space-4);
    padding: var(--nb-space-4);
    border-radius: var(--nb-radius-lg);
    background: var(--m3c-surface-container);
  }

  .nb-vocabulario-dato {
    margin: 0;
    display: flex;
    justify-content: flex-start;
    align-items: baseline;
    gap: var(--nb-space-2);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    overflow: hidden;
  }

  /*
   * El título del libro se corta con puntos suspensivos y no se desplaza.
   *
   * Tenía una marquesina en bucle **infinito**, prohibida por la sección 12 de
   * `ui-conventions.md`, que además se movía `-100%`: al llegar al
   * final el título había desaparecido entero de su hueco. Se encendía con
   * `onmouseenter` y se apagaba con `onmouseleave`, así que era una vía de solo
   * ratón, que esta interfaz no admite como única. Y llevaba `role="marquee"`,
   * que anuncia a un lector de pantalla una región que se actualiza sola: el
   * título de un libro no lo es.
   */
  .nb-vocabulario-titulo {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .nb-vocabulario-definiciones {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
    margin-top: var(--nb-space-4);
  }

  .nb-vocabulario-definicion {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
  }

  .nb-vocabulario-cuerpo {
    margin: 0;
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
    line-height: 1.6;
    white-space: pre-wrap;
  }

  .nb-vocabulario-fuente {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
    font-style: italic;
  }

  .nb-vocabulario-dato strong {
    color: var(--m3c-on-surface-variant);
    font-weight: 500;
    font-size: var(--nb-text-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .nb-vocabulario-dato span {
    color: var(--m3c-on-surface);
    font-weight: 500;
  }
  .nb-enlaces-sueltos {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-2);
  }
</style>
