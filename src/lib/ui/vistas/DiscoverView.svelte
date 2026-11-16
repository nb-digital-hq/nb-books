<!--
  @component
  Descubrir: navegar los catálogos OPDS que el usuario ha añadido.

  Contrato: ADR-013.

  **Ningún catálogo viene puesto**, y por eso este destino no aparece hasta que
  hay al menos uno: un destino vacío que solo sirve para enseñar un formulario de
  configuración es la regla 5 de ADR-013 vista del otro lado. Se añaden en
  Ajustes, que es donde va lo que se configura una vez.

  Lo que este componente **no** hace, y son las dos cosas que hacen comprobable
  su criterio:

  1. **No descarga.** Avisa de qué se ha pulsado; quien sale a la red es el
     runtime, detrás de la puerta de `net.rs`, que comprueba el interruptor y el
     consentimiento del anfitrión antes de abrir nada.
  2. **No decide si un archivo sirve.** Lo que se descarga entra por la
     canalización de importación de siempre, que reconoce el formato por
     contenido. Aquí el tipo que declara el catálogo se **enseña**, no se cree.
-->
<script lang="ts">
  import { textos } from '$lib/i18n';

  import type { Catalog, Feed, FeedEntry } from '$lib/bindings';

  import Spinner from '../Spinner.svelte';
  import Button from '../Button.svelte';
  import NavItem from '../NavItem.svelte';
  import { usarCarcasa } from '../carcasa.svelte';
  import Icon from '../Icon.svelte';
  import EmptyState from './EmptyState.svelte';
  import Seccion from './Seccion.svelte';

  interface Props {
    /** Catálogos añadidos. Vacío es un estado válido, no un error. */
    catalogos?: Catalog[];
    /** El feed que se está viendo, o nulo si todavía no se ha abierto ninguno. */
    feed?: Feed | null;
    /** Dirección del feed abierto. Es a lo que se autoriza cuando falta permiso. */
    abierto?: string | null;
    /**
     * Dirección del catálogo **desde el que se entró**, que no cambia al
     * navegar dentro.
     *
     * Va aparte de `abierto` porque son dos cosas distintas y confundirlas se
     * veía: marcando el catálogo activo por el feed en curso, entrar en
     * cualquier sección lo desmarcaba y dejaba de saberse en qué catálogo se
     * estaba. Es el mismo criterio de la barra lateral: lo marcado es dónde
     * estás, no qué acabas de pulsar.
     */
    raiz?: string | null;
    /** Si hay algún sitio al que volver dentro de este catálogo. */
    puedeVolver?: boolean;
    alVolver?: () => void;
    cargando?: boolean;
    /** Código canónico del fallo, nunca un mensaje. */
    error?: string | null;
    /** Cómo terminó la última descarga. */
    descarga?: { titulo: string; estado: 'ok' | 'repetido' | 'rechazado' } | null;
    descargando?: boolean;
    alAbrirCatalogo?: (url: string) => void;
    /**
     * El anfitrión al que falta autorizar, dicho por el runtime, y si es en
     * claro. Puede no ser el del catálogo: es el de la redirección que se seguía,
     * o el del servidor de las descargas.
     */
    permiso?: { host: string; cleartext: boolean } | null;
    alAutorizar?: (host: string, cleartext: boolean) => void;
    alNavegar?: (url: string) => void;
    alDescargar?: (entrada: FeedEntry) => void;
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    catalogos = [],
    feed = null,
    abierto = null,
    raiz = null,
    puedeVolver = false,
    alVolver,
    cargando = false,
    error = null,
    descarga = null,
    descargando = false,
    alAbrirCatalogo,
    permiso = null,
    alAutorizar,
    alNavegar,
    alDescargar,
    etiquetas
  }: Props = $props();

  const t = $derived({ ...textos().descubrir, ...etiquetas } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  /*
   * La clase de ancho la decide la carcasa, no una consulta de medios de aquí.
   * Es la regla de ADR-012 que `check-ui.mjs` hace cumplir, y tiene motivo: una
   * vista dentro de un panel mide menos que la ventana, así que preguntarle al
   * navegador por el ancho de la ventana daría la respuesta equivocada.
   */
  const carcasa = usarCarcasa();
  const enUnaColumna = $derived(carcasa.claseDelContenido === 'compacto');
</script>

<div class="nb-descubrir" aria-label={texto('region')} data-una-columna={enUnaColumna}>
  <Seccion titulo={texto('catalogos')}>
    {#if catalogos.length === 0}
      <p class="nb-ayuda">{texto('sinCatalogos')}</p>
    {:else}
      <ul class="nb-lista">
        {#each catalogos as catalogo (catalogo.id)}
          <li>
            <NavItem
              icono="descubrir"
              etiqueta={catalogo.title}
              activo={raiz === catalogo.url}
              onclick={() => alAbrirCatalogo?.(catalogo.url)}
            />
          </li>
        {/each}
      </ul>
    {/if}
  </Seccion>

  <div class="nb-contenido">
    {#if cargando}
      <Spinner etiqueta={texto('cargando')} />
    {:else if error}
      <!--
        Tres códigos, tres textos y tres acciones distintas. Con uno solo, la
        interfaz diría "no se pudo" cuando lo que hace falta es apagar el modo
        sin red o conceder un permiso.
      -->
      <div class="nb-error-accionable" role="status">
        {#if error === 'CONSENT_REQUIRED' && permiso}
          <!--
            Se nombra el anfitrión: puede no ser el del catálogo que se abrió, y
            autorizar sin saber a quién es justo lo que producía el bucle.
          -->
          <p class="nb-aviso">
            <Icon nombre={permiso.cleartext ? 'advertencia' : 'informacion'} tamano={15} />
            <span>
              {permiso.cleartext ? texto('faltaPermisoEnClaro') : texto('faltaPermiso')}
              <strong class="nb-anfitrion">{permiso.host}</strong>
            </span>
          </p>
          <Button onclick={() => alAutorizar?.(permiso.host, permiso.cleartext)}>
            {permiso.cleartext ? texto('autorizarEnClaro') : texto('autorizar')}
          </Button>
        {:else}
          <p class="nb-aviso">
            <Icon nombre="informacion" tamano={15} />
            <span>{texto(`error-${error}`)}</span>
          </p>
        {/if}
      </div>
    {:else if !feed}
      <EmptyState icono="descubrir" mensaje={texto('elige')} />
    {:else}
      <Seccion titulo={feed.title || texto('catalogo')}>
        {#snippet accion()}
          {#if puedeVolver}
            <!--
              Volver, no "ir al inicio del catálogo".

              Navegar un catálogo es entrar en secciones, y sin vuelta atrás
              entrar es un viaje de ida: la única salida era pulsar el catálogo
              otra vez y perder el camino recorrido.
            -->
            <Button onclick={() => alVolver?.()}>{texto('volver')}</Button>
          {/if}
        {/snippet}
        {#if feed.entries.length === 0}
          <p class="nb-ayuda">{texto('vacio')}</p>
        {:else}
          <ul class="nb-lista">
            {#each feed.entries as entrada, indice (`${entrada.title}-${indice}`)}
              <li class="nb-entrada">
                <span class="nb-fila-titulo">{entrada.title}</span>
                {#if entrada.authors}
                  <span class="nb-ayuda">{entrada.authors}</span>
                {/if}
                {#if entrada.summary}
                  <span class="nb-ayuda nb-resumen">{entrada.summary}</span>
                {/if}
                <span class="nb-acciones">
                  {#if entrada.navigation}
                    <Button onclick={() => alNavegar?.(entrada.navigation ?? '')}>
                      {texto('abrir')}
                    </Button>
                  {/if}
                  {#if entrada.download}
                    <Button variante="primario" onclick={() => alDescargar?.(entrada)}>
                      {descargando ? texto('descargando') : texto('descargar')}
                    </Button>
                    {#if entrada.mediaType}
                      <!-- Lo que **dice** que es. No se cree: la importación lo
                           reconoce por contenido. -->
                      <span class="nb-ayuda">{entrada.mediaType}</span>
                    {/if}
                  {/if}
                </span>
              </li>
            {/each}
          </ul>
        {/if}

        {#if feed.next}
          <Button onclick={() => alNavegar?.(feed?.next ?? '')}>{texto('siguiente')}</Button>
        {/if}
      </Seccion>
    {/if}

    {#if descarga}
      <p class="nb-aviso" role="status">
        <Icon nombre={descarga.estado === 'rechazado' ? 'advertencia' : 'correcto'} tamano={15} />
        <span>{texto(`descarga-${descarga.estado}`)} {descarga.titulo}</span>
      </p>
    {/if}
  </div>
</div>

<style>
  .nb-descubrir {
    display: grid;
    grid-template-columns: var(--nb-lateral-ancho) minmax(0, 1fr);
    gap: var(--nb-space-4);
    height: 100%;
    padding: var(--nb-space-4);
    overflow: auto;
  }

  /* En clase compacta, una columna: la lista arriba y el feed debajo. */
  .nb-descubrir[data-una-columna='true'] {
    grid-template-columns: minmax(0, 1fr);
  }

  /*
   * Los catálogos van en su propia capa, a la izquierda de lo que se está viendo.
   * Sin separarlos, elegir a la izquierda y ver a la derecha no se lee como una
   * relación sino como dos listas sueltas; lo separa la capa y no
   * una raya.
   */
  .nb-descubrir > :first-child {
    align-self: start;
    padding: var(--nb-space-4) var(--nb-space-3);
    border-radius: var(--nb-radius-lg);
    background: var(--m3c-surface-container);
  }

  .nb-contenido {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
    min-width: 0;
  }

  .nb-lista {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-entrada {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    padding: var(--nb-space-4);
    border-radius: var(--nb-radius-lg);
    background: var(--m3c-surface-container);
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-entrada:hover {
    background: var(--m3c-surface-container-high);
  }

  .nb-fila-titulo {
    color: var(--m3c-on-surface);
    font-family: var(--nb-font-display);
    font-size: var(--nb-text-md);
    font-weight: 600;
  }

  .nb-resumen {
    /* Un resumen puede traer un párrafo entero: se enseñan tres líneas, que es
       lo que se lee de un vistazo antes de decidir. */
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
  }

  .nb-acciones {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-2);
    margin-top: var(--nb-space-1);
  }

  .nb-ayuda {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }

  .nb-error-accionable {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--nb-space-3);
  }

  .nb-anfitrion {
    font-family: monospace;
    word-break: break-all;
  }

  .nb-aviso {
    display: flex;
    align-items: flex-start;
    gap: var(--nb-space-2);
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }
</style>
