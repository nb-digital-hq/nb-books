<!--
  @component
  Los nombres del libro, con dónde aparecen: local y sin IA.

  Va en el panel del índice, plegado: es otra forma de encontrar un sitio del
  libro -"¿quién era este?"- y no un destino. **Se calcula al desplegarlo**, no al
  abrir el panel: recorre el libro entero la primera vez, y quien abre el índice
  para ir al capítulo tres no lo ha pedido.

  Contrato: `src-tauri/src/entidades.rs`, que dice qué hace y qué no.
-->
<script lang="ts">
  import { bookEntities, entityPlaces } from '$core/ipc/lector';
  import type { Entity, EntityIndex, EntityPlaces, Locator } from '$lib/bindings';
  import { idioma } from '$lib/i18n';
  import { TEXTOS_DEL_LECTOR } from '$lib/i18n/locales/lector';

  import Spinner from '../Spinner.svelte';
  import Icon from '../Icon.svelte';

  interface Props {
    fileId: string;
    alSaltar: (locator: Locator) => void;
  }

  let { fileId, alSaltar }: Props = $props();

  const t = $derived(TEXTOS_DEL_LECTOR[idioma()] as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;
  const cifra = $derived(new Intl.NumberFormat(idioma()));

  let abierto = $state(false);
  let indice = $state<EntityIndex | null>(null);
  let cargando = $state(false);
  let sinTexto = $state(false);
  /** El nombre desplegado, para enseñar sus sitios. */
  let elegido = $state<string | null>(null);
  /** Los sitios del desplegado, según el índice de búsqueda si está. */
  let sitios = $state<EntityPlaces | null>(null);

  // Otro libro, otros nombres: lo de antes no dice nada del nuevo.
  $effect(() => {
    void fileId;
    indice = null;
    elegido = null;
    sitios = null;
    sinTexto = false;
  });

  $effect(() => {
    if (!abierto || indice || cargando || sinTexto) return;
    cargando = true;
    const pedido = fileId;
    bookEntities(pedido)
      .then((respuesta) => {
        if (pedido === fileId) indice = respuesta;
      })
      .catch(() => {
        if (pedido === fileId) sinTexto = true;
      })
      .finally(() => {
        cargando = false;
      });
  });

  /** Lo que se ve de un sitio: el capítulo y el arranque del fragmento. */
  function sitio(locator: Locator): string {
    const cita = locator.text?.highlight?.trim() ?? '';
    return [locator.title, cita ? `«${cita}…»` : null].filter(Boolean).join(' · ');
  }

  /**
   * Despliega un nombre y le pregunta al índice dónde aparece. Mientras contesta se
   * enseñan los sitios del recorrido, que ya están: el índice los mejora, no es
   * condición para tenerlos.
   */
  function alternar(entidad: Entity) {
    if (elegido === entidad.name) {
      elegido = null;
      return;
    }
    elegido = entidad.name;
    sitios = { places: entidad.places, fromIndex: false };
    const pedido = entidad.name;
    entityPlaces(fileId, pedido)
      .then((respuesta) => {
        if (elegido === pedido) sitios = respuesta;
      })
      .catch(() => undefined);
  }
</script>

<details class="nb-nombres" bind:open={abierto}>
  <summary class="nb-nombres-titulo">
    <!-- El triángulo del navegador es un carácter haciendo de icono. -->
    <span class="nb-nombres-chevron" data-abierto={abierto}
      ><Icon nombre="abajo" tamano={16} /></span
    >
    {texto('nombresTitulo')}
  </summary>
  <p class="nb-nombres-ayuda">{texto('nombresAyuda')}</p>
  {#if cargando}
    <Spinner etiqueta={texto('nombresCargando')} />
  {:else if sinTexto}
    <p class="nb-nombres-ayuda" role="status">{texto('nombresSinTexto')}</p>
  {:else if indice}
    {#if indice.entities.length === 0}
      <p class="nb-nombres-ayuda">{texto('nombresVacio')}</p>
    {:else}
      <ul class="nb-nombres-lista">
        {#each indice.entities as entidad (entidad.name)}
          <li>
            <button
              type="button"
              class="nb-nombre"
              aria-expanded={elegido === entidad.name}
              onclick={() => alternar(entidad)}
            >
              <span>{entidad.name}</span>
              <span class="nb-nombre-cuenta">
                {cifra.format(entidad.mentions)}
                {texto('nombresMenciones')}
              </span>
            </button>
            {#if elegido === entidad.name && sitios}
              {#if !sitios.fromIndex}
                <p class="nb-nombres-ayuda" role="status">{texto('nombresSinIndice')}</p>
              {/if}
              <ul class="nb-sitios">
                {#each sitios.places as lugar, i (i)}
                  <li>
                    <button
                      type="button"
                      class="nb-sitio"
                      aria-label={`${texto('nombresSitio')}: ${sitio(lugar)}`}
                      onclick={() => alSaltar(lugar)}
                    >
                      {sitio(lugar)}
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</details>

<style>
  .nb-nombres {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding-block-end: var(--nb-space-3);
    margin-block-end: var(--nb-space-3);
    border-block-end: 1px solid var(--nb-border);
  }

  .nb-nombres-titulo {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    list-style: none;
    font-size: var(--nb-text-md);
    font-weight: 600;
    color: var(--m3c-on-surface);
    cursor: pointer;
  }

  .nb-nombres-ayuda {
    margin: var(--nb-space-1) 0;
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
  }

  .nb-nombres-lista,
  .nb-sitios {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-sitios {
    padding-inline-start: var(--nb-space-3);
  }

  .nb-nombre,
  .nb-sitio {
    display: flex;
    justify-content: space-between;
    gap: var(--nb-space-2);
    inline-size: 100%;
    padding: var(--nb-space-1) var(--nb-space-2);
    border: none;
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-on-surface);
    font-family: inherit;
    font-size: var(--nb-text-sm);
    text-align: start;
    cursor: pointer;
  }

  .nb-sitio {
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
  }

  .nb-nombre:hover,
  .nb-sitio:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-nombre[aria-expanded='true'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }

  .nb-nombre-cuenta {
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }
  .nb-nombres-titulo::-webkit-details-marker {
    display: none;
  }

  .nb-nombres-chevron {
    display: inline-flex;
    transition: transform var(--nb-motion-fast) var(--nb-easing);
    transform: rotate(-90deg);
  }

  .nb-nombres-chevron[data-abierto='true'] {
    transform: none;
  }
</style>
