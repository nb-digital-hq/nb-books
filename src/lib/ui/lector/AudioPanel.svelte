<!--
  @component
  Preparar el audio de un capítulo antes de escucharlo, y escuchar lo preparado
  -el capítulo o el libro entero- siguiendo el texto.

  # Por qué esto existe, dicho donde se usa

  Porque en la mayoría de las máquinas la voz neuronal **no narra en directo**:
  decir un párrafo tarda más de lo que el párrafo dura, y adelantar la síntesis no
  arregla lo que no se sostiene en régimen. Así que se paga el tiempo una vez,
  **con el coste delante**, y después se escucha como un audiolibro.

  # Lo que se corrigió el 2026-09-22, revisando la fase

  Este panel no funcionaba en la aplicación, y la suite no podía verlo:

  - Reproducía con un `<audio src>` directo, que **WebKitGTK no reproduce** con el
    esquema propio de la aplicación (performance-budgets 4.3.1). Ahora lo hace el
    reproductor de la página, que trae el audio como `blob:`.
  - El audio vivía dentro del panel, que es momentáneo: dejaba de sonar al tocar el
    texto. El reproductor vive en la página.
  - Las marcas se contaban y no se usaban. Ahora el lector **sigue el texto** y se
    puede **empezar donde se va leyendo**.
  - Una preparación que fallaba volvía al botón **sin decir nada**.
  - El libro entero preparado no se podía escuchar en ningún sitio. Ahora aparece
    aquí, empezando por el capítulo que se lee.
  - "Quitar lo preparado" se llevaba el audio de todo el libro.

  Contrato: `docs/architecture/voice-services.md`.
-->
<script lang="ts">
  import { onDomainEvent } from '$core/ipc';
  import { idioma, textos } from '$lib/i18n';
  import { TEXTOS_DE_AUDIO } from '$lib/i18n/locales/audio';
  import { mensajeDeError } from '$lib/i18n/errores';
  import type {
    AudioEstimate,
    AudioJob,
    AudioScope,
    Locator,
    NavigationEntry,
    PreparedAudio
  } from '$lib/bindings';
  import { enBlob } from '$lib/voz/preparado';
  import type { ReproductorDePreparado } from '$lib/voz/reproductor.svelte';

  import { capituloDe } from './capitulo';

  import Button from '../Button.svelte';
  import Spinner from '../Spinner.svelte';

  interface Props {
    editionId: string;
    /** El título del libro, para poder nombrar lo que se prepara. */
    titulo: string;
    /** El índice del documento, para saber dónde acaba el capítulo. */
    entradas: NavigationEntry[];
    /** Dónde se está leyendo. */
    actual: Locator;
    /** La voz elegida, o nada para la de fábrica del modelo. */
    voz?: string | null;
    /** El reproductor de la página, que sigue sonando con el panel cerrado. */
    reproductor: ReproductorDePreparado;
    /** Textos. Anulación, no fuente: el diccionario manda. */
    etiquetas?: Readonly<Record<string, string>>;
  }

  let { editionId, titulo, entradas, actual, voz = null, reproductor, etiquetas }: Props = $props();

  // Los textos de preparar viajan con el panel y no en el diccionario del
  // arranque: ver `locales/audio.ts`.
  const t = $derived({
    ...textos().voz,
    ...TEXTOS_DE_AUDIO[idioma()],
    ...etiquetas
  } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;
  const numero = $derived(new Intl.NumberFormat(idioma(), { maximumFractionDigits: 1 }));

  /**
   * Los comandos de preparar, traídos **al usarlos**.
   *
   * Con una importación estática, abrir cualquier panel del lector cargaría
   * también este módulo, porque los paneles viajan juntos.
   */
  const comandos = () => import('$core/ipc/voz');

  const capitulo = $derived(capituloDe(entradas, actual));
  // Tipado contra el binding y sin forzar: antes un
  // `as unknown as AudioScope` tapaba que se mandaba `Chapter` y el runtime
  // espera `chapter`, y el audio preparado no funcionaba en ningún sitio.
  const alcance = $derived<AudioScope>({
    kind: 'chapter',
    editionId,
    hrefs: capitulo.hrefs,
    fromPosition: capitulo.desde,
    toPosition: capitulo.hasta,
    title: capitulo.titulo || titulo
  });
  const delLibro = $derived<AudioScope>({ kind: 'book', editionId, title: titulo });

  let coste = $state<AudioEstimate | null>(null);
  let trabajo = $state<AudioJob | null>(null);
  let delCapitulo = $state<PreparedAudio | null>(null);
  let libroEntero = $state<PreparedAudio | null>(null);
  let muestra = $state<string | null>(null);
  let fallo = $state<string | null>(null);
  let aviso = $state<string | null>(null);
  let pidiendo = $state(false);

  /** Lo que se escucha: el capítulo si está, y si no el libro entero. */
  const listo = $derived(delCapitulo ?? libroEntero);
  const esElLibro = $derived(!delCapitulo && libroEntero !== null);
  const cargado = $derived(listo !== null && reproductor.preparado?.path === listo.path);

  async function mirar() {
    try {
      fallo = null;
      const { audioJob, estimateAudio, preparedAudio } = await comandos();
      [coste, delCapitulo, libroEntero, trabajo] = await Promise.all([
        estimateAudio(alcance, voz),
        preparedAudio(alcance, voz),
        preparedAudio(delLibro, voz),
        audioJob()
      ]);
    } catch (e) {
      fallo = mensajeDeError(e);
    }
  }

  // Al abrir el panel y cada vez que cambia el capítulo: lo que se enseña es de
  // **este** capítulo, no del que estaba abierto al montar.
  $effect(() => {
    void alcance;
    void mirar();
  });

  $effect(() => {
    const suscripciones = [
      onDomainEvent('AudioJobProgress', (evento) => {
        if (evento.editionId !== editionId) return;
        aviso = null;
        trabajo = {
          editionId: evento.editionId,
          title: trabajo?.title ?? capitulo.titulo,
          done: evento.done,
          total: evento.total,
          remainingMs: evento.remainingMs,
          queued: evento.queued
        };
      }),
      onDomainEvent('AudioJobFinished', (evento) => {
        if (evento.editionId !== editionId) return;
        trabajo = null;
        // **Lo que pasó se dice.** Antes el panel volvía al botón sin
        // más, y una preparación que fallaba parecía no haber empezado nunca.
        aviso =
          evento.outcome === 'done'
            ? texto('prepararTerminado')
            : evento.outcome === 'cancelled'
              ? texto('prepararCancelado')
              : `${texto('prepararSeDetuvo')} ${mensajeDeError({ code: evento.reasonCode })}`;
        void mirar();
      })
    ];
    return () => {
      for (const suscripcion of suscripciones) void suscripcion.then((quitar) => quitar());
    };
  });

  async function preparar(queAlcance = alcance) {
    pidiendo = true;
    aviso = null;
    try {
      const { audioJob, startAudio } = await comandos();
      await startAudio(queAlcance, voz);
      trabajo = await audioJob();
    } catch (e) {
      fallo = mensajeDeError(e);
    } finally {
      pidiendo = false;
    }
  }

  async function escucharMuestra() {
    pidiendo = true;
    try {
      const { sampleAudio } = await comandos();
      const ruta = await sampleAudio(alcance, voz);
      if (muestra) URL.revokeObjectURL(muestra);
      muestra = await enBlob(ruta);
    } catch (e) {
      fallo = mensajeDeError(e);
    } finally {
      pidiendo = false;
    }
  }

  async function escucharDesdeAqui() {
    if (!listo) return;
    await reproductor.cargar(listo);
    reproductor.desde(actual);
  }

  async function quitar() {
    if (!listo) return;
    const deQue = esElLibro ? delLibro : alcance;
    if (cargado) reproductor.parar();
    const { forgetAudio } = await comandos();
    await forgetAudio(deQue);
    await mirar();
  }

  /** Minutos y segundos, que es como se lee una espera. */
  function duracion(segundos: number): string {
    const minutos = Math.floor(segundos / 60);
    if (minutos < 1) return `${Math.max(1, Math.round(segundos))} s`;
    if (minutos < 60) return `${minutos} min`;
    return `${numero.format(minutos / 60)} h`;
  }

  const megas = (bytes: number) => `${numero.format(bytes / 1024 ** 2)} MB`;
</script>

<section class="nb-preparar">
  <h3 class="nb-preparar-titulo">{texto('prepararTitulo')}</h3>
  <p class="nb-preparar-ayuda">{texto('prepararAyuda')}</p>

  {#if fallo}
    <p class="nb-fallo" role="alert">{fallo}</p>
  {/if}
  {#if aviso}
    <p class="nb-preparar-avance" role="status">{aviso}</p>
  {/if}

  {#if listo}
    <!--
      Lo preparado se escucha desde aquí, **siguiendo el texto**, y sigue sonando con
      el panel cerrado: el reproductor es de la página.
    -->
    <p class="nb-preparar-avance" role="status">
      {esElLibro ? texto('prepararLibroListo') : texto('prepararListo')}
      {#if listo.status === 'partial'}· {texto('prepararAMedias')}{/if}
      · {duracion(listo.durationMs / 1000)} · {megas(listo.bytes)}
    </p>
    <p class="nb-preparar-ayuda">
      {texto('preparadoConVoz').replace('{voz}', listo.voice).replace('{modelo}', listo.modelRev)}
    </p>
    {#if !listo.matchesCurrent}
      <p class="nb-fallo" role="status">
        {texto('preparadoVozDiferente')}
      </p>
    {/if}
    <div class="nb-preparar-acciones">
      {#if cargado}
        <Button variante="primario" onclick={() => reproductor.alternar()}>
          {reproductor.sonando ? texto('prepararPausar') : texto('prepararSeguirEscuchando')}
        </Button>
      {/if}
      <Button
        variante={cargado ? 'secundario' : 'primario'}
        onclick={() => void escucharDesdeAqui()}
      >
        {texto('prepararDesdeAqui')}
      </Button>
      <label class="nb-preparar-seguir">
        <input type="checkbox" bind:checked={reproductor.seguir} />
        {texto('prepararSeguirTexto')}
      </label>
    </div>
    {#if cargado && reproductor.marca >= 0}
      <p class="nb-preparar-ayuda">
        {texto('prepararFragmento')}
        {reproductor.marca + 1}
        {texto('prepararDe')}
        {listo.marks.length}
      </p>
    {/if}
    {#if reproductor.fallo && cargado}
      <p class="nb-fallo" role="alert">{texto('prepararNoSuena')}</p>
    {/if}
    <div class="nb-preparar-acciones">
      {#if !listo.matchesCurrent}
        <Button
          variante="secundario"
          onclick={() => void preparar(esElLibro ? delLibro : alcance)}
          deshabilitado={pidiendo}
        >
          {texto('prepararRegenerar')}
        </Button>
      {/if}
      <Button onclick={() => void quitar()}>
        {esElLibro ? texto('prepararQuitarLibro') : texto('prepararOlvidar')}
      </Button>
    </div>
  {/if}

  {#if trabajo}
    <p class="nb-preparar-avance" role="status">
      {trabajo.done}
      {texto('prepararDe')}
      {trabajo.total}
      {#if trabajo.remainingMs !== null}
        · {texto('prepararFalta')}
        {duracion(trabajo.remainingMs / 1000)}
      {/if}
    </p>
    <div
      class="nb-barra-cobertura"
      role="progressbar"
      aria-valuemin="0"
      aria-valuemax={trabajo.total}
      aria-valuenow={trabajo.done}
      aria-label={texto('prepararTitulo')}
    >
      <span style:width="{(trabajo.done / Math.max(trabajo.total, 1)) * 100}%"></span>
    </div>
    <div class="nb-preparar-acciones">
      <Button onclick={() => void comandos().then(({ cancelAudio }) => cancelAudio())}>
        {texto('prepararCancelar')}
      </Button>
    </div>
  {:else if !delCapitulo && coste}
    <dl class="nb-preparar-coste">
      <div>
        <dt>{texto('prepararDurara')}</dt>
        <dd>{duracion(coste.audioSeconds)}</dd>
      </div>
      <div>
        <dt>{texto('prepararOcupara')}</dt>
        <dd>{megas(coste.bytes)}</dd>
      </div>
      <div>
        <dt>{texto('prepararTardara')}</dt>
        <dd>
          {coste.workSeconds === null ? texto('prepararSinMedir') : duracion(coste.workSeconds)}
        </dd>
      </div>
    </dl>

    {#if !coste.fits}
      <p class="nb-fallo" role="alert">{mensajeDeError({ code: 'STORAGE_BUDGET_REACHED' })}</p>
    {/if}

    {#if muestra}
      <!--
        La muestra es un `blob:`, que WebKitGTK sí reproduce; y dura unos segundos,
        así que puede vivir en el panel.
      -->
      <!-- svelte-ignore a11y_media_has_caption -->
      <audio controls src={muestra}></audio>
    {/if}

    <div class="nb-preparar-acciones">
      <Button onclick={() => void escucharMuestra()} deshabilitado={pidiendo}>
        {texto('prepararMuestra')}
      </Button>
      <Button onclick={() => void preparar()} deshabilitado={pidiendo || !coste.fits}>
        {texto('prepararEmpezar')}
      </Button>
      {#if pidiendo}<Spinner etiqueta={texto('prepararTitulo')} />{/if}
    </div>
  {:else if !listo}
    <Spinner etiqueta={texto('prepararTitulo')} />
  {/if}
</section>

<style>
  .nb-preparar {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding-block-start: var(--nb-space-3);
    border-block-start: 1px solid var(--nb-border);
  }

  .nb-preparar-titulo {
    margin: 0;
    font-size: var(--nb-text-md);
    font-weight: 600;
    color: var(--nb-text);
  }

  .nb-preparar-ayuda {
    margin: 0;
    font-size: var(--nb-text-sm);
    color: var(--nb-text-muted);
  }

  .nb-preparar-avance {
    margin: 0;
    font-size: var(--nb-text-sm);
    color: var(--nb-text);
  }

  .nb-preparar-coste {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: var(--nb-space-1) var(--nb-space-2);
    margin: 0;
    font-size: var(--nb-text-sm);
  }

  .nb-preparar-coste div {
    display: contents;
  }

  .nb-preparar-coste dt {
    color: var(--nb-text-muted);
  }

  .nb-preparar-coste dd {
    margin: 0;
    text-align: end;
    color: var(--nb-text);
  }

  .nb-preparar-acciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
    align-items: center;
  }

  audio {
    inline-size: 100%;
  }

  .nb-preparar-seguir {
    display: inline-flex;
    gap: var(--nb-space-1);
    align-items: center;
    font-size: var(--nb-text-sm);
    color: var(--nb-text);
  }
</style>
