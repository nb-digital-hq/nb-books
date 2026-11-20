<!--
  @component
  Preparar la búsqueda por significado de un documento o de una colección
 .

  **El coste va delante del botón, siempre.** Es el criterio: el
  usuario conoce el disco y el tiempo antes de aceptar. Por eso lo primero que hace
  el componente es pedir la estimación, y "Preparar" no aparece hasta tenerla.
  Nada se prepara solo: Se decidió el 2026-09-16.

  Llama al puente desde aquí, como hace el lector, y no desde quien lo monta: la
  carcasa está en el paquete de arranque, y esto solo lo usan piezas que se cargan
  a demanda.
-->
<script lang="ts">
  import {
    cancelVectors,
    estimateVectors,
    pauseVectors,
    setVectorExclusion,
    startVectors,
    vectorExclusion
  } from '$core/ipc';
  import { idioma } from '$lib/i18n';
  import { traducirError } from '$lib/i18n/errores';
  import { TEXTOS_DE_SIGNIFICADO } from '$lib/i18n/locales/significado';
  import type { VectorEstimate, VectorScope } from '$lib/bindings';

  import Button from '../Button.svelte';
  import Switch from '../Switch.svelte';
  import { preparacion, seguirPreparacion } from './significado.svelte';

  interface Props {
    /** Qué se prepara. */
    alcance: VectorScope;
  }

  let { alcance }: Props = $props();

  const t = $derived(TEXTOS_DE_SIGNIFICADO[idioma()] as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;
  const cifra = $derived(new Intl.NumberFormat(idioma(), { maximumFractionDigits: 1 }));

  let estimacion = $state<VectorEstimate | null>(null);
  let excluido = $state(false);
  let fallo = $state<string | null>(null);

  const tipo = $derived(alcance.scope === 'document' ? 'edition' : 'collection');
  const id = $derived(alcance.scope === 'document' ? alcance.editionId : alcance.collectionId);

  /** El trabajo en marcha, si es de este documento; en una colección, cualquiera. */
  const trabajo = $derived(
    preparacion.trabajo?.running &&
      (alcance.scope === 'collection' || preparacion.trabajo.editionId === id)
      ? preparacion.trabajo
      : null
  );

  async function estimar() {
    try {
      [estimacion, excluido] = await Promise.all([
        estimateVectors(alcance),
        vectorExclusion(tipo, id)
      ]);
      fallo = null;
    } catch (e) {
      fallo = traducirError(e);
    }
  }

  $effect(() => {
    seguirPreparacion();
  });

  // Se vuelve a estimar al cambiar de alcance y cada vez que termina un trabajo:
  // lo que costaba antes de preparar ya no es lo que cuesta después.
  $effect(() => {
    void alcance;
    void preparacion.terminadas;
    void estimar();
  });

  /** Unos bytes, legibles. */
  function disco(bytes: number): string {
    if (bytes < 1024 ** 2) return `${cifra.format(Math.max(1, bytes / 1024))} KB`;
    return `${cifra.format(bytes / 1024 ** 2)} MB`;
  }

  /** Unos segundos, legibles y redondos: es una estimación. */
  function duracion(segundos: number): string {
    if (segundos < 90) return `${Math.max(1, Math.round(segundos))} s`;
    if (segundos < 90 * 60) return `${Math.round(segundos / 60)} min`;
    return `${cifra.format(segundos / 3600)} h`;
  }

  async function hacer(accion: () => Promise<void>) {
    try {
      await accion();
      fallo = null;
    } catch (e) {
      fallo = traducirError(e);
    }
  }
</script>

<section class="nb-significado" aria-label={texto('titulo')}>
  <h3 class="nb-significado-titulo">{texto('titulo')}</h3>

  {#if fallo}
    <p class="nb-significado-fallo" role="alert">{fallo}</p>
  {/if}

  {#if estimacion && !estimacion.modelReady}
    <p class="nb-significado-texto">{texto('sinModelo')}</p>
  {:else if estimacion}
    <div class="nb-significado-fila">
      <span class="nb-significado-texto">{texto('excluir')}</span>
      <Switch
        etiqueta={texto('excluir')}
        activo={excluido}
        alCambiar={(valor) =>
          void hacer(async () => {
            await setVectorExclusion(tipo, id, valor);
            await estimar();
          })}
      />
    </div>

    {#if excluido}
      <p class="nb-significado-texto">{texto('excluido')}</p>
    {:else if trabajo}
      <p class="nb-significado-texto" role="status">
        {texto('preparando')}
        {cifra.format(trabajo.done)}
        {texto('de')}
        {cifra.format(trabajo.total)}
        {texto('fragmentos')}{#if trabajo.queued > 0}, {cifra.format(trabajo.queued)}
          {texto('enCola')}{/if}.
        {#if trabajo.pauseReason}
          {texto(`motivo-${trabajo.pauseReason}`)}
        {/if}
      </p>
      <div
        class="nb-significado-barra"
        role="progressbar"
        aria-valuemin="0"
        aria-valuemax={trabajo.total}
        aria-valuenow={trabajo.done}
        aria-label={texto('preparando')}
      >
        <span style:width="{(trabajo.done / Math.max(trabajo.total, 1)) * 100}%"></span>
      </div>
      <div class="nb-significado-acciones">
        <Button onclick={() => void hacer(() => pauseVectors(!trabajo.paused))}>
          {trabajo.paused ? texto('reanudar') : texto('pausar')}
        </Button>
        <Button onclick={() => void hacer(cancelVectors)}>{texto('cancelar')}</Button>
      </div>
    {:else if estimacion.pending === 0 && estimacion.fragments > 0}
      <p class="nb-significado-texto" role="status">
        {texto('listo')}
        {cifra.format(estimacion.fragments)}
        {texto('fragmentos')}.
      </p>
    {:else if estimacion.documents > 0}
      <!-- El coste, entero y antes del botón: es el criterio. -->
      <p class="nb-significado-texto">
        {texto('costeDisco')}
        <strong>{disco(estimacion.bytes)}</strong>
        {texto('costeTiempo')}
        <strong>{duracion(estimacion.seconds)}</strong>
        {texto('costeProcesador')} ({estimacion.measuredRate
          ? texto('medido')
          : texto('supuesto')}),
        {texto('para')}
        {cifra.format(estimacion.pending)}
        {texto('fragmentos')}{#if alcance.scope === 'collection'}
          {texto('de')} {cifra.format(estimacion.documents)} {texto('documentos')}{/if}.
        {#if estimacion.notIndexed > 0}
          {texto('sinIndexar')}
        {/if}
      </p>
      {#if estimacion.fits}
        <div class="nb-significado-acciones">
          <Button onclick={() => void hacer(() => startVectors(alcance))}
            >{texto('preparar')}</Button
          >
        </div>
      {:else}
        <p class="nb-significado-fallo" role="alert">{texto('noCabe')}</p>
      {/if}
    {/if}
  {/if}
</section>

<style>
  .nb-significado {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding: var(--nb-space-3);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container);
  }

  .nb-significado-titulo {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    font-weight: 600;
  }

  .nb-significado-texto {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    line-height: 1.5;
  }

  .nb-significado-fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-3);
  }

  .nb-significado-fallo {
    margin: 0;
    color: var(--m3c-error);
    font-size: var(--nb-text-sm);
  }

  .nb-significado-acciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
  }

  .nb-significado-barra {
    height: 4px;
    overflow: hidden;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-highest);
  }

  .nb-significado-barra span {
    display: block;
    height: 100%;
    background: var(--m3c-primary);
  }
</style>
