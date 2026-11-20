<!--
  @component
  Estado vacío: icono, una frase y una acción. En ese orden.

  Contrato: docs/design/ui-conventions.md, sección 4.

  Es el único sitio de la aplicación donde el texto se centra, y por eso existe
  como componente: centrado repartido por las vistas acaba centrando cosas que
  deberían leerse en columna.

  **Una frase, no dos.** Un estado vacío que se explica en un párrafo es una
  vista mal diseñada con un párrafo encima.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  import Icon from '../Icon.svelte';
  import type { NombreDeIcono } from '../iconos';

  interface Props {
    icono: NombreDeIcono;
    /** Una frase. Se traduce. */
    mensaje: string;
    /** La acción que saca de aquí, si la hay. */
    accion?: Snippet;
  }

  let { icono, mensaje, accion }: Props = $props();
</script>

<div class="nb-vacio">
  <span class="nb-vacio-icono"><Icon nombre={icono} tamano={32} /></span>
  <p class="nb-vacio-mensaje">{mensaje}</p>
  {#if accion}
    <div>{@render accion()}</div>
  {/if}
</div>

<style>
  .nb-vacio {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--nb-space-3);
    height: 100%;
    min-height: 12rem;
    padding: var(--nb-space-6);
    text-align: center;
    color: var(--m3c-on-surface-variant);
  }

  .nb-vacio-icono {
    color: var(--m3c-on-surface-variant);
  }

  .nb-vacio-mensaje {
    max-width: 32ch;
    margin: 0;
    font-size: var(--nb-text-sm);
  }
</style>
