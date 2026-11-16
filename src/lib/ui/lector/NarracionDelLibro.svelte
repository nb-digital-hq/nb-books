<!--
  @component
  La narración que trae el propio libro: escucharla siguiendo el texto.

  # Por qué va en el panel de voz y no en un sitio propio

  Porque es la misma pregunta -"léemelo"- con una respuesta mejor: la leyó una
  persona. Va **encima** de la voz sintética y marcada como la recomendada, sin
  esconder la otra (principio 11 de la filosofía: recomendar no es obligar). No
  añade un destino ni un modo, que es la regla de la Fase 6.

  El audio lo suena el reproductor de la página, el mismo que lo preparado: sigue
  con el panel cerrado, mueve el lector a cada tramo y el lector resalta el bloque
  que suena igual que con la voz. Qué capítulo suena y cuándo se pasa al siguiente
  vive en `narracion.svelte.ts`, fuera del paquete de arranque.

  Contrato: docs/architecture/reader-contracts.md, sección 5.5.
-->
<script lang="ts">
  import { idioma } from '$lib/i18n';
  import { TEXTOS_DE_AUDIO } from '$lib/i18n/locales/audio';
  import type { Locator, NavigationEntry } from '$lib/bindings';
  import type { ReproductorDePreparado } from '$lib/voz/reproductor.svelte';

  import Button from '../Button.svelte';
  import { delDocumento, escuchar, narracion } from './narracion.svelte';

  interface Props {
    reproductor: ReproductorDePreparado;
    fileId: string;
    entradas: readonly NavigationEntry[];
    /** Por dónde se va leyendo, para empezar ahí. */
    desde: Locator | null;
    /** Lleva el lector a un sitio: el capítulo siguiente, al acabar uno. */
    alIr: (locator: Locator) => void;
    /** Antes de sonar: calla la voz en vivo. */
    alEmpezar: () => void;
  }

  let { reproductor, fileId, entradas, desde, alIr, alEmpezar }: Props = $props();

  $effect(() => delDocumento(fileId));

  const cargada = $derived(narracion.href !== null && reproductor.preparado !== null);

  const t = $derived(TEXTOS_DE_AUDIO[idioma()] as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;
</script>

<section class="nb-narracion" aria-label={texto('narracionTitulo')}>
  <h3 class="nb-narracion-titulo">{texto('narracionTitulo')}</h3>
  <p class="nb-narracion-ayuda">{texto('narracionAyuda')}</p>
  <div class="nb-narracion-acciones">
    {#if cargada}
      <Button variante="primario" onclick={() => reproductor.alternar()}>
        {reproductor.sonando ? texto('narracionPausar') : texto('narracionSeguir')}
      </Button>
    {/if}
    <Button
      variante={cargada ? 'secundario' : 'primario'}
      onclick={() => {
        if (desde) void escuchar({ reproductor, fileId, entradas, alIr, alEmpezar }, desde);
      }}
    >
      {texto('narracionDesdeAqui')}
    </Button>
  </div>
  {#if narracion.sinCapitulo}
    <p class="nb-narracion-ayuda" role="status">{texto('narracionSinCapitulo')}</p>
  {/if}
  {#if narracion.terminada}
    <p class="nb-narracion-ayuda" role="status">{texto('narracionTerminada')}</p>
  {/if}
  {#if cargada && reproductor.fallo}
    <p class="nb-narracion-fallo" role="alert">{texto('narracionNoSuena')}</p>
  {/if}
</section>

<style>
  .nb-narracion {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding-block-end: var(--nb-space-3);
    border-block-end: 1px solid var(--nb-border);
  }

  .nb-narracion-titulo {
    margin: 0;
    font-size: var(--nb-text-md);
    font-weight: 600;
    color: var(--nb-text);
  }

  .nb-narracion-ayuda {
    margin: 0;
    font-size: var(--nb-text-sm);
    color: var(--nb-text-muted);
  }

  .nb-narracion-acciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
  }

  .nb-narracion-fallo {
    margin: 0;
    font-size: var(--nb-text-sm);
    color: var(--nb-danger);
  }
</style>
