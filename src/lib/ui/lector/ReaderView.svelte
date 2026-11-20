<!--
  @component
  La vista de lector: el documento abierto, sus controles y sus paneles.

  Contrato: docs/architecture/reader-contracts.md y
  ui-components.md..

  Tres cosas que este componente **no** hace, y son las que hacen comprobable su
  criterio de aceptación:

  1. **No decide qué controles existen.** Recorre `comandosDisponibles`, que
     los deriva de las capacidades que declara el proveedor. Un control que un
     formato no soporta se omite, no se atenúa: un botón en gris es una promesa
     a medias, el usuario lo ve, lo intenta y no pasa nada.
  2. **No interpreta teclas.** Se las pasa a `comandoParaTecla`, que es pura, y
     aplica lo que devuelve. Por eso el flujo completo por teclado se prueba en
     `teclado.test.ts` sin navegador.
  3. **No guarda el estado de lectura.** Vive fuera, en la máquina de `estado.ts`,
     que es lo que permite que entrar y salir de la lectura inmersiva no pierda
     la posición.
-->
<script lang="ts">
  import { fondoFiltrado } from './fondo';
  import { MODOS as MODOS_DE_LECTURA, type Modo, modoPorNombre, siguienteModo } from './modos';
  import { codigoDeError, idioma, mensajeDeError, textos } from '$lib/i18n';
  import { TEXTOS_DEL_LECTOR } from '$lib/i18n/locales/lector';
  import { ETIQUETAS_DE_COMANDOS } from '$lib/i18n/locales/comandos';
  import { readingPace } from '$core/ipc/lector';
  import { mobileReleaseControl, readerPublishState } from '$core/ipc/movil';
  import type { ReadingPace } from '$lib/bindings';
  import { queda } from './ritmo';
  import { textoDeQueda } from './texto-de-ritmo';
  import { narrador } from '$lib/voz';
  import { tick, untrack } from 'svelte';

  import type {
    RemoteOrder,
    Locator,
    OutlineEntry,
    PageSize,
    Rendered,
    SessionInfo,
    TextLayer
  } from '$lib/bindings';

  import {
    createAnnotation,
    createBookmark,
    locatorForSelection,
    type LookupOutcome,
    lookupSelection,
    pageSize,
    renderDocument,
    saveProgress,
    setFullscreen,
    textLayer
  } from '$core/ipc';
  import { Button, Icon, IconButton, notificar } from '$ui';

  import {
    atajoLegible,
    barraDelLector,
    type Comando,
    comandoParaTecla,
    comandosDisponibles,
    COMANDOS,
    type IdDeComando,
    pulsacionDe,
    type PanelDelLector,
    type Pulsacion
  } from './comandos';
  import { anuncioDePosicion, hayPosicion, posicionDe } from './posicion';
  import {
    anotacionesDelCapitulo,
    desplazamientoDe,
    marcadoresDelCapitulo,
    tintaDelCapitulo,
    type InformeDeMarcas
  } from './proyeccion';
  import {
    aplicar,
    type Bloque,
    citaSeleccionada,
    seleccionarLibre,
    type Efecto,
    estadoInicial,
    redactar,
    seleccion,
    zonasVisibles
  } from './estado';
  import { type EstadoDeConsulta, terminoInicial } from './consulta';
  import { COMPOSICION_POR_DEFECTO, type Composicion, cuerpoSiguiente } from './libro';
  import ReadingSurface from './ReadingSurface.svelte';
  import { avisarQueElLectorPinta } from '../vistas/significado.svelte';

  interface Props {
    /** Lo que devolvió abrir el documento. Es de donde sale todo lo demás. */
    sesion: SessionInfo;
    composicion?: Composicion;
    /**
     * Los modos de lectura que aportan las extensiones. Sin ellos, uno elegido
     * se guardaba y se marcaba, pero el texto se pintaba con los colores de
     * "día": el nombre no estaba entre los del núcleo y caía al de siempre.
     */
    modosDeExtension?: readonly Modo[];
    /**
     * Las anotaciones del documento, tal y como las da el runtime.
     *
     * Llegan de fuera porque el panel ya las tiene: pedirlas otra vez aquí daría
     * dos listas del mismo dato que se desincronizarían al anotar.
     */
    anotaciones?: readonly OutlineEntry[];
    /** Se avisa al señalar una marca en el texto. */
    alSenalarMarca?: (id: string) => void;
    /** Se avisa de qué marcas se pudieron pintar en el capítulo y cuáles no. */
    alColocarMarcas?: (informe: InformeDeMarcas) => void;
    /** Se avisa cuando cambia lo que la carcasa tiene que saber. */
    alCambiarModo?: (estado: { inmersivo: boolean; panel: PanelDelLector | null }) => void;
    /** Se avisa al crear una anotación, para que el panel se entere. */
    alAnotar?: () => void;
    /**
     * Lo que pide un comando cuya consecuencia no es de este lector.
     *
     * La división la gobierna la carcasa —cada mitad es un lector entero, y
     * meterla aquí obligaría a que un lector supiera del otro—, así que estos
     * cuatro comandos se **cuentan** en lugar de aplicarse. Antes no se
     * contaban a nadie: la tecla `D` y su botón existían, no hacían nada, y la
     * vista dividida no se podía abrir por ningún camino.
     */
    alDividir?: () => void;
    alCambiarMitad?: () => void;
    /**
     * Cambia un ajuste del libro desde el mando del teléfono: el modo o el
     * tamaño de letra, que son de la carcasa y no del lector.
     */
    alComponer?: (campo: 'modo' | 'cuerpo', valor: string | number) => void;
    /** Para o vuelve a poner el fondo sonoro desde el mando. */
    alAlternarFondo?: () => void;
    alRepartir?: (pasos: 1 | -1) => void;
    /**
     * Si esta mitad recibe los comandos del teclado.
     *
     * Los dos lectores escuchan en `window`, así que sin esto una pulsación
     * llegaría a los dos y bajar el cursor movería las dos mitades a la vez.
     * Quién manda lo decide `division.foco`, que vive en la carcasa.
     */
    activo?: boolean;
    /** Si la ventana da para dos columnas de lectura, y si ya hay dos. */
    dividido?: boolean;
    /**
     * Si el fondo sonoro está sonando ahora.
     *
     * Llega de fuera porque quien lo hace sonar es la aplicación, no el lector:
     * el sonido sobrevive a cerrar el documento. Se usa para marcar su botón como
     * funcionando, que es lo que permite pararlo sin abrir el panel a ver.
     */
    fondoSonando?: boolean;
    cabeDividir?: boolean;
    /**
     * Si hay quien genere las funciones de estudio. Sin eso, estudiar no
     * tiene botón ni tecla.
     */
    hayIa?: boolean;
    /**
     * Se avisa de lo que el panel de consulta tiene que enseñar.
     *
     * La consulta la hace esta vista y no la carcasa porque aquí está la sesión
     * —qué edición es y por dónde va—, y sin eso el runtime no podría apuntar la
     * palabra en el vocabulario. Pintarla es de la carcasa, que es quien monta
     * los paneles.
     */
    alConsultar?: (consulta: EstadoDeConsulta) => void;
    /**
     * Palabra que el panel pide consultar, cuando la elige quien lee.
     *
     * Entra por propiedad y no por una llamada por el mismo motivo que
     * `objetivo`: los paneles los monta la carcasa, así que no tienen forma de
     * hablar con esta vista. Cambiar este valor consulta; volver a ponerlo a
     * nulo, no.
     */
    palabraPedida?: string | null;
    /** El párrafo del que salió la palabra pedida, que viaja como contexto. */
    seleccionConsultada?: string;
    /**
     * Destino al que saltar, cuando el salto lo pide un panel.
     *
     * Entra por aquí y no por una llamada porque los paneles los monta la
     * carcasa —son su zona `complementary`— y esta vista es quien sabe mover la
     * lectura. Cambiar este valor mueve; volver a ponerlo a nulo, no.
     */
    /** Solicitado cuando el comando salir ya no tiene nada interno que cerrar. */
    alSalir?: () => void;
    objetivo?: Locator | null;
    /**
     * Por dónde se iba la última vez que esta vista estuvo montada.
     *
     * El lector no se cierra al ir a otro destino, pero la vista sí se desmonta,
     * y antes al volver empezaba en `sesion.locator`, que es dónde
     * estaba el libro **al abrirlo**: quien leía tres capítulos, miraba la
     * biblioteca y volvía, aparecía en el primero. La sesión del runtime sabía
     * dónde estaba y la vista no. Se lee solo al montar.
     */
    desde?: Locator | null;
    /** Cada vez que la lectura se mueve, con el Locator al que llegó. */
    alMoverse?: (locator: Locator) => void;
    /**
     * Por dónde va un audio que se sigue con el texto -lo preparado, o la
     * narración del propio libro-, o nulo. Se resalta su bloque como el de la voz.
     */
    sonandoEn?: Locator | null;
    /** Textos. Se traducen. */
    etiquetas?: Readonly<Record<string, string>>;
    /**
     * El nombre del teléfono que maneja este lector con Leer en el PC, o nulo.
     * Se dice siempre: un lector que se mueve solo sin explicación se lee como
     * una avería.
     */
    controladoDesde?: string | null;
  }

  let {
    sesion,
    composicion = COMPOSICION_POR_DEFECTO,
    modosDeExtension = [],
    anotaciones = [],
    alSenalarMarca,
    alColocarMarcas,
    alCambiarModo,
    alAnotar,
    alDividir,
    alCambiarMitad,
    alComponer,
    alAlternarFondo,
    alRepartir,
    alSalir,
    activo = true,
    dividido = false,
    fondoSonando = false,
    cabeDividir = true,
    hayIa = false,
    alConsultar,
    palabraPedida = null,
    seleccionConsultada = '',
    objetivo = null,
    desde = null,
    alMoverse,
    sonandoEn = null,
    etiquetas,
    controladoDesde = null
  }: Props = $props();

  // El diccionario del idioma en uso, con las anulaciones de quien monta el
  // componente encima. `$derived` y no una constante: cambiar de idioma tiene
  // que repintar, y una constante capturaría el diccionario de un solo momento.
  const t = $derived({ ...textos().lector, ...etiquetas } as Record<string, string>);

  // Las etiquetas de los comandos van en su propio grupo, con la clave puesta
  // en el identificador del comando: el registro declara qué hace cada tecla,
  // no cómo se dice en cada idioma.
  const nombres = $derived(ETIQUETAS_DE_COMANDOS[idioma()] as Record<string, string>);
  const nombreDelComando = (id: string): string => nombres[id] ?? id;
  // Los avisos son de la aplicación entera y no de esta vista, así que no
  // admiten anulación por `etiquetas`: un lector montado en una prueba con otros
  // nombres sigue confirmando lo mismo que el de verdad.
  const avisos = () => textos().avisos;
  const texto = (clave: string): string => t[clave] ?? clave;

  function sesionInicial() {
    return desde ?? sesion.locator;
  }

  // Se inicializa vacío y lo llena el efecto de abajo con la sesión: leer
  // `sesion` aquí capturaría solo su primer valor, y abrir otro documento
  // dejaría la posición del anterior.
  let estado = $state(estadoInicial(sesionInicial()));
  let contenido = $state<Rendered | null>(null);
  let tamano = $state<PageSize | null>(null);
  let capa = $state<TextLayer | null>(null);
  let fallo = $state<string | null>(null);
  let campoDeNota: HTMLTextAreaElement | undefined = $state();
  let cajaDeNota: HTMLElement | undefined = $state();
  let superficie: ReturnType<typeof ReadingSurface> | undefined = $state();
  let direccionPase = $state<1 | -1 | null>(null);

  const controles = $derived(comandosDisponibles(sesion.capabilities));

  /**
   * La frase que dice qué trae el capítulo y no se muestra, o nula si no trae nada.
   *
   * Las clases las da el runtime y los nombres el diccionario; se unen con
   * `Intl.ListFormat` para que "vídeo, audio y un formulario" salga como se dice
   * en cada idioma y no con comas escritas a mano.
   */
  const omitido = $derived.by(() => {
    const clases = contenido?.kind === 'markup' ? (contenido.omitted ?? []) : [];
    if (clases.length === 0) return null;
    const diferidos = TEXTOS_DEL_LECTOR[idioma()] as Record<string, string>;
    const nombres = clases.map(
      (c) => diferidos[`omitido${c.charAt(0).toUpperCase()}${c.slice(1)}`] ?? c
    );
    const lista = new Intl.ListFormat(idioma(), { type: 'conjunction' }).format(nombres);
    return (diferidos.omitido ?? '').replace('{lista}', lista);
  });
  /**
   * La barra, ya agrupada y sin lo que no aplica ahora mismo.
   *
   * Depende de si hay selección: la familia que actúa sobre lo elegido aparece
   * cuando hay algo elegido. Antes se pintaban los veintidós controles siempre,
   * seguidos y del mismo tamaño, y cuatro de ellos no hacían nada al pulsarlos.
   */
  const todasLasFamilias = $derived(
    barraDelLector({
      capacidades: sesion.capabilities,
      haySeleccion: seleccion(estado) !== null,
      dividido,
      cabeDividir,
      hayIa
    })
  );

  /**
   * La barra de siempre y la de lo seleccionado, **separadas**.
   *
   * Se pidió el 2026-09-18: al elegir una línea, las acciones sobre lo
   * elegido entraban en la misma barra y la alargaban, así que todo lo demás se
   * desplazaba justo cuando la mano ya iba hacia un botón. Ahora salen en una
   * barra propia **al lado**, con la misma forma: la de siempre no cambia de
   * tamaño ni de sitio, que es lo que permite aprenderse dónde está cada cosa.
   */
  const familias = $derived(todasLasFamilias.filter((f) => f.grupo !== 'seleccion'));
  const familiaDeSeleccion = $derived(
    todasLasFamilias.find((f) => f.grupo === 'seleccion') ?? null
  );

  /**
   * Cómo se ve un control según lo que esté pasando.
   *
   * Tres estados y no dos, porque significan cosas distintas: **abierto** es un
   * panel desplegado o un modo puesto —se anuncia con `aria-expanded` o
   * `aria-pressed`, según sea una divulgación o una elección—; **funcionando** es
   * algo que está ocurriendo ahora mismo, como la voz narrando o el fondo
   * sonoro, y lleva su propio color. Antes la barra no decía
   * ninguno de los dos salvo para los paneles: leer en voz alta y tener el panel
   * de voz abierto se veían idénticos.
   */
  /** Los que son un modo puesto: se anuncian como elección, siempre, puesta o no. */
  const MODOS: readonly IdDeComando[] = ['inmersion', 'dividir', 'trazo', 'pantalla-completa'];

  function estadoDelComando(id: IdDeComando): {
    abierto: boolean;
    esModo: boolean;
    elegido: boolean;
    funcionando: boolean;
  } {
    const comando = COMANDOS.find((c) => c.id === id);
    const panelAbierto = comando?.panel ? estado.panel === comando.panel : false;
    const elegido =
      (id === 'inmersion' && estado.inmersivo) ||
      (id === 'dividir' && dividido) ||
      (id === 'trazo' && estado.dibujando) ||
      (id === 'pantalla-completa' && estado.pantallaCompleta);
    const funcionando =
      (id === 'voz' && narrador.estado.estado !== 'parada') ||
      (id === 'fondo-sonoro' && fondoSonando);
    return { abierto: panelAbierto, esModo: MODOS.includes(id), elegido, funcionando };
  }

  /**
   * Las teclas que se recuerdan al entrar en inmersión.
   *
   * En inmersión no queda ningún control a la vista, así que hay que decir por
   * dónde se vuelve. Se muestra al entrar y se retira solo: un recordatorio
   * permanente sería otra barra, y la inmersión existe para no tener ninguna.
   */
  const recordatorio = $derived(
    controles.filter((c) =>
      ['anotar', 'anotaciones', 'indice', 'ajustes-del-libro', 'salir'].includes(c.id)
    )
  );

  let recordando = $state(false);

  $effect(() => {
    if (!estado.inmersivo) {
      recordando = false;
      return;
    }
    recordando = true;
    const temporizador = setTimeout(() => (recordando = false), 4000);
    return () => clearTimeout(temporizador);
  });
  const rango = $derived(seleccion(estado));

  /*
   * **Lo que se narra, leído aparte del estado entero.** Medida del 2026-09-11.
   *
   * El efecto de abajo leía `estado.bloques` y `estado.locator` directamente, así
   * que corría con cada tecla, porque cada tecla sustituye `estado`. Y cada vez el
   * narrador volvía a partir en frases el capítulo entero: perfilado en Chromium
   * sobre un capítulo de mil seiscientos párrafos, partir frases se llevaba 1,9 de
   * los 3 segundos que costaban cuarenta flechas. Leídos aquí, la lista de bloques y
   * el locator siguen siendo los mismos objetos al mover el cursor, y el efecto no
   * vuelve a correr hasta que cambian de verdad.
   */
  const bloquesParaNarrar = $derived(estado.bloques);
  const locatorParaNarrar = $derived(estado.locator);

  /*
   * Lo que hay para narrar. El narrador es uno solo en toda la aplicación
   * —hay una salida de audio—, así que el lector no lo posee: le dice qué tiene
   * delante y él decide. Cambiar de documento le hace callar, que es lo que hay
   * que hacer con una voz que estaba diciendo un texto que ya no está.
   */
  $effect(() => {
    narrador.registrar(bloquesParaNarrar, locatorParaNarrar, sesion.fileId, sesion.language);
  });

  /** Índice plano del documento, para navegar por él con el teclado. */
  const indice = $derived(sesion.navigation.entries);

  /** Dónde está la lectura, para la barra de posición. */
  const posicion = $derived(posicionDe(estado.locator, indice));

  /*
   * Lo que queda del libro a tu ritmo. El ritmo se pide una vez por
   * documento -sale de las sesiones ya cerradas, que no cambian mientras se lee- y
   * lo que queda se recalcula con la posición de ahora. Sin ritmo, no se dice nada.
   */
  let ritmo = $state<ReadingPace | null>(null);
  $effect(() => {
    const edicion = sesion.editionId;
    ritmo = null;
    readingPace(edicion)
      .then((r) => {
        if (sesion.editionId === edicion) ritmo = r;
      })
      .catch(() => undefined);
  });
  const loQueQueda = $derived(
    textoDeQueda(queda(ritmo, estado.locator.locations.totalProgression ?? null))
  );

  let anuncioDondeEstoy = $state('');

  function anunciarDondeEstoy(): void {
    const encabezado = superficie?.encabezadoMasCercano?.() ?? null;
    const nuevo = anuncioDePosicion(posicion, encabezado, {
      capitulo: texto('capitulo'),
      de: texto('de'),
      pagina: texto('pagina')
    });
    if (!nuevo) return;
    if (anuncioDondeEstoy === nuevo) {
      anuncioDondeEstoy = '';
      setTimeout(() => {
        anuncioDondeEstoy = nuevo;
      }, 50);
    } else {
      anuncioDondeEstoy = nuevo;
    }
  }

  /*
   * **El capítulo como texto, y no como el estado entero.** Es una medida del
   * 2026-09-11.
   *
   * `estado` se sustituye con cada tecla, y las marcas, los puntos y la tinta del
   * capítulo leían `estado.locator.href` directamente: cada flecha los rehacía como
   * listas nuevas, y la superficie, al recibir listas nuevas, volvía a recorrer y
   * normalizar el capítulo entero para colocarlas. Medido en WebKitGTK, mover el
   * cursor costaba 18 ms con cincuenta párrafos, 35 con cuatrocientos y 115 con mil
   * seiscientos, casi todo en ese recálculo.
   *
   * Leído aquí como cadena, el valor no cambia al mover el cursor, y lo que depende
   * de él no se vuelve a calcular hasta que cambia de verdad el capítulo.
   */
  const capituloActual = $derived(estado.locator.href);

  /*
   * Las anotaciones de **este** capítulo, en coordenadas del runtime.
   *
   * Se recalculan al cambiar de capítulo y al anotar, que son las dos veces que
   * cambia lo que hay que pintar. No dependen de la composición: mover el cuerpo
   * de letra no mueve un carácter de sitio dentro del texto.
   */
  const marcasDelCapitulo = $derived(anotacionesDelCapitulo(anotaciones, capituloActual));

  /*
   * Los puntos: los marcadores del capítulo y el separador.
   *
   * El separador sale de `sesion.locator` y **no** de `estado.locator`, y la
   * diferencia es la que lo hace útil: el segundo se mueve contigo mientras
   * lees, así que una cinta ahí señalaría siempre donde ya estás mirando. El
   * primero es por dónde ibas **al abrir el libro**, que es justo lo que una
   * cinta de papel recuerda mientras el libro está abierto.
   */
  const puntosDelCapitulo = $derived.by(() => {
    const marcadores = marcadoresDelCapitulo(anotaciones, capituloActual);
    const inicio = desplazamientoDe(sesion.locator, capituloActual);
    if (inicio === null) return marcadores;
    return [...marcadores, { id: 'separador', clase: 'separador' as const, inicio }];
  });

  /**
   * Guarda un trazo recién dibujado.
   *
   * Se ancla al bloque donde se apoyó el lápiz, con la misma forma de locator que
   * un marcador: el trazo señala una zona, no un tramo de caracteres.
   */
  async function guardarTrazo(cita: string, tinta: string) {
    try {
      const ancla = await locatorForSelection(sesion.fileId, cita);
      await createAnnotation(sesion.editionId, 'drawing', null, ancla, tinta);
      alAnotar?.();
      notificar.exito(avisos().trazoGuardado);
    } catch (e) {
      // Un trazo que no se puede guardar **se dice**: la tinta desaparece de la
      // pantalla al soltar, así que sin aviso parecería que se guardó.
      notificar.error(codigoDeError(e) ?? 'INTERNAL');
    }
  }

  /** Los trazos libres del capítulo, con su geometría ya interpretada. */
  const tintaDelCapituloActual = $derived(tintaDelCapitulo(anotaciones, capituloActual));

  // Abrir otro documento reinicia la lectura, pero no el modo: quien lee inmerso
  // y abre el siguiente libro no quiere que le devuelvan la carcasa.
  //
  // La única dependencia es el documento. El modo se lee con `untrack` porque
  // este efecto **escribe** el estado: leerlo sin más lo convertiría en
  // dependencia de sí mismo y el efecto se repetiría sin fin.
  $effect(() => {
    const documento = sesion.fileId;
    const anterior = untrack(() => estado.inmersivo);
    const inicio = untrack(() => desde ?? sesion.locator);
    estado = { ...estadoInicial(inicio), inmersivo: anterior };
    void pintar(inicio);
    void documento;
  });

  // Soltada la selección libre -marcada, o movido el cursor-, se quita también la
  // del navegador: si no, seguiría pintada encima de lo que ya se anotó.
  $effect(() => {
    if (!estado.libre) untrack(() => window.getSelection()?.removeAllRanges());
  });

  // Quien monta esta vista guarda por dónde va, para volver ahí si la desmonta.
  $effect(() => {
    alMoverse?.(estado.locator);
  });

  // Quien escucha guarda el modo, y guardarlo vuelve a renderizar el lector: si
  // se avisara en cada pasada del efecto, el aviso se alimentaría a sí mismo. Se
  // avisa del cambio, no de la pasada. No es estado reactivo a propósito: lo lee
  // el mismo efecto que lo escribe.
  let ultimoModo: string | null = null;

  $effect(() => {
    const modo = { inmersivo: estado.inmersivo, panel: estado.panel };
    const huella = JSON.stringify(modo);
    if (huella === ultimoModo) return;
    ultimoModo = huella;
    alCambiarModo?.(modo);
  });

  // El destino entra por la prop y la navegación es asíncrona: leer el estado
  // dentro de `irA` ocurre ya fuera del rastreo, así que este efecto solo
  // depende de `objetivo`, que es lo que se quiere.
  $effect(() => {
    if (objetivo) void irA(objetivo);
  });

  // El campo de la nota recibe el foco al abrirse: si no, escribir exigiría
  // alcanzarlo con el ratón, que es justo lo que el criterio prohíbe.
  $effect(() => {
    if (!estado.redaccion) return;
    campoDeNota?.focus();
    // Y lo que se anota queda a la vista, por encima del editor.
    const alto = cajaDeNota?.getBoundingClientRect().height ?? 0;
    untrack(() => superficie?.despejar(alto + 16));
  });

  async function pintar(locator: Locator) {
    fallo = null;
    try {
      contenido = await renderDocument(sesion.fileId, locator);
    } catch (error) {
      contenido = null;
      fallo = mensajeDeError(error);
      return;
    }

    // El tamaño y la capa solo existen para lo que se pinta como imagen. Que un
    // formato no los tenga no es un fallo: es lo que su contrato anticipa.
    // Solo lo que se pinta como imagen tiene tamaño de página y capa de texto.
    // Pedirlos para un EPUB costaba dos viajes al runtime que volvían con
    // FORMAT_UNSUPPORTED en cada apertura.
    const esImagen = contenido.kind !== 'markup';
    tamano = esImagen ? await opcional(() => pageSize(sesion.fileId, locator)) : null;
    capa = esImagen ? await opcional(() => textLayer(sesion.fileId, locator)) : null;

    // Y la vista va a donde dice el locator, no al principio del capítulo.
    await tick();
    requestAnimationFrame(() => superficie?.situar(locator));
  }

  /**
   * Apunta ya por dónde se va, sin esperar al reposo del desplazamiento.
   *
   * Lo llama la carcasa antes de cerrar el documento: cerrar con la cruz no
   * pasaba por `salir` y la posición de los últimos segundos se perdía.
   */
  export async function guardarPosicion(): Promise<void> {
    const cita = superficie?.citaActual() ?? null;
    if (cita) await anotarDondeSeQuedo(cita);
    else await saveProgress(sesion.fileId, estado.locator).catch(() => {});
  }

  /** Mueve la lectura a un locator y lo deja apuntado en el progreso. */
  async function irA(destino: Locator) {
    fallo = null;
    try {
      await saveProgress(sesion.fileId, destino);
    } catch (error) {
      fallo = mensajeDeError(error);
      return;
    }
    estado = { ...estado, locator: destino, cursor: -1, ancla: null };
    await pintar(destino);
  }

  async function opcional<T>(pedir: () => Promise<T>): Promise<T | null> {
    try {
      return await pedir();
    } catch {
      return null;
    }
  }

  async function ejecutar(efecto: Efecto) {
    fallo = null;
    try {
      switch (efecto.tipo) {
        case 'pagina': {
          if (superficie && superficie.avanzar(efecto.direccion)) return;
          const destino = vecino(efecto.direccion);
          if (!destino) return;
          direccionPase = efecto.direccion;
          // El progreso lo mueve el runtime, que además dice con qué precisión
          // se resolvió: la interfaz avisa en lugar de colocar al usuario en un
          // sitio parecido sin decírselo.
          await irA(destino);
          return;
        }

        case 'consultar':
          await consultar(terminoInicial(efecto.seleccion), efecto.seleccion);
          return;

        case 'anotar': {
          const ancla = await locatorForSelection(sesion.fileId, efecto.cita);
          await createAnnotation(sesion.editionId, efecto.clase, efecto.cuerpo, ancla);
          alAnotar?.();
          // Se confirma porque **no se ve**: la anotación se guarda en un panel
          // que puede estar cerrado, y en lectura inmersiva no hay panel
          // ninguno. Sin esto, anotar y no anotar se veían igual.
          /*
           * Cada clase confirma con su nombre.
           *
           * Con dos clases un ternario se leía; con tres deja de leerse, y sobre
           * todo deja de avisar cuando llega una cuarta: un mapa por clase obliga
           * a decidir el texto al añadirla, mientras que el ternario habría
           * mandado el subrayado a decir "anotación guardada" sin que nadie se
           * enterase.
           */
          const CONFIRMACION = {
            highlight: avisos().resaltadoGuardado,
            underline: avisos().subrayadoGuardado,
            note: avisos().anotacionGuardada
          } as const;
          notificar.exito(CONFIRMACION[efecto.clase]);
          return;
        }

        case 'marcador':
          await createBookmark(sesion.fileId);
          alAnotar?.();
          notificar.exito(avisos().marcadorPuesto);
          return;

        case 'donde-estoy':
          anunciarDondeEstoy();
          return;

        case 'capitulo': {
          const destino = vecino(efecto.direccion);
          if (!destino) return;
          direccionPase = efecto.direccion;
          await irA(destino);
          return;
        }

        case 'avance':
          fijarAvance(velocidadDeAvance > 0 ? 0 : 1);
          return;

        case 'pantalla-completa':
          await setFullscreen(efecto.activa);
      }
    } catch (error) {
      fallo = mensajeDeError(error);
      notificar.error(fallo, codigoDeError(error) ?? undefined);
    }
  }

  /**
   * Consulta una palabra y va contando lo que pasa.
   *
   * Avisa dos veces: al empezar, para que el panel enseñe la palabra y que se
   * está consultando, y al terminar con lo que haya. Sin lo primero, el panel
   * aparecería vacío hasta que respondiera el diccionario, y un panel vacío se
   * lee como "no hay nada".
   *
   * **No toca el estado del lector.** Ni el locator, ni el cursor, ni la
   * selección: es el criterio de aceptación, y aquí es que no hay
   * ninguna asignación que pudiera moverlos.
   */
  async function consultar(termino: string, seleccion: string) {
    if (!termino) {
      alConsultar?.({ termino: '', seleccion, resultado: null, cargando: false, error: null });
      return;
    }

    alConsultar?.({ termino, seleccion, resultado: null, cargando: true, error: null });

    let resultado: LookupOutcome | null = null;
    let error: string | null = null;
    try {
      // El párrafo viaja como contexto y la palabra como término: un
      // diccionario responde por la palabra y un traductor usa la frase. Es la
      // distinción que el contrato ya declaraba.
      resultado = await lookupSelection(
        termino,
        seleccion || null,
        null,
        sesion.editionId,
        estado.locator
      );
    } catch (fallo) {
      // El código, no el mensaje: el panel distingue "no hay ningún diccionario"
      // de "falló", y son dos textos distintos para quien lo lee.
      error = fallo instanceof Error && 'code' in fallo ? String(fallo.code) : 'INTERNAL';
    }

    alConsultar?.({ termino, seleccion, resultado, cargando: false, error });
  }

  // Elegir otra palabra en el panel entra por aquí, igual que el salto del
  // índice entra por `objetivo`: los paneles los monta la carcasa y no pueden
  // llamar a esta vista.
  $effect(() => {
    if (palabraPedida)
      void consultar(
        palabraPedida,
        untrack(() => seleccionConsultada)
      );
  });

  /** Entrada del índice contigua a la posición actual. */
  function vecino(direccion: 1 | -1): Locator | null {
    const actual = indice.findIndex((e) => e.locator.href === estado.locator.href);
    const siguiente = (actual < 0 ? 0 : actual) + direccion;
    return indice[siguiente]?.locator ?? null;
  }

  /**
   * Controles a los que el teclado les pertenece mientras tienen el foco.
   *
   * Un botón no está: sobre un botón las letras no son letras, y los atajos del
   * lector tienen que seguir funcionando con el foco en la barra.
   */
  const CAMPOS = new Set(['INPUT', 'TEXTAREA', 'SELECT']);

  /**
   * Si el foco está dentro de un campo, sea de quien sea.
   *
   * **No basta con mirar la redacción de este lector.** Lo aprendimos con el
   * panel de anotaciones: escribir "empieza" dentro de su campo
   * disparaba la lectura inmersiva por la `z`, porque el lector escucha en
   * `window` y no sabía que había otro campo abierto en la carcasa. El registro
   * ya decía que `escribiendo` significa "el foco está dentro de un campo de
   * texto"; lo que había debajo era una aproximación que dejó de valer en cuanto
   * hubo un segundo campo.
   */
  function elFocoEstaEnUnCampo(): boolean {
    const activo = document.activeElement as HTMLElement | null;
    if (!activo) return false;
    return CAMPOS.has(activo.tagName) || activo.isContentEditable;
  }

  /**
   * Comandos cuya consecuencia es de la carcasa y no de este lector.
   *
   * Se avisa y se aplica igual la transición, que para estos cuatro no toca
   * nada: así el registro de comandos sigue siendo la única fuente de lo que
   * hace cada tecla, y no hay una segunda lista aquí que pueda divergir.
   */
  function contarALaCarcasa(comando: Comando): void {
    switch (comando.id) {
      case 'dividir':
        alDividir?.();
        return;
      case 'cambiar-mitad':
        alCambiarMitad?.();
        return;
      case 'estrechar-mitad':
        alRepartir?.(-1);
        return;
      case 'ensanchar-mitad':
        alRepartir?.(1);
        return;
      default:
    }
  }

  /**
   * Apunta la posición a la que el usuario ha llegado desplazándose.
   *
   * El Locator lo construye **el runtime** a partir de la cita, no esta capa.
   * Escribirle solo la progresión, como se hacía, dejaba los demás selectores
   * apuntando a donde se estaba antes: un Locator que dice dos cosas a la vez,
   * que es justo lo que ADR-004 existe para que no ocurra. Y como la biblioteca
   * lee `totalProgression`, que ese camino nunca tocaba, la barra de progreso no
   * se movía por mucho que se leyera.
   *
   * Un fallo aquí no interrumpe la lectura: desplazarse tiene que seguir
   * funcionando aunque no se pueda apuntar dónde se llegó.
   */
  async function anotarDondeSeQuedo(cita: string) {
    try {
      const ancla = await locatorForSelection(sesion.fileId, cita);
      estado = { ...estado, locator: ancla };
      await saveProgress(sesion.fileId, ancla);
    } catch {
      // El runtime ya lo registró. Insistir aquí solo llenaría la consola.
    }
  }

  function ejecutarComando(comando: Comando): void {
    // Pasar página pinta aunque no desplace: la preparación de fondo se aparta
    // igual.
    if (comando.id.startsWith('pagina-') || comando.id.startsWith('bloque-')) {
      avisarQueElLectorPinta();
    }
    // Sin cursor todavía, la primera flecha empieza por el párrafo que se ve y no
    // por el primero del capítulo: al reabrir un libro a mitad de escena, la
    // flecha devolvía al título de la escena.
    if (estado.cursor < 0 && /^(bloque|extender)-/.test(comando.id) && superficie) {
      const visible = superficie.bloqueParaNarrar();
      const ajuste = comando.id.endsWith('siguiente')
        ? -1
        : comando.id.endsWith('anterior')
          ? 1
          : 0;
      estado = {
        ...estado,
        cursor: comando.id.startsWith('extender') ? visible : visible + ajuste
      };
    }
    const transicion = aplicar(estado, comando);
    // Salir con el estado intacto significa que no quedaba nada interno que
    // cerrar —ni panel, ni selección, ni nota a medias—, así que el comando
    // sale del lector. Se apunta la posición antes: cerrar sin apuntarla
    // devuelve al usuario unos párrafos más atrás de donde lo dejó.
    if (transicion.estado === estado && comando.id === 'salir') {
      void saveProgress(sesion.fileId, estado.locator)
        .catch(() => {
          // Guardado por el runtime o no, salir no se puede quedar bloqueado.
        })
        .finally(() => alSalir?.());
      return;
    }
    estado = transicion.estado;
    if (transicion.efecto) void ejecutar(transicion.efecto);
    contarALaCarcasa(comando);
  }

  function despachar(pulsacion: Pulsacion) {
    const comando = comandoParaTecla(pulsacion, {
      capacidades: sesion.capabilities,
      haySeleccion: rango !== null,
      escribiendo: estado.redaccion !== null || elFocoEstaEnUnCampo(),
      hayIa
    });
    if (!comando) return false;

    ejecutarComando(comando);
    return true;
  }

  /*
   * El avance automático: la lectura se mueve sola a un ritmo.
   *
   * En continuo se desplaza poco a poco; en paginado pasa una página cada cierto
   * tiempo, porque desplazar una página que no se desplaza no mueve nada. Al
   * acabar el capítulo sigue con el siguiente por el mismo camino que la tecla
   * de pasar página. Lo enciende la tecla `g` o el teléfono con Leer en el PC.
   */
  /** Píxeles por segundo a velocidad 1, en continuo. */
  const PIXELES_POR_SEGUNDO = 40;
  /** Segundos por página a velocidad 1, en paginado. */
  const SEGUNDOS_POR_PAGINA = 20;
  let velocidadDeAvance = $state(0);
  let marcoDeAvance: number | null = null;
  let relojDeAvance: ReturnType<typeof setTimeout> | null = null;
  let ultimoMarco: number | null = null;
  let acumuladoDeAvance = 0;

  function pararAvance() {
    if (marcoDeAvance !== null) cancelAnimationFrame(marcoDeAvance);
    if (relojDeAvance !== null) clearTimeout(relojDeAvance);
    marcoDeAvance = null;
    relojDeAvance = null;
    ultimoMarco = null;
    acumuladoDeAvance = 0;
  }

  /*
   * En paginado basta un reloj de cuarto de segundo: pedir sesenta fotogramas
   * por segundo para esperar veinte a pasar página es trabajo de balde, y el
   * navegador no da fotogramas a una ventana tapada, así que el avance que se
   * pide desde el teléfono con otra ventana delante no pasaría de página.
   */
  function siguientePaso() {
    if (superficie?.paginada()) {
      relojDeAvance = setTimeout(() => pasoDeAvance(performance.now()), 250);
    } else {
      marcoDeAvance = requestAnimationFrame(pasoDeAvance);
    }
  }

  function pasoDeAvance(ahora: number) {
    marcoDeAvance = null;
    relojDeAvance = null;
    if (velocidadDeAvance <= 0 || !superficie) return;
    const paginada = superficie.paginada();
    const tope = paginada ? 1 : 0.25;
    const segundos = ultimoMarco === null ? 0 : Math.min((ahora - ultimoMarco) / 1000, tope);
    ultimoMarco = ahora;
    if (paginada) {
      acumuladoDeAvance += segundos * velocidadDeAvance;
      if (acumuladoDeAvance >= SEGUNDOS_POR_PAGINA) {
        acumuladoDeAvance = 0;
        void ejecutar({ tipo: 'pagina', direccion: 1 });
      }
    } else {
      acumuladoDeAvance += segundos * velocidadDeAvance * PIXELES_POR_SEGUNDO;
      const px = Math.floor(acumuladoDeAvance);
      if (px >= 1) {
        acumuladoDeAvance -= px;
        if (!superficie.desplazar(px)) void ejecutar({ tipo: 'pagina', direccion: 1 });
      }
    }
    siguientePaso();
  }

  function fijarAvance(velocidad: number) {
    velocidadDeAvance = Math.max(0, Math.min(velocidad, 10));
    pararAvance();
    if (velocidadDeAvance > 0) siguientePaso();
  }

  $effect(() => () => pararAvance());

  /**
   * Una orden de Leer en el PC (ADR-016).
   *
   * Los comandos entran por `ejecutarComando`, el mismo camino que el teclado:
   * el teléfono es un teclado más y no una segunda forma de mover el lector. Un
   * comando que este documento no ofrece se ignora, igual que su tecla.
   */
  function comandoRemoto(id: string): void {
    // Lo que en el teclado abre un panel, el mando lo hace directamente: no ve
    // el panel. La letra cambia el cuerpo de un texto que fluye y, en una página
    // que se pinta como imagen (PDF, cómic), la amplía.
    if (id === 'fondo-sonoro') return alAlternarFondo?.();
    if (id === 'modo-de-lectura') {
      const nombres = [...MODOS_DE_LECTURA, ...modosDeExtension].map((m) => m.nombre);
      return alComponer?.('modo', siguienteModo(composicion.modo, nombres));
    }
    if (id === 'letra-mayor' || id === 'letra-menor') {
      const direccion = id === 'letra-mayor' ? 1 : -1;
      alComponer?.('cuerpo', cuerpoSiguiente(composicion.cuerpo, direccion));
      return comandoRemoto(direccion === 1 ? 'acercar' : 'alejar');
    }
    const comando = comandosDisponibles(sesion.capabilities).find((c) => c.id === id);
    if (!comando) return;
    // La tecla de la voz abre su panel para elegir voz y velocidad; el mando no
    // ve el panel, así que hace lo que hace su botón: empezar, pausar o seguir.
    if (id === 'voz') {
      narrador.alternar(bloqueParaNarrar());
      return;
    }
    if (comando.necesitaSeleccion && rango === null) return;
    if (comando.soloConIa && !hayIa) return;
    ejecutarComando(comando);
  }

  function irAProgresion(progresion: number): void {
    // Sin un Locator por progresión, se va al capítulo que la contiene: es la
    // barra de posición del teléfono, y su precisión es la del índice.
    if (indice.length === 0) return;
    const n = Math.min(indice.length - 1, Math.floor(progresion * indice.length));
    const destino = indice[n]?.locator;
    if (destino) void irA(destino);
  }

  export function ordenRemota(orden: RemoteOrder): void {
    switch (orden.tipo) {
      case 'comando':
        comandoRemoto(orden.id);
        return;
      case 'irAlLocator':
        void irA(orden.locator);
        return;
      case 'irA':
        irAProgresion(orden.progresion);
        return;
      case 'desplazar':
        if (!superficie?.desplazar(orden.dy) && Math.abs(orden.dy) > 0) {
          void ejecutar({ tipo: 'pagina', direccion: orden.dy > 0 ? 1 : -1 });
        }
        return;
      case 'avance':
        fijarAvance(orden.velocidad);
        return;
      case 'abrir':
        // Abrir es de la carcasa, que es quien monta el lector.
        return;
    }
  }

  // Dónde está el lector, para el teléfono que lo maneja. Solo la mitad que
  // recibe el teclado: con la vista dividida, el teléfono maneja esa.
  $effect(() => {
    if (!activo) return;
    const posicion = posicionDe(estado.locator, indice);
    void readerPublishState({
      edicion: sesion.editionId,
      titulo: sesion.title,
      locator: estado.locator,
      progresion: estado.locator.locations.totalProgression ?? null,
      capitulo: posicion.capitulo,
      capituloNumero: posicion.capituloNumero,
      capitulosTotales: posicion.capitulosTotales,
      paginado: Boolean(composicion?.paginado),
      avance: velocidadDeAvance,
      voz: narrador.estado.estado,
      fondoSonoro: fondoSonando,
      modo: composicion?.modo ?? 'dia',
      inmersivo: estado.inmersivo
    }).catch(() => {
      // Sin servidor encendido no hay a quién contárselo.
    });
  });

  /**
   * Cierra el panel abierto.
   *
   * Existe porque quien pinta los paneles es la carcasa y quien sabe cuál está
   * abierto es el lector, y el estado tiene que ser **uno**. Antes la carcasa
   * guardaba su propia copia y la cerraba por su cuenta: el panel desaparecía y
   * el botón de la barra seguía marcado como abierto, porque el lector no se
   * enteraba. Con esto el aviso va en un sentido —el lector dice qué panel hay—
   * y la orden en el otro.
   */
  export function cerrarPanel() {
    estado = { ...estado, panel: null };
  }

  /**
   * Abre un panel desde fuera.
   *
   * El otro sentido del mismo estado único que documenta `cerrarPanel`. Hace
   * falta para el gesto de señalar una marca en la página: el clic ocurre en la
   * superficie, el panel lo pinta la carcasa, y quién decide cuál está abierto
   * sigue siendo el lector y nadie más.
   */
  export function abrirPanel(cual: PanelDelLector) {
    estado = { ...estado, panel: cual };
  }

  /**
   * Dónde se está y qué hay seleccionado, para estudiar.
   *
   * La selección lleva su locator, que construye el runtime a partir de la cita
   * como en cualquier anotación: es a donde lleva la cita de lo que se genere
   * sobre ella.
   */
  export async function paraEstudiar(): Promise<{
    locator: Locator;
    seleccion: { locator: Locator; texto: string } | null;
  }> {
    const cita = citaSeleccionada(estado);
    if (!cita) return { locator: estado.locator, seleccion: null };
    try {
      const locator = await locatorForSelection(sesion.fileId, cita);
      return { locator: estado.locator, seleccion: { locator, texto: cita } };
    } catch {
      return { locator: estado.locator, seleccion: null };
    }
  }

  /**
   * Desde qué bloque empezaría a leer la voz.
   *
   * El cursor manda si lo hay, porque es una elección explícita de quien navega
   * con el teclado; si no, el primer bloque a la vista. Lo que no puede seguir
   * pasando es empezar por el principio del capítulo: pedir que te lean en la
   * página doce y volver a la uno no se parece a nada que haga un libro.
   */
  export function bloqueParaNarrar(): number {
    if (estado.cursor >= 0) return estado.cursor;
    return superficie?.bloqueParaNarrar() ?? 0;
  }

  function alPulsarTecla(evento: KeyboardEvent) {
    // La mitad que no manda no escucha. Con los dos lectores atendiendo a
    // `window`, sin esto una sola pulsación movería las dos a la vez.
    if (!activo) return;
    // Lo que ocurre dentro de un campo es del campo, salvo los dos atajos que
    // el registro marca como válidos ahí. Sin esto, escribir una nota
    // dispararía comandos con cada letra.
    if (despachar(evento)) evento.preventDefault();
  }
  /*
   * El modo de lectura, derivado igual que en la superficie: detrás de la barra
   * se pinta lo mismo que detrás del texto, con su brillo y su temperatura, pero
   * **ya calculado** y no con un filtro: ver `fondoFiltrado`.
   */
  const modoDelLibro = $derived(modoPorNombre(composicion?.modo ?? 'dia', modosDeExtension));
  const fondoDelLibro = $derived(fondoFiltrado(modoDelLibro));
</script>

<svelte:window onkeydown={alPulsarTecla} />

{#snippet control(comando: Comando)}
  {@const marca = estadoDelComando(comando.id)}
  <!--
    Un botón que abre un panel es una **divulgación**, así que lleva
    `aria-expanded` y no `aria-pressed`. La diferencia no es de vocabulario:
    `Panel` cierra el panel momentáneo al pulsar fuera y deja pasar los controles
    con `aria-expanded`, que son los que ya alternan por su cuenta. Con
    `aria-pressed` el mismo clic abría el panel y lo cerraba a continuación, así
    que ninguno llegaba a verse.

    Y los que no abren panel pero **son un modo puesto** —inmersión, vista
    dividida, trazo, pantalla completa— llevan `aria-pressed`: son una elección
    vigente, no algo desplegado. Antes ninguno decía nada.
  -->
  <IconButton
    etiqueta="{nombreDelComando(comando.id)} ({atajoLegible(comando.atajo, textos().teclas)})"
    expandido={comando.panel ? marca.abierto : undefined}
    activo={marca.esModo ? marca.elegido : undefined}
    funcionando={marca.funcionando}
    onclick={() => ejecutarComando(comando)}
  >
    <Icon nombre={comando.icono} />
  </IconButton>
{/snippet}

<section
  class="nb-lector"
  aria-label={texto('region')}
  style:--nb-libro-fondo-filtrado={fondoDelLibro}
>
  {#if !zonasVisibles(estado).banner}
    <!-- En inmersión no hay barra: el texto se queda solo. -->
  {:else}
    <!--
      Dos barras y no una que crece.

      La de siempre lleva lo que está siempre; la de lo seleccionado aparece **al
      lado** cuando hay algo elegido. Antes entraban en la misma, así que elegir
      una línea alargaba la barra y desplazaba todos los botones justo cuando la
      mano ya iba hacia uno. Se pidió el 2026-09-18.
    -->
    <div class="nb-barras">
      <div class="nb-controles" role="toolbar" aria-label={texto('controles')}>
        {#each familias as familia (familia.grupo)}
          <div class="nb-familia" data-grupo={familia.grupo}>
            {#each familia.comandos as comando (comando.id)}
              {@render control(comando)}
            {/each}
          </div>
        {/each}
      </div>

      {#if familiaDeSeleccion}
        <div
          class="nb-controles nb-controles-seleccion"
          role="toolbar"
          aria-label={texto('controlesDeLoElegido')}
        >
          {#each familiaDeSeleccion.comandos as comando (comando.id)}
            {@render control(comando)}
          {/each}
        </div>
      {/if}
    </div>
    {#if hayPosicion(posicion)}
      <!--
      Dónde estás. Es lo que los cuatro lectores de referencia ponen en una barra
      inferior y aquí no existía: se leía sin saber por qué capítulo se iba ni
      cuánto quedaba.

      **Va arriba, debajo de la barra, y no flotando sobre el texto.** Lo pidió
      el equipo el 2026-09-18: abajo se apoyaba sobre la última línea y tapaba justo
      lo que se estaba leyendo. Arriba ocupa su sitio en la columna —no se
      superpone a nada y no se mueve—, que es lo que se le pide a algo que se
      mira de reojo mientras se lee.

      La barra **no se arrastra**. Mover la lectura con un puntero sobre una
      línea de cuatro píxeles es un gesto de ratón sin equivalente de teclado, y
      además una progresión no siempre se puede invertir: el runtime resuelve una
      posición desde un locator, no desde un porcentaje. Para moverse están el
      índice, la búsqueda y las teclas de capítulo, que sí llevan a un sitio
      exacto.
    -->
      <div class="nb-posicion">
        {#if posicion.capitulo}
          <span class="nb-posicion-capitulo">{posicion.capitulo}</span>
        {/if}
        {#if posicion.capituloNumero !== null && posicion.capitulosTotales !== null}
          <span class="nb-posicion-dato">
            {texto('capitulo')}
            {posicion.capituloNumero}
            {texto('de')}
            {posicion.capitulosTotales}
          </span>
        {/if}

        <span class="nb-posicion-hueco"></span>

        {#if posicion.pagina !== null}
          <span class="nb-posicion-dato">
            {posicion.paginasTotales !== null
              ? `${texto('pagina')} ${posicion.pagina} ${texto('de')} ${posicion.paginasTotales}`
              : `${texto('pagina')} ${posicion.pagina}`}
          </span>
        {/if}
        {#if posicion.porcentaje !== null}
          <span
            class="nb-posicion-barra"
            role="progressbar"
            aria-label={texto('progreso')}
            aria-valuenow={posicion.porcentaje}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <span style:width="{posicion.porcentaje}%"></span>
          </span>
          <span class="nb-posicion-dato nb-posicion-cifra">{posicion.porcentaje} %</span>
          {#if loQueQueda}
            <span class="nb-posicion-dato" title={TEXTOS_DEL_LECTOR[idioma()].ritmoAyuda}>
              {loQueQueda}
            </span>
          {/if}
        {/if}
      </div>
    {/if}
  {/if}

  {#if sesion.approximate}
    <p class="nb-aviso" role="status">{texto('aproximado')}</p>
  {/if}

  {#if controladoDesde && activo}
    <p class="nb-aviso nb-controlado" role="status">
      {TEXTOS_DEL_LECTOR[idioma()].controladoDesde.replace('{nombre}', controladoDesde)}
      <Button variante="secundario" onclick={() => void mobileReleaseControl()}>
        {TEXTOS_DEL_LECTOR[idioma()].controladoCortar}
      </Button>
    </p>
  {/if}

  {#if omitido}
    <!--
      Lo que el capítulo trae y no se pinta se dice, igual que se dice que un
      escaneado no tiene texto. Quitarlo sin avisar dejaba un hueco que se
      leía como un defecto del libro.
    -->
    <p class="nb-aviso" role="status">{omitido}</p>
  {/if}

  {#if fallo}
    <p class="nb-fallo" role="alert">{fallo}</p>
  {/if}

  <ReadingSurface
    bind:this={superficie}
    {contenido}
    {tamano}
    {capa}
    cursor={estado.cursor}
    narrado={narrador.estado.bloque}
    sonando={sonandoEn ? desplazamientoDe(sonandoEn, capituloActual) : null}
    rango={estado.libre ? null : rango}
    zoom={estado.zoom ?? 1}
    {composicion}
    {modosDeExtension}
    {direccionPase}
    alAnimarPase={() => (direccionPase = null)}
    anotaciones={marcasDelCapitulo}
    puntos={puntosDelCapitulo}
    tinta={tintaDelCapituloActual}
    dibujando={estado.dibujando}
    alDibujar={(cita, tinta) => void guardarTrazo(cita, tinta)}
    {alSenalarMarca}
    {alColocarMarcas}
    alCambiarBloques={(bloques: Bloque[]) => (estado = { ...estado, bloques })}
    alDesplazarse={(cita) => void anotarDondeSeQuedo(cita)}
    alSeleccionarTexto={(desde, hasta, texto) =>
      (estado = seleccionarLibre(estado, desde, hasta, texto))}
    alTocar={(zona) => {
      // Las tres zonas hacen lo mismo que tres comandos que ya existen, y por
      // eso se despachan por su atajo en lugar de duplicar el efecto aquí: si
      // el comando cambia, la zona cambia con él y no se quedan discrepando.
      if (zona === 'izquierda') despachar(pulsacionDe('pagina-anterior'));
      else if (zona === 'derecha') despachar(pulsacionDe('pagina-siguiente'));
      else despachar(pulsacionDe('inmersion'));
    }}
  />

  {#if estado.inmersivo && recordando}
    <!--
      Lo único que flota sobre el texto en inmersión, y solo unos segundos.
      Con movimiento reducido aparece y desaparece sin transición, pero el
      recordatorio sigue estando: es información, no un adorno.
    -->
    <p class="nb-recordatorio" role="status">
      {#each recordatorio as comando (comando.id)}
        <span>
          <kbd>{atajoLegible(comando.atajo, textos().teclas)}</kbd>
          {nombreDelComando(comando.id)}
        </span>
      {/each}
    </p>
  {/if}

  {#if estado.redaccion}
    <!--
      La nota se escribe **aquí**, sin salir de la lectura y sin haber tenido que
      resaltar antes. Es el mandato de la investigación de la experiencia de
      lectura, 1.1: anotar tiene que costar menos que resaltar.
    -->
    <div class="nb-nota" bind:this={cajaDeNota}>
      <p class="nb-cita">{texto('citando')}: {citaSeleccionada(estado)}</p>
      <textarea
        bind:this={campoDeNota}
        class="nb-cuerpo"
        aria-label={texto('nota')}
        value={estado.redaccion.cuerpo}
        oninput={(e) => (estado = redactar(estado, e.currentTarget.value))}></textarea>
      <div class="nb-acciones">
        <Button variante="primario" onclick={() => despachar({ key: 'Enter', ctrlKey: true })}>
          {texto('guardar')}
        </Button>
        <Button onclick={() => despachar({ key: 'Escape' })}>{texto('cancelar')}</Button>
      </div>
    </div>
  {/if}

  <!--
    Región viva para anuncios de posición ("¿Dónde estoy?") accesible para
    lectores de pantalla.
  -->
  <p class="nb-anuncio" role="status" aria-live="polite" aria-atomic="true">
    {anuncioDondeEstoy}
  </p>
</section>

<style>
  /*
   * La sección entera lleva el fondo del modo de lectura, y no solo la superficie:
   * con la barra sobre el color de la aplicación, la página quedaba cortada por
   * una franja de otro color encima del texto.
   */
  /*
   * Detrás de la barra va el fondo del modo de lectura, y no el de la aplicación:
   * con la barra sobre otro color, la página quedaba cortada por una franja
   * encima del texto. Va en una capa aparte porque la superficie aplica al fondo
   * el brillo y la temperatura del modo con un filtro, y el filtro no se puede
   * poner en la sección sin alterar también la barra.
   *
   * **Sin `filter`.** Lo llevaba, y con él abrir cualquier panel del
   * lector costaba 90-110 ms en WebKitGTK contra 30: el panel se apoya encima y el
   * motor volvía a filtrar el fondo entero. Como es un color liso, el color filtrado
   * se calcula una vez en `fondoFiltrado`.
   */
  .nb-lector {
    position: relative;
    isolation: isolate;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .nb-lector::before {
    content: '';
    position: absolute;
    inset: 0;
    z-index: -1;
    background: var(--nb-libro-fondo-filtrado, var(--m3c-surface-container-low));
    pointer-events: none;
  }

  /*
   * Ahora la barra es una píldora flotante centrada, como en el diseño
   * aprobado: se apoya sobre la página sin partirla. Opaca y sin desenfoque, por
   * lo mismo que el resto del cromo: debajo pasa el texto.
   */
  /*
   * Las dos barras van en una fila centrada: la de siempre y, a su derecha, la
   * de lo seleccionado cuando la hay. La de siempre no cambia de tamaño al
   * aparecer la otra, que es todo el punto de haberlas separado.
   */
  .nb-barras {
    position: relative;
    z-index: 20;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: var(--nb-space-2);
    max-width: 100%;
  }

  .nb-controles {
    position: relative;
    z-index: 20;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    align-self: center;
    gap: var(--nb-space-1) var(--nb-space-1);
    max-width: calc(100% - 2 * var(--nb-space-3));
    margin: var(--nb-space-3) 0 var(--nb-space-1);
    padding: var(--nb-space-1) var(--nb-space-2);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-high);
    box-shadow: var(--m3-elevation-2);
  }

  /*
   * La de lo seleccionado se distingue de la de siempre: mismo tamaño y misma
   * forma, otro color de fondo. Sin eso, al aparecer parecía que la barra de
   * siempre se había partido en dos.
   */
  .nb-controles-seleccion {
    margin: var(--nb-space-3) 0 var(--nb-space-1);
    background: var(--m3c-secondary-container);
  }

  .nb-familia {
    display: flex;
    align-items: center;
    gap: var(--nb-space-1);
  }

  /*
   * La separación ahora usa un borde sutil para que case con el diseño de cristal.
   */
  .nb-familia + .nb-familia {
    margin-left: var(--nb-space-1);
    padding-left: var(--nb-space-2);
    border-left: 1px solid var(--m3c-outline-variant);
  }

  /* Salir se va al otro extremo: no es una herramienta más. */
  .nb-familia[data-grupo='salida'] {
    margin-left: var(--nb-space-1);
  }

  /*
   * Arriba y en la columna, no flotando sobre el texto.
   *
   * Estaba en `position: absolute` a 32 px del borde inferior, así que se
   * apoyaba sobre la última línea del párrafo y tapaba justo lo que se estaba
   * leyendo. Ahora ocupa su sitio debajo de la barra: no se superpone a nada y
   * **no se mueve**, que es lo que se le pide a algo que se mira de reojo.
   */
  .nb-posicion {
    position: relative;
    z-index: 20;
    align-self: center;
    margin-bottom: var(--nb-space-1);
    max-width: calc(100% - 2 * var(--nb-space-3));

    display: flex;
    align-items: center;
    gap: var(--nb-space-3);
    padding: var(--nb-space-2) var(--nb-space-4);

    /*
     * Opaca, y no translúcida como el resto del cristal: esta píldora flota
     * sobre el **texto del libro**, cuyo fondo lo elige el modo de lectura y no
     * el tema. Compuesta sobre él, su contraste medía 4,06:1 y la auditoría de
     * accesibilidad la rechazaba. Sobre una superficie del tema, el contraste
     * es el que `tema.test.ts` ya garantiza.
     */
    background: var(--m3c-surface-container-high);
    border-radius: var(--nb-radius-full);
    box-shadow: var(--m3-elevation-2);

    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);

    transition: opacity 0.2s cubic-bezier(0.2, 0, 0, 1);
  }

  /*
   * No se atenúa en reposo. Lo hacía al 30 %, y el único gesto que la devolvía
   * era el del ratón: quien navega con el teclado no la recuperaba nunca, y
   * mientras tanto leía un contraste por debajo del umbral. Para no competir con
   * el texto ya está su tamaño, su color apagado y estar fuera de la columna.
   */

  .nb-posicion-capitulo {
    min-width: 0;
    max-width: 32ch;
    overflow: hidden;
    color: var(--m3c-on-surface);
    white-space: nowrap;
    text-overflow: ellipsis;
    font-weight: 500;
  }

  .nb-posicion-dato {
    flex: 0 0 auto;
    color: var(--m3c-on-surface-variant);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .nb-posicion-hueco {
    /* Ya no usamos hueco de flex, porque la barra debe ajustarse a su contenido */
    display: none;
  }

  /*
   * El carril se pinta con el trazo y no con la superficie hundida: sobre el
   * plano elevado de la barra, `surface-sunken` es casi el mismo color, y una
   * barra al cero por ciento se veía como si no hubiera barra.
   */
  .nb-posicion-barra {
    flex: 0 1 12rem;
    height: 4px;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-outline-variant);
    overflow: hidden;
  }

  .nb-posicion-barra span {
    display: block;
    height: 100%;
    background: var(--m3c-primary);
  }

  .nb-posicion-cifra {
    min-width: 4ch;
    text-align: right;
  }

  .nb-aviso,
  .nb-fallo {
    margin: 0;
    padding: var(--nb-space-2) var(--nb-space-3);
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
    background: var(--m3c-surface-container-highest);
  }

  .nb-fallo {
    color: var(--m3c-error);
  }

  .nb-controlado {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-3);
  }

  .nb-recordatorio {
    position: absolute;
    left: 50%;
    bottom: 24px;
    transform: translateX(-50%);
    display: flex;
    gap: var(--nb-space-4);
    margin: 0;
    padding: var(--nb-space-2) var(--nb-space-4);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    box-shadow: var(--m3-elevation-2);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
    white-space: nowrap;
    animation: nb-aparecer 0.2s cubic-bezier(0.2, 0, 0, 1);
  }

  .nb-recordatorio span {
    display: flex;
    align-items: center;
    gap: var(--nb-space-1);
  }

  .nb-recordatorio kbd {
    padding: 0 var(--nb-space-1);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-xs);
    color: var(--m3c-on-surface-variant);
    font-family: monospace;
    font-size: var(--nb-text-xs);
  }

  @keyframes nb-aparecer {
    from {
      opacity: 0;
      transform: translate(-50%, 8px);
    }
    to {
      opacity: 1;
      transform: translate(-50%, 0);
    }
  }

  .nb-nota {
    position: absolute;
    bottom: 16px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 30;
    width: min(calc(100% - 16px * 2), 40rem);

    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
    padding: var(--nb-space-4);

    /*
     * Aquí el desenfoque sí se queda: es un flotante que aparece, se usa y se
     * va. Lo que no se puede pagar en cromo permanente se paga sin problema en
     * algo que dura lo que dura escribir una nota.
     */
    background: var(--m3c-surface-container-high);
    border-radius: var(--nb-radius-lg);
    box-shadow: var(--m3-elevation-3);

    animation: nb-nota-subir 0.2s cubic-bezier(0.2, 0, 0, 1);
  }

  @keyframes nb-nota-subir {
    from {
      opacity: 0;
      transform: translate(-50%, 10%);
    }
    to {
      opacity: 1;
      transform: translate(-50%, 0);
    }
  }

  .nb-cita {
    margin: 0;
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    padding-left: var(--nb-space-2);
    border-left: 2px solid var(--m3c-primary);
  }

  .nb-cuerpo {
    min-height: 5rem;
    padding: var(--nb-space-3);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface);
    font-family: var(--nb-font-ui);
    font-size: var(--nb-text-md);
    resize: vertical;
    transition: none;
  }

  .nb-cuerpo:focus-visible {
    border-color: var(--m3c-primary);
  }

  .nb-acciones {
    display: flex;
    justify-content: flex-end;
    gap: var(--nb-space-2);
  }

  /*
   * Fuera de la vista pero disponible para el lector de pantalla. No se usa
   * `display: none`, que lo dejaría también fuera del árbol de accesibilidad.
   */
  .nb-anuncio {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
