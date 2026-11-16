<!--
  @component
  Un destino de la barra lateral.

  Contrato: docs/design/ui-conventions.md y ADR-013.

  El mismo elemento sirve plegado y desplegado: **plegado desaparece el texto,
  nunca el destino**. Plegado, el nombre pasa al atributo y a una etiqueta emergente
  (`Tooltip`) accesible por teclado y puntero fuera de la barra que recorta,
  lo que hace que la barra plegada siga diciendo lo mismo a un lector de pantalla que
  la desplegada sin depender de un `title` nativo.

  El destino activo se marca con `aria-current`, que es lo que un lector de
  pantalla anuncia como "página actual". Un color no dice nada a quien no lo ve.

  Tiene la forma de Material 3: una píldora de tres unidades de
  alto, y el activo sobre el contenedor del acento. El dato de la derecha
  —cuántas obras hay— es para la vista y va fuera del nombre accesible: el
  destino se llama Biblioteca, no "Biblioteca 151 mil".
-->
<script lang="ts">
  import Icon from './Icon.svelte';
  import Tooltip from './Tooltip.svelte';
  import type { NombreDeIcono } from './iconos';

  interface Props {
    icono: NombreDeIcono;
    /** Nombre del destino, en formato de título. */
    etiqueta: string;
    activo?: boolean;
    /** Solo el icono. Lo decide la carcasa, no este componente. */
    plegado?: boolean;
    /** Contador discreto: documentos pendientes, notas nuevas. */
    insignia?: number;
    /** Un dato corto a la derecha, como cuántas obras hay. */
    extra?: string;
    onclick?: () => void;
  }

  let {
    icono,
    etiqueta,
    activo = false,
    plegado = false,
    insignia,
    extra,
    onclick
  }: Props = $props();
</script>

{#if plegado}
  <Tooltip text={etiqueta} position="right">
    {#snippet children({ describedBy })}
      <button
        class="nb-destino"
        type="button"
        aria-current={activo ? 'page' : undefined}
        aria-label={etiqueta}
        aria-describedby={describedBy}
        data-plegado={true}
        {onclick}
      >
        <Icon nombre={icono} />
      </button>
    {/snippet}
  </Tooltip>
{:else}
  <button
    class="nb-destino"
    type="button"
    aria-current={activo ? 'page' : undefined}
    data-plegado={false}
    {onclick}
  >
    <Icon nombre={icono} />
    <span class="nb-destino-texto">{etiqueta}</span>
    {#if extra}
      <span class="nb-destino-extra" aria-hidden="true">{extra}</span>
    {:else if insignia !== undefined && insignia > 0}
      <span class="nb-destino-extra">{insignia}</span>
    {/if}
  </button>
{/if}

<style>
  .nb-destino {
    display: flex;
    align-items: center;
    gap: var(--nb-space-3);
    width: 100%;
    min-height: 3rem;
    padding: 0 var(--nb-space-4);
    border: none;
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    font-family: inherit;
    font-size: var(--nb-text-sm);
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition:
      background var(--nb-motion-fast) var(--nb-easing),
      color var(--nb-motion-fast) var(--nb-easing);
  }

  /*
   * Plegado, un círculo de 2,75 rem: 44 px, el objetivo mínimo de WCAG 2.5.5. Es
   * lo más pequeño que puede ser sin dejar de ser fácil de acertar, y así cabe en
   * la barra plegada con aire a los lados en lugar de salirse de ella.
   */
  .nb-destino[data-plegado='true'] {
    flex: 0 0 auto;
    justify-content: center;
    width: 2.75rem;
    min-height: 2.75rem;
    margin: 0 auto;
    padding: 0;
  }

  .nb-destino:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
    color: var(--m3c-on-surface);
  }

  .nb-destino:active {
    background: color-mix(in srgb, var(--m3c-on-surface) 12%, transparent);
  }

  .nb-destino[aria-current='page'] {
    background: var(--m3c-primary-container);
    color: var(--m3c-on-primary-container);
    font-weight: 600;
  }

  /*
   * El nombre no se desplaza al pasar el ratón: tenía una marquesina en bucle
   * infinito, que es lo que prohíbe la sección 12 de `ui-conventions.md`, y la
   * rechaza `check-ui.mjs`. Si un nombre no cabe, la respuesta es plegar la
   * barra, que ya lo pone en el atributo.
   */
  .nb-destino-texto {
    flex: 1 1 auto;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .nb-destino-extra {
    flex: 0 0 auto;
    font-size: var(--nb-text-xs);
    font-variant-numeric: tabular-nums;
    color: var(--m3c-on-surface-variant);
  }

  .nb-destino[aria-current='page'] .nb-destino-extra {
    color: inherit;
  }
</style>
