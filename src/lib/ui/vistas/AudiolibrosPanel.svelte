<!--
  @component
  Preparar el libro entero en audio, desde la barra superior de Biblioteca.

  # Por qué está aquí y no es un destino

  Porque preparar un audiolibro es un proceso con progreso, como importar, y el
  criterio de ADR-013 dice que un destino es un sitio donde se permanece y se
  trabaja. Importar tampoco es un destino: es una acción de esta misma barra. Lo
  que sí es propio de Biblioteca es el filtro que esto deja detrás -qué libros se
  pueden escuchar-, y ese sí vive en la vista.

  # Lo que se enseña antes de que nadie espere

  Cuánto durará, cuánto ocupará, cuánto tardará en esta máquina y **si cabe** en
  el sitio reservado a lo que la aplicación deriva de la biblioteca. Las cuatro
  salen del runtime. Y antes de decidir se puede escuchar una muestra **del propio
  libro**, no una frase de ejemplo: quien va a esperar una hora tiene derecho a oír
  cómo suena su libro con sus nombres propios.

  Contrato: `docs/architecture/voice-services.md`.
-->
<script lang="ts">
  import { onDomainEvent, rutaDeImagen } from '$core/ipc';
  import { idioma, textos } from '$lib/i18n';
  import { TEXTOS_DE_AUDIO } from '$lib/i18n/locales/audio';
  import { mensajeDeError } from '$lib/i18n/errores';
  import type {
    AudioEstimate,
    AudioJob,
    AudioQueued,
    AudioScope,
    PreparedAudio
  } from '$lib/bindings';

  import Button from '../Button.svelte';
  import Spinner from '../Spinner.svelte';

  interface Props {
    /** El libro que la biblioteca tiene enfocado, si hay alguno. */
    libro?: { editionId: string; title: string } | null;
    /** La colección abierta, para poder encolarla entera. */
    coleccion?: { id: string; name: string } | null;
    /** La voz elegida, o nada para la de fábrica del modelo. */
    voz?: string | null;
    /** Enseñar en la biblioteca lo que ya está preparado. */
    alVerPreparados?: () => void;
    /** Textos. Anulación, no fuente: el diccionario manda. */
    etiquetas?: Readonly<Record<string, string>>;
  }

  let { libro = null, coleccion = null, voz = null, alVerPreparados, etiquetas }: Props = $props();

  const t = $derived({
    ...textos().voz,
    ...TEXTOS_DE_AUDIO[idioma()],
    ...etiquetas
  } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;
  const numero = $derived(new Intl.NumberFormat(idioma(), { maximumFractionDigits: 1 }));

  /** Los comandos de preparar, traídos al usarlos: ver `AudioPanel.svelte`. */
  const comandos = () => import('$core/ipc/voz');

  const alcance = $derived<AudioScope | null>(
    libro ? { kind: 'book', editionId: libro.editionId, title: libro.title } : null
  );

  let coste = $state<AudioEstimate | null>(null);
  /** Lo que ya hay preparado de este libro, si hay algo. */
  let preparado = $state<PreparedAudio | null>(null);
  let trabajo = $state<AudioJob | null>(null);
  let cola = $state<AudioQueued[]>([]);
  let muestra = $state<string | null>(null);
  let fallo = $state<string | null>(null);
  let aviso = $state<string | null>(null);
  let pidiendo = $state(false);

  async function mirar() {
    try {
      fallo = null;
      const { audioJob, audioQueue, estimateAudio, preparedAudio } = await comandos();
      [trabajo, cola] = await Promise.all([audioJob(), audioQueue()]);
      coste = alcance ? await estimateAudio(alcance, voz) : null;
      preparado = alcance ? await preparedAudio(alcance, voz) : null;
    } catch (e) {
      fallo = mensajeDeError(e);
    }
  }

  // Al abrir el panel y cada vez que cambia el libro enfocado: lo que se enseña
  // es de **este** libro, no del que estaba elegido al montar.
  $effect(() => {
    void alcance;
    void mirar();
  });

  $effect(() => {
    const suscripciones = [
      onDomainEvent('AudioJobProgress', (evento) => {
        trabajo = {
          editionId: evento.editionId,
          title: trabajo?.title ?? '',
          done: evento.done,
          total: evento.total,
          remainingMs: evento.remainingMs,
          queued: evento.queued
        };
      }),
      onDomainEvent('AudioJobFinished', (evento) => {
        trabajo = null;
        // **Lo que pasó se dice**, y no solo que ya no hay trabajo: una preparación que
        // falla volvía al botón sin más, y parecía no haber empezado nunca.
        aviso =
          evento.outcome === 'done'
            ? texto('audiolibrosTerminado')
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

  async function preparar() {
    if (!alcance) return;
    pidiendo = true;
    try {
      const { audioJob, audioQueue, startAudio } = await comandos();
      await startAudio(alcance, voz);
      [trabajo, cola] = await Promise.all([audioJob(), audioQueue()]);
    } catch (e) {
      fallo = mensajeDeError(e);
    } finally {
      pidiendo = false;
    }
  }

  async function encolarColeccion() {
    if (!coleccion) return;
    pidiendo = true;
    try {
      const { audioJob, audioQueue, startAudioCollection } = await comandos();
      const cuantas = await startAudioCollection(coleccion.id, voz);
      aviso = `${cuantas} ${texto('audiolibrosEncoladas')}`;
      [trabajo, cola] = await Promise.all([audioJob(), audioQueue()]);
    } catch (e) {
      fallo = mensajeDeError(e);
    } finally {
      pidiendo = false;
    }
  }

  async function escucharMuestra() {
    if (!alcance) return;
    pidiendo = true;
    try {
      const { sampleAudio } = await comandos();
      const ruta = await sampleAudio(alcance, voz);
      const respuesta = await fetch(rutaDeImagen(ruta));
      const blob = await respuesta.blob();
      if (muestra) URL.revokeObjectURL(muestra);
      muestra = URL.createObjectURL(blob);
    } catch (e) {
      fallo = mensajeDeError(e);
    } finally {
      pidiendo = false;
    }
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

<section class="nb-audiolibros">
  <p class="nb-audiolibros-ayuda">{texto('audiolibrosAyuda')}</p>

  {#if fallo}
    <p class="nb-fallo" role="alert">{fallo}</p>
  {/if}
  {#if aviso}
    <p class="nb-audiolibros-avance" role="status">{aviso}</p>
  {/if}

  {#if trabajo}
    <p class="nb-audiolibros-avance" role="status">
      {trabajo.title || texto('audiolibrosTitulo')} · {trabajo.done}
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
      aria-label={texto('audiolibrosTitulo')}
    >
      <span style:width="{(trabajo.done / Math.max(trabajo.total, 1)) * 100}%"></span>
    </div>
    <div class="nb-audiolibros-acciones">
      <Button
        onclick={() =>
          void comandos()
            .then(({ cancelAudio }) => cancelAudio())
            .then(mirar)}
      >
        {texto('prepararCancelar')}
      </Button>
    </div>
  {/if}

  {#if cola.length > 0}
    <p class="nb-audiolibros-ayuda">{texto('audiolibrosEnCola')}</p>
    <ul class="nb-audiolibros-cola">
      {#each cola as espera (espera.editionId)}
        <li>{espera.title}</li>
      {/each}
    </ul>
  {/if}

  {#if libro && preparado}
    <!--
      Ya está preparado: se dice, y se dice **dónde se escucha**, que es el panel de voz
      del lector, siguiendo el texto y empezando por el capítulo que se lea.
    -->
    <p class="nb-audiolibros-libro">{libro.title}</p>
    <p class="nb-audiolibros-avance" role="status">
      {texto('prepararLibroListo')}{#if preparado.status === 'partial'}
        · {texto('prepararAMedias')}{/if}.
      {texto('audiolibrosDondeSeEscucha')}
    </p>
    <p class="nb-audiolibros-ayuda">
      {texto('preparadoConVoz')
        .replace('{voz}', preparado.voice)
        .replace('{modelo}', preparado.modelRev)}
    </p>
    {#if !preparado.matchesCurrent}
      <p class="nb-fallo" role="status">
        {texto('preparadoVozDiferente')}
      </p>
    {/if}
    <div class="nb-audiolibros-acciones">
      {#if !preparado.matchesCurrent}
        <Button onclick={() => void preparar()} deshabilitado={pidiendo}>
          {texto('prepararRegenerar')}
        </Button>
      {/if}
      <Button
        onclick={() =>
          void comandos()
            .then(({ forgetAudio }) => (alcance ? forgetAudio(alcance) : undefined))
            .then(mirar)}
      >
        {texto('prepararQuitarLibro')}
      </Button>
    </div>
  {/if}

  {#if !libro}
    <p class="nb-audiolibros-ayuda">{texto('audiolibrosElige')}</p>
  {:else if preparado}
    <!-- Nada más que ofrecer para este libro: ya está arriba. -->
  {:else if coste}
    <p class="nb-audiolibros-libro">{libro.title}</p>
    <dl class="nb-audiolibros-coste">
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
      {#if coste.freeBytes !== null}
        <div>
          <dt>{texto('audiolibrosQueda')}</dt>
          <dd>{megas(coste.freeBytes)}</dd>
        </div>
      {/if}
    </dl>

    {#if !coste.fits}
      <p class="nb-fallo" role="alert">{texto('audiolibrosNoCabe')}</p>
    {/if}

    {#if muestra}
      <!-- svelte-ignore a11y_media_has_caption -->
      <audio controls src={muestra}></audio>
    {/if}

    <div class="nb-audiolibros-acciones">
      <Button onclick={() => void escucharMuestra()} deshabilitado={pidiendo}>
        {texto('prepararMuestra')}
      </Button>
      <Button onclick={() => void preparar()} deshabilitado={pidiendo || !coste.fits}>
        {texto('prepararEmpezar')}
      </Button>
      {#if pidiendo}<Spinner etiqueta={texto('audiolibrosTitulo')} />{/if}
    </div>
  {:else}
    <Spinner etiqueta={texto('audiolibrosTitulo')} />
  {/if}

  <div class="nb-audiolibros-acciones">
    {#if coleccion}
      <Button onclick={() => void encolarColeccion()} deshabilitado={pidiendo}>
        {texto('audiolibrosColeccion')}: {coleccion.name}
      </Button>
    {/if}
    {#if alVerPreparados}
      <Button onclick={alVerPreparados}>{texto('audiolibrosVerPreparados')}</Button>
    {/if}
  </div>
</section>

<style>
  .nb-audiolibros {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
  }

  .nb-audiolibros-ayuda {
    margin: 0;
    font-size: var(--nb-text-sm);
    color: var(--nb-text-muted);
  }

  .nb-audiolibros-libro {
    margin: 0;
    font-size: var(--nb-text-md);
    font-weight: 600;
    color: var(--nb-text);
  }

  .nb-audiolibros-avance {
    margin: 0;
    font-size: var(--nb-text-sm);
    color: var(--nb-text);
  }

  .nb-audiolibros-cola {
    margin: 0;
    padding-inline-start: var(--nb-space-4);
    font-size: var(--nb-text-sm);
    color: var(--nb-text-muted);
  }

  .nb-audiolibros-coste {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: var(--nb-space-1) var(--nb-space-2);
    margin: 0;
    font-size: var(--nb-text-sm);
  }

  .nb-audiolibros-coste div {
    display: contents;
  }

  .nb-audiolibros-coste dt {
    color: var(--nb-text-muted);
  }

  .nb-audiolibros-coste dd {
    margin: 0;
    text-align: end;
    color: var(--nb-text);
  }

  .nb-audiolibros-acciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
    align-items: center;
  }

  audio {
    inline-size: 100%;
  }
</style>
