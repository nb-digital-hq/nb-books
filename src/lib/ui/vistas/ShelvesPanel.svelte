<!--
  @component
  Estanterías: autores, series y materias por las que acotar la biblioteca.

  Contrato: `src-tauri/src/collections.rs`.

  **Se busca, no se despliega**, y es la misma decisión que tomó H6 por el mismo
  motivo: el tamaño. Elegir entre unas decenas es un gesto; elegir entre decenas
  de miles no lo es, y un desplegable de miles de elementos no es un desplegable,
  es una búsqueda peor hecha. Con 152.080 obras hay decenas de miles de autores,
  así que aquí se escribe y el runtime filtra en SQL.

  Sin escribir nada se enseñan **las más numerosas**, que es lo útil de partida:
  una estantería con doscientos libros importa más que una con uno. Y cada
  sección dice cuántas cumplen en total, no solo cuántas se ven: enseñar veinte
  como si fueran todas sería mentir sobre el tamaño de la biblioteca.

  Este componente **no consulta**: recibe lo que hay y avisa de lo que se pide.
  Quien habla con el runtime es la aplicación, que es también quien sabe qué
  filtro está puesto.
-->
<script lang="ts">
  import { idioma, textos } from '$lib/i18n';
  import type { Shelf, ShelfKind } from '$lib/bindings';

  import Field from '../Field.svelte';
  import Icon from '../Icon.svelte';
  import Spinner from '../Spinner.svelte';

  /** Una sección del panel: su clase, lo que se ha traído y cuántas hay. */
  export interface Seccion {
    kind: ShelfKind;
    shelves: Shelf[];
    total: number;
  }

  interface Props {
    secciones: Seccion[];
    /** Lo que está escrito en el campo. Lo gobierna quien monta el panel. */
    busqueda?: string;
    cargando?: boolean;
    /** La estantería abierta ahora mismo, para marcarla. */
    activa?: { kind: ShelfKind; key: string } | null;
    alBuscar?: (texto: string) => void;
    alElegir?: (estanteria: Shelf) => void;
    /** Quita el filtro y vuelve a la biblioteca entera. */
    alQuitar?: () => void;
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    secciones,
    busqueda = '',
    cargando = false,
    activa = null,
    alBuscar,
    alElegir,
    alQuitar,
    etiquetas
  }: Props = $props();

  const t = $derived({ ...textos().estanterias, ...etiquetas } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  const cifra = $derived(new Intl.NumberFormat(idioma()));

  /** Icono de cada clase. Todos existen ya en el catálogo. */
  const ICONOS: Record<ShelfKind, 'autor' | 'serie' | 'materia' | 'idioma' | 'lector' | 'plan'> = {
    author: 'autor',
    series: 'serie',
    subject: 'materia',
    language: 'idioma',
    format: 'lector',
    year: 'plan'
  };

  const nombresDeIdioma = $derived.by(() => {
    try {
      return new Intl.DisplayNames([idioma()], { type: 'language' });
    } catch {
      return null;
    }
  });

  /**
   * Lo que se enseña de idioma y formato sale de su código: el runtime da `es` y
   * `application/pdf`, que no son nombres para nadie.
   */
  function nombreDe(estanteria: Shelf): string {
    if (estanteria.kind === 'language') {
      try {
        const nombre = nombresDeIdioma?.of(estanteria.key);
        if (nombre) return nombre.charAt(0).toLocaleUpperCase() + nombre.slice(1);
      } catch {
        // Un código que Intl no reconoce se enseña tal cual.
      }
      return estanteria.label;
    }
    if (estanteria.kind === 'format') return t[`formato-${estanteria.key}`] ?? estanteria.label;
    return estanteria.label;
  }

  const hayAlgo = $derived(secciones.some((s) => s.shelves.length > 0));
</script>

<div class="nb-estanterias">
  <Field etiqueta={texto('buscar')}>
    {#snippet children({ id })}
      <input
        {id}
        class="nb-entrada"
        type="search"
        autocomplete="off"
        spellcheck="false"
        placeholder={texto('buscarAyuda')}
        value={busqueda}
        oninput={(evento) => alBuscar?.(evento.currentTarget.value)}
      />
    {/snippet}
  </Field>

  {#if activa}
    <!--
      Lo que está puesto se ve y se quita desde aquí. Sin esto, quien abre una
      estantería y luego el panel no sabría cuál está mirando, porque la lista
      enseña las mismas de siempre.
    -->
    <button class="nb-quitar" type="button" onclick={() => alQuitar?.()}>
      <Icon nombre="cerrar" tamano={14} />
      {texto('quitar')}
    </button>
  {/if}

  {#if cargando}
    <Spinner etiqueta={texto('cargando')} />
  {:else if !hayAlgo}
    <p class="nb-ayuda">{busqueda.trim() ? texto('sinResultados') : texto('vacio')}</p>
  {:else}
    {#each secciones as seccion (seccion.kind)}
      {#if seccion.shelves.length > 0}
        <section class="nb-seccion">
          <h3 class="nb-seccion-titulo">
            {texto(`clase-${seccion.kind}`)}
            <!--
              "20 de 3.412" y no "20": el segundo número es lo que dice que hay
              más y que hace falta escribir para llegar a ellas.
            -->
            <span class="nb-cuenta">
              {#if seccion.total > seccion.shelves.length}
                {cifra.format(seccion.shelves.length)}
                {texto('deTotal')}
                {cifra.format(seccion.total)}
              {:else}
                {cifra.format(seccion.total)}
              {/if}
            </span>
          </h3>
          <ul class="nb-lista">
            {#each seccion.shelves as estanteria (`${estanteria.kind}-${estanteria.key}`)}
              <li>
                <button
                  class="nb-estanteria"
                  type="button"
                  aria-current={activa?.kind === estanteria.kind && activa?.key === estanteria.key
                    ? 'true'
                    : undefined}
                  onclick={() => alElegir?.(estanteria)}
                >
                  <Icon nombre={ICONOS[estanteria.kind]} tamano={15} />
                  <span class="nb-estanteria-nombre">{nombreDe(estanteria)}</span>
                  <span class="nb-estanteria-cuenta">{cifra.format(estanteria.count)}</span>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    {/each}
  {/if}
</div>

<style>
  .nb-estanterias {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
  }

  .nb-entrada {
    box-sizing: border-box;
    width: 100%;
    min-height: 2.5rem;
    padding: 0 var(--nb-space-4);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-sm);
  }

  .nb-quitar {
    display: inline-flex;
    align-items: center;
    align-self: flex-start;
    gap: var(--nb-space-1);
    min-height: 2rem;
    padding: 0 var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font: inherit;
    font-size: var(--nb-text-xs);
    font-weight: 600;
    cursor: pointer;
  }

  .nb-seccion {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
  }

  .nb-seccion-titulo {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--nb-space-2);
    margin: 0;
    font-size: var(--nb-text-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--m3c-on-surface-variant);
  }

  .nb-cuenta {
    font-weight: 400;
    letter-spacing: 0;
    text-transform: none;
    font-variant-numeric: tabular-nums;
  }

  .nb-lista {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-estanteria {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    width: 100%;
    min-height: 2.5rem;
    padding: 0 var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-sm);
    text-align: left;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-estanteria:hover,
  .nb-estanteria:focus-visible {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-estanteria[aria-current='true'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font-weight: 600;
  }

  /* El nombre se recorta y la cuenta no: la cuenta son tres caracteres y es lo
     que distingue una estantería grande de una de dos libros. */
  .nb-estanteria-nombre {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .nb-estanteria-cuenta {
    flex: 0 0 auto;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
    font-variant-numeric: tabular-nums;
  }

  .nb-ayuda {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }
</style>
