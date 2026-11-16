/**
 * Punto único de acceso al runtime de Rust.
 *
 * Ningún componente invoca el puente directamente: todo pasa por aquí, donde se
 * centraliza el tipado, la traducción de códigos de error y el registro. Un
 * lint del proyecto rechaza cualquier importación de `@tauri-apps/api` fuera de
 * este archivo. Ver `docs/architecture/ipc-contracts.md`.
 *
 * Los tipos vienen de `$lib/bindings`, generados desde Rust. No se escriben a
 * mano y la integración continua falla si quedan desactualizados.
 */

import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';

import type {
  AiModels,
  AiProbe,
  AiProviderStatus,
  AiReport,
  AiReportDraft,
  AiSend,
  Annotation,
  AnnotationCounts,
  AnnotationKind,
  AvailabilityReport,
  Board,
  BoardColumn,
  BoardLimits,
  BrowsePage,
  BrowseQuery,
  BulkReviewSummary,
  Catalog,
  CatalogDownload,
  CatalogPage,
  CitationFormat,
  CollectionSummary,
  ComponentStatus,
  Concept,
  ConceptKind,
  CredentialStatus,
  DiagnosticsPackage,
  DictionaryManifest,
  DomainEvent,
  EditionDetails,
  EnrichReport,
  ExtensionApi,
  ExtensionCommand,
  ExtensionInstall,
  ExtensionReadingMode,
  ExtensionRenderedView,
  ExtensionSoundscape,
  ExtensionStudyView,
  ExtensionTheme,
  ExtensionView,
  HealthReport,
  HomeRow,
  ImportDuplicate,
  ImportRejection,
  ImportReport,
  ImportRun,
  IndexingReport,
  Lectura,
  LibraryAnnotation,
  LibraryFacets,
  LibraryRootView,
  Locator,
  LookupOutcome,
  MetadataCandidate,
  MetadataSuggestion,
  NetworkReport,
  NewConcept,
  NewStudyCard,
  Note,
  OutlineEntry,
  PageSize,
  RecoveryReport,
  Rendered,
  Resolution,
  ReviewRating,
  Scope,
  Results as SearchResults,
  SearchStatus,
  SemanticHit,
  SessionInfo,
  Setting,
  SettingValue,
  ShelfPage,
  ShelfQuery,
  StudyAvailability,
  StudyCard,
  StudyCardCounts,
  StudyCardEdit,
  StudyCounts,
  StudyFunction,
  StudyGeneration,
  StudyScope,
  StudyScopeEstimate,
  TextLayer,
  VectorEstimate,
  VectorExclusionKind,
  VectorJob,
  VectorScope,
  VocabularyItem,
  VoiceStatus,
  Withdrawal
} from '$lib/bindings';

export type {
  AiInstallProgress,
  AiModel,
  AiModelStatus,
  AiModels,
  AiProbe,
  AiProviderStatus,
  AiSend,
  AnchoredAnnotation,
  AnchorReport,
  Annotation,
  AnnotationKind,
  AudioEstimate,
  AudioJob,
  AudioMark,
  AudioScope,
  Catalog,
  ComponentState,
  ComponentStatus,
  Consent,
  CredentialStatus,
  DiagnosticsPackage,
  DictionaryManifest,
  DomainEvent,
  EnrichReport,
  ExtensionApi,
  ExtensionCommand,
  ExtensionInstall,
  ExtensionPending,
  ExtensionReadingMode,
  ExtensionRenderedView,
  ExtensionStudyView,
  ExtensionTheme,
  ExtensionView,
  Feed,
  FeedEntry,
  HealthReport,
  HomeRow,
  ImportDuplicate,
  ImportRejection,
  ImportReport,
  ImportRun,
  IndexingReport,
  Lectura,
  LibraryAnnotation,
  Locator,
  LookupEntry,
  LookupKind,
  LookupOutcome,
  LookupSkipped,
  MetadataCandidate,
  MetadataSource,
  MetadataSuggestion,
  NativePosition,
  NetworkLogEntry,
  NetworkOutcome,
  NetworkReport,
  NeuralVoice,
  Note,
  OutlineEntry,
  PageSize,
  Precision,
  PreparedAudio,
  ReaderCapabilities,
  RecoveryReport,
  Rejected,
  Rendered,
  Resolution,
  Results as SearchResults,
  ReviewRating,
  Scope,
  SearchStatus,
  SessionInfo,
  Setting,
  SettingValue,
  SkipReason,
  StudyCardCounts,
  SuggestionStatus,
  SynchronizedAudio,
  SystemVoice,
  TextLayer,
  TextLayerStatus,
  TextSpan,
  VocabularyEntry,
  VocabularyItem,
  VoiceStatus
} from '$lib/bindings';

/**
 * Cuántas filas se piden de una lista cuando nadie dice otra cosa.
 *
 * La regla 1 del contrato de carga -sección 1.3 de `optimization-catalog.md`-
 * dice que ninguna consulta de la interfaz devuelve un número de filas que
 * dependa del tamaño de la biblioteca. El valor por defecto vive aquí y no en
 * cada vista porque una lista sin cota no se nota hasta que hay ciento cincuenta
 * mil documentos dentro, y entonces ya está en cinco sitios.
 *
 * El runtime tiene además su propio techo, que es el que manda: este número es
 * lo que la interfaz pide, no lo que el runtime consiente.
 */
export const POR_PAGINA = 50;

/**
 * Error de dominio recibido del runtime.
 *
 * Rust nunca envía mensajes legibles: envía un código canónico estable. El
 * mensaje que ve el usuario lo produce la capa de internacionalización a partir
 * de ese código.
 */
export class NbError extends Error {
  readonly code: string;

  constructor(code: string) {
    super(code);
    this.name = 'NbError';
    this.code = code;
  }
}

function toNbError(cause: unknown): NbError {
  if (typeof cause === 'object' && cause !== null && 'code' in cause) {
    const { code } = cause as { code: unknown };
    if (typeof code === 'string') {
      return new NbError(code);
    }
  }
  // Un fallo sin código canónico significa que algo se saltó el contrato. No se
  // propaga su contenido: podría incluir rutas del usuario o detalles internos.
  return new NbError('INTERNAL');
}

/**
 * Espera a que el runtime tenga su estado.
 *
 * La ventana se abre antes que la base, para que una migración larga no deje
 * minutos sin nada en pantalla. Mientras tanto ningún comando puede contestar, y
 * en lugar de recibir un error por llamada, toda llamada espera aquí. Se pregunta
 * con un comando que no toma estado. Sin el puente de Tauri -las pruebas
 * unitarias- no hay nada que esperar.
 */
let esperaDelRuntime: Promise<void> | null = null;
export function esperarRuntime(): Promise<void> {
  if (!(globalThis as Record<string, unknown>).__TAURI_INTERNALS__) return Promise.resolve();
  esperaDelRuntime ??= (async () => {
    for (;;) {
      try {
        if ((await runtimeListo()) !== false) return;
      } catch {
        // Un puente que no conoce el comando es uno que no necesita esperar.
        return;
      }
      await new Promise((resolver) => setTimeout(resolver, 150));
    }
  })();
  return esperaDelRuntime;
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  // La pregunta de si ya está listo no puede esperar a estar listo.
  if (command !== 'runtime_ready') await esperarRuntime();
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toNbError(cause);
  }
}

/** Si el runtime ya tiene su estado. Ver [`esperarRuntime`]. */
export function runtimeListo(): Promise<boolean> {
  return call<boolean>('runtime_ready');
}

/** Comprueba que el runtime responde y devuelve su versión y esquema. */
export function healthCheck(): Promise<HealthReport> {
  return call<HealthReport>('health_check');
}

/**
 * Estado de salud de los componentes del runtime.
 *
 * La interfaz lo usa para decidir qué funciones ofrece: una función cuyo
 * componente no está disponible se oculta, no se muestra rota.
 */
export function componentStates(): Promise<ComponentStatus[]> {
  return call<ComponentStatus[]>('component_states');
}

/**
 * Todas las preferencias con su valor efectivo.
 *
 * Se invoca al montar la interfaz: la caché de arranque ya pintó, y esto la
 * corrige si difiere. Ver `docs/adr/010-settings-persistence.md`.
 */
export function settingsSnapshot(): Promise<Setting[]> {
  return call<Setting[]>('settings_snapshot');
}

/**
 * Escribe una preferencia y devuelve cómo quedó almacenada.
 *
 * Devuelve el valor guardado, no un acuse: ante discrepancia gana SQLite, así
 * que quien llama actualiza su estado con la respuesta y no con lo que creía
 * estar escribiendo.
 */
export function updateSetting(key: string, value: SettingValue): Promise<Setting> {
  return call<Setting>('update_setting', { key, value });
}

/**
 * Abre un documento para leerlo.
 *
 * Recibe el identificador del **archivo**, no una ruta: el webview no elige qué
 * se abre del disco, elige qué fila del catálogo se abre. La ruta la pone el
 * runtime desde su propia base de datos, y así no hay ruta que canonicalizar ni
 * ámbito que comprobar en esta frontera.
 *
 * Lo que devuelve en `capabilities` es lo que la interfaz pinta: la lista de
 * controles se deriva de ahí con `controlesDelLector`, nunca del tipo de medio.
 *
 * Abrir dos veces el mismo documento devuelve la misma sesión, con la posición
 * donde estaba.
 */
export function openDocument(fileId: string): Promise<SessionInfo> {
  return call<SessionInfo>('open_document', { fileId });
}

/**
 * Cierra una sesión de lectura.
 *
 * Se puede invocar al desmontar un componente sin saber si llegó a abrirse:
 * cerrar lo que no está abierto no es un error.
 */
export function closeDocument(fileId: string): Promise<void> {
  return call<void>('close_document', { fileId });
}

/**
 * Apunta dónde está leyendo el usuario ahora.
 *
 * Se puede llamar tan a menudo como haga falta: el runtime agrupa las
 * escrituras. Lo que no es opcional es `closeDocument` al terminar, porque es lo
 * que vacía lo que quedó apuntado y sin escribir.
 *
 * Devuelve con qué precisión se resolvió: si es `approximate`, la interfaz avisa
 * en lugar de colocar al usuario en un sitio parecido sin decírselo.
 */
export function saveProgress(fileId: string, locator: Locator): Promise<Resolution> {
  return call<Resolution>('save_progress', { fileId, locator });
}

/**
 * Pide que se mire si algo cambió en las carpetas referenciadas.
 *
 * Devuelve enseguida: lo que se encuentre llega por el bus, como cualquier otra
 * importación. Pedirlo muchas veces no cuesta: el runtime no repasa si lo hizo
 * hace un momento o si hay una importación en marcha.
 */
export function rescanRoots(): Promise<void> {
  return call<void>('rescan_roots');
}

/**
 * Escribe ya la última posición apuntada, **sin cerrar** el documento.
 *
 * Se llama al dejar de ver el lector con el libro abierto: el runtime agrupa las
 * escrituras, y sin esto la biblioteca enseñaría la barra de hace unos segundos
 * mientras el lector enseña la de ahora.
 */
export function flushProgress(fileId: string): Promise<void> {
  return call<void>('flush_progress', { fileId });
}

/**
 * Pinta una posición del documento abierto.
 *
 * Sin locator, la posición actual. **Pedir que se pinte no mueve la lectura**:
 * la superficie pinta la página siguiente antes de que el usuario llegue, y si
 * eso adelantara el progreso, al volver aparecería más allá de donde se quedó.
 *
 * Devuelve marcado **ya saneado en Rust** o la ruta de una imagen. La ruta se
 * convierte con `rutaDeImagen` antes de ponerla en un `src`.
 */
export function renderDocument(fileId: string, locator?: Locator): Promise<Rendered> {
  return call<Rendered>('render_document', { fileId, locator: locator ?? null });
}

/**
 * Tamaño intrínseco de la página que se pinta en una posición.
 *
 * Se pide **antes** de tener la imagen, para reservar el hueco: con la imagen ya
 * puesta, el salto de maquetación ya ha ocurrido. Un formato que refluye
 * responde `FORMAT_UNSUPPORTED`, porque su tamaño lo pone la ventana.
 */
export function pageSize(fileId: string, locator?: Locator): Promise<PageSize> {
  return call<PageSize>('page_size', { fileId, locator: locator ?? null });
}

/**
 * Capa de texto de la posición, para superponerla sobre la imagen.
 *
 * Es lo que hace que un PDF no sea una imagen opaca para un lector de pantalla.
 * Una página sin texto llega **declarada como ausente**, que es distinto de una
 * lista vacía: la lista vacía sería indistinguible de una página en blanco.
 */
export function textLayer(fileId: string, locator?: Locator): Promise<TextLayer> {
  return call<TextLayer>('text_layer', { fileId, locator: locator ?? null });
}

/**
 * Ancla una cita que el usuario acaba de seleccionar.
 *
 * La cita **se busca en el documento**: este lado no puede fabricar un ancla,
 * porque una anotación con una cita que no está en el libro no se podría
 * reanclar nunca. Si la cita no aparece, no hay anotación.
 */
export function locatorForSelection(fileId: string, quote: string): Promise<Locator> {
  return call<Locator>('locator_for_selection', { fileId, quote });
}

/**
 * Dirección con la que el webview puede pedir una imagen ya rendida.
 *
 * El protocolo de assets está acotado en `tauri.conf.json` a los directorios de
 * caché de la aplicación: no hay forma de pedir por aquí un archivo de la
 * biblioteca del usuario.
 */
export function rutaDeImagen(ruta: string): string {
  return convertFileSrc(ruta);
}

/**
 * Pide al sistema la pantalla completa de verdad.
 *
 * Es el segundo nivel de la lectura inmersiva: el primero retira la carcasa
 * dentro de la ventana y este pide la ventana entera. **La concede el sistema
 * operativo, no nosotros**; si una plataforma no la concede, se declara y no se
 * simula ocultando el escritorio (`ui-components.md`, sección 3).
 */
export async function setFullscreen(activa: boolean): Promise<void> {
  // La API de ventanas de Tauri se carga aquí y no al arrancar: es la mayor de las
  // que usa la aplicación, y esta es la única llamada que le hace.
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().setFullscreen(activa);
}

/**
 * Crea una anotación anclada a un Locator.
 */
export function createAnnotation(
  editionId: string,
  kind: AnnotationKind,
  body: string | null,
  target: Locator,
  /**
   * Geometría, solo para el trazo libre.
   *
   * Va como objeto JSON serializado y no como estructura tipada porque su forma
   * es de la interfaz: el runtime la guarda y la devuelve sin mirarla, igual que
   * hace la base. Lo único que comprueba es que un trazo la traiga, porque un
   * trazo sin geometría no se puede pintar.
   */
  payload: string | null = null
): Promise<Annotation> {
  return call<Annotation>('create_annotation', { editionId, kind, body, target, payload });
}

/**
 * Edita una anotación.
 *
 * Devuelve una con **identificador nuevo**: una anotación no se edita, se da de
 * baja y se crea su sucesora. Quien llame tiene que quedarse con el nuevo, no
 * con el que envió, porque el anterior sigue existiendo y ya no es el vigente.
 *
 * Omitir `body` o `target` conserva el de la versión anterior.
 */
export function editAnnotation(
  id: string,
  cambio: { body?: string | null; target?: Locator }
): Promise<Annotation> {
  return call<Annotation>('edit_annotation', {
    id,
    body: 'body' in cambio ? cambio.body : undefined,
    target: cambio.target
  });
}

/**
 * Qué puede hacer la voz **del runtime** en esta máquina.
 *
 * Existe porque el webview de Linux no tiene voz: WebKitGTK declara la
 * característica activada y `window.speechSynthesis` llega indefinido de todos
 * modos. Se pregunta al webview primero y a esto después.
 */
export function voiceStatus(): Promise<VoiceStatus> {
  return call<VoiceStatus>('voice_status');
}

/**
 * Manda decir un tramo. **Vuelve enseguida.**
 *
 * El fin llega por el bus como `VoiceFinished`: hablar tarda lo que dura el
 * párrafo, y esperar aquí dejaría la llamada abierta minutos.
 */
export function voiceSpeak(
  block: number,
  text: string,
  rate: number,
  voice: string | null
): Promise<void> {
  return call<void>('voice_speak', { block, text, rate, voice });
}

/** Corta lo que la voz esté diciendo. */
export function voiceStop(): Promise<void> {
  return call<void>('voice_stop');
}

/**
 * Sintetiza un tramo en el servicio remoto y devuelve la ruta del audio.
 *
 * Una ruta y no el audio: son cientos de kilobytes por párrafo, y el webview ya
 * sabe leerlos de la caché por el protocolo de assets. Lo que cruza el puente es
 * el texto —que es justo lo que el usuario autorizó a enviar— y un nombre de
 * archivo.
 */
export function synthesizeSpeech(text: string, rate: number): Promise<string> {
  return call<string>('synthesize_speech', { text, rate });
}

/**
 * Sintetiza un tramo con la voz neuronal local y devuelve la ruta del audio.
 *
 * Una ruta y no el audio, por lo mismo que la remota. Aquí no sale nada del
 * dispositivo: lo dice el nombre del comando y lo sostiene que no pasa por
 * `net.rs`.
 */
export function synthesizeNeuralSpeech(
  text: string,
  voice: string | null,
  rate: number
): Promise<string> {
  return call<string>('synthesize_neural_speech', { text, voice, rate });
}

/**
 * La dirección exacta a la que se mandaría el texto, como la entiende el runtime.
 *
 * Nula si no hay servidor escrito. Ajustes la enseña debajo del campo: el
 * servidor se puede escribir de muchas formas, y lo que importa es la que resulta.
 */
export function speechEndpoint(): Promise<string | null> {
  return call<string | null>('speech_endpoint');
}

/**
 * Autoriza al servicio de síntesis **que está en las preferencias** a recibir el
 * texto del libro.
 *
 * No lleva anfitrión: lo pone el runtime desde lo que el usuario escribió. Un
 * comando que lo aceptara convertiría al webview en el sitio donde se decide a
 * dónde sale el texto de los libros.
 */
export function allowSpeechHost(): Promise<void> {
  return call<void>('allow_speech_host');
}

/**
 * Si cabe guardar una clave en este sistema, y si el servidor ya tiene una.
 *
 * **Nunca devuelve la clave.** Dos booleanos bastan para pintar los dos estados
 * que la interfaz necesita, y una credencial que llega al webview está a una
 * extensión de distancia de salir.
 */
export function speechCredentialStatus(): Promise<CredentialStatus> {
  return call<CredentialStatus>('speech_credential_status');
}

/**
 * Guarda en el llavero del sistema la clave del servidor configurado.
 *
 * No lleva anfitrión, por lo mismo que `allowSpeechHost`: lo pone el runtime.
 * Si el webview pudiera decir de quién es la clave, podría guardarla a nombre de
 * otro servicio y conseguir que se la mandaran a él.
 */
export function setSpeechCredential(secret: string): Promise<void> {
  return call<void>('set_speech_credential', { secret });
}

/** Borra del llavero la clave del servidor configurado. */
export function clearSpeechCredential(): Promise<void> {
  return call<void>('clear_speech_credential');
}

/**
 * Qué pasó al abrir la biblioteca, cuando hubo que recuperarla.
 *
 * `null` en un arranque normal. Cuando trae algo, la interfaz **tiene que
 * decirlo**: la biblioteca está vacía y el usuario no lo pidió, y una biblioteca
 * vacía sin explicación es una pérdida silenciosa.
 */
export function recoveryReport(): Promise<RecoveryReport | null> {
  return call<RecoveryReport | null>('recovery_report');
}

/**
 * Paquete de diagnóstico, **para verlo antes de guardarlo**.
 *
 * Devuelve el paquete por partes y no el texto: quien lo va a enviar tiene
 * derecho a leer antes lo que envía, y un bloque de JSON no se lee. Consentir
 * sin ver no es consentir.
 */
export function diagnosticsPreview(): Promise<DiagnosticsPackage> {
  return call<DiagnosticsPackage>('diagnostics_preview');
}

/**
 * Guarda el paquete de diagnóstico donde el usuario elija.
 *
 * El destino lo elige el **diálogo del sistema** y no nosotros: es la misma
 * regla que al importar, y la aplicación no propone rutas por su cuenta ni
 * escribe donde el usuario no ha dicho.
 *
 * Quien escribe es el runtime, no el webview. Podría haberse hecho con el
 * complemento de sistema de archivos de Tauri, pero eso es una dependencia nueva
 * y, sobre todo, una capacidad de escribir cualquier archivo desde el webview:
 * un comando que solo sabe escribir el diagnóstico es una superficie mucho más
 * pequeña por exactamente el mismo resultado.
 *
 * Devuelve `false` si el usuario cancela, que no es un error.
 */
export async function guardarDiagnostico(
  nombrePorDefecto: string,
  titulo: string
): Promise<boolean> {
  const destino = await save({ defaultPath: nombrePorDefecto, title: titulo });
  if (destino === null) return false;
  await call<void>('save_diagnostics', { path: destino });
  return true;
}

/**
 * Guarda las anotaciones de una edición donde el usuario diga.
 *
 * El comando devuelve el JSON y no escribe en disco, que es lo que decidió
 * elegir destino es del diálogo nativo. Quien lee, compone y escribe es
 * el runtime; traer el JSON aquí para devolverlo movería la biblioteca de
 * anotaciones por el canal sin motivo, y el webview no escribe en disco.
 *
 * Devuelve `false` si se cancela, que no es un error.
 */
export async function guardarAnotaciones(
  editionId: string,
  nombrePorDefecto: string,
  titulo: string,
  format?: 'markdown' | 'text',
  language?: string
): Promise<boolean> {
  const destino = await save({ defaultPath: nombrePorDefecto, title: titulo });
  if (destino === null) return false;
  await call<void>('save_annotations', { editionId, path: destino, format, language });
  return true;
}

/**
 * Exporta la biblioteca entera donde el usuario diga.
 *
 * Como el diagnóstico y las anotaciones: el diálogo nativo elige el destino, y
 * quien lee y escribe es el runtime. El archivo es el formato de
 * `portability.rs`, legible sin NB Books.
 *
 * Devuelve cuántos documentos salieron, o `null` si se cancela, que no es un
 * error.
 */
export async function guardarBiblioteca(
  nombrePorDefecto: string,
  titulo: string
): Promise<number | null> {
  const destino = await save({ defaultPath: nombrePorDefecto, title: titulo });
  if (destino === null) return null;
  return call<number>('save_library_export', { path: destino });
}

/**
 * Cita una anotación desde una nota.
 *
 * Una nota cita a otra escribiendo `[[Título]]`, porque el título es lo que una
 * persona escribe y recuerda. Una anotación no tiene nombre: es un trozo de un
 * libro, y lo único que la identifica es dónde está. Así que se cita
 * señalándola, y el enlace vive en su propia tabla en lugar de derivarse del
 * cuerpo. Los dos referentes son distintos y por eso los dos gestos lo son.
 */
export function citeAnnotation(noteId: string, annotationId: string): Promise<void> {
  return call<void>('cite_annotation', { noteId, annotationId });
}

/** Deshace la cita. Que no existiera no es un error. */
export function unciteAnnotation(noteId: string, annotationId: string): Promise<void> {
  return call<void>('uncite_annotation', { noteId, annotationId });
}

/**
 * Los pasajes que una nota cita, con el libro del que salió cada uno.
 *
 * Vienen con su contexto y no como identificadores porque una cita que solo dice
 * un identificador no se puede leer: quien mira una nota necesita ver de qué
 * pasaje habla.
 */
export function citedAnnotations(noteId: string): Promise<LibraryAnnotation[]> {
  return call<LibraryAnnotation[]>('cited_annotations', { noteId });
}

/**
 * Notas que citan una anotación.
 *
 * Es el retroenlace visto desde el documento, y como todos los de este proyecto
 * **no se guarda: se consulta**.
 */
export function notesCiting(annotationId: string): Promise<Note[]> {
  return call<Note[]>('notes_citing', { annotationId });
}

/**
 * Anotaciones de **toda** la biblioteca, de la más reciente a la más antigua.
 *
 * Es el otro alcance del mismo dato: `annotationOutline` da las del documento
 * abierto en orden de lectura, y esto da las de todos por fecha. La frontera es
 * la misma que separa la búsqueda de catálogo de la de corpus.
 *
 * **No dice dónde ha quedado cada una**, porque saberlo exigiría abrir todos los
 * documentos. `fileId` nulo significa que ninguna copia está disponible, y
 * entonces no hay salto que ofrecer.
 */
export function libraryAnnotations(
  kind?: AnnotationKind,
  limit: number = POR_PAGINA,
  offset = 0
): Promise<LibraryAnnotation[]> {
  return call<LibraryAnnotation[]>('library_annotations', { kind: kind ?? null, limit, offset });
}

/**
 * Todas las versiones de una anotación, de la más antigua a la más reciente.
 */
export function annotationHistory(id: string): Promise<Annotation[]> {
  return call<Annotation[]>('annotation_history', { id });
}

/**
 * Da de baja una anotación. La fila se conserva.
 */
export function deleteAnnotation(id: string): Promise<void> {
  return call<void>('delete_annotation', { id });
}

/**
 * Panel de anotaciones del documento abierto, en orden de lectura.
 *
 * Las que no se pudieron colocar van al final, con `lost` en cierto, en la misma
 * lista: esconderlas en otra pestaña las convertiría en algo que el usuario
 * descubre por casualidad.
 */
export function annotationOutline(fileId: string): Promise<OutlineEntry[]> {
  return call<OutlineEntry[]>('annotation_outline', { fileId });
}

/**
 * Pone un marcador donde está la lectura ahora.
 *
 * La posición la pone el runtime: un marcador tiene que caer donde está el
 * usuario, no donde el webview crea que está.
 */
export function createBookmark(fileId: string): Promise<Annotation> {
  return call<Annotation>('create_bookmark', { fileId });
}

/**
 * Avance de la indexación de texto completo.
 *
 * `pauseReason` es un código canónico, no un mensaje: el texto lo pone la
 * interfaz. Una barra detenida sin explicación se lee como que la aplicación se
 * ha colgado.
 */
export function indexingProgress(): Promise<IndexingReport> {
  return call<IndexingReport>('indexing_progress');
}

/**
 * Detiene o reanuda la indexación.
 *
 * Es la pausa **del usuario**, distinta de la que el runtime decide solo por
 * batería baja: reanudar a mano no hace que se ignore una batería al 8 %.
 */
export function setIndexingPaused(paused: boolean): Promise<void> {
  return call<void>('set_indexing_paused', { paused });
}

/**
 * Si un documento se puede buscar, y por qué no.
 *
 * No devuelve un booleano: "todavía no se ha indexado" y "no tiene texto" son
 * cosas distintas, y presentarlas igual haría que la interfaz dijera "no"
 * cuando la respuesta correcta es "aún no".
 */
export function searchStatus(editionId: string): Promise<SearchStatus> {
  return call<SearchStatus>('search_status', { editionId });
}

/**
 * Busca en el alcance que se le diga.
 *
 * Los tres alcances devuelven cosas distintas a propósito: el catálogo devuelve
 * **documentos** y los otros dos, **párrafos**. Por eso el resultado viene
 * etiquetado en lugar de ser una lista a secas.
 *
 * El acotado por colección va en el alcance y lo resuelve el runtime. Filtrar
 * aquí obligaría a traerse la colección entera por el puente en cada pulsación.
 */
export function search(
  query: string,
  scope: Scope,
  limit = 20,
  offset = 0
): Promise<SearchResults> {
  return call<SearchResults>('search', { query, scope, limit, offset });
}

/**
 * Rehace el índice de búsqueda desde cero.
 *
 * Devuelve cuántas ediciones quedan encoladas. El trabajo ocurre en segundo
 * plano y este comando no lo espera: sobre una biblioteca grande son horas. El
 * avance llega por `IndexingProgress`.
 *
 * No hay nada que perder al hacerlo: el índice es caché y los documentos siguen
 * donde estaban.
 */
export function reindex(): Promise<number> {
  return call<number>('reindex');
}

/**
 * Importa un archivo o una carpeta entera. **Una carpeta no espera a terminar.**
 *
 * La biblioteca de referencia tiene 152.080 obras y recorrerla son horas: el
 * comando devuelve en cuanto arranca, con `running` en cierto, y el avance llega
 * por el evento `BulkImportProgress`. Un archivo suelto sí se importa en el
 * acto, porque uno solo no tarda.
 *
 * La ruta **solo puede venir de un gesto del usuario**: soltarla sobre la
 * ventana o escribirla. El runtime la canonicaliza y la acota antes de tocar
 * nada, porque este lado no es de fiar.
 *
 * `copy` decide el modo de almacenamiento: copiar el archivo a la biblioteca
 * administrada, o dejarlo donde está y referenciarlo. Referenciar no duplica
 * nada, pero desconectar el volumen deja el documento marcado como no
 * disponible, con sus anotaciones intactas.
 *
 * Reanudar es volver a llamar: la identidad por contenido hace que pasar otra
 * vez por un archivo ya importado sea barato y sin efecto.
 */
export function importPath(path: string, copy = false): Promise<ImportReport> {
  return call<ImportReport>('import_path', { path, copy });
}

/**
 * Abre el selector de archivos del sistema y devuelve lo elegido.
 *
 * **Es la vía por la que una ruta entra al proyecto con consentimiento por
 * construcción**: la devuelve el sistema operativo después de que el usuario
 * señale una carpeta o unos archivos en su propio escritorio, así que no es una
 * ruta que este lado se invente. El runtime la canonicaliza igual, porque el
 * webview sigue sin ser de fiar.
 *
 * Devuelve `null` si el usuario cancela, que no es un error: cambiar de opinión
 * es una respuesta.
 */
export async function elegirParaImportar(carpeta: boolean): Promise<string[] | null> {
  const elegido = await open({
    directory: carpeta,
    multiple: !carpeta,
    title: carpeta ? 'Elegir una carpeta' : 'Elegir documentos'
  });

  if (elegido === null) return null;
  return Array.isArray(elegido) ? elegido : [elegido];
}

/**
 * Cancela la importación en curso.
 *
 * Cancelar lo que no está corriendo no es un error: quien pulsa puede hacerlo
 * justo cuando acaba de terminar. **Nunca corta a mitad de un archivo**, así que
 * no deja copias parciales.
 */
export function cancelImport(): Promise<void> {
  return call<void>('cancel_import');
}

/**
 * La importación que quedó a medias, si la hay.
 *
 * Reanudar es gratis por la identidad de contenido —lo ya importado se reconoce
 * y se salta—, pero solo servía si el usuario **se acordaba** de
 * volver a lanzarla sobre la misma carpeta, y nadie recuerda una carpeta que
 * eligió hace tres días. Una ejecución sin cerrar es la señal, y funciona aunque
 * el proceso se caiga: nadie tiene que escribir nada al morir.
 */
export function pendingImport(): Promise<ImportRun | null> {
  return call<ImportRun | null>('pending_import');
}

/** La última importación, terminada o no, para poder enseñar su informe. */
export function lastImport(): Promise<ImportRun | null> {
  return call<ImportRun | null>('last_import');
}

/**
 * Lo que una importación dejó fuera, y por qué.
 *
 * Puede ser menos de lo que dice su recuento: por encima del tope se dejan de
 * guardar filas y la cifra sigue siendo la de verdad.
 */
export function importRejections(
  runId: string,
  limit: number = POR_PAGINA,
  offset = 0
): Promise<ImportRejection[]> {
  return call<ImportRejection[]>('import_rejections', { runId, limit, offset });
}

/**
 * Las copias exactas que encontró una importación, con el título y la ruta de lo
 * que ya estaba. Como los rechazos, puede ser menos que su recuento.
 */
export function importDuplicates(
  runId: string,
  limit: number = POR_PAGINA,
  offset = 0
): Promise<ImportDuplicate[]> {
  return call<ImportDuplicate[]>('import_duplicates', { runId, limit, offset });
}

/**
 * Da por zanjada una importación a medias sin continuarla.
 *
 * Sin esto se ofrecería en cada arranque, que es el aviso que la gente aprende a
 * ignorar.
 */
export function discardPendingImport(runId: string): Promise<void> {
  return call<void>('discard_pending_import', { runId });
}

/**
 * Lo que Inicio muestra sobre la lectura: minutos, racha y cuántos hay en curso.
 *
 * Sale de datos que ya existen. La racha es un dato, no una obligación: ADR-013
 * deja fuera la culpa administrada por el software.
 */
export function readingStats(): Promise<Lectura> {
  return call<Lectura>('reading_stats');
}

/**
 * Las filas de Inicio para una visita.
 *
 * Locales, explicables y reproducibles (enmienda de ADR-013, 2026-09-12): cada
 * fila trae **por qué aparece** en su clase, y la misma `seed` da la misma
 * pantalla. La interfaz pone una semilla nueva en cada visita, que es lo que
 * hace que Inicio cambie sin que el runtime tenga que recordar nada.
 */
export function homeRows(seed: number): Promise<HomeRow[]> {
  return call<HomeRow[]>('home_rows', { seed });
}

/** Notas propias del usuario, de la más reciente a la más antigua. */
export function listNotes(limit: number = POR_PAGINA, offset = 0): Promise<Note[]> {
  return call<Note[]>('list_notes', { limit, offset });
}

/** Una nota por su identificador, o nulo si ya no existe. La abre una tarjeta de un tablero. */
export function findNote(id: string): Promise<Note | null> {
  return call<Note | null>('find_note', { id });
}

/** Crea una nota propia. El título es único: es como la enlazan las demás. */
export function createNote(title: string, body: string): Promise<Note> {
  return call<Note>('create_note', { title, body });
}

/**
 * Edita una nota propia.
 *
 * Una nota **sí se edita**, a diferencia de una anotación: es del usuario y no
 * ancla a nada, así que no hay ancla que romper.
 */
export function updateNote(id: string, title: string, body: string): Promise<Note> {
  return call<Note>('update_note', { id, title, body });
}

/** Borra una nota propia y sus enlaces. */
export function deleteNote(id: string): Promise<void> {
  return call<void>('delete_note', { id });
}

/**
 * Notas que apuntan a esta.
 *
 * El retroenlace no se guarda: es el mismo enlace leído del otro lado, y por eso
 * no puede desincronizarse.
 */
export function noteBacklinks(id: string): Promise<Note[]> {
  return call<Note[]>('note_backlinks', { id });
}

/**
 * Pide una ventana de la biblioteca, ya ordenada y filtrada.
 *
 * **Ordenar y filtrar los hace el runtime, no esta capa.** La biblioteca de
 * referencia tiene 152.080 obras y el presupuesto de nodos del documento es de
 * 1.500: traerla entera para ordenarla aquí no cabe. Se pide la ventana que se
 * va a pintar y nada más.
 */
export function browseLibrary(query: BrowseQuery): Promise<BrowsePage> {
  return call<BrowsePage>('browse_library', { query });
}

/**
 * Cuántas obras hay detrás de cada filtro fijo de la biblioteca.
 *
 * Es lo que decide qué fichas se ofrecen: un filtro sin obras no se pinta.
 */
export function libraryFacets(): Promise<LibraryFacets> {
  return call<LibraryFacets>('library_facets');
}

/**
 * Estanterías de una clase: autores, series o materias, con su tamaño.
 *
 * **Se busca, no se lista.** Con 152.080 obras hay decenas de miles de autores,
 * y el filtrado va en SQL por lo mismo que el de la biblioteca: traerlos para
 * elegir uno no cabe en el presupuesto de nodos. Sin texto se devuelven las más
 * numerosas, que es lo útil de partida.
 */
export function libraryShelves(query: ShelfQuery): Promise<ShelfPage> {
  return call<ShelfPage>('library_shelves', { query });
}

/**
 * Consulta la selección en los proveedores instalados.
 *
 * # Por qué viaja también dónde se estaba leyendo
 *
 * Porque consultar una palabra deja constancia: el runtime la apunta en el
 * vocabulario con el punto exacto donde apareció. Sin el locator, la
 * lista de Estudio tendría palabras sin sitio al que volver, que es justo lo que
 * su criterio de aceptación prohíbe.
 *
 * **Ninguna de las dos cosas mueve la posición de lectura.** El runtime no
 * escribe progreso aquí; el locator entra como dato y sale apuntado.
 *
 * Lanza `COMPONENT_UNAVAILABLE` cuando esta instalación no tiene ningún
 * proveedor, que es cuando la interfaz oculta la función. Que ninguno cubra el
 * idioma **no** es eso: es una respuesta vacía con su motivo dentro.
 */
export function lookupSelection(
  term: string,
  context: string | null,
  language: string | null,
  editionId: string | null,
  locator: Locator | null
): Promise<LookupOutcome> {
  return call<LookupOutcome>('lookup_selection', {
    term,
    context,
    language,
    editionId,
    locator
  });
}

/** Diccionarios instalados en esta máquina. */
export function listDictionaries(): Promise<DictionaryManifest[]> {
  return call<DictionaryManifest[]>('list_dictionaries');
}

/**
 * Abre el selector del sistema para elegir el `.ifo` de un diccionario.
 *
 * Misma vía que la importación: la ruta la devuelve el sistema tras un gesto del
 * usuario, y el runtime la canonicaliza igual porque el webview no es de fiar.
 * Devuelve `null` si se cancela, que no es un error.
 */
export async function elegirDiccionario(): Promise<string | null> {
  const elegido = await open({
    directory: false,
    multiple: false,
    title: 'Elegir un diccionario',
    filters: [{ name: 'Diccionario StarDict', extensions: ['ifo'] }]
  });
  if (elegido === null) return null;
  return Array.isArray(elegido) ? (elegido[0] ?? null) : elegido;
}

/**
 * Pide la carpeta a la que se mudó una raíz de biblioteca.
 *
 * Misma vía que la importación: la ruta la devuelve el sistema tras un gesto del
 * usuario, y el runtime comprueba que sea una carpeta antes de escribirla.
 * Devuelve `null` si se cancela.
 */
export async function elegirCarpetaDeRaiz(): Promise<string | null> {
  const elegido = await open({
    directory: true,
    multiple: false,
    title: 'Elegir la carpeta donde están ahora los libros'
  });
  if (elegido === null) return null;
  return Array.isArray(elegido) ? (elegido[0] ?? null) : elegido;
}

/** Instala un diccionario a partir de su archivo `.ifo`. */
export function installDictionary(
  path: string,
  language: string | null
): Promise<DictionaryManifest> {
  return call<DictionaryManifest>('install_dictionary', { path, language });
}

/** Desinstala un diccionario. Que no estuviera no es un error. */
export function uninstallDictionary(id: string): Promise<void> {
  return call<void>('uninstall_dictionary', { id });
}

/** El vocabulario consultado, de lo más reciente a lo más antiguo. */
export function listVocabulary(limit: number = POR_PAGINA, offset = 0): Promise<VocabularyItem[]> {
  return call<VocabularyItem[]>('list_vocabulary', { limit, offset });
}

/** Borra una palabra del vocabulario. */
export function deleteVocabulary(id: string): Promise<void> {
  return call<void>('delete_vocabulary', { id });
}

/** Borra el vocabulario entero. Devuelve cuántas palabras se borraron. */
export function clearVocabulary(): Promise<number> {
  return call<number>('clear_vocabulary');
}

/**
 * Las raíces de la biblioteca, con cuántos archivos tiene cada una y cuántos
 * faltan.
 *
 * Una raíz referenciada sin ruta es una raíz **no localizada**: el estado en que
 * quedan al restaurar en otro equipo, y el que hay que resolver reapuntándola.
 */
export function libraryRoots(): Promise<LibraryRootView[]> {
  return call<LibraryRootView[]>('library_roots');
}

/**
 * Vuelve a apuntar una raíz a donde estén ahora sus archivos.
 *
 * Es la mudanza entera: una llamada por raíz devuelve todos sus documentos, sin
 * reimportar y sin tocar una anotación. Lo que devuelve es el recuento después
 * de revisar, que es lo que hay que enseñar para que "se arregló" sea un número
 * y no una promesa.
 */
export function relocateLibraryRoot(rootId: string, path: string): Promise<AvailabilityReport> {
  return call<AvailabilityReport>('relocate_library_root', { rootId, path });
}

/** Revisa dónde está cada archivo de la biblioteca. */
export function refreshAvailability(): Promise<AvailabilityReport> {
  return call<AvailabilityReport>('refresh_availability');
}

/**
 * Estado de la salida de red: interruptor, permisos concedidos y registro.
 *
 * Las tres cosas vienen juntas porque son tres respuestas a una sola pregunta
 * —¿qué sale de este equipo?— y pedirlas por separado pintaría la pantalla tres
 * veces con verdades parciales.
 */
export function networkReport(): Promise<NetworkReport> {
  return call<NetworkReport>('network_report');
}

/** Revoca un permiso de salida. Que no existiera no es un error. */
export function revokeNetworkConsent(host: string, purpose: string): Promise<void> {
  return call<void>('revoke_network_consent', { host, purpose });
}

/**
 * Autoriza a los catálogos a salir a un destino.
 *
 * Añadir un catálogo no lo autoriza: anotar una dirección y permitir una salida
 * hacia ella son dos actos distintos, y solo el segundo lo decide el usuario.
 * El propósito es siempre `catalog` y no es parámetro, por lo mismo que en la
 * consulta: un permiso para navegar un catálogo no puede convertirse en otro.
 */
/**
 * Los modelos de IA del catálogo, como los enseña Ajustes.
 *
 * La licencia viaja siempre, esté instalado o no: el usuario la tiene que ver
 * **antes** de descargar.
 */
export function aiModels(): Promise<AiModels> {
  return call<AiModels>('ai_models');
}

/**
 * Empieza a instalar un modelo. **No espera a terminar.**
 *
 * Devuelve `true` si ya había una instalación en marcha. El avance llega por
 * `AiModelProgress` y el final por `AiModelFinished`, que dice si terminó, si se
 * canceló, si falta autorizar un anfitrión —con su nombre— o con qué código falló.
 */
export function installAiModel(id: string): Promise<boolean> {
  return call<boolean>('install_ai_model', { id });
}

/** Pide cortar la instalación en curso. */
export function cancelAiModelInstall(): Promise<void> {
  return call<void>('cancel_ai_model_install');
}

/** Quita un modelo instalado. */
export function uninstallAiModel(id: string): Promise<void> {
  return call<void>('uninstall_ai_model', { id });
}

/**
 * Autoriza la descarga de modelos desde un anfitrión.
 *
 * El runtime solo lo concede si lo propuso él: el del catálogo o la red de
 * distribución a la que redirigió la última instalación.
 */
export function allowModelHost(host: string): Promise<void> {
  return call<void>('allow_model_host', { host });
}

/**
 * El proveedor de IA externo: cuál es, a dónde sale, si está autorizado
 * y si tiene credencial. **Nunca la credencial.**
 */
export function aiProviderStatus(): Promise<AiProviderStatus> {
  return call<AiProviderStatus>('ai_provider_status');
}

/**
 * Prueba el proveedor externo.
 *
 * **La primera vez no envía nada**: devuelve `needsConsent` con el mensaje entero
 * que enviaría. Solo después de `allowAiProvider` sale algo.
 */
export function probeAiProvider(): Promise<AiProbe> {
  return call<AiProbe>('probe_ai_provider');
}

/** Autoriza el envío al proveedor externo configurado. No recibe a cuál. */
export function allowAiProvider(): Promise<void> {
  return call<void>('allow_ai_provider');
}

/** Guarda en el llavero la credencial del proveedor externo que hay escrito. */
export function setAiCredential(secret: string): Promise<void> {
  return call<void>('set_ai_credential', { secret });
}

/** Borra la credencial del proveedor externo que hay escrito. */
export function clearAiCredential(): Promise<void> {
  return call<void>('clear_ai_credential');
}

/** Lo enviado a proveedores externos, de lo más reciente a lo más antiguo. */
export function aiSends(limit: number): Promise<AiSend[]> {
  return call<AiSend[]>('ai_sends', { limit });
}

/** Borra el registro de envíos. Devuelve cuántos había. */
export function clearAiSends(): Promise<number> {
  return call<number>('clear_ai_sends');
}

/**
 * Marca una salida generada como incorrecta.
 *
 * Local, y **sin el texto del documento**: lo que viaja es la salida del modelo,
 * con qué plantilla y qué proveedor se hizo, y qué está mal.
 */
export function reportAiOutput(report: AiReportDraft): Promise<number> {
  return call<number>('report_ai_output', { report });
}

/** Lo marcado como incorrecto, de lo más reciente a lo más antiguo. */
export function aiReports(limit: number): Promise<AiReport[]> {
  return call<AiReport[]>('ai_reports', { limit });
}

/** Quita un reporte. Devuelve si existía. */
export function forgetAiReport(id: number): Promise<boolean> {
  return call<boolean>('forget_ai_report', { id });
}

/** Borra los reportes. Devuelve cuántos había. */
export function clearAiReports(): Promise<number> {
  return call<number>('clear_ai_reports');
}

/**
 * Guarda los reportes donde el usuario diga, para mirarlos antes de decidir nada.
 *
 * Como el diagnóstico: el diálogo nativo elige el destino y quien escribe es el
 * runtime. **No los manda a ninguna parte**, porque no hay a dónde.
 *
 * Devuelve `false` si se cancela, que no es un error.
 */
export async function guardarReportesDeIa(
  nombrePorDefecto: string,
  titulo: string
): Promise<boolean> {
  const destino = await save({ defaultPath: nombrePorDefecto, title: titulo });
  if (destino === null) return false;
  await call<void>('save_ai_reports', { path: destino });
  return true;
}

/**
 * Lo que costaría preparar la búsqueda por significado de un alcance: documentos,
 * fragmentos, disco y tiempo. Se pide **antes** de ofrecer empezar.
 */
export function estimateVectors(scope: VectorScope): Promise<VectorEstimate> {
  return call<VectorEstimate>('estimate_vectors', { scope });
}

/** Empieza a preparar un alcance en segundo plano. El avance llega por evento. */
export function startVectors(scope: VectorScope): Promise<void> {
  return call<void>('start_vectors', { scope });
}

export function pauseVectors(paused: boolean): Promise<void> {
  return call<void>('pause_vectors', { paused });
}

/** Corta la preparación. Lo ya calculado se queda. */
export function cancelVectors(): Promise<void> {
  return call<void>('cancel_vectors');
}

export function vectorJob(): Promise<VectorJob> {
  return call<VectorJob>('vector_job');
}

/** El lector está pintando: la preparación de fondo se aparta un momento. */
export function readerActivity(): Promise<void> {
  return call<void>('reader_activity');
}

export function vectorExclusion(kind: VectorExclusionKind, id: string): Promise<boolean> {
  return call<boolean>('vector_exclusion', { kind, id });
}

/**
 * Busca por significado en lo preparado de un alcance.
 *
 * **Aparte de `search` y después de ella**: si esto falla o no existe, lo léxico ya
 * está en pantalla. Sin nada preparado contesta vacío sin calcular nada.
 */
export function semanticSearch(query: string, scope: Scope, limit = 10): Promise<SemanticHit[]> {
  return call<SemanticHit[]>('semantic_search', { query, scope, limit });
}

/**
 * Quién generaría las funciones de estudio y si puede. Cada pantalla de IA lo
 * enseña: `ai/privacy.md` sección 6.
 */
export function studyAvailability(): Promise<StudyAvailability> {
  return call<StudyAvailability>('study_availability');
}

/**
 * Estima los caracteres y tokens aproximados que saldrían para un alcance.
 */
export function estimateStudyScope(scope: StudyScope): Promise<StudyScopeEstimate> {
  return call<StudyScopeEstimate>('estimate_study_scope', { scope });
}

/**
 * Hace una función de estudio sobre una parte del documento abierto.
 *
 * Tarda: con un modelo local, minutos. `CONSENT_REQUIRED` significa que quien
 * genera es externo y no se ha autorizado; no ha salido nada.
 */
export function generateStudy(
  studyFunction: StudyFunction,
  scope: StudyScope
): Promise<StudyGeneration> {
  return call<StudyGeneration>('generate_study', { function: studyFunction, scope });
}

/** Corta la generación en curso. */
export function cancelStudy(): Promise<void> {
  return call<void>('cancel_study');
}

/** Guarda tarjetas y preguntas aceptadas: todas o ninguna. */
export function saveStudyCards(cards: NewStudyCard[]): Promise<string[]> {
  return call<string[]>('save_study_cards', { cards });
}

/** Las tarjetas y preguntas guardadas, de un libro o de todos. */
export function studyCards(
  editionId: string | null,
  limit: number,
  offset = 0,
  dueOnly = false
): Promise<StudyCard[]> {
  return call<StudyCard[]>('study_cards', { editionId, dueOnly, limit, offset });
}

/** Registra el repaso de una tarjeta o pregunta recalculando su intervalo y próxima fecha. */
export function reviewStudyCard(id: string, rating: ReviewRating): Promise<StudyCard> {
  return call<StudyCard>('review_study_card', { id, rating });
}

/** Cantidad total de tarjetas y cuántas tocan hoy para repasar. */
export function studyCardCounts(editionId: string | null = null): Promise<StudyCardCounts> {
  return call<StudyCardCounts>('study_card_counts', { editionId });
}

export function editStudyCard(id: string, edit: StudyCardEdit): Promise<void> {
  return call<void>('edit_study_card', { id, edit });
}

export function deleteStudyCard(id: string): Promise<void> {
  return call<void>('delete_study_card', { id });
}

/** Guarda conceptos aceptados. Uno que ya estaba en el libro junta sus fuentes. */
export function saveConcepts(concepts: NewConcept[]): Promise<string[]> {
  return call<string[]>('save_concepts', { concepts });
}

/** Los conceptos guardados, de un libro o de todos. */
export function concepts(editionId: string | null, limit: number, offset = 0): Promise<Concept[]> {
  return call<Concept[]>('concepts', { editionId, limit, offset });
}

export function editConcept(
  id: string,
  name: string,
  kind: ConceptKind,
  description: string
): Promise<void> {
  return call<void>('edit_concept', { id, name, kind, description });
}

export function deleteConcept(id: string): Promise<void> {
  return call<void>('delete_concept', { id });
}

/** Excluye o vuelve a incluir. Excluir borra lo ya preparado. */
export function setVectorExclusion(
  kind: VectorExclusionKind,
  id: string,
  excluded: boolean
): Promise<void> {
  return call<void>('set_vector_exclusion', { kind, id, excluded });
}

export function allowCatalogHost(host: string, cleartext = false): Promise<void> {
  return call<void>('allow_catalog_host', { host, cleartext });
}

/**
 * Autoriza a los proveedores de consulta a salir a un destino.
 *
 * **Se manda el anfitrión que el propio runtime propuso** al omitir la consulta,
 * no uno inventado aquí: al conceder, el runtime comprueba que algún proveedor
 * instalado lo reconoce como suyo. El propósito es siempre `lookup` y no es
 * parámetro, porque un permiso para consultar no puede convertirse en uno para
 * descargar.
 */
/** Autoriza el catálogo abierto de metadatos desde la ficha. */
export function allowMetadataHost(): Promise<void> {
  return call<void>('allow_metadata_host');
}

export function allowLookupHost(host: string): Promise<void> {
  return call<void>('allow_lookup_host', { host });
}

/**
 * Qué se envía al salir con cada propósito, en lenguaje llano.
 *
 * Sale del runtime porque vive junto al propósito: una explicación escrita aquí
 * se quedaría vieja en cuanto el propósito cambiara, y el criterio
 * dice que se diga **qué texto se va a enviar**.
 */
export function whatIsSent(purpose: string): Promise<string> {
  return call<string>('what_is_sent', { purpose });
}

/**
 * Busca en el catálogo abierto candidatos para completar una edición.
 *
 * **Explícita, nunca automática al importar.** Es el criterio: una
 * biblioteca de ciento cincuenta mil obras que consultara sola al importar haría
 * ciento cincuenta mil peticiones que nadie pidió.
 *
 * Lo que se manda fuera lo compone el runtime con lo que hay en el catálogo
 * local, no esta capa: es el título del usuario lo que sale, y tiene que ser el
 * que hay.
 */
export function searchOnlineMetadata(editionId: string): Promise<MetadataCandidate[]> {
  return call<MetadataCandidate[]>('search_online_metadata', { editionId });
}

/**
 * Aplica un candidato a una edición, campo por campo.
 *
 * **Nada de esto sobrescribe una edición manual**: cada campo pasa por la regla
 * de procedencia, que lo deja como propuesta en lugar de escribirlo. Y
 * lo que se rechazó no se vuelve a ofrecer.
 */
export function applyOnlineMetadata(
  editionId: string,
  candidate: MetadataCandidate
): Promise<EnrichReport> {
  return call<EnrichReport>('apply_online_metadata', { editionId, candidate });
}

/** Descarga la portada de un candidato y la deja apuntada en la edición. */
export function fetchOnlineCover(editionId: string, url: string): Promise<string> {
  return call<string>('fetch_online_cover', { editionId, url });
}

/** La ficha de una edición: editorial, fecha, idioma, materias y copias. */
export function editionDetails(editionId: string): Promise<EditionDetails> {
  return call<EditionDetails>('edition_details', { editionId });
}

/** Propuestas vivas de una edición: lo que llegó y no se pudo aplicar. */
export function pendingSuggestions(editionId: string): Promise<MetadataSuggestion[]> {
  return call<MetadataSuggestion[]>('pending_suggestions', { editionId });
}

/** Acepta una propuesta: la decisión pasa a ser del usuario. */
export function acceptSuggestion(id: string): Promise<void> {
  return call<void>('accept_suggestion', { id });
}

/** Descarta una propuesta. No se vuelve a ofrecer. */
export function rejectSuggestion(id: string): Promise<void> {
  return call<void>('reject_suggestion', { id });
}

/**
 * Catálogos OPDS que el usuario ha añadido.
 *
 * **Ninguno viene puesto.** Un catálogo preconfigurado es una recomendación
 * comercial disfrazada de función.
 */
export function listCatalogs(): Promise<Catalog[]> {
  return call<Catalog[]>('list_catalogs');
}

/** Añade un catálogo por su dirección. Solo `https`. */
export function addCatalog(title: string, url: string): Promise<Catalog> {
  return call<Catalog>('add_catalog', { title, url });
}

/** Quita un catálogo. Que no estuviera no es un error. */
export function removeCatalog(id: string): Promise<void> {
  return call<void>('remove_catalog', { id });
}

/**
 * Pide un feed de un catálogo y lo devuelve analizado.
 *
 * Un catálogo inalcanzable y uno que devuelve basura llegan con **códigos
 * distintos**: piden cosas distintas al usuario.
 */
export function browseCatalog(url: string): Promise<CatalogPage> {
  return call<CatalogPage>('browse_catalog', { url });
}

/**
 * Descarga un documento de un catálogo y lo importa.
 *
 * Entra por la canalización de importación de siempre, que reconoce el formato
 * **por contenido**: un catálogo que anuncia un EPUB y sirve otra cosa se
 * rechaza ahí, y no queda registro de nada.
 */
export function downloadFromCatalog(url: string): Promise<CatalogDownload> {
  return call<CatalogDownload>('download_from_catalog', { url });
}

/**
 * Se suscribe a un evento del runtime.
 *
 * Devuelve la función de baja; los componentes deben invocarla al destruirse
 * para no acumular escuchas.
 */
export function onDomainEvent<T extends DomainEvent['type']>(
  type: T,
  handler: (event: Extract<DomainEvent, { type: T }>) => void
): Promise<UnlistenFn> {
  return listen<Extract<DomainEvent, { type: T }>>(type, ({ payload }) => handler(payload));
}

// ---------------------------------------------------------------------------
// Extensiones
// ---------------------------------------------------------------------------

/**
 * Lo que hay instalado, con sus permisos y su estado.
 *
 * Devuelve lista vacía si el subsistema no arrancó, en lugar de propagar el
 * error: Ajustes tiene que poder abrirse aunque las extensiones estén rotas.
 */
export async function extensionesInstaladas(): Promise<ExtensionView[]> {
  try {
    return await call<ExtensionView[]>('extensions_list');
  } catch {
    return [];
  }
}

/**
 * Qué versión de la API de extensiones habla esta aplicación.
 *
 * La cifra sale del anfitrión y no se escribe aquí: la sección 2 de
 * `versioning.md` exige que el número del compromiso viva en un solo sitio, y
 * una copia en el frontend sería la segunda.
 */
export async function versionDeApiDeExtensiones(): Promise<ExtensionApi> {
  return await call<ExtensionApi>('extensions_api');
}

/**
 * Pide la carpeta de una extensión.
 *
 * Es una carpeta y no un archivo porque un paquete son su manifiesto y sus
 * módulos. Devuelve `null` si quien la abre cambia de opinión, que no es un
 * error.
 */
export async function elegirExtension(): Promise<string | null> {
  const elegido = await open({
    directory: true,
    multiple: false,
    title: 'Elegir la carpeta de una extensión'
  });
  if (elegido === null) return null;
  return Array.isArray(elegido) ? (elegido[0] ?? null) : elegido;
}

/**
 * Instala una extensión desde una carpeta.
 *
 * Puede devolver que **falta consentimiento**, y entonces no se instaló nada.
 */
export function instalarExtension(ruta: string): Promise<ExtensionInstall> {
  return call<ExtensionInstall>('extensions_install', { path: ruta });
}

/**
 * Concede a una extensión salida a uno de los anfitriones que declara.
 *
 * Lleva la ruta y no el identificador porque se concede **antes** de instalar:
 * en ese momento su manifiesto solo existe en el disco de quien la instala.
 */
export function concederAExtension(ruta: string, anfitrion: string): Promise<void> {
  return call<void>('extensions_grant', { path: ruta, host: anfitrion });
}

/** Revoca un anfitrión a una extensión instalada. */
export function revocarAExtension(id: string, anfitrion: string): Promise<void> {
  return call<void>('extensions_revoke', { id, host: anfitrion });
}

/** Enciende o apaga una extensión. Devuelve la lista ya actualizada. */
export function habilitarExtension(id: string, activa: boolean): Promise<ExtensionView[]> {
  return call<ExtensionView[]>('extensions_set_enabled', { id, enabled: activa });
}

/** Desinstala una extensión y sus permisos. Devuelve la lista ya actualizada. */
export function desinstalarExtension(id: string): Promise<ExtensionView[]> {
  return call<ExtensionView[]>('extensions_uninstall', { id });
}

/**
 * Revisa por lotes los metadatos de unas ediciones con una extensión.
 *
 * **No escribe nada**: lo que la extensión proponga queda como propuesta, y una
 * edición manual gana igual. Por eso corregir mil documentos es reversible sin
 * que haya nada que revertir.
 */
export function revisarMetadatosPorLotes(
  extension: string,
  ediciones: string[]
): Promise<BulkReviewSummary> {
  return call<BulkReviewSummary>('review_metadata_in_bulk', { extension, editionIds: ediciones });
}

/**
 * Cuánto cabe en un tablero: columnas y tarjetas.
 *
 * Lo decide el runtime: es lo que ata el tablero al presupuesto de nodos. La
 * interfaz lo usa para dejar de ofrecer una columna o una tarjeta más cuando ya
 * no caben, en lugar de ofrecerla y fallar.
 */
export function limitesDeTablero(): Promise<BoardLimits> {
  return call<BoardLimits>('board_limits');
}

/** Los tableros que hay. */
export function tableros(limit: number = POR_PAGINA, offset = 0): Promise<Board[]> {
  return call<Board[]>('boards_list', { limit, offset });
}

/** Crea un tablero con sus primeras columnas, nombradas en el idioma de la interfaz. */
export function crearTablero(titulo: string, columnas: string[]): Promise<Board> {
  return call<Board>('boards_create', { title: titulo, columns: columnas });
}

/** Borra un tablero. **Las notas siguen donde estaban.** */
export function borrarTablero(id: string): Promise<boolean> {
  return call<boolean>('boards_delete', { id });
}

/**
 * Las colecciones, con cuántos miembros tiene cada una.
 *
 * El orden lo pone el runtime —las personales primero, cada grupo por nombre—
 * para que el menú y cualquier otra lista digan lo mismo.
 */
export function colecciones(): Promise<CollectionSummary[]> {
  return call<CollectionSummary[]>('collections_list');
}

/** Crea una colección del usuario. Nace vacía: los libros se añaden desde su ficha. */
export function crearColeccion(nombre: string): Promise<CollectionSummary> {
  return call<CollectionSummary>('collections_create', { name: nombre });
}

/** Cambia el nombre de una colección del usuario. */
export function renombrarColeccion(id: string, nombre: string): Promise<void> {
  return call<void>('collections_rename', { id, name: nombre });
}

/** Borra una colección. **Sus libros siguen en la biblioteca.** */
export function borrarColeccion(id: string): Promise<boolean> {
  return call<boolean>('collections_delete', { id });
}

/** Añade ediciones a una colección, todas o ninguna, y dice cuántas entraron nuevas. */
export function anadirAColeccion(id: string, ediciones: string[]): Promise<number> {
  return call<number>('collections_add', { id, editionIds: ediciones });
}

/** Quita una edición de una colección. */
export function quitarDeColeccion(id: string, edicion: string): Promise<void> {
  return call<void>('collections_remove', { id, editionId: edicion });
}

/** Las colecciones del usuario en las que está una edición, para su ficha. */
export function coleccionesDeLaEdicion(edicion: string): Promise<string[]> {
  return call<string[]>('collections_containing', { editionId: edicion });
}

/**
 * Las columnas de un tablero con sus tarjetas, en orden.
 *
 * Es la única lectura del tablero: las columnas y la lista equivalente se pintan
 * de esta misma respuesta. Todas las escrituras de abajo devuelven lo mismo, ya
 * actualizado, para que la interfaz no recomponga el tablero por su cuenta.
 */
export function columnasDeTablero(tablero: string): Promise<BoardColumn[]> {
  return call<BoardColumn[]>('board_columns', { boardId: tablero });
}

/** Añade una columna al final. */
export function anadirColumna(tablero: string, titulo: string): Promise<BoardColumn[]> {
  return call<BoardColumn[]>('board_add_column', { boardId: tablero, title: titulo });
}

/** Cambia el nombre de una columna. */
export function renombrarColumna(
  tablero: string,
  columna: string,
  titulo: string
): Promise<BoardColumn[]> {
  return call<BoardColumn[]>('board_rename_column', {
    boardId: tablero,
    columnId: columna,
    title: titulo
  });
}

/** Borra una columna y sus tarjetas. **Las notas siguen existiendo.** */
export function borrarColumna(tablero: string, columna: string): Promise<BoardColumn[]> {
  return call<BoardColumn[]>('board_delete_column', { boardId: tablero, columnId: columna });
}

/** Lleva una columna a otro lugar del tablero. */
export function moverColumna(
  tablero: string,
  columna: string,
  lugar: number
): Promise<BoardColumn[]> {
  return call<BoardColumn[]>('board_move_column', {
    boardId: tablero,
    columnId: columna,
    position: lugar
  });
}

/** Pone una nota al final de una columna; si ya estaba en el tablero, la mueve. */
export function ponerNotaEnColumna(
  tablero: string,
  columna: string,
  nota: string
): Promise<BoardColumn[]> {
  return call<BoardColumn[]>('board_add_note', {
    boardId: tablero,
    columnId: columna,
    noteId: nota
  });
}

/** Pone al final de una columna un enlace a otro tablero. */
export function enlazarEnColumna(
  tablero: string,
  columna: string,
  destino: string
): Promise<BoardColumn[]> {
  return call<BoardColumn[]>('board_add_link', {
    boardId: tablero,
    columnId: columna,
    linksTo: destino
  });
}

/** Crea una nota y la deja al final de una columna, en una sola escritura. */
export function crearNotaEnColumna(
  tablero: string,
  columna: string,
  titulo: string,
  cuerpo: string
): Promise<BoardColumn[]> {
  return call<BoardColumn[]>('board_create_note', {
    boardId: tablero,
    columnId: columna,
    title: titulo,
    body: cuerpo
  });
}

/** Lleva una tarjeta a un lugar de una columna, la misma u otra. */
export function moverTarjeta(
  tablero: string,
  tarjeta: string,
  columna: string,
  lugar: number
): Promise<BoardColumn[]> {
  return call<BoardColumn[]>('board_move_card', {
    boardId: tablero,
    cardId: tarjeta,
    columnId: columna,
    position: lugar
  });
}

/** Quita una tarjeta del tablero. La nota sigue existiendo. */
export function quitarTarjeta(tablero: string, tarjeta: string): Promise<BoardColumn[]> {
  return call<BoardColumn[]>('board_remove_card', { boardId: tablero, cardId: tarjeta });
}

/**
 * Cuántas anotaciones hay de cada clase, para no ofrecer un filtro que no lleva a
 * nada.
 */
/** Cuántas tarjetas y conceptos hay guardados, para las pestañas de Estudio. */
export function cuentasDeEstudio(): Promise<StudyCounts> {
  return call<StudyCounts>('study_counts');
}

export function cuentasDeAnotaciones(): Promise<AnnotationCounts> {
  return call<AnnotationCounts>('annotation_counts');
}

/**
 * Los formatos de cita que aportan las extensiones instaladas.
 *
 * Devuelve lista vacía si el subsistema no arrancó: exportar citas es una
 * contribución, no una función del núcleo.
 */
export async function formatosDeCita(): Promise<CitationFormat[]> {
  try {
    return await call<CitationFormat[]>('extensions_citation_formats');
  } catch {
    return [];
  }
}

/**
 * Guarda las citas de un documento en el formato que aporte una extensión.
 *
 * Devuelve si se guardó: cancelar el diálogo no es un fallo.
 */
export async function guardarCitas(
  editionId: string,
  formato: string,
  nombre: string,
  titulo: string
): Promise<boolean> {
  const destino = await save({ defaultPath: nombre, title: titulo });
  if (destino === null) return false;
  await call<void>('save_citations', { editionId, format: formato, path: destino });
  return true;
}

/**
 * Los fondos sonoros que aportan los paquetes instalados.
 *
 * Devuelve lista vacía si el subsistema no arrancó.
 */
export async function fondosSonoros(): Promise<ExtensionSoundscape[]> {
  try {
    return await call<ExtensionSoundscape[]>('extensions_soundscapes');
  } catch {
    return [];
  }
}

/**
 * Los modos de lectura que aportan los paquetes de personalización.
 *
 * Devuelve lista vacía si el subsistema no arrancó: no tener extensiones no es
 * un error de la pantalla.
 */
export async function modosDeExtension(): Promise<ExtensionReadingMode[]> {
  try {
    return await call<ExtensionReadingMode[]>('extensions_reading_modes');
  } catch {
    return [];
  }
}

/**
 * Los avisos de retirada que ya se conocen.
 *
 * No sale a la red: son los que se guardaron en la última comprobación, y
 * tienen que verse aunque hoy no haya red. Devuelve lista vacía si el
 * subsistema no arrancó.
 */
export async function retiradasDeExtensiones(): Promise<Withdrawal[]> {
  try {
    return await call<Withdrawal[]>('extensions_withdrawals');
  } catch {
    return [];
  }
}

/**
 * Le pregunta al registro qué ha retirado.
 *
 * Sale a la red, así que **no se llama sola**: la lanza quien pulsa. Lo que
 * resulte malicioso se apaga, y ninguna extensión se desinstala.
 */
export function comprobarRetiradasDeExtensiones(): Promise<Withdrawal[]> {
  return call<Withdrawal[]>('extensions_check_withdrawals');
}

/** Las generaciones a las que se puede volver. */
export function generacionesDeExtensiones(): Promise<number[]> {
  return call<number[]>('extensions_generations');
}

/** Vuelve a la generación anterior. Devuelve la lista ya actualizada. */
export function volverAtrasExtensiones(): Promise<ExtensionView[]> {
  return call<ExtensionView[]>('extensions_rollback');
}

/**
 * Las órdenes que las extensiones ponen en un sitio de la interfaz.
 *
 * Salen del manifiesto, así que existen **antes** de que se cargue una sola
 * instrucción de la extensión. Devuelve lista vacía si el subsistema no
 * arrancó: que no haya extensiones no es un error de la pantalla.
 */
export async function ordenesDeExtension(sitio: string): Promise<ExtensionCommand[]> {
  try {
    return await call<ExtensionCommand[]>('extensions_context_commands', { site: sitio });
  } catch {
    return [];
  }
}

/**
 * Ejecuta una orden de una extensión. **Aquí se compila su código.**
 *
 * Devuelve el texto corto que la extensión quiera enseñar, o nada.
 */
export function ejecutarOrdenDeExtension(extension: string, orden: string): Promise<string | null> {
  return call<string | null>('extensions_run_command', { extension, command: orden });
}

/**
 * Las vistas que las extensiones aportan a Estudio.
 *
 * Salen del manifiesto, así que las pestañas existen antes de que se compile
 * una sola instrucción de la extensión.
 */
export async function vistasDeExtension(): Promise<ExtensionStudyView[]> {
  try {
    return await call<ExtensionStudyView[]>('extensions_study_views');
  } catch {
    return [];
  }
}

/**
 * Compone una vista de una extensión. **Aquí se carga su código.**
 *
 * Devuelve bloques tipados, no marcado: la interfaz la pinta el núcleo.
 */
export function componerVistaDeExtension(
  extension: string,
  vista: string
): Promise<ExtensionRenderedView> {
  return call<ExtensionRenderedView>('extensions_render_view', { extension, view: vista });
}

/**
 * Los temas que aportan las extensiones instaladas.
 *
 * Un tema no trae código: es un manifiesto con colores y nada más, y el runtime
 * rechaza uno que declare proveedores, órdenes, vistas o permisos.
 */
export async function temasDeExtension(): Promise<ExtensionTheme[]> {
  try {
    return await call<ExtensionTheme[]>('extensions_themes');
  } catch {
    return [];
  }
}

/** Cuántas preparaciones de audio se borraron por usar una voz retirada; se dice una vez. */
export function avisoDeAudioRetirado(): Promise<number | null> {
  return call<number | null>('retired_audio_notice');
}
