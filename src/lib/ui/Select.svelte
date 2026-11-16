<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';

  interface Props {
    etiqueta: string;
    valor: string;
    opciones: ReadonlyArray<readonly [string, string]>;
    alCambiar: (valor: string) => void;
    prefijo?: string;
    id?: string;
    alineacion?: 'izquierda' | 'derecha';
  }

  let {
    etiqueta,
    valor,
    opciones,
    alCambiar,
    prefijo,
    id,
    alineacion = 'izquierda'
  }: Props = $props();

  const visible = $derived(opciones.find(([v]) => v === valor)?.[1] ?? valor);
  let abierto = $state(false);
  let contenedor: HTMLElement;

  function toggle() {
    abierto = !abierto;
  }

  function seleccionar(v: string) {
    alCambiar(v);
    abierto = false;
  }

  function clickFuera(e: MouseEvent) {
    if (abierto && contenedor && !contenedor.contains(e.target as Node)) {
      abierto = false;
    }
  }

  function alPulsarTecla(e: KeyboardEvent) {
    if (!abierto) {
      if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown') {
        e.preventDefault();
        abierto = true;
      }
      return;
    }

    if (e.key === 'Escape') {
      abierto = false;
      return;
    }

    const index = opciones.findIndex(([v]) => v === valor);
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      const next = opciones[Math.min(index + 1, opciones.length - 1)];
      if (next) alCambiar(next[0]);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      const prev = opciones[Math.max(index - 1, 0)];
      if (prev) alCambiar(prev[0]);
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      abierto = false;
    }
  }

  onMount(() => {
    document.addEventListener('click', clickFuera);
    return () => document.removeEventListener('click', clickFuera);
  });
</script>

<div class="nb-select" bind:this={contenedor} onkeydown={alPulsarTecla} role="presentation">
  <button
    {id}
    class="nb-select-cara"
    type="button"
    aria-haspopup="listbox"
    aria-expanded={abierto}
    aria-label={etiqueta}
    onclick={toggle}
  >
    {#if prefijo}<span class="nb-select-prefijo">{prefijo}</span>{/if}
    <span class="nb-select-valor">{visible}</span>
    <Icon nombre="abajo" tamano={14} />
  </button>

  {#if abierto}
    <ul class="nb-select-menu" data-alineacion={alineacion} role="listbox" aria-label={etiqueta}>
      {#each opciones as [v, texto]}
        <li>
          <button
            class="nb-select-opcion"
            type="button"
            role="option"
            aria-selected={v === valor}
            onclick={() => seleccionar(v)}
          >
            {texto}
            {#if v === valor}
              <Icon nombre="comprobado" tamano={14} />
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .nb-select {
    position: relative;
    display: inline-flex;
  }

  /* El valor elegido se lee con el color de texto pleno; el prefijo, apagado. */
  .nb-select-valor {
    color: var(--m3c-on-surface);
  }

  .nb-select-cara {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    min-height: 2.5rem;
    padding: 0 var(--nb-space-3) 0 var(--nb-space-4);
    border: none;
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
    font-weight: 500;
    white-space: nowrap;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-select-prefijo {
    color: var(--m3c-on-surface-variant);
  }

  .nb-select-cara:hover {
    background: var(--m3c-surface-container-highest);
  }

  .nb-select-cara:focus-visible {
    outline: 2px solid var(--nb-focus);
    outline-offset: 2px;
  }

  /*
   * Opaco y sin desenfoque: el menú se apoya sobre la biblioteca, que se desplaza,
   * y lo separa su capa y la sombra de su altura, como a los menús de la barra.
   */
  .nb-select-menu {
    position: absolute;
    top: calc(100% + var(--nb-space-1));
    z-index: 50;
    display: flex;
    flex-direction: column;
    min-width: 10rem;
    margin: 0;
    padding: var(--nb-space-2);
    list-style: none;
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    box-shadow: var(--m3-elevation-2);
  }

  .nb-select-menu[data-alineacion='izquierda'] {
    left: 0;
  }

  .nb-select-menu[data-alineacion='derecha'] {
    right: 0;
  }

  .nb-select-opcion {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    min-height: 2.5rem;
    padding: 0 var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-sm);
    background: transparent;
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
    text-align: left;
    cursor: pointer;
    transition: none;
  }

  .nb-select-opcion:hover,
  .nb-select-opcion:focus-visible {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-select-opcion[aria-selected='true'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font-weight: 600;
  }
</style>
