<!--
  @component
  El índice navegable del documento abierto.

  Contrato: docs/architecture/reader-contracts.md, sección 4.
.

  El índice es un árbol —partes, capítulos, secciones— y se pinta como un árbol
  de listas anidadas, no como una lista plana con sangrías: la sangría es una
  pista visual y el anidamiento es información que el lector de pantalla
  anuncia. Cada entrada es un botón, así que el tabulador la alcanza y Enter la
  activa sin que haga falta añadir nada.

  Un documento sin índice no muestra un panel vacío: lo dice.
-->
<script lang="ts">
  import { textos } from '$lib/i18n';
  import type { Locator, NavigationEntry } from '$lib/bindings';

  interface Props {
    entradas: NavigationEntry[];
    /** Locator donde está la lectura, para marcar dónde se está. */
    actual?: Locator;
    alSaltar: (locator: Locator) => void;
    /** Textos. Se traducen. */
    etiquetas?: Partial<{ vacio: string; aqui: string }>;
  }

  let { entradas, actual, alSaltar, etiquetas }: Props = $props();

  const t = $derived({ ...textos().indice, ...etiquetas });

  /*
   * Una sola entrada es "aquí". Comparar solo el recurso marcaba todas las
   * páginas de un PDF (comparten el mismo) y todas las secciones de un capítulo
   * con anclas. Entre las del mismo recurso gana la última que empieza antes de
   * la posición actual; sin posiciones, la primera.
   */
  function aplanar(lista: NavigationEntry[]): NavigationEntry[] {
    return lista.flatMap((e) => [e, ...aplanar(e.children ?? [])]);
  }
  const entradaActual = $derived.by(() => {
    if (!actual) return null;
    const mismas = aplanar(entradas).filter((e) => e.locator.href === actual.href);
    const aqui = actual.locations?.position;
    if (aqui != null) {
      let elegida: NavigationEntry | null = null;
      for (const e of mismas) {
        const p = e.locator.locations?.position;
        if (p != null && p <= aqui) elegida = e;
      }
      if (elegida) return elegida;
    }
    return mismas[0] ?? null;
  });
  function esAqui(entrada: NavigationEntry): boolean {
    return entrada === entradaActual;
  }
</script>

{#snippet rama(lista: NavigationEntry[])}
  <ul>
    {#each lista as entrada, indice (indice)}
      <li>
        <button
          type="button"
          class="nb-entrada"
          aria-current={esAqui(entrada) ? 'true' : undefined}
          onclick={() => alSaltar(entrada.locator)}
        >
          {entrada.title}
          {#if esAqui(entrada)}<span class="nb-aqui">{t.aqui}</span>{/if}
        </button>
        {#if entrada.children.length > 0}
          {@render rama(entrada.children)}
        {/if}
      </li>
    {/each}
  </ul>
{/snippet}

{#if entradas.length === 0}
  <p class="nb-vacio">{t.vacio}</p>
{:else}
  {@render rama(entradas)}
{/if}

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /*
   * Solo se sangran las listas anidadas: sangrar también la raíz gastaría ancho
   * sin decir nada, porque no hay nada de lo que colgar.
   */
  li ul {
    padding-left: var(--nb-space-3);
  }

  .nb-entrada {
    display: block;
    width: 100%;
    padding: var(--nb-space-1) var(--nb-space-2);
    border: none;
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-on-surface);
    font-family: inherit;
    font-size: var(--nb-text-sm);
    text-align: left;
    cursor: pointer;
  }

  .nb-entrada:active {
    background: color-mix(in srgb, var(--m3c-on-surface) 12%, transparent);
  }

  .nb-entrada:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-entrada[aria-current='true'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }

  .nb-aqui {
    display: block;
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
  }

  .nb-vacio {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }
</style>
