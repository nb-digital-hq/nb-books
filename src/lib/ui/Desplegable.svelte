<!--
  @component
  Un botón de la barra superior que abre su emergente debajo.

  Contrato: docs/design/ui-conventions.md, sección 4.1..

  Es la forma de Importar, Colecciones y Estanterías: los tres abren algo que se
  consulta y se cierra. El que lleva la acción principal de la pantalla va con el
  contenedor del acento, y el abierto se marca con el color y con la flecha, que
  se da la vuelta. `aria-expanded` es lo que se lo dice a quien no ve ninguna de
  las dos cosas.

  Cuando la barra no tiene sitio para el texto se queda en el icono, y el nombre
  pasa al atributo: desaparece la palabra, nunca el control.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  import Emergente from './Emergente.svelte';
  import Icon from './Icon.svelte';
  import type { NombreDeIcono } from './iconos';

  interface Props {
    /** Lo que se lee en el botón. */
    texto: string;
    /**
     * Nombre accesible, cuando dice más que el texto. Tiene que **empezar** por
     * el texto visible: quien usa control por voz dice lo que ve.
     */
    etiqueta?: string;
    /** Nombre de la zona que se abre. */
    titulo: string;
    icono: NombreDeIcono;
    abierto?: boolean;
    /** La acción principal de la barra. */
    principal?: boolean;
    /** Sin sitio para el texto. */
    soloIcono?: boolean;
    ancho?: string;
    alAlternar?: (abierto: boolean) => void;
    children: Snippet;
  }

  let {
    texto,
    etiqueta,
    titulo,
    icono,
    abierto = $bindable(false),
    principal = false,
    soloIcono = false,
    ancho,
    alAlternar,
    children
  }: Props = $props();

  const id = $props.id();
  let contenedor: HTMLElement | undefined = $state();
  let boton: HTMLButtonElement | undefined = $state();

  function cambiar(valor: boolean) {
    abierto = valor;
    alAlternar?.(valor);
  }
</script>

<div class="nb-desplegable" bind:this={contenedor}>
  <button
    bind:this={boton}
    class="nb-desplegable-boton"
    type="button"
    data-principal={principal}
    data-solo-icono={soloIcono}
    aria-expanded={abierto}
    aria-controls={abierto ? id : undefined}
    aria-label={soloIcono ? (etiqueta ?? texto) : etiqueta}
    title={soloIcono ? (etiqueta ?? texto) : undefined}
    onclick={() => cambiar(!abierto)}
  >
    <Icon nombre={icono} tamano={20} />
    {#if !soloIcono}
      <span class="nb-desplegable-texto">{texto}</span>
      <Icon nombre={abierto ? 'arriba' : 'abajo'} tamano={18} />
    {/if}
  </button>

  {#if abierto}
    <Emergente
      {id}
      {titulo}
      {ancho}
      ancla={contenedor}
      alCerrar={(motivo) => {
        cambiar(false);
        if (motivo === 'tecla') boton?.focus();
      }}
    >
      {@render children()}
    </Emergente>
  {/if}
</div>

<style>
  .nb-desplegable {
    position: relative;
    flex: 0 0 auto;
  }

  .nb-desplegable-boton {
    display: inline-flex;
    align-items: center;
    gap: var(--nb-space-2);
    min-height: 3rem;
    padding: 0 var(--nb-space-3) 0 var(--nb-space-4);
    border: none;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-sm);
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background var(--nb-motion-fast) var(--nb-easing),
      color var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-desplegable-boton:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, var(--m3c-surface-container-high));
  }

  .nb-desplegable-boton[aria-expanded='true'] {
    background: var(--m3c-primary-container);
    color: var(--m3c-on-primary-container);
  }

  .nb-desplegable-boton[data-principal='true'] {
    background: var(--m3c-primary-container);
    color: var(--m3c-on-primary-container);
    box-shadow: var(--m3-elevation-1);
  }

  .nb-desplegable-boton[data-principal='true']:hover {
    background: color-mix(
      in srgb,
      var(--m3c-on-primary-container) 8%,
      var(--m3c-primary-container)
    );
  }

  .nb-desplegable-boton[data-principal='true'][aria-expanded='true'] {
    background: var(--m3c-primary);
    color: var(--m3c-on-primary);
  }

  .nb-desplegable-boton[data-solo-icono='true'] {
    justify-content: center;
    width: 3rem;
    padding: 0;
  }

  .nb-desplegable-texto {
    line-height: 1;
  }
</style>
