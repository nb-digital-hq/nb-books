<!--
  @component
  Una fila de Inicio: su título y las obras que caben, con flechas para ver las
  siguientes.

  Contrato: docs/design/ui-conventions.md, sección 4, "Nada se desplaza en
  horizontal"..

  **No se desplaza: se pagina.** Caben las que caben en el ancho de la fila, sin
  ninguna cortada por el borde, y las flechas enseñan las siguientes. Son botones,
  así que se alcanzan con el tabulador y se pulsan con Intro: nada queda detrás
  de un gesto de arrastre, que es lo que la regla prohíbe. Y "Ver todo" lleva a la
  lista entera, para quien no quiere ir de página en página.

  Cuántas caben lo decide el ancho de verdad, medido, y no un número escrito: la
  misma fila enseña seis portadas en una ventana ancha y dos en una estrecha, y al
  ensanchar la ventana no se pierde de vista lo que se estaba mirando.
-->
<script lang="ts" generics="T">
  import { textos } from '$lib/i18n';
  import type { Snippet } from 'svelte';

  import Icon from '../Icon.svelte';
  import IconButton from '../IconButton.svelte';

  interface Props {
    titulo: string;
    subtitulo?: string;
    elementos: T[];
    /** Identidad estable de cada elemento, para no rehacer lo que no cambia. */
    clave: (elemento: T) => string;
    /** Ancho mínimo de cada elemento, en píxeles. */
    anchoMinimo?: number;
    alVerTodo?: () => void;
    elemento: Snippet<[T]>;
  }

  let {
    titulo,
    subtitulo,
    elementos,
    clave,
    anchoMinimo = 160,
    alVerTodo,
    elemento
  }: Props = $props();

  const t = $derived(textos().inicio);
  const id = $props.id();

  /** El hueco entre elementos: `--nb-space-4`, que es lo que pone la hoja. */
  const HUECO = 16;

  let pista: HTMLElement | undefined = $state();
  // Seis de partida: es lo que se pinta en el servidor y en el primer fotograma, antes
  // de medir, y seis es lo que cabe en la ventana de referencia.
  let porPagina = $state(6);
  let desde = $state(0);

  $effect(() => {
    if (!pista || typeof ResizeObserver === 'undefined') return;
    const observador = new ResizeObserver(([entrada]) => {
      if (!entrada) return;
      const ancho = entrada.contentRect.width;
      // Al fotograma siguiente: ver AppShell.
      requestAnimationFrame(
        () => (porPagina = Math.max(1, Math.floor((ancho + HUECO) / (anchoMinimo + HUECO))))
      );
    });
    observador.observe(pista);
    return () => observador.disconnect();
  });

  /*
   * Si al ensanchar la ventana caben más, la última página se rellena hacia atrás
   * en lugar de quedarse con huecos: lo que se estaba mirando sigue a la vista.
   */
  const inicio = $derived(Math.max(0, Math.min(desde, elementos.length - porPagina)));
  const visibles = $derived(elementos.slice(inicio, inicio + porPagina));
  const hayAnteriores = $derived(inicio > 0);
  const haySiguientes = $derived(inicio + porPagina < elementos.length);

  function mover(paso: 1 | -1) {
    desde = Math.max(0, Math.min(inicio + paso * porPagina, elementos.length - porPagina));
  }
</script>

<section class="nb-fila" aria-labelledby={id}>
  <div class="nb-fila-cabecera">
    <h2 class="nb-fila-titulo" {id}>{titulo}</h2>
    {#if subtitulo}
      <span class="nb-fila-subtitulo">{subtitulo}</span>
    {/if}
    <span class="nb-fila-hueco"></span>
    {#if alVerTodo}
      <button class="nb-fila-ver-todo" type="button" onclick={() => alVerTodo?.()}>
        {t.verTodo}
        <Icon nombre="derecha" tamano={18} />
      </button>
    {/if}
    {#if elementos.length > porPagina}
      <IconButton
        etiqueta={`${t.anteriores}: ${titulo}`}
        deshabilitado={!hayAnteriores}
        onclick={() => mover(-1)}
      >
        <Icon nombre="izquierda" />
      </IconButton>
      <IconButton
        etiqueta={`${t.siguientes}: ${titulo}`}
        deshabilitado={!haySiguientes}
        onclick={() => mover(1)}
      >
        <Icon nombre="derecha" />
      </IconButton>
    {/if}
  </div>

  <div class="nb-fila-pista" bind:this={pista} style:--nb-fila-columnas={porPagina}>
    <!--
      **Sin animación al pasar de página, y es una medida.** La entrada tenía un
      desplazamiento del 2 % y un fundido, y en WebKitGTK cada pulsación dejaba unos
      tres fotogramas por encima de 20 ms, con picos de 50. Sin ella, pasar de página
      en la fila de novedades no deja ninguno. Una animación que no cabe se quita,
      no se recorta: performance-budgets.md, sección 4.12.

      Tampoco se rehace el carril entero al cambiar de página, que era lo que la
      entrada necesitaba para notarse. Medido con y sin rehacerlo, sobre servidores
      recién arrancados, da lo mismo, así que sobraba.
    -->
    <ul class="nb-fila-carril">
      {#each visibles as item (clave(item))}
        <li class="nb-fila-elemento">{@render elemento(item)}</li>
      {/each}
    </ul>
  </div>
</section>

<style>
  /* Una fila de Inicio fuera de la vista no se maqueta hasta que se acerca: es lo
     mismo que la rejilla de Biblioteca, y por el mismo motivo. */
  .nb-fila {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
    min-width: 0;
    content-visibility: auto;
    contain-intrinsic-size: auto 20rem;
  }

  .nb-fila-cabecera {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-1) var(--nb-space-2);
    min-height: 2.5rem;
  }

  .nb-fila-titulo {
    margin: 0;
    font-size: var(--nb-text-lg);
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--m3c-on-surface);
  }

  .nb-fila-subtitulo {
    margin-left: var(--nb-space-1);
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
    font-variant-numeric: tabular-nums;
  }

  .nb-fila-hueco {
    flex: 1 1 auto;
  }

  .nb-fila-ver-todo {
    display: inline-flex;
    align-items: center;
    gap: var(--nb-space-1);
    min-height: 2.5rem;
    padding: 0 var(--nb-space-2) 0 var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-primary);
    font: inherit;
    font-size: var(--nb-text-sm);
    font-weight: 600;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-fila-ver-todo:hover {
    background: color-mix(in srgb, var(--m3c-primary) 8%, transparent);
  }

  .nb-fila-pista {
    min-width: 0;
  }

  .nb-fila-carril {
    display: grid;
    grid-template-columns: repeat(var(--nb-fila-columnas), minmax(0, 1fr));
    gap: var(--nb-space-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-fila-elemento {
    min-width: 0;
  }
</style>
