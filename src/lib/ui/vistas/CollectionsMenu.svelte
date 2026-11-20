<!--
  @component
  Colecciones: las agrupaciones del usuario, en el menú de la barra superior.

  Contrato: `src-tauri/src/collections.rs`.

  Una colección es un filtro de la biblioteca, igual que una estantería: elegirla
  lleva a la Biblioteca con esa colección puesta, no a otra pantalla. Lo que la
  distingue es de quién es. La estantería sale de los metadatos; la colección la
  hace el usuario, así que aquí también se crea, se renombra y se borra. Añadir un
  libro se hace desde su ficha, que es donde está el libro.

  Este componente **no consulta**: recibe la lista y avisa de lo que se pide.

  Borrar pide confirmación en la misma fila y dice lo que no se pierde: los libros
  se quedan. Sin eso, "borrar la colección" se lee como "borrar sus libros", que
  es justo el miedo que impide usarlas.
-->
<script lang="ts">
  import { idioma, textos } from '$lib/i18n';
  import type { CollectionSummary } from '$lib/bindings';

  import Button from '../Button.svelte';
  import Field from '../Field.svelte';
  import Icon from '../Icon.svelte';
  import IconButton from '../IconButton.svelte';
  import Spinner from '../Spinner.svelte';
  import { TEXTOS_DE_SIGNIFICADO } from '$lib/i18n/locales/significado';
  import PrepararSignificado from './PrepararSignificado.svelte';

  interface Props {
    colecciones: CollectionSummary[];
    /** La colección que está filtrando la biblioteca, si hay una. */
    activa?: string | null;
    cargando?: boolean;
    alElegir?: (coleccion: CollectionSummary) => void;
    /** Quita el filtro y vuelve a la biblioteca entera. */
    alQuitar?: () => void;
    alCrear?: (nombre: string) => void;
    alRenombrar?: (id: string, nombre: string) => void;
    alBorrar?: (id: string) => void;
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    colecciones,
    activa = null,
    cargando = false,
    alElegir,
    alQuitar,
    alCrear,
    alRenombrar,
    alBorrar,
    etiquetas
  }: Props = $props();

  const t = $derived({ ...textos().colecciones, ...etiquetas } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;
  const cifra = $derived(new Intl.NumberFormat(idioma()));

  let creando = $state(false);
  let nombreNuevo = $state('');
  let errorNuevo = $state<string | null>(null);
  let editando = $state<string | null>(null);
  let nombreEditado = $state('');
  let errorEditado = $state<string | null>(null);
  let borrando = $state<string | null>(null);
  /**
   * La colección cuya búsqueda por significado se está mirando. Se abre
   * debajo de su fila, con el coste delante del botón de preparar.
   */
  let significado = $state<string | null>(null);
  const porSignificado = $derived(TEXTOS_DE_SIGNIFICADO[idioma()].titulo);

  /**
   * Por qué no vale un nombre, o nada si vale.
   *
   * Se comprueba con la lista que ya se tiene, antes de llamar. El runtime también
   * lo rechaza, pero con un fallo de almacenamiento que diría "no se pudo
   * escribir" cuando lo que pasa es que ya existe. Solo cuentan las personales,
   * que es donde el nombre tiene que ser único, y sin distinguir mayúsculas:
   * "Tesis" y "tesis" en la misma lista no se distinguen de un vistazo.
   */
  function motivoDeRechazo(nombre: string, salvo: string | null = null): string | null {
    const limpio = nombre.trim();
    if (limpio.length === 0) return texto('sinNombre');
    const repetido = colecciones.some(
      (c) =>
        c.origin === 'personal' &&
        c.id !== salvo &&
        c.name.trim().toLocaleLowerCase() === limpio.toLocaleLowerCase()
    );
    return repetido ? texto('repetido') : null;
  }

  function dejarDeCrear() {
    creando = false;
    nombreNuevo = '';
    errorNuevo = null;
  }

  function crear(evento: SubmitEvent) {
    evento.preventDefault();
    errorNuevo = motivoDeRechazo(nombreNuevo);
    if (errorNuevo) return;
    alCrear?.(nombreNuevo.trim());
    dejarDeCrear();
  }

  function guardarNombre(evento: SubmitEvent, id: string) {
    evento.preventDefault();
    errorEditado = motivoDeRechazo(nombreEditado, id);
    if (errorEditado) return;
    alRenombrar?.(id, nombreEditado.trim());
    editando = null;
  }

  /**
   * Escape deja de escribir **sin cerrar el menú entero**.
   *
   * El menú también se cierra con Escape, y lo escucha más arriba. Sin detener la
   * tecla aquí, arrepentirse de un nombre a medias se llevaba por delante la lista
   * que se estaba mirando.
   */
  function cancelarConEscape(evento: KeyboardEvent, cancelar: () => void) {
    if (evento.key !== 'Escape') return;
    evento.stopPropagation();
    cancelar();
  }

  /** Quien pulsa "Nueva Colección" va a escribir: el campo recibe el foco. */
  function enfocar(nodo: HTMLInputElement) {
    nodo.focus();
    nodo.select();
  }
</script>

<div class="nb-colecciones">
  {#if activa}
    <button class="nb-quitar" type="button" onclick={() => alQuitar?.()}>
      <Icon nombre="cerrar" tamano={14} />
      {texto('verToda')}
    </button>
  {/if}

  {#if cargando && colecciones.length === 0}
    <Spinner etiqueta={texto('cargando')} />
  {:else if colecciones.length === 0}
    <p class="nb-ayuda">{texto('vacio')}</p>
  {:else}
    <ul class="nb-lista">
      {#each colecciones as coleccion (coleccion.id)}
        <li class="nb-fila">
          {#if editando === coleccion.id}
            <form class="nb-formulario" onsubmit={(e) => guardarNombre(e, coleccion.id)}>
              <Field etiqueta={texto('nombre')} error={errorEditado ?? undefined}>
                {#snippet children({ id, 'aria-describedby': descrita })}
                  <input
                    {id}
                    class="nb-entrada"
                    type="text"
                    autocomplete="off"
                    aria-describedby={descrita}
                    bind:value={nombreEditado}
                    use:enfocar
                    onkeydown={(e) => cancelarConEscape(e, () => (editando = null))}
                  />
                {/snippet}
              </Field>
              <div class="nb-acciones">
                <Button variante="primario" tipo="submit">{texto('guardar')}</Button>
                <Button onclick={() => (editando = null)}>{texto('cancelar')}</Button>
              </div>
            </form>
          {:else if borrando === coleccion.id}
            <div
              class="nb-formulario"
              role="group"
              aria-label={`${texto('borrar')}: ${coleccion.name}`}
            >
              <p class="nb-ayuda">{texto('borrarPregunta')}</p>
              <div class="nb-acciones">
                <Button
                  variante="peligro"
                  onclick={() => {
                    alBorrar?.(coleccion.id);
                    borrando = null;
                  }}
                >
                  {texto('borrar')}
                </Button>
                <Button onclick={() => (borrando = null)}>{texto('cancelar')}</Button>
              </div>
            </div>
          {:else}
            <button
              class="nb-coleccion"
              type="button"
              aria-current={activa === coleccion.id ? 'true' : undefined}
              onclick={() => alElegir?.(coleccion)}
            >
              <Icon nombre={coleccion.kind === 'rule' ? 'filtrar' : 'carpeta'} tamano={18} />
              <span class="nb-coleccion-nombre">{coleccion.name}</span>
              <span class="nb-coleccion-cuenta">{cifra.format(coleccion.members)}</span>
            </button>
            <IconButton
              etiqueta={`${porSignificado}: ${coleccion.name}`}
              onclick={() => (significado = significado === coleccion.id ? null : coleccion.id)}
            >
              <Icon nombre="ia" tamano={18} />
            </IconButton>
            {#if coleccion.origin === 'personal'}
              <IconButton
                etiqueta={`${texto('renombrar')}: ${coleccion.name}`}
                onclick={() => {
                  editando = coleccion.id;
                  nombreEditado = coleccion.name;
                  errorEditado = null;
                  borrando = null;
                }}
              >
                <Icon nombre="editar" tamano={18} />
              </IconButton>
              <IconButton
                etiqueta={`${texto('borrar')}: ${coleccion.name}`}
                onclick={() => {
                  borrando = coleccion.id;
                  editando = null;
                }}
              >
                <Icon nombre="borrar" tamano={18} />
              </IconButton>
            {/if}
          {/if}
          {#if significado === coleccion.id}
            <div class="nb-significado-de-coleccion">
              <PrepararSignificado alcance={{ scope: 'collection', collectionId: coleccion.id }} />
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if creando}
    <form class="nb-formulario" onsubmit={crear}>
      <Field etiqueta={texto('nombre')} error={errorNuevo ?? undefined}>
        {#snippet children({ id, 'aria-describedby': descrita })}
          <input
            {id}
            class="nb-entrada"
            type="text"
            autocomplete="off"
            aria-describedby={descrita}
            bind:value={nombreNuevo}
            use:enfocar
            onkeydown={(e) => cancelarConEscape(e, dejarDeCrear)}
          />
        {/snippet}
      </Field>
      <div class="nb-acciones">
        <Button variante="primario" tipo="submit">{texto('crear')}</Button>
        <Button onclick={dejarDeCrear}>{texto('cancelar')}</Button>
      </div>
    </form>
  {:else}
    <button
      class="nb-nueva"
      type="button"
      onclick={() => {
        creando = true;
        editando = null;
        borrando = null;
      }}
    >
      <Icon nombre="mas" tamano={20} />
      {texto('nueva')}
    </button>
  {/if}
</div>

<style>
  .nb-colecciones {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
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

  .nb-lista {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-fila {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-1);
  }

  .nb-significado-de-coleccion {
    flex-basis: 100%;
  }

  .nb-coleccion {
    display: flex;
    align-items: center;
    gap: var(--nb-space-3);
    flex: 1 1 auto;
    min-width: 0;
    min-height: 3rem;
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

  .nb-coleccion:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-coleccion[aria-current='true'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font-weight: 600;
  }

  .nb-coleccion-nombre {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .nb-coleccion-cuenta {
    flex: 0 0 auto;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
    font-variant-numeric: tabular-nums;
  }

  .nb-formulario {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    flex: 1 1 auto;
    min-width: 0;
    padding: var(--nb-space-3);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-highest);
  }

  .nb-entrada {
    box-sizing: border-box;
    width: 100%;
    min-height: 2.5rem;
    padding: 0 var(--nb-space-3);
    border: 1px solid var(--m3c-outline);
    border-radius: var(--nb-radius-sm);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-sm);
  }

  .nb-acciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
  }

  .nb-nueva {
    display: flex;
    align-items: center;
    gap: var(--nb-space-3);
    min-height: 3rem;
    padding: 0 var(--nb-space-3);
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

  .nb-nueva:hover {
    background: color-mix(in srgb, var(--m3c-primary) 8%, transparent);
  }

  .nb-ayuda {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    line-height: 1.45;
  }
</style>
