<!--
  @component
  Anclas alfabéticas de la biblioteca: una fila de iniciales que lleva a la página
  donde empieza cada una. Se carga aparte porque solo aparece con bibliotecas
  grandes ordenadas por título o autor.
-->
<script lang="ts">
  import { anclasAlfabeticas } from '$core/ipc/ficha';
  import type { BrowseQuery, LetterAnchor } from '$lib/bindings';

  interface Props {
    consulta: BrowseQuery;
    porPagina: number;
    pagina: number;
    etiqueta: string;
    alIr: (pagina: number) => void;
  }

  let { consulta, porPagina, pagina, etiqueta, alIr }: Props = $props();

  // Se piden de nuevo al cambiar el orden o el filtro; la página no las cambia.
  let anclas = $state<LetterAnchor[]>([]);
  $effect(() => {
    anclasAlfabeticas({ ...consulta, offset: 0, limit: 0 })
      .then((a) => (anclas = a))
      .catch(() => (anclas = []));
  });
  const paginaDe = (ancla: LetterAnchor) => Math.floor(ancla.offset / porPagina) + 1;
</script>

{#if anclas.length > 1}
  <nav class="nb-anclas" aria-label={etiqueta}>
    {#each anclas as ancla (ancla.letter)}
      <button
        class="nb-ancla"
        type="button"
        aria-current={paginaDe(ancla) === pagina ? 'true' : undefined}
        onclick={() => alIr(paginaDe(ancla))}>{ancla.letter}</button
      >
    {/each}
  </nav>
{/if}

<style>
  .nb-anclas {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-1);
    padding: 0 var(--nb-space-4) var(--nb-space-2);
  }

  .nb-ancla {
    min-width: 2rem;
    min-height: 2rem;
    border: none;
    border-radius: var(--nb-radius-sm);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    font: inherit;
    font-size: var(--nb-text-sm);
    font-weight: 600;
    cursor: pointer;
  }

  .nb-ancla:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-ancla[aria-current='true'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }
</style>
