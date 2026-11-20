<!--
  @component
  El mapa del libro: el libro entero de un vistazo.

  Una franja de principio a fin con los capítulos por tramos, lo que va de lectura,
  dónde se está y las anotaciones. Va arriba del índice, en el mismo panel: no
  añade un destino ni un modo, que es la regla de la Fase 6, y el índice es donde
  ya se va a buscar "dónde estoy en el libro".

  **Cada marca lleva a su sitio por `Locator`**, nunca por porcentaje: el
  porcentaje solo decide dónde se dibuja. Con muchas anotaciones juntas se reúnen,
  y el grupo lleva a la primera y dice cuántas son (ver `mapa.ts`).

  **Con el teclado**: cada fila es un solo punto de tabulación, y las flechas la
  recorren. Cien botones con el tabulador serían cien pulsaciones para cruzar el
  panel.

  Contrato: ADR-004 y `docs/design/reading-experience/README.md`, sección 2.2.
-->
<script lang="ts">
  import { porcentajeLeido } from '$core/progreso';
  import type { AnnotationKind, Locator, NavigationEntry, OutlineEntry } from '$lib/bindings';
  import { idioma } from '$lib/i18n';
  import { TEXTOS_DEL_LECTOR } from '$lib/i18n/locales/lector';

  import { type Grupo, mapaDelLibro, type Tramo } from './mapa';

  interface Props {
    entradas: readonly NavigationEntry[];
    anotaciones: readonly OutlineEntry[];
    /** Dónde está la lectura. */
    actual: Locator | null;
    alSaltar: (locator: Locator) => void;
  }

  let { entradas, anotaciones, actual, alSaltar }: Props = $props();

  const t = $derived(TEXTOS_DEL_LECTOR[idioma()] as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  const mapa = $derived(mapaDelLibro(entradas, anotaciones, actual));

  /** Lo que dice un grupo en voz alta: "2 resaltados y 1 nota". */
  function descripcion(grupo: Grupo): string {
    const partes = (Object.entries(grupo.cuantas) as [AnnotationKind, number][]).map(
      ([clase, n]) => {
        const nombre = clase.charAt(0).toUpperCase() + clase.slice(1);
        return `${n} ${texto(n === 1 ? `mapa${nombre}` : `mapa${nombre}s`)}`;
      }
    );
    return new Intl.ListFormat(idioma(), { type: 'conjunction' }).format(partes);
  }

  function etiquetaDeTramo(tramo: Tramo): string {
    const reunidos =
      tramo.cuantos > 1 ? ` ${texto('mapaReunidos').replace('{n}', `${tramo.cuantos - 1}`)}` : '';
    return `${tramo.titulo}${reunidos}${tramo.actual ? ` · ${texto('mapaAqui')}` : ''}`;
  }

  /*
   * Un solo punto de tabulación por fila: el primero de la fila, o el actual en la
   * de capítulos. Las flechas mueven el foco entre los de la misma fila.
   */
  function recorrer(evento: KeyboardEvent) {
    if (evento.key !== 'ArrowRight' && evento.key !== 'ArrowLeft') return;
    const fila = (evento.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>(
      'button'
    );
    const botones = Array.from(fila);
    const ahora = botones.indexOf(document.activeElement as HTMLButtonElement);
    if (ahora < 0) return;
    evento.preventDefault();
    const siguiente =
      evento.key === 'ArrowRight'
        ? Math.min(botones.length - 1, ahora + 1)
        : Math.max(0, ahora - 1);
    botones[siguiente]?.focus();
  }

  const indiceDelActual = $derived(
    Math.max(
      0,
      mapa.tramos.findIndex((tramo) => tramo.actual)
    )
  );
</script>

<section class="nb-mapa" aria-label={texto('mapaTitulo')}>
  <h3 class="nb-mapa-titulo">
    {texto('mapaTitulo')}
    {#if mapa.leido !== null}
      <span class="nb-mapa-leido">{porcentajeLeido(mapa.leido)} % {texto('mapaLeido')}</span>
    {/if}
  </h3>

  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    class="nb-mapa-fila nb-mapa-capitulos"
    role="toolbar"
    aria-label={texto('mapaCapitulos')}
    onkeydown={recorrer}
  >
    {#each mapa.tramos as tramo, i (i)}
      <button
        type="button"
        class="nb-mapa-tramo"
        style:left="{tramo.desde * 100}%"
        style:width="{Math.max(0.4, (tramo.hasta - tramo.desde) * 100)}%"
        data-par={i % 2 === 0}
        aria-current={tramo.actual ? 'true' : undefined}
        aria-label={etiquetaDeTramo(tramo)}
        title={etiquetaDeTramo(tramo)}
        tabindex={i === indiceDelActual ? 0 : -1}
        onclick={() => alSaltar(tramo.locator)}
      ></button>
    {/each}
    {#if mapa.leido !== null}
      <span class="nb-mapa-hecho" style:width="{mapa.leido * 100}%" aria-hidden="true"></span>
      <span class="nb-mapa-aqui" style:left="{mapa.leido * 100}%" aria-hidden="true"></span>
    {/if}
  </div>

  {#if mapa.grupos.length === 0}
    <p class="nb-mapa-ayuda">{texto('mapaSinMarcas')}</p>
  {:else}
    <!-- svelte-ignore a11y_interactive_supports_focus -->
    <div
      class="nb-mapa-fila nb-mapa-marcas"
      role="toolbar"
      aria-label={texto('mapaMarcas')}
      onkeydown={recorrer}
    >
      {#each mapa.grupos as grupo, i (i)}
        <button
          type="button"
          class="nb-mapa-marca"
          data-clase={grupo.clase}
          style:left="{grupo.posicion * 100}%"
          aria-label={descripcion(grupo)}
          title={descripcion(grupo)}
          tabindex={i === 0 ? 0 : -1}
          onclick={() => alSaltar(grupo.locator)}
        ></button>
      {/each}
    </div>
  {/if}
  <p class="nb-mapa-ayuda">{texto('mapaAyuda')}</p>
</section>

<style>
  .nb-mapa {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding-block-end: var(--nb-space-3);
    margin-block-end: var(--nb-space-3);
    border-block-end: 1px solid var(--nb-border);
  }

  .nb-mapa-titulo {
    display: flex;
    justify-content: space-between;
    gap: var(--nb-space-2);
    margin: 0;
    font-size: var(--nb-text-md);
    font-weight: 600;
    color: var(--m3c-on-surface);
  }

  .nb-mapa-leido {
    font-size: var(--nb-text-sm);
    font-weight: 400;
    color: var(--m3c-on-surface-variant);
  }

  .nb-mapa-fila {
    position: relative;
    inline-size: 100%;
  }

  .nb-mapa-capitulos {
    block-size: 1.75rem;
    border-radius: var(--nb-radius-sm);
    overflow: hidden;
    background: var(--m3c-surface-container-high);
  }

  .nb-mapa-tramo {
    position: absolute;
    inset-block: 0;
    padding: 0;
    border: none;
    background: var(--m3c-surface-container-highest);
    cursor: pointer;
  }

  .nb-mapa-tramo[data-par='false'] {
    background: var(--m3c-surface-container);
  }

  .nb-mapa-tramo[aria-current='true'] {
    background: var(--m3c-secondary-container);
  }

  .nb-mapa-tramo:hover {
    background: var(--m3c-secondary-container);
  }

  .nb-mapa-hecho {
    position: absolute;
    inset-block-end: 0;
    inset-inline-start: 0;
    block-size: 0.25rem;
    background: var(--nb-accent);
    pointer-events: none;
  }

  .nb-mapa-aqui {
    position: absolute;
    inset-block: 0;
    inline-size: 2px;
    background: var(--m3c-on-surface);
    pointer-events: none;
  }

  .nb-mapa-marcas {
    block-size: 1.25rem;
  }

  /*
   * Cada clase se distingue por la forma y no solo por el color, como en el texto:
   * el resaltado es un bloque lleno, la nota un anillo, el marcador una raya alta.
   */
  .nb-mapa-marca {
    position: absolute;
    inset-block-start: 0.25rem;
    inline-size: 0.625rem;
    block-size: 0.625rem;
    margin-inline-start: -0.3125rem;
    padding: 0;
    border: 2px solid var(--nb-accent);
    border-radius: var(--nb-radius-full);
    background: var(--nb-accent);
    cursor: pointer;
  }

  .nb-mapa-marca[data-clase='note'] {
    background: transparent;
  }

  .nb-mapa-marca[data-clase='bookmark'] {
    inset-block-start: 0;
    inline-size: 0.25rem;
    block-size: 1.25rem;
    margin-inline-start: -0.125rem;
    border-radius: 0;
  }

  .nb-mapa-marca[data-clase='underline'],
  .nb-mapa-marca[data-clase='drawing'] {
    border-radius: 0;
  }

  .nb-mapa-ayuda {
    margin: 0;
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
  }
</style>
