<!--
  @component
  Un tablero de notas por columnas con nombre.

  Contrato: `src-tauri/src/boards.rs`.

  **Por qué columnas y no una rejilla.** el equipo, el 2026-09-12, de la rejilla de
  6x8: "inútil, feo y no se entiende en lo absoluto". Una celda no tiene nombre,
  así que colocar una nota no decía nada. Una columna se llama algo, y mover una
  tarjeta de una a otra es decir algo de esa nota.

  **Todo lo que hace el ratón lo hace el teclado**, que es el criterio de la
  ficha: las flechas recorren las tarjetas, Alt o Control con las flechas las
  mueven, Supr las quita del tablero e Intro las abre. Arrastrar es la otra vía,
  no la única.

  **Y todo tablero tiene equivalente en lista**, sobre la misma respuesta del
  runtime: la lista y las columnas no pueden decir cosas distintas.

  **Nada se desplaza en horizontal.** Las columnas que no caben bajan a la fila
  siguiente, como exige la sección 4 de ui-conventions.md; un tablero es finito
  (ocho columnas), así que eso son dos filas como mucho.
-->
<script lang="ts">
  import { tick } from 'svelte';

  import type { BoardColumn, BoardLimits, Card, Note } from '$lib/bindings';
  import { textos } from '$lib/i18n';

  import Button from '../Button.svelte';
  import Icon from '../Icon.svelte';
  import IconButton from '../IconButton.svelte';
  import Select from '../Select.svelte';

  interface Props {
    titulo: string;
    columnas?: readonly BoardColumn[];
    limites: BoardLimits;
    /** Las notas que se pueden poner. */
    notas?: readonly Note[];
    /** Los demás tableros, para poder enlazarlos desde una columna. */
    otrosTableros?: readonly { id: string; title: string }[];
    alAnadirColumna?: (titulo: string) => void;
    alRenombrarColumna?: (columna: string, titulo: string) => void;
    alBorrarColumna?: (columna: string) => void;
    alMoverColumna?: (columna: string, lugar: number) => void;
    alPonerNota?: (columna: string, nota: string) => void;
    alEnlazar?: (columna: string, tablero: string) => void;
    alCrearNota?: (columna: string, titulo: string) => void;
    alMoverTarjeta?: (tarjeta: string, columna: string, lugar: number) => void;
    alQuitarTarjeta?: (tarjeta: string) => void;
    alAbrirNota?: (nota: string) => void;
    alAbrirTablero?: (id: string) => void;
    alBorrarTablero?: () => void;
  }

  let {
    titulo,
    columnas = [],
    limites,
    notas = [],
    otrosTableros = [],
    alAnadirColumna,
    alRenombrarColumna,
    alBorrarColumna,
    alMoverColumna,
    alPonerNota,
    alEnlazar,
    alCrearNota,
    alMoverTarjeta,
    alQuitarTarjeta,
    alAbrirNota,
    alAbrirTablero,
    alBorrarTablero
  }: Props = $props();

  const t = $derived(textos().tableros as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;
  const id = $props.id();

  /** Si se enseña la lista en lugar de las columnas. Los mismos datos. */
  let comoLista = $state(false);
  let columnaNueva = $state('');
  /** La columna cuyo nombre se está cambiando, y lo escrito. */
  let renombrando = $state<string | null>(null);
  let nombreEscrito = $state('');
  /** La columna donde está abierto el formulario de añadir. */
  let anadiendoEn = $state<string | null>(null);
  let tituloDeNota = $state('');

  const total = $derived(columnas.reduce((suma, c) => suma + c.cards.length, 0));
  const lleno = $derived(total >= limites.cards);
  const enElTablero = $derived(
    new Set(columnas.flatMap((c) => c.cards.map((tarjeta) => tarjeta.noteId)))
  );
  /** Las notas que todavía no están en este tablero: ponerlas es lo que se ofrece. */
  const notasFuera = $derived(notas.filter((n) => !enElTablero.has(n.id)));

  let marco: HTMLElement | undefined = $state();

  /**
   * La tarjeta que tiene que recuperar el foco cuando llegue el tablero nuevo.
   *
   * Mover una tarjeta con el teclado vuelve a pintar su columna, y sin esto el
   * foco se quedaría en el cuerpo del documento: para moverla dos sitios habría
   * que volver a buscarla.
   */
  // `raw`: se compara por identidad con las columnas de antes, y un estado
  // profundo las envolvería en otro objeto.
  let enfocarTras = $state.raw<{ tarjeta: string; antes: readonly BoardColumn[] } | null>(null);

  $effect(() => {
    const ahora = columnas;
    const pendiente = enfocarTras;
    // Hasta que no llega el tablero nuevo no hay nada que enfocar: la tarjeta
    // todavía está en su sitio viejo, y al llegar se vuelve a crear en otra
    // columna.
    if (!pendiente || ahora === pendiente.antes) return;
    enfocarTras = null;
    void tick().then(() => {
      const boton = marco?.querySelector<HTMLElement>(`[data-tarjeta="${pendiente.tarjeta}"]`);
      boton?.focus();
      // Se dice dónde quedó: el foco se queda en la misma tarjeta, y un
      // lector de pantalla no anuncia que su nombre cambió.
      anuncio = boton?.getAttribute('aria-label') ?? '';
    });
  });

  let anuncio = $state('');

  function lugarDe(tarjeta: string): { c: number; t: number } | null {
    for (const [c, columna] of columnas.entries()) {
      const indice = columna.cards.findIndex((x) => x.id === tarjeta);
      if (indice >= 0) return { c, t: indice };
    }
    return null;
  }

  function enfocarTarjeta(c: number, t: number) {
    const columna = columnas[c];
    if (!columna) return;
    const destino = columna.cards[Math.max(0, Math.min(t, columna.cards.length - 1))];
    const selector = destino ? `[data-tarjeta="${destino.id}"]` : `[data-anadir="${columna.id}"]`;
    marco?.querySelector<HTMLElement>(selector)?.focus();
  }

  /**
   * El teclado de las tarjetas, atendido una vez para todo el tablero.
   *
   * Los eventos suben desde el botón de la tarjeta enfocada. Un manejador por
   * tarjeta serían ciento veinte funciones para hacer lo mismo, y el presupuesto
   * de nodos del tablero no las necesita.
   */
  function alPulsar(evento: KeyboardEvent) {
    const boton = (evento.target as HTMLElement | null)?.closest<HTMLElement>('[data-tarjeta]');
    const tarjeta = boton?.dataset.tarjeta;
    if (!tarjeta) return;
    const donde = lugarDe(tarjeta);
    if (!donde) return;
    const mover = evento.altKey || evento.ctrlKey;
    const columna = columnas[donde.c];
    if (!columna) return;

    switch (evento.key) {
      case 'ArrowUp':
      case 'ArrowDown': {
        const paso = evento.key === 'ArrowUp' ? -1 : 1;
        const siguiente = donde.t + paso;
        if (siguiente < 0 || siguiente >= columna.cards.length) break;
        if (mover) {
          enfocarTras = { tarjeta, antes: columnas };
          alMoverTarjeta?.(tarjeta, columna.id, siguiente);
        } else {
          enfocarTarjeta(donde.c, siguiente);
        }
        break;
      }
      case 'ArrowLeft':
      case 'ArrowRight': {
        const otra = donde.c + (evento.key === 'ArrowLeft' ? -1 : 1);
        const vecina = columnas[otra];
        if (!vecina) break;
        if (mover) {
          enfocarTras = { tarjeta, antes: columnas };
          alMoverTarjeta?.(tarjeta, vecina.id, Math.min(donde.t, vecina.cards.length));
        } else {
          enfocarTarjeta(otra, donde.t);
        }
        break;
      }
      case 'Delete':
      case 'Backspace': {
        // El foco va a la de al lado cuando llegue el tablero nuevo, no al
        // principio del documento.
        const vecina = columna.cards[donde.t + 1] ?? columna.cards[donde.t - 1];
        if (vecina) enfocarTras = { tarjeta: vecina.id, antes: columnas };
        alQuitarTarjeta?.(tarjeta);
        break;
      }
      default:
        return;
    }
    evento.preventDefault();
  }

  function abrir(tarjeta: Card) {
    if (tarjeta.linksTo) alAbrirTablero?.(tarjeta.linksTo);
    else if (tarjeta.noteId) alAbrirNota?.(tarjeta.noteId);
  }

  // --- Arrastrar ------------------------------------------------------------

  let arrastrada = $state<string | null>(null);
  /** Dónde caería: la columna y el lugar dentro de ella. */
  let sobre = $state<{ columna: string; lugar: number } | null>(null);

  function alEmpezarArrastre(evento: DragEvent) {
    const tarjeta = (evento.target as HTMLElement | null)?.closest<HTMLElement>('[data-tarjeta]')
      ?.dataset.tarjeta;
    if (!tarjeta || !evento.dataTransfer) return;
    arrastrada = tarjeta;
    evento.dataTransfer.effectAllowed = 'move';
    evento.dataTransfer.setData('text/plain', tarjeta);
  }

  /**
   * El lugar donde caería, según el puntero.
   *
   * Encima de la mitad de una tarjeta cae antes que ella, y debajo, después.
   * Fuera de las tarjetas, al final de la columna. Se mide la caja al moverse y
   * no se guarda: la columna puede haber cambiado desde el último movimiento.
   */
  function alPasarPorEncima(evento: DragEvent) {
    if (!arrastrada) return;
    const objetivo = evento.target as HTMLElement | null;
    const columna = objetivo?.closest<HTMLElement>('[data-columna]');
    if (!columna?.dataset.columna) return;
    evento.preventDefault();
    if (evento.dataTransfer) evento.dataTransfer.dropEffect = 'move';
    const items = Array.from(columna.querySelectorAll<HTMLElement>('[data-lugar]'));
    let lugar = items.length;
    for (const item of items) {
      const caja = item.getBoundingClientRect();
      if (evento.clientY < caja.top + caja.height / 2) {
        lugar = Number(item.dataset.lugar);
        break;
      }
    }
    if (sobre?.columna !== columna.dataset.columna || sobre.lugar !== lugar) {
      sobre = { columna: columna.dataset.columna, lugar };
    }
  }

  function alSoltar(evento: DragEvent) {
    evento.preventDefault();
    if (arrastrada && sobre) {
      const origen = lugarDe(arrastrada);
      const destino = columnas.findIndex((c) => c.id === sobre?.columna);
      // Soltarla debajo de sí misma en su columna la deja donde estaba: el lugar
      // se cuenta sin ella, y sin esta corrección bajaría una posición.
      const lugar =
        origen && origen.c === destino && sobre.lugar > origen.t ? sobre.lugar - 1 : sobre.lugar;
      if (!origen || origen.c !== destino || origen.t !== lugar) {
        alMoverTarjeta?.(arrastrada, sobre.columna, lugar);
      }
    }
    arrastrada = null;
    sobre = null;
  }

  function alTerminarArrastre() {
    arrastrada = null;
    sobre = null;
  }

  // --- Formularios -----------------------------------------------------------

  function empezarARenombrar(columna: BoardColumn) {
    renombrando = columna.id;
    nombreEscrito = columna.title;
    void tick().then(() => marco?.querySelector<HTMLInputElement>('[data-renombrar]')?.select());
  }

  function terminarDeRenombrar(guardar: boolean) {
    const columna = renombrando;
    const nombre = nombreEscrito.trim();
    renombrando = null;
    if (guardar && columna && nombre) alRenombrarColumna?.(columna, nombre);
  }

  function abrirAnadir(columna: string) {
    anadiendoEn = columna;
    tituloDeNota = '';
    void tick().then(() => marco?.querySelector<HTMLInputElement>('[data-nota-nueva]')?.focus());
  }

  function crearNota(evento: SubmitEvent) {
    evento.preventDefault();
    const nombre = tituloDeNota.trim();
    if (!anadiendoEn || !nombre) return;
    alCrearNota?.(anadiendoEn, nombre);
    tituloDeNota = '';
  }

  function anadirColumna(evento: SubmitEvent) {
    evento.preventDefault();
    const nombre = columnaNueva.trim();
    if (!nombre) return;
    alAnadirColumna?.(nombre);
    columnaNueva = '';
  }

  function nombreDeTarjeta(tarjeta: Card, columna: BoardColumn, lugar: number): string {
    const que = tarjeta.linksTo
      ? `${texto('lleva')} ${tarjeta.linksToTitle ?? ''}`
      : (tarjeta.title ?? '');
    return `${que}, ${texto('en')} ${columna.title}, ${texto('posicion')} ${lugar + 1} ${texto('tarjetasDe')} ${columna.cards.length}`;
  }
</script>

<div class="nb-tablero" bind:this={marco}>
  <div class="nb-tablero-barra">
    <h3 class="nb-tablero-titulo">{titulo}</h3>
    <span class="nb-tablero-cifra">{total} {texto('tarjetas')}</span>
    <span class="nb-tablero-hueco"></span>
    <Button onclick={() => (comoLista = !comoLista)}>
      {comoLista ? texto('verColumnas') : texto('verLista')}
    </Button>
    <Button variante="peligro" onclick={() => alBorrarTablero?.()}>{texto('borrar')}</Button>
  </div>

  {#if lleno}
    <p class="nb-tablero-aviso">{texto('lleno')}</p>
  {:else}
    <p class="nb-tablero-ayuda">{texto('ayuda')}</p>
  {/if}

  {#if comoLista}
    <!--
      La lista **no es un resumen**: es la misma respuesta del runtime, columna a
      columna y en orden. Quien no pueda usar las columnas tiene los mismos datos.
    -->
    <ol class="nb-tablero-lista" aria-label={texto('lista')}>
      {#each columnas as columna (columna.id)}
        <li>
          <h4 class="nb-tablero-lista-columna">
            {columna.title}
            <span class="nb-columna-cuenta">{columna.cards.length}</span>
          </h4>
          <ol>
            {#each columna.cards as tarjeta (tarjeta.id)}
              <li>
                <button
                  type="button"
                  class="nb-tablero-lista-tarjeta"
                  onclick={() => abrir(tarjeta)}
                >
                  {#if tarjeta.linksTo}
                    {texto('lleva')} {tarjeta.linksToTitle}
                  {:else}
                    {tarjeta.title}
                  {/if}
                </button>
                {#if tarjeta.preview}<span class="nb-tarjeta-anticipo">{tarjeta.preview}</span>{/if}
              </li>
            {:else}
              <li class="nb-tarjeta-anticipo">{texto('vacio')}</li>
            {/each}
          </ol>
        </li>
      {/each}
    </ol>
  {:else}
    <!--
      El teclado y el arrastre se atienden aquí, una vez para todo el tablero: los
      eventos suben desde las tarjetas, que son botones y tienen el foco. Por eso
      el contenedor lleva manejadores sin ser interactivo él mismo.
    -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="nb-tablero-columnas"
      role="region"
      aria-label={texto('columnas')}
      onkeydown={alPulsar}
      ondragstart={alEmpezarArrastre}
      ondragover={alPasarPorEncima}
      ondrop={alSoltar}
      ondragend={alTerminarArrastre}
    >
      {#each columnas as columna, c (columna.id)}
        <section
          class="nb-columna"
          aria-labelledby={`${id}-${columna.id}`}
          data-columna={columna.id}
          data-destino={sobre?.columna === columna.id}
        >
          <header class="nb-columna-cabecera">
            {#if renombrando === columna.id}
              <input
                class="nb-columna-nombre-campo"
                type="text"
                aria-label={texto('nombreDeColumna')}
                data-renombrar
                bind:value={nombreEscrito}
                onkeydown={(evento) => {
                  if (evento.key === 'Enter') terminarDeRenombrar(true);
                  if (evento.key === 'Escape') terminarDeRenombrar(false);
                }}
                onblur={() => terminarDeRenombrar(true)}
              />
            {:else}
              <h4 class="nb-columna-nombre" id={`${id}-${columna.id}`}>{columna.title}</h4>
              <span class="nb-columna-cuenta">{columna.cards.length}</span>
            {/if}
            <span class="nb-columna-acciones">
              <IconButton
                etiqueta={`${texto('renombrar')}: ${columna.title}`}
                onclick={() => empezarARenombrar(columna)}
              >
                <Icon nombre="editar" tamano={18} />
              </IconButton>
              {#if c > 0}
                <IconButton
                  etiqueta={`${texto('moverIzquierda')}: ${columna.title}`}
                  onclick={() => alMoverColumna?.(columna.id, c - 1)}
                >
                  <Icon nombre="izquierda" tamano={18} />
                </IconButton>
              {/if}
              {#if c < columnas.length - 1}
                <IconButton
                  etiqueta={`${texto('moverDerecha')}: ${columna.title}`}
                  onclick={() => alMoverColumna?.(columna.id, c + 1)}
                >
                  <Icon nombre="derecha" tamano={18} />
                </IconButton>
              {/if}
              <IconButton
                etiqueta={`${texto('borrarColumna')}: ${columna.title}`}
                onclick={() => alBorrarColumna?.(columna.id)}
              >
                <Icon nombre="borrar" tamano={18} />
              </IconButton>
            </span>
          </header>

          <ol class="nb-columna-tarjetas">
            {#each columna.cards as tarjeta, lugar (tarjeta.id)}
              <li
                class="nb-tarjeta"
                data-lugar={lugar}
                data-soltar={sobre?.columna === columna.id && sobre.lugar === lugar}
              >
                <button
                  type="button"
                  class="nb-tarjeta-boton"
                  class:nb-tarjeta-enlace={tarjeta.linksTo !== null}
                  data-tarjeta={tarjeta.id}
                  data-arrastrada={arrastrada === tarjeta.id}
                  draggable="true"
                  aria-label={nombreDeTarjeta(tarjeta, columna, lugar)}
                  onclick={() => abrir(tarjeta)}
                >
                  {#if tarjeta.linksTo}
                    <span class="nb-tarjeta-titulo">
                      <Icon nombre="enlace" tamano={16} />
                      {tarjeta.linksToTitle}
                    </span>
                  {:else}
                    <span class="nb-tarjeta-titulo">{tarjeta.title}</span>
                    {#if tarjeta.preview}
                      <span class="nb-tarjeta-anticipo">{tarjeta.preview}</span>
                    {/if}
                  {/if}
                </button>
              </li>
            {/each}
          </ol>
          {#if sobre?.columna === columna.id && sobre.lugar === columna.cards.length}
            <span class="nb-columna-final" aria-hidden="true"></span>
          {/if}

          {#if anadiendoEn === columna.id}
            <form class="nb-columna-formulario" onsubmit={crearNota}>
              <input
                class="nb-columna-nombre-campo"
                type="text"
                aria-label={texto('tituloDeNota')}
                placeholder={texto('tituloDeNota')}
                data-nota-nueva
                bind:value={tituloDeNota}
                onkeydown={(evento) => {
                  if (evento.key === 'Escape') anadiendoEn = null;
                }}
              />
              <div class="nb-columna-formulario-acciones">
                <Button
                  tipo="submit"
                  variante="primario"
                  deshabilitado={tituloDeNota.trim() === ''}
                >
                  {texto('crearNota')}
                </Button>
                <Button onclick={() => (anadiendoEn = null)}>{texto('cancelar')}</Button>
              </div>
              {#if notasFuera.length > 0}
                <Select
                  etiqueta={texto('notaExistente')}
                  valor=""
                  opciones={[
                    ['', texto('notaExistente')] as const,
                    ...notasFuera.map((nota) => [nota.id, nota.title] as const)
                  ]}
                  alCambiar={(nota) => {
                    if (!nota) return;
                    alPonerNota?.(columna.id, nota);
                    // Puesta la nota, el formulario sobra: se quedaba abierto.
                    anadiendoEn = null;
                  }}
                />
              {/if}
              {#if otrosTableros.length > 0}
                <!--
                  Enlazar otro tablero es lo que hace que los tableros sean finitos
                  y suficientes: sin esto, la única forma de tener más sitio sería
                  un tablero más grande, que es el lienzo infinito por la puerta de
                  atrás.
                -->
                <Select
                  etiqueta={texto('enlazarTablero')}
                  valor=""
                  opciones={[
                    ['', texto('enlazarTablero')] as const,
                    ...otrosTableros.map((otro) => [otro.id, otro.title] as const)
                  ]}
                  alCambiar={(tablero) => {
                    if (!tablero) return;
                    alEnlazar?.(columna.id, tablero);
                    anadiendoEn = null;
                  }}
                />
              {/if}
            </form>
          {:else if !lleno}
            <button
              type="button"
              class="nb-columna-anadir"
              data-anadir={columna.id}
              onclick={() => abrirAnadir(columna.id)}
            >
              <Icon nombre="mas" tamano={18} />
              {texto('anadirNota')}
            </button>
          {/if}
        </section>
      {/each}

      {#if columnas.length < limites.columns}
        <form class="nb-columna nb-columna-nueva" onsubmit={anadirColumna}>
          <input
            class="nb-columna-nombre-campo"
            type="text"
            aria-label={texto('nombreDeColumna')}
            placeholder={texto('nombreDeColumna')}
            bind:value={columnaNueva}
          />
          <Button tipo="submit" deshabilitado={columnaNueva.trim() === ''}>
            {texto('anadirColumna')}
          </Button>
        </form>
      {/if}
    </div>
    {#if columnas.length === 0}
      <p class="nb-tablero-ayuda">{texto('sinColumnas')}</p>
    {/if}
  {/if}
</div>

<p class="nb-anuncio-tablero" role="status" aria-live="polite">{anuncio}</p>

<style>
  .nb-tablero {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
    min-width: 0;
  }

  .nb-tablero-barra {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-2) var(--nb-space-3);
  }

  .nb-tablero-titulo {
    margin: 0;
    font-family: var(--nb-font-display);
    font-size: var(--nb-text-xl);
    font-weight: 600;
    color: var(--m3c-on-surface);
  }

  .nb-tablero-cifra {
    font-size: var(--nb-text-sm);
    font-variant-numeric: tabular-nums;
    color: var(--m3c-on-surface-variant);
  }

  .nb-tablero-hueco {
    flex: 1 1 auto;
  }

  .nb-tablero-ayuda,
  .nb-tarjeta-anticipo {
    margin: 0;
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }

  .nb-tablero-aviso {
    margin: 0;
    padding: var(--nb-space-2) var(--nb-space-3);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-tertiary-container);
    color: var(--m3c-on-tertiary-container);
    font-size: var(--nb-text-sm);
  }

  /*
   * Columnas que bajan a la fila siguiente cuando no caben, y nunca un
   * desplazamiento lateral. `min(100%, …)` hace que en una ventana estrecha la
   * columna ocupe el ancho entero en lugar de desbordarlo.
   */
  .nb-tablero-columnas {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 15rem), 1fr));
    align-items: start;
    gap: var(--nb-space-3);
  }

  .nb-columna {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    min-width: 0;
    padding: var(--nb-space-3);
    border-radius: var(--nb-radius-lg);
    background: var(--m3c-surface-container);
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-columna[data-destino='true'] {
    background: var(--m3c-surface-container-high);
  }

  .nb-columna-nueva {
    background: transparent;
    border: 1px dashed var(--m3c-outline-variant);
  }

  /*
   * El nombre ocupa la primera línea entera y las acciones van debajo. En la misma
   * línea, cuatro botones dejaban el nombre en tres letras y puntos suspensivos,
   * y lo que dice una columna es precisamente su nombre.
   */
  .nb-columna-cabecera {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0 var(--nb-space-2);
    min-height: 2.5rem;
  }

  .nb-columna-nombre {
    flex: 1 1 auto;
    min-width: 0;
    margin: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: var(--nb-text-md);
    font-weight: 600;
    color: var(--m3c-on-surface);
  }

  .nb-columna-cuenta {
    flex: 0 0 auto;
    padding: 0 var(--nb-space-2);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-highest);
    font-size: var(--nb-text-xs);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--m3c-on-surface-variant);
  }

  .nb-columna-acciones {
    display: flex;
    flex: 1 1 100%;
    justify-content: flex-end;
    margin-block-start: calc(-1 * var(--nb-space-1));
  }

  .nb-columna-nombre-campo {
    flex: 1 1 auto;
    min-width: 0;
    box-sizing: border-box;
    width: 100%;
    padding: var(--nb-space-2) var(--nb-space-3);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-sm);
  }

  .nb-columna-nombre-campo:focus {
    border-color: var(--m3c-primary);
  }

  .nb-columna-tarjetas {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-tarjeta {
    position: relative;
  }

  /*
   * Dónde caería lo que se arrastra: una raya en el acento encima de la tarjeta.
   * Una sombra y no un hueco, para que la columna no se recomponga en cada
   * movimiento del puntero.
   */
  .nb-tarjeta[data-soltar='true'] .nb-tarjeta-boton {
    box-shadow: 0 -3px 0 0 var(--m3c-primary);
  }

  .nb-columna-final {
    display: block;
    height: 3px;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-primary);
  }

  .nb-tarjeta-boton {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    width: 100%;
    padding: var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-lowest);
    color: var(--m3c-on-surface);
    font: inherit;
    text-align: start;
    cursor: grab;
    box-shadow: var(--m3-elevation-1);
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-tarjeta-boton:hover {
    background: var(--m3c-surface-container-low);
  }

  .nb-tarjeta-boton[data-arrastrada='true'] {
    opacity: 0.5;
  }

  .nb-tarjeta-titulo {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    overflow-wrap: anywhere;
    font-size: var(--nb-text-sm);
    font-weight: 600;
    line-height: 1.3;
  }

  .nb-tarjeta-enlace .nb-tarjeta-titulo {
    color: var(--m3c-primary);
  }

  .nb-tarjeta-boton .nb-tarjeta-anticipo {
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    font-size: var(--nb-text-xs);
    line-height: 1.4;
  }

  .nb-columna-anadir {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    width: 100%;
    min-height: 2.5rem;
    padding: 0 var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-md);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    font: inherit;
    font-size: var(--nb-text-sm);
    font-weight: 500;
    text-align: start;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-columna-anadir:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
    color: var(--m3c-on-surface);
  }

  .nb-columna-formulario {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
  }

  .nb-columna-formulario-acciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
  }

  .nb-tablero-lista {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-tablero-lista ol {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    margin: var(--nb-space-2) 0 0;
    padding-inline-start: var(--nb-space-5);
  }

  .nb-tablero-lista-columna {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    margin: 0;
    font-size: var(--nb-text-md);
  }

  .nb-tablero-lista-tarjeta {
    padding: 0;
    border: none;
    background: transparent;
    color: var(--m3c-on-surface);
    font: inherit;
    font-weight: 600;
    text-align: start;
    cursor: pointer;
    margin-inline-end: var(--nb-space-2);
  }

  .nb-tablero-lista-tarjeta:hover {
    color: var(--m3c-primary);
  }
  .nb-anuncio-tablero {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
