<!--
  @component
  La obra seleccionada: portada grande, progreso y lo que se sabe de ella.

  Contrato: docs/design/ui-conventions.md, sección 4..

  Existe por dos motivos. Uno, que la biblioteca deje de ser una rejilla sobre un
  fondo vacío: con el panel siempre hay algo a la derecha. Y dos, que la ficha de
  una obra tenga un sitio; sin él, los datos que el runtime ya guarda —copias,
  disponibilidad, anotaciones— no se ven en ninguna parte.

  Sigue al **foco** y no a un clic aparte: el foco llega igual con el teclado que
  con el ratón, así que no hace falta inventar un segundo gesto para elegir.
-->
<script lang="ts">
  import { porcentajeLeido } from '$core/progreso';
  import { editionDetails, rutaDeImagen } from '$core/ipc';
  import { editarCampoDeEdicion, quitarDocumento } from '$core/ipc/ficha';
  import { readingPace } from '$core/ipc/lector';
  import { idioma, textos } from '$lib/i18n';
  import { TEXTOS_DE_LA_FICHA } from '$lib/i18n/locales/ficha';
  import type { EditionDetails, ReadingPace } from '$lib/bindings';
  import type {
    BrowseItem,
    CollectionSummary,
    ExtensionCommand,
    MetadataCandidate,
    MetadataSuggestion
  } from '$lib/bindings';

  import Button from '../Button.svelte';
  import Field from '../Field.svelte';
  import Icon from '../Icon.svelte';
  import Switch from '../Switch.svelte';
  import { queda } from '../lector/ritmo';
  import { textoDeQueda } from '../lector/texto-de-ritmo';
  import Cover from './Cover.svelte';
  import EmptyState from './EmptyState.svelte';

  interface Props {
    item: BrowseItem | null;
    alAbrir?: (item: BrowseItem) => void;
    /**
     * Candidatos del catálogo abierto para completar esta ficha.
     *
     * Vacío mientras nadie los pida: la consulta es **explícita, nunca
     * automática**. Una biblioteca de ciento cincuenta mil obras que consultara
     * sola al importar haría ciento cincuenta mil peticiones que nadie pidió.
     */
    candidatos?: MetadataCandidate[];
    /**
     * Lo que las extensiones aportan sobre este documento.
     *
     * Sale del manifiesto, así que está aquí **antes** de que se cargue una
     * sola instrucción de la extensión: pulsar es lo que la carga.
     */
    ordenesDeExtension?: ExtensionCommand[];
    alEjecutarOrden?: (extension: string, orden: string) => void;
    buscandoEnLinea?: boolean;
    /** Código canónico del fallo de la última consulta, si lo hubo. */
    errorEnLinea?: string | null;
    alBuscarEnLinea?: () => void;
    /** Autoriza el catálogo de metadatos y vuelve a buscar. */
    alAutorizarEnLinea?: () => void;
    alAplicarCandidato?: (candidato: MetadataCandidate) => void;
    /**
     * Lo que llegó de fuera y **no se aplicó** porque el usuario ya había
     * decidido ese campo. No se descarta: se ofrece.
     */
    propuestas?: MetadataSuggestion[];
    alAceptarPropuesta?: (id: string) => void;
    alRechazarPropuesta?: (id: string) => void;
    /**
     * Las colecciones del usuario, para poner o quitar esta obra.
     *
     * Aquí y no en el menú de colecciones: añadir un libro se hace donde está el
     * libro. Solo se ofrecen las manuales, porque en una por regla la
     * pertenencia la decide su criterio.
     */
    colecciones?: CollectionSummary[];
    /** En cuáles de ellas está ya. */
    enColecciones?: string[];
    alCambiarColeccion?: (coleccion: string, dentro: boolean) => void;
    /** Lo que hay que hacer antes de la baja, como cerrar el documento si está abierto. */
    alAntesDeQuitar?: (editionId: string) => Promise<void>;
    /** El documento se quitó de la biblioteca: quien monta la ficha la cierra. */
    alQuitado?: (editionId: string) => void;
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    item,
    alAbrir,
    candidatos = [],
    ordenesDeExtension = [],
    alEjecutarOrden,
    buscandoEnLinea = false,
    errorEnLinea = null,
    alBuscarEnLinea,
    alAutorizarEnLinea,
    alAplicarCandidato,
    propuestas = [],
    alAceptarPropuesta,
    alRechazarPropuesta,
    colecciones = [],
    enColecciones = [],
    alCambiarColeccion,
    alAntesDeQuitar,
    alQuitado,
    etiquetas
  }: Props = $props();

  // El diccionario del idioma en uso, con las anulaciones de quien monta el
  // componente encima. `$derived` y no una constante: cambiar de idioma tiene
  // que repintar, y una constante capturaría el diccionario de un solo momento.
  const t = $derived({
    ...textos().ficha,
    ...TEXTOS_DE_LA_FICHA[idioma()],
    ...etiquetas
  } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  const porcentaje = $derived(porcentajeLeido(item?.progression));
  const disponible = $derived(item?.availability === 'available');

  /*
   * Lo que queda a tu ritmo: el ritmo sale de las sesiones de esta obra,
   * y sin bastantes no se dice nada.
   */
  let ritmo = $state<ReadingPace | null>(null);
  $effect(() => {
    const edicion = item?.editionId;
    ritmo = null;
    if (!edicion || item?.progression === null) return;
    readingPace(edicion)
      .then((r) => {
        if (item?.editionId === edicion) ritmo = r;
      })
      .catch(() => undefined);
  });
  /*
   * Lo que la base sabe de la obra y la ficha no enseñaba: editorial,
   * fecha, idioma, materias y dónde está cada copia.
   */
  let detalles = $state<EditionDetails | null>(null);
  $effect(() => {
    const edicion = item?.editionId;
    detalles = null;
    if (!edicion) return;
    editionDetails(edicion)
      .then((d) => {
        if (item?.editionId === edicion) detalles = d;
      })
      .catch(() => undefined);
  });

  /** Una fecha de la base, en el idioma de la interfaz. Un año solo se deja solo. */
  function fecha(valor: string | null | undefined): string | null {
    if (!valor) return null;
    if (/^\d{4}$/.test(valor)) return valor;
    const f = new Date(valor);
    if (Number.isNaN(f.getTime())) return valor;
    return new Intl.DateTimeFormat(idioma(), { dateStyle: 'long' }).format(f);
  }

  function nombreDeIdioma(codigo: string | null | undefined): string | null {
    if (!codigo) return null;
    try {
      const nombre = new Intl.DisplayNames([idioma()], { type: 'language' }).of(codigo);
      return nombre ? nombre.charAt(0).toUpperCase() + nombre.slice(1) : codigo;
    } catch {
      return codigo;
    }
  }

  const FORMATOS: Record<string, string> = {
    'application/epub+zip': 'EPUB',
    'application/pdf': 'PDF',
    'application/vnd.comicbook+zip': 'CBZ'
  };

  function tamano(bytes: number): string {
    const mb = bytes / (1024 * 1024);
    const n = new Intl.NumberFormat(idioma(), { maximumFractionDigits: mb < 10 ? 1 : 0 });
    return mb >= 1 ? `${n.format(mb)} MB` : `${n.format(bytes / 1024)} KB`;
  }

  const loQueQueda = $derived(textoDeQueda(queda(ritmo, item?.progression ?? null)));
  const manuales = $derived(
    colecciones.filter((c) => c.kind === 'manual' && c.origin === 'personal')
  );

  /*
   * Corregir la ficha a mano. Lo escrito aquí gana sobre el archivo, Calibre y
   * el catálogo en línea, y lo dice la procedencia. El autor no se edita aquí:
   * es una persona compartida con otras obras.
   */
  const CAMPOS = [
    'title',
    'series_title',
    'series_index',
    'publisher',
    'published_date',
    'language',
    'description'
  ] as const;
  type Campo = (typeof CAMPOS)[number];
  let editando = $state(false);
  let borrador = $state<Record<Campo, string>>({} as Record<Campo, string>);
  let fallo = $state<string | null>(null);

  function valoresActuales(): Record<Campo, string> {
    return {
      title: item?.title ?? '',
      series_title: item?.seriesTitle ?? '',
      series_index: item?.seriesIndex != null ? String(item.seriesIndex) : '',
      publisher: detalles?.publisher ?? '',
      published_date: detalles?.publishedDate ?? '',
      language: detalles?.language ?? '',
      description: detalles?.description ?? ''
    };
  }

  function empezarAEditar(): void {
    borrador = valoresActuales();
    fallo = null;
    editando = true;
  }

  async function guardarEdicion(): Promise<void> {
    const edicion = item?.editionId;
    if (!edicion) return;
    if (borrador.title.trim() === '') {
      fallo = texto('tituloObligatorio');
      return;
    }
    const antes = valoresActuales();
    try {
      for (const campo of CAMPOS) {
        if (borrador[campo].trim() === antes[campo].trim()) continue;
        await editarCampoDeEdicion(edicion, campo, borrador[campo].trim() || null);
      }
      editando = false;
      // La rejilla se relee sola por el anuncio del runtime; la ficha pide lo suyo.
      detalles = await editionDetails(edicion);
    } catch {
      fallo = texto('noSeGuardo');
    }
  }

  /*
   * Quitar de la biblioteca. Dos pasos, porque se lleva anotaciones, notas
   * ancladas y progreso. El archivo referenciado no se toca nunca; la copia que
   * hizo la aplicación solo se borra si se marca.
   */
  let confirmandoBaja = $state(false);
  let borrarCopia = $state(false);
  const tieneCopiaPropia = $derived((detalles?.files ?? []).some((f) => f.managed));

  async function quitar(): Promise<void> {
    const edicion = item?.editionId;
    if (!edicion) return;
    try {
      await alAntesDeQuitar?.(edicion);
      await quitarDocumento(edicion, tieneCopiaPropia && borrarCopia);
      confirmandoBaja = false;
      alQuitado?.(edicion);
    } catch {
      fallo = texto('noSeQuito');
    }
  }

  // Otra obra elegida: lo que estaba a medias era de la anterior.
  $effect(() => {
    void item?.editionId;
    editando = false;
    confirmandoBaja = false;
    borrarCopia = false;
    fallo = null;
  });
</script>

{#if !item}
  <EmptyState icono="biblioteca" mensaje={texto('vacio')} />
{:else}
  <div class="nb-detalle">
    <Cover
      titulo={item.title}
      autor={item.authors}
      imagen={item.coverPath ? rutaDeImagen(item.coverPath) : null}
      ancho={132}
      progreso={item.progression}
    />

    <div class="nb-cabecera">
      <h3 class="nb-detalle-titulo">{item.title}</h3>
      {#if item.authors}<p class="nb-detalle-autor">{item.authors}</p>{/if}
    </div>

    {#if !disponible}
      <!--
        No disponible no es un error de la aplicación ni una obra perdida: es un
        volumen que no está. Se dice con lo que hay que hacer, y se dice que las
        anotaciones no se han ido, que es lo que de verdad preocupa.
      -->
      <p class="nb-aviso" role="status">
        <Icon nombre="noDisponible" tamano={15} />
        <span>
          {texto('noDisponible')}
          <span class="nb-aviso-ayuda">{texto('noDisponibleAyuda')}</span>
        </span>
      </p>
    {:else}
      <Button variante="primario" onclick={() => alAbrir?.(item)}>
        {porcentaje !== null && porcentaje > 0 ? texto('continuar') : texto('leer')}
      </Button>
    {/if}

    {#if detalles?.description}
      <!-- La sinopsis del propio libro, que no se leía. -->
      <p class="nb-sinopsis">{detalles.description}</p>
    {/if}

    <dl class="nb-datos">
      <div>
        <dt>{texto('progreso')}</dt>
        <dd>
          {#if porcentaje === null}
            {texto('sinEmpezar')}
          {:else if porcentaje >= 99}
            {texto('terminado')}
          {:else}
            {porcentaje} %{#if loQueQueda}, {loQueQueda}{/if}
          {/if}
        </dd>
      </div>
      {#if item.seriesTitle}
        <div>
          <dt>{texto('serie')}</dt>
          <dd>{item.seriesTitle}{item.seriesIndex ? ` · ${item.seriesIndex}` : ''}</dd>
        </div>
      {/if}
      <div>
        <dt>{texto('anadido')}</dt>
        <dd>{fecha(item.addedAt)}</dd>
      </div>
      {#if detalles?.publisher}
        <div>
          <dt>{texto('editorial')}</dt>
          <dd>{detalles.publisher}</dd>
        </div>
      {/if}
      {#if detalles?.publishedDate}
        <div>
          <dt>{texto('publicado')}</dt>
          <dd>{fecha(detalles.publishedDate)}</dd>
        </div>
      {/if}
      {#if detalles?.language}
        <div>
          <dt>{texto('idioma')}</dt>
          <dd>{nombreDeIdioma(detalles.language)}</dd>
        </div>
      {/if}
      {#if detalles && detalles.subjects.length > 0}
        <div>
          <dt>{texto('materias')}</dt>
          <dd>{detalles.subjects.join(', ')}</dd>
        </div>
      {/if}
      {#each detalles?.files ?? [] as copia (copia.relativePath)}
        <div>
          <dt>{texto('archivo')}</dt>
          <dd class="nb-dato-archivo">
            {FORMATOS[copia.mediaType] ?? copia.mediaType} · {tamano(copia.byteSize)}
            <span class="nb-dato-ruta" title={copia.relativePath}
              >{copia.root} · {copia.relativePath}</span
            >
          </dd>
        </div>
      {/each}
      {#if item.copies > 1}
        <div>
          <dt>{texto('copias')}</dt>
          <dd class="nb-dato-marcado">{item.copies} {texto('copiasValor')}</dd>
        </div>
      {/if}
    </dl>

    <!--
      A qué colecciones pertenece, y se cambia pulsando. Cada una es un
      conmutador con su estado dicho por `aria-pressed`: el color de la marcada
      no se lo dice a quien no lo ve.
    -->
    <section class="nb-en-linea" aria-label={texto('colecciones')}>
      <h4 class="nb-subtitulo">{texto('colecciones')}</h4>
      {#if manuales.length === 0}
        <p class="nb-aviso-ayuda">{texto('coleccionesVacio')}</p>
      {:else}
        <ul class="nb-chips">
          {#each manuales as coleccion (coleccion.id)}
            {@const dentro = enColecciones.includes(coleccion.id)}
            <li>
              <button
                type="button"
                class="nb-chip"
                aria-pressed={dentro}
                onclick={() => alCambiarColeccion?.(coleccion.id, !dentro)}
              >
                {#if dentro}<Icon nombre="comprobado" tamano={16} />{/if}
                {coleccion.name}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <!--
      Lo que aportan las extensiones sobre este documento.
      
      Van aquí y no en un menú que se abre con el botón derecho: ADR-012,
      restricción 5, prohíbe la acción exclusiva de ratón, y un menú contextual
      clásico lo sería. La ficha **es** el contexto de un documento y ya se
      alcanza con el teclado, así que la contribución tiene sitio sin inventar
      un gesto que dejaría fuera a quien no usa ratón.
      
      Los títulos los pone quien escribió la extensión y no se traducen: son
      suyos, igual que su nombre.
    -->
    {#if ordenesDeExtension.length > 0}
      <section class="nb-en-linea" aria-label={texto('ordenesDeExtension')}>
        <p class="nb-aviso-ayuda">{texto('ordenesDeExtensionAyuda')}</p>
        <ul class="nb-candidatos">
          {#each ordenesDeExtension as orden (`${orden.extension}-${orden.id}`)}
            <li>
              <button
                type="button"
                class="nb-candidato"
                onclick={() => alEjecutarOrden?.(orden.extension, orden.id)}
              >
                <span class="nb-candidato-titulo">{orden.title}</span>
                <span class="nb-aviso-ayuda">
                  {[orden.category, orden.extension].filter(Boolean).join(' · ')}
                </span>
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <!--
      Completar la ficha desde un catálogo abierto. El botón está
      siempre, y no solo cuando faltan datos: quien quiere corregir una ficha
      completa pero equivocada tiene el mismo derecho que quien la tiene vacía.
    -->
    <section class="nb-en-linea" aria-label={texto('completar')}>
      <Button onclick={() => alBuscarEnLinea?.()}>
        {buscandoEnLinea ? texto('buscando') : texto('completar')}
      </Button>
      <p class="nb-aviso-ayuda">{texto('completarAyuda')}</p>

      {#if errorEnLinea}
        <p class="nb-aviso" role="status">
          <Icon nombre="informacion" tamano={15} />
          <span>{texto(`error-${errorEnLinea}`)}</span>
        </p>
        {#if errorEnLinea === 'CONSENT_REQUIRED' && alAutorizarEnLinea}
          <!-- Se autoriza aquí, como en Descubrir, y no mandando a Ajustes. -->
          <Button variante="primario" onclick={() => alAutorizarEnLinea?.()}>
            {texto('autorizar')}
          </Button>
        {/if}
      {:else if candidatos.length > 0}
        <ul class="nb-candidatos">
          {#each candidatos as candidato, indice (`${candidato.title}-${indice}`)}
            <li>
              <button
                type="button"
                class="nb-candidato"
                onclick={() => alAplicarCandidato?.(candidato)}
              >
                <span class="nb-candidato-titulo">{candidato.title}</span>
                <span class="nb-aviso-ayuda">
                  {[
                    candidato.authors,
                    candidato.publisher,
                    candidato.publishedDate,
                    // De dónde salió. Solo aparece cuando lo propone una
                    // extensión: quien elige entre varias fichas tiene que poder
                    // distinguir la del catálogo de siempre de la de algo que
                    // instaló la semana pasada.
                    candidato.source
                  ]
                    .filter(Boolean)
                    .join(' · ')}
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    {#if propuestas.length > 0}
      <!--
        Lo que llegó y no se aplicó porque el usuario ya había decidido ese
        campo. Descartarlo en silencio perdería información igual que
        sobrescribir: por eso se ofrece, y rechazarlo no vuelve a preguntar.
      -->
      <section class="nb-en-linea" aria-label={texto('propuestas')}>
        <h4 class="nb-subtitulo">{texto('propuestas')}</h4>
        <ul class="nb-candidatos">
          {#each propuestas as propuesta (propuesta.id)}
            <li class="nb-propuesta">
              <span class="nb-candidato-titulo">{propuesta.proposedValue ?? ''}</span>
              <span class="nb-aviso-ayuda">{texto(`campo-${propuesta.field}`)}</span>
              <span class="nb-acciones-propuesta">
                <Button onclick={() => alAceptarPropuesta?.(propuesta.id)}>
                  {texto('aceptar')}
                </Button>
                <Button variante="peligro" onclick={() => alRechazarPropuesta?.(propuesta.id)}>
                  {texto('rechazar')}
                </Button>
              </span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <section class="nb-en-linea" aria-label={texto('editar')}>
      {#if editando}
        <h4 class="nb-subtitulo">{texto('editar')}</h4>
        {#each CAMPOS as campo (campo)}
          <Field etiqueta={texto(`campo-${campo}`)}>
            {#snippet children({ id })}
              {#if campo === 'description'}
                <textarea
                  {id}
                  class="nb-entrada nb-entrada-larga"
                  rows="5"
                  bind:value={borrador[campo]}></textarea>
              {:else}
                <input {id} class="nb-entrada" type="text" bind:value={borrador[campo]} />
              {/if}
            {/snippet}
          </Field>
        {/each}
        <p class="nb-aviso-ayuda">{texto('editarAyuda')}</p>
        <span class="nb-acciones-propuesta">
          <Button variante="primario" onclick={guardarEdicion}>{texto('guardar')}</Button>
          <Button onclick={() => (editando = false)}>{texto('cancelar')}</Button>
        </span>
      {:else}
        <Button onclick={empezarAEditar}>{texto('editar')}</Button>
      {/if}
    </section>

    <section class="nb-en-linea" aria-label={texto('quitar')}>
      {#if confirmandoBaja}
        <p class="nb-aviso" role="alert">
          <Icon nombre="informacion" tamano={15} />
          <span>{texto('quitarAviso')}</span>
        </p>
        {#if tieneCopiaPropia}
          <div class="nb-conmutador">
            <span>{texto('quitarCopia')}</span>
            <Switch
              etiqueta={texto('quitarCopia')}
              activo={borrarCopia}
              alCambiar={(v) => (borrarCopia = v)}
            />
          </div>
        {/if}
        <span class="nb-acciones-propuesta">
          <Button variante="peligro" onclick={quitar}>{texto('quitarConfirmar')}</Button>
          <Button onclick={() => (confirmandoBaja = false)}>{texto('cancelar')}</Button>
        </span>
      {:else}
        <Button variante="peligro" onclick={() => (confirmandoBaja = true)}>
          {texto('quitar')}
        </Button>
      {/if}
    </section>

    {#if fallo}
      <p class="nb-aviso" role="alert">
        <Icon nombre="informacion" tamano={15} />
        <span>{fallo}</span>
      </p>
    {/if}
  </div>
{/if}

<style>
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

  .nb-entrada-larga {
    padding: var(--nb-space-2) var(--nb-space-3);
    resize: vertical;
  }

  .nb-entrada:focus-visible {
    border-color: var(--m3c-primary);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--m3c-primary) 20%, transparent);
  }

  .nb-conmutador {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-3);
    font-size: var(--nb-text-sm);
  }

  .nb-en-linea {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding-top: var(--nb-space-2);
  }

  .nb-chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-chip {
    display: inline-flex;
    align-items: center;
    gap: var(--nb-space-1);
    min-height: 2rem;
    padding: 0 var(--nb-space-3);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-sm);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    font: inherit;
    font-size: var(--nb-text-xs);
    font-weight: 500;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-chip:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-chip[aria-pressed='true'] {
    border-color: transparent;
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font-weight: 600;
  }

  .nb-subtitulo {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .nb-candidatos {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-candidato {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: var(--nb-space-2) var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-candidato:active {
    background: color-mix(in srgb, var(--m3c-primary) 15%, transparent);
  }

  .nb-candidato:hover {
    background: color-mix(in srgb, var(--m3c-surface-container-highest) 50%, transparent);
  }

  .nb-candidato-titulo {
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .nb-propuesta {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
  }

  .nb-acciones-propuesta {
    display: flex;
    gap: var(--nb-space-2);
  }

  .nb-detalle {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-4);
  }

  .nb-cabecera {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
  }

  .nb-detalle-titulo {
    margin: 0;
    font-family: var(--nb-font-ui);
    font-size: var(--nb-text-lg);
    font-weight: 600;
    line-height: 1.2;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .nb-detalle-autor {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .nb-aviso {
    display: flex;
    align-items: flex-start;
    gap: var(--nb-space-2);
    margin: 0;
    padding: var(--nb-space-3);
    border-radius: var(--nb-radius-xs);
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-error);
    font-size: var(--nb-text-sm);
    overflow-wrap: anywhere;
    word-break: break-word;
    line-height: 1.4;
  }

  .nb-aviso-ayuda {
    display: block;
    margin-top: var(--nb-space-1);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }

  .nb-datos {
    display: flex;
    flex-direction: column;
    margin: 0;
  }

  .nb-datos > div {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--nb-space-3);
    min-height: var(--nb-fila-compacta);
    padding: var(--nb-space-1) 0;
    font-size: var(--nb-text-sm);
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .nb-datos dt {
    color: var(--m3c-on-surface-variant);
  }

  .nb-datos dd {
    margin: 0;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .nb-dato-marcado {
    color: var(--m3c-primary);
  }
  .nb-sinopsis {
    margin: 0;
    font-size: var(--nb-text-sm);
    line-height: 1.5;
    color: var(--m3c-on-surface-variant);
    max-height: 12lh;
    overflow-y: auto;
  }

  .nb-dato-archivo {
    display: grid;
    gap: var(--nb-space-1);
    min-width: 0;
  }

  .nb-dato-ruta {
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
    overflow-wrap: anywhere;
  }
</style>
