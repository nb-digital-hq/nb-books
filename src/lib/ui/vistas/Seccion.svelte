<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    /** En formato de título. Se traduce. */
    titulo: string;
    children: Snippet;
    /** Acción de la sección, alineada a la derecha del título. */
    accion?: Snippet;
  }

  let { titulo, children, accion }: Props = $props();
</script>

<section class="nb-seccion">
  <div class="nb-seccion-cabecera">
    <h2 class="nb-seccion-titulo">{titulo}</h2>
    {#if accion}
      <div>{@render accion()}</div>
    {/if}
  </div>
  {@render children()}
</section>

<style>
  .nb-seccion {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-4);
  }

  .nb-seccion-cabecera {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-4);
    padding: 0 var(--nb-space-1);
  }

  .nb-seccion-titulo {
    margin: 0;
    font-family: var(--nb-font-ui);
    font-size: var(--nb-text-lg);
    font-weight: 400;
    line-height: 1.75rem;
    color: var(--m3c-on-surface);
  }
</style>
