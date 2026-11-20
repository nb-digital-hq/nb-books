<!--
  @component
  Una tarjeta viva de la barra lateral, que se puede plegar.

  Contrato: docs/design/ui-conventions.md, sección 4..

  **Por qué se pliega.** Las tarjetas son dinámicas —aparecen cuando dicen algo y
  se van cuando dejan de decirlo—, pero algunas dicen lo mismo durante horas: la
  del índice de búsqueda estuvo el 2026-09-12 una tarde entera diciendo
  "detenido" con la misma cifra. En uso real: "esas cosas dinámicas que ocupan
  espacio se deberían poder achicar o expandir porque incomodan a veces".

  **Plegada no desaparece**: queda en una línea, con su icono, su nombre y su
  cifra. Se pliega lo que ya se sabe, no se esconde lo que pasa, y si la cifra
  cambia se sigue viendo.

  **Dos botones hermanos, no uno dentro de otro.** La tarjeta entera lleva a su
  sitio, y plegar es otro gesto: un botón dentro de otro no es válido, y el lector
  de pantalla no sabría cuál se pulsa. El de plegar va encima de la esquina, fuera
  del botón grande.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  import Icon from './Icon.svelte';
  import type { NombreDeIcono } from './iconos';

  interface Props {
    /** Icono de la cabecera. Sin él, la cabecera es solo texto. */
    icono?: NombreDeIcono;
    titulo: string;
    /** Lo corto que se ve también plegada: un porcentaje, "Pausado". */
    cifra?: string;
    plegada?: boolean;
    /** Nombres accesibles del botón de plegar, en los dos sentidos. */
    etiquetaPlegar: string;
    etiquetaDesplegar: string;
    alAlternar: (plegada: boolean) => void;
    /** Lo que hace pulsar la tarjeta. */
    onclick: () => void;
    /** Lo que se ve desplegada, debajo de la cabecera. */
    children?: Snippet;
  }

  let {
    icono,
    titulo,
    cifra,
    plegada = false,
    etiquetaPlegar,
    etiquetaDesplegar,
    alAlternar,
    onclick,
    children
  }: Props = $props();
</script>

<div class="nb-tarjeta-lateral" data-plegada={plegada}>
  <button class="nb-tarjeta-lateral-principal" type="button" {onclick}>
    <span class="nb-tarjeta-lateral-cabecera">
      {#if icono}<Icon nombre={icono} tamano={18} />{/if}
      <span class="nb-tarjeta-lateral-titulo">{titulo}</span>
      {#if cifra}<span class="nb-tarjeta-lateral-cifra">{cifra}</span>{/if}
    </span>
    {#if !plegada && children}
      {@render children()}
    {/if}
  </button>
  <button
    class="nb-tarjeta-lateral-plegar"
    type="button"
    aria-label={plegada ? etiquetaDesplegar : etiquetaPlegar}
    title={plegada ? etiquetaDesplegar : etiquetaPlegar}
    aria-expanded={!plegada}
    onclick={() => alAlternar(!plegada)}
  >
    <Icon nombre={plegada ? 'abajo' : 'arriba'} tamano={16} />
  </button>
</div>

<style>
  .nb-tarjeta-lateral {
    position: relative;
  }

  /*
   * La misma forma para todas las tarjetas, para que la que se añada mañana no
   * sea otra cosa: la capa de encima de la barra, sin borde.
   */
  .nb-tarjeta-lateral-principal {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    width: 100%;
    padding: var(--nb-space-3) var(--nb-space-4);
    border: none;
    border-radius: var(--nb-radius-lg);
    background: var(--m3c-surface-container);
    color: var(--m3c-on-surface);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-tarjeta-lateral-principal:hover {
    background: var(--m3c-surface-container-high);
  }

  .nb-tarjeta-lateral[data-plegada='true'] .nb-tarjeta-lateral-principal {
    padding-block: var(--nb-space-2);
  }

  .nb-tarjeta-lateral-cabecera {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    min-height: 1.75rem;
    /* Sitio para el botón de plegar, que va encima de esta esquina. */
    padding-inline-end: var(--nb-space-6);
  }

  .nb-tarjeta-lateral-titulo {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: var(--nb-text-sm);
    font-weight: 600;
  }

  .nb-tarjeta-lateral-cifra {
    flex: 0 0 auto;
    color: var(--m3c-primary);
    font-size: var(--nb-text-xs);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .nb-tarjeta-lateral-plegar {
    position: absolute;
    top: var(--nb-space-2);
    right: var(--nb-space-2);
    display: grid;
    place-items: center;
    width: 1.75rem;
    height: 1.75rem;
    padding: 0;
    border: none;
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-tarjeta-lateral[data-plegada='true'] .nb-tarjeta-lateral-plegar {
    top: calc(var(--nb-space-2) - 2px);
  }

  .nb-tarjeta-lateral-plegar:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 10%, transparent);
    color: var(--m3c-on-surface);
  }
</style>
