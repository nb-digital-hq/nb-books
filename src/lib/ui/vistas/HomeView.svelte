<!--
  @component
  Inicio: por dónde iba el usuario, no qué tiene.

  Contrato: ADR-013, regla 1, y su enmienda de recomendaciones reproducibles.

  Es la pantalla de arranque, y esa es la decisión de producto más consecuente
  del ADR: una cuadrícula de portadas responde a "qué tengo", y quien abre un
  lector quiere "seguir donde estaba".

  **Cero red y cero IA.** Todo sale de datos que ya existen: progreso, autores,
  materias y colecciones. Las filas son locales, explicables y reproducibles, y
  cada una dice **por qué** aparece; sin ese motivo no se pintan. Es lo que impide
  que Inicio degenere en un escaparate.

  **Ahora muchas filas y distintas en cada visita.** Observación de uso del
  2026-09-12: dos filas, "y lo peor es que son duplicados". Las filas las decide
  el runtime (`home::filas`) con la semilla de la visita: ninguna con menos de
  ocho obras, ninguna obra dos veces, y tres formas —portadas, portadas grandes y
  lista— para que la pantalla no sea la misma hilera repetida. La marquesina y
  "Seguir leyendo" no cambian: responden a lo que se está haciendo.

  **Ahora una marquesina y filas**, como se aprobó el 2026-09-11 sobre
  el diseño: lo que se retoma, grande arriba, y debajo una fila por pregunta —qué
  más estoy leyendo, qué ha llegado, por qué me puede interesar esto—. Las filas
  **no se desplazan en horizontal**: se paginan con flechas, que son botones, y
  llevan "Ver todo". Ver `Fila.svelte` y la sección 4 de `ui-conventions.md`.
-->
<script lang="ts">
  import { autoresBreves } from '$core/autores';
  import { porcentajeLeido } from '$core/progreso';
  import { rutaDeImagen } from '$core/ipc';
  import { idioma, textos } from '$lib/i18n';
  import type { BrowseItem, BrowsePage, HomeRow, Lectura, Shelf } from '$lib/bindings';

  import Icon from '../Icon.svelte';
  import Cover from './Cover.svelte';
  import EmptyState from './EmptyState.svelte';
  import Fila from './Fila.svelte';

  interface Props {
    /** Documentos empezados y sin terminar. El primero es el que se retoma. */
    enCurso: BrowsePage;
    novedades: BrowsePage;
    /** Las filas de esta visita, en el orden en que se pintan. */
    filas?: HomeRow[];
    /**
     * Si se enseñan las filas que sugieren. Apagado quedan las novedades, que no
     * sugieren nada: la segunda mitigación de ADR-013 contra un Inicio que empuja.
     */
    sugerir?: boolean;
    lectura: Lectura | null;
    /** Las materias con más obras, para explorar por ellas. */
    materias?: Shelf[];
    /** La frase donde se quedó, si se sabe. Es lo que de verdad ayuda a retomar. */
    citaDondeSeQuedo?: string | null;
    alAbrir?: (item: BrowseItem) => void;
    /** La ficha de una obra, que vive en la biblioteca. */
    alVerFicha?: (item: BrowseItem) => void;
    alIrABiblioteca?: () => void;
    /** La biblioteca con lo empezado: el "ver todo" de lo que se está leyendo. */
    alVerEmpezados?: () => void;
    /** Una estantería: el "ver todo" de las filas de materia y de autor. */
    alAbrirMateria?: (materia: Shelf) => void;
    /** Una colección, por su identificador: el "ver todo" de su fila. */
    alAbrirColeccion?: (id: string) => void;
    alImportar?: () => void;
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    enCurso,
    novedades,
    filas = [],
    sugerir = true,
    lectura,
    materias = [],
    citaDondeSeQuedo = null,
    alAbrir,
    alVerFicha,
    alIrABiblioteca,
    alVerEmpezados,
    alAbrirMateria,
    alAbrirColeccion,
    alImportar,
    etiquetas
  }: Props = $props();

  // El diccionario del idioma en uso, con las anulaciones de quien monta el
  // componente encima. `$derived` y no una constante: cambiar de idioma tiene
  // que repintar, y una constante capturaría el diccionario de un solo momento.
  const t = $derived({ ...textos().inicio, ...etiquetas } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;
  const cifra = $derived(new Intl.NumberFormat(idioma()));
  const id = $props.id();

  const bibliotecaVacia = $derived(novedades.total === 0);
  /** El que se retoma es el más avanzado en el tiempo, no uno cualquiera. */
  const retomar = $derived(enCurso.items[0] ?? null);
  const otrosEnCurso = $derived(enCurso.items.slice(1));

  /**
   * Las dos portadas de detrás de la marquesina: lo siguiente que se lee, o lo
   * último que llegó. Dan profundidad sin moverse, que es la única profundidad que
   * aquí se permite.
   */
  const detras = $derived(
    [...otrosEnCurso, ...novedades.items.filter((i) => i.editionId !== retomar?.editionId)].slice(
      0,
      2
    )
  );

  const porcentaje = (progresion: number | null) => porcentajeLeido(progresion);

  /**
   * Las filas que se pintan.
   *
   * Con pocas obras el runtime no devuelve ninguna —ocho es el mínimo de una
   * fila—, y entonces lo que hay se enseña entero como novedades: en una
   * biblioteca de cinco obras esas cinco son todo lo que hay, no una fila a medias.
   * Con 152.080 siempre hay filas, y esto no se ve nunca.
   */
  const pintadas = $derived.by<HomeRow[]>(() => {
    const permitidas = sugerir ? filas : filas.filter((f) => f.kind.kind === 'novedades');
    if (permitidas.length > 0 || novedades.items.length === 0) return permitidas;
    return [
      { id: 'novedades', kind: { kind: 'novedades' }, layout: 'portadas', items: novedades.items }
    ];
  });

  /** Dónde va la rejilla de materias: entre filas, para romper la hilera. */
  const LUGAR_DE_MATERIAS = 3;

  function tituloDeFila(fila: HomeRow): string {
    const clase = fila.kind;
    switch (clase.kind) {
      case 'novedades':
        return texto('novedades');
      case 'porqueLees':
        return `${texto('masDe')} ${clase.value.author}`;
      case 'materia':
        return clase.value.label;
      case 'autor':
        return clase.value;
      case 'coleccion':
        return clase.value.name;
      case 'redescubrir':
        return texto('redescubrir');
      case 'alAzar':
        return texto('alAzar');
    }
  }

  /** El porqué de la fila. Va siempre: una fila sin porqué no se pinta. */
  function porqueDeFila(fila: HomeRow): string {
    const clase = fila.kind;
    switch (clase.kind) {
      case 'novedades':
        return texto('porqueNovedades');
      case 'porqueLees':
        return `${texto('porqueLees')} ${clase.value.title}`;
      case 'materia':
        return texto('porqueMateria');
      case 'autor':
        return texto('porqueAutor');
      case 'coleccion':
        return texto('porqueColeccion');
      case 'redescubrir':
        return texto('porqueRedescubrir');
      case 'alAzar':
        return texto('porqueAlAzar');
    }
  }

  /** A dónde lleva "Ver todo". Sin destino que lo contenga entero, no se ofrece. */
  function verTodoDeFila(fila: HomeRow): (() => void) | undefined {
    const clase = fila.kind;
    switch (clase.kind) {
      case 'novedades':
        return alIrABiblioteca;
      case 'porqueLees':
        return () =>
          alAbrirMateria?.({
            kind: 'author',
            key: clase.value.author,
            label: clase.value.author,
            count: 0
          });
      case 'autor':
        return () =>
          alAbrirMateria?.({ kind: 'author', key: clase.value, label: clase.value, count: 0 });
      case 'materia':
        return () =>
          alAbrirMateria?.({
            kind: 'subject',
            key: clase.value.key,
            label: clase.value.label,
            count: 0
          });
      case 'coleccion':
        return () => alAbrirColeccion?.(clase.value.id);
      case 'redescubrir':
      case 'alAzar':
        return undefined;
    }
  }
</script>

{#snippet rejillaDeMaterias()}
  {#if materias.length > 0}
    <section class="nb-materias" aria-labelledby={`${id}-materias`}>
      <h2 class="nb-materias-titulo" id={`${id}-materias`}>{texto('materias')}</h2>
      <ul class="nb-materias-rejilla">
        {#each materias as materia (materia.key)}
          <li>
            <button class="nb-materia" type="button" onclick={() => alAbrirMateria?.(materia)}>
              <span class="nb-materia-icono"><Icon nombre="materia" /></span>
              <span class="nb-materia-nombre">{materia.label}</span>
              <span class="nb-materia-cuenta">
                {cifra.format(materia.count)}
                {texto('obras')}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
{/snippet}

<div class="nb-inicio" aria-label={texto('region')}>
  {#if bibliotecaVacia}
    <EmptyState icono="biblioteca" mensaje={texto('vacio')}>
      {#snippet accion()}
        <button class="nb-accion nb-accion-principal" type="button" onclick={() => alImportar?.()}>
          <Icon nombre="importar" tamano={20} />
          {texto('importar')}
        </button>
      {/snippet}
    </EmptyState>
  {:else}
    {#if retomar}
      {@const avance = porcentaje(retomar.progression)}
      <!--
        Una sola pieza grande y no una rejilla de cifras: lo que el usuario viene
        a hacer es seguir leyendo, y todo lo demás de esta pantalla es secundario.
      -->
      <section class="nb-marquesina" aria-labelledby={`${id}-retomar`}>
        <div class="nb-marquesina-texto">
          <span class="nb-etiqueta">
            <Icon nombre="lector" tamano={18} />
            {texto('retomar')}
          </span>
          <h2 class="nb-marquesina-titulo" id={`${id}-retomar`}>{retomar.title}</h2>
          {#if retomar.authors || retomar.seriesTitle}
            <p class="nb-marquesina-datos">
              {#if retomar.authors}<span>{retomar.authors}</span>{/if}
              {#if retomar.authors && retomar.seriesTitle}
                <span class="nb-punto" aria-hidden="true"></span>
              {/if}
              {#if retomar.seriesTitle}<span>{retomar.seriesTitle}</span>{/if}
            </p>
          {/if}

          {#if citaDondeSeQuedo}
            <blockquote class="nb-cita">{citaDondeSeQuedo}</blockquote>
          {/if}

          {#if avance !== null}
            <div class="nb-avance">
              <span class="nb-avance-barra" aria-hidden="true">
                <span style:width="{avance}%"></span>
              </span>
              <span class="nb-avance-texto"><strong>{avance} %</strong> {texto('leido')}</span>
            </div>
          {/if}

          <div class="nb-marquesina-acciones">
            <button
              class="nb-accion nb-accion-principal"
              type="button"
              onclick={() => alAbrir?.(retomar)}
            >
              <Icon nombre="reproducir" tamano={20} />
              {texto('continuar')}
            </button>
            <button class="nb-accion" type="button" onclick={() => alVerFicha?.(retomar)}>
              <Icon nombre="informacion" tamano={20} />
              {texto('detalles')}
            </button>
          </div>

          {#if lectura}
            <!--
              Cifras, no medallas: la racha se dice porque es un dato, y no se
              acompaña de ninguna presión. ADR-013 deja fuera la culpa administrada
              por el software.
            -->
            <p class="nb-cifras">
              <span
                ><strong>{cifra.format(lectura.minutesToday)}</strong> {texto('minutosHoy')}</span
              >
              <span class="nb-punto" aria-hidden="true"></span>
              <span
                ><strong>{cifra.format(lectura.minutesWeek)}</strong> {texto('minutosSemana')}</span
              >
              <span class="nb-punto" aria-hidden="true"></span>
              <span>
                <strong>{cifra.format(lectura.streakDays)}</strong>
                {lectura.streakDays === 1 ? texto('rachaUno') : texto('racha')}
              </span>
            </p>
          {/if}
        </div>

        <div class="nb-pila" aria-hidden="true">
          {#each detras as obra, lugar (obra.editionId)}
            <div class="nb-pila-portada" data-lugar={lugar}>
              <Cover
                titulo={obra.title}
                autor={obra.authors}
                imagen={obra.coverPath ? rutaDeImagen(obra.coverPath) : null}
                fluida
              />
            </div>
          {/each}
          <div class="nb-pila-portada" data-lugar="frente">
            <Cover
              titulo={retomar.title}
              autor={retomar.authors}
              imagen={retomar.coverPath ? rutaDeImagen(retomar.coverPath) : null}
              fluida
              progreso={retomar.progression}
            />
          </div>
        </div>
      </section>
    {/if}

    <div class="nb-filas">
      {#if otrosEnCurso.length > 0}
        <Fila
          titulo={texto('seguirLeyendo')}
          subtitulo={`${cifra.format(enCurso.total)} ${texto('empezados')}`}
          elementos={otrosEnCurso}
          clave={(item) => item.editionId}
          anchoMinimo={288}
          alVerTodo={alVerEmpezados}
        >
          {#snippet elemento(item)}
            {@const avance = porcentaje(item.progression)}
            <button class="nb-empezado" type="button" onclick={() => alAbrir?.(item)}>
              <Cover
                titulo={item.title}
                imagen={item.coverPath ? rutaDeImagen(item.coverPath) : null}
                ancho={72}
              />
              <span class="nb-empezado-texto">
                <span class="nb-empezado-titulo">{item.title}</span>
                {#if item.authors}<span class="nb-empezado-autor"
                    >{autoresBreves(item.authors, textos().ficha.autoresMas)}</span
                  >{/if}
                {#if avance !== null}
                  <span class="nb-empezado-pie">
                    <span class="nb-avance-barra" aria-hidden="true">
                      <span style:width="{avance}%"></span>
                    </span>
                    <span class="nb-empezado-cifra">{avance} %</span>
                  </span>
                {/if}
              </span>
            </button>
          {/snippet}
        </Fila>
      {:else if retomar === null}
        <p class="nb-nota">{texto('sinEmpezar')}</p>
      {/if}

      {#each pintadas as fila, lugar (fila.id)}
        {#if lugar === LUGAR_DE_MATERIAS}{@render rejillaDeMaterias()}{/if}
        {#if fila.layout === 'lista'}
          <!--
            La lista numerada no se pagina: son diez, y caben. Se lee de arriba abajo
            en lugar de recorrerse, y por eso es otra forma y no otra hilera.
          -->
          <section class="nb-lista" aria-labelledby={`${id}-${fila.id}`}>
            <div class="nb-lista-cabecera">
              <h2 class="nb-lista-titulo" id={`${id}-${fila.id}`}>{tituloDeFila(fila)}</h2>
              <span class="nb-lista-porque">{porqueDeFila(fila)}</span>
            </div>
            <ol class="nb-lista-obras">
              {#each fila.items as item, puesto (item.editionId)}
                <li>
                  <button
                    class="nb-lista-obra nb-obra-de-fila"
                    type="button"
                    disabled={!item.fileId}
                    onclick={() => alAbrir?.(item)}
                  >
                    <span class="nb-lista-puesto">{cifra.format(puesto + 1)}</span>
                    <Cover
                      titulo={item.title}
                      imagen={item.coverPath ? rutaDeImagen(item.coverPath) : null}
                      ancho={48}
                    />
                    <span class="nb-lista-texto">
                      <span class="nb-empezado-titulo">{item.title}</span>
                      {#if item.authors}<span class="nb-empezado-autor"
                          >{autoresBreves(item.authors, textos().ficha.autoresMas)}</span
                        >{/if}
                    </span>
                  </button>
                </li>
              {/each}
            </ol>
          </section>
        {:else}
          <Fila
            titulo={tituloDeFila(fila)}
            subtitulo={porqueDeFila(fila)}
            elementos={fila.items}
            clave={(item) => item.editionId}
            anchoMinimo={fila.layout === 'grandes' ? 216 : 148}
            alVerTodo={verTodoDeFila(fila)}
          >
            {#snippet elemento(item)}
              <!--
                Se deshabilita sin archivo disponible en lugar de esconderse: la obra
                sigue siendo de esta fila, y esconderla la haría desaparecer sin
                explicación. Lo que no puede es ofrecer abrir lo que no se abre.
              -->
              <button
                class="nb-obra nb-obra-de-fila"
                type="button"
                disabled={!item.fileId}
                onclick={() => alAbrir?.(item)}
              >
                <Cover
                  titulo={item.title}
                  autor={autoresBreves(item.authors, textos().ficha.autoresMas)}
                  imagen={item.coverPath ? rutaDeImagen(item.coverPath) : null}
                  fluida
                  progreso={item.progression}
                />
                <span class="nb-obra-titulo" title={item.title}>{item.title}</span>
                {#if item.authors}<span class="nb-obra-autor"
                    >{autoresBreves(item.authors, textos().ficha.autoresMas)}</span
                  >{/if}
              </button>
            {/snippet}
          </Fila>
        {/if}
      {/each}
      {#if pintadas.length <= LUGAR_DE_MATERIAS}{@render rejillaDeMaterias()}{/if}
    </div>
  {/if}
</div>

<style>
  .nb-inicio {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    padding-bottom: var(--nb-space-7);
  }

  /*
   * La marquesina. El brillo del fondo sale del acento del tema y es estático:
   * da presencia sin moverse, y un fondo que se mueve compite por el fotograma
   * con el desplazamiento de la página, que es lo que retiró el fondo vivo de la
   * carcasa.
   */
  .nb-marquesina {
    position: relative;
    display: grid;
    /*
     * Dos columnas que nunca se apilan: el texto se lleva lo que sobra y la pila de
     * portadas encoge con la ventana hasta su mínimo. Apilada, la pila empujaba la
     * primera fila fuera de la pantalla a 900 píxeles, que es justo lo que Inicio
     * no puede hacer.
     */
    grid-template-columns: minmax(0, 1fr) minmax(8rem, min(22rem, 34%));
    align-items: center;
    gap: var(--nb-space-6);
    min-height: 26rem;
    padding: var(--nb-space-6) clamp(var(--nb-space-4), 4vw, var(--nb-space-8));
    overflow: hidden;
  }

  .nb-marquesina::before {
    content: '';
    position: absolute;
    inset: 0;
    /*
     * La primera capa funde el brillo con el lienzo antes del borde inferior: sin
     * ella el resplandor se cortaba en seco donde acaba la marquesina, y se leía
     * como una caja de otro color.
     */
    background:
      linear-gradient(180deg, transparent 55%, var(--m3c-surface-container-low)),
      radial-gradient(
        60% 75% at 78% 30%,
        color-mix(in srgb, var(--m3c-primary) 18%, transparent),
        transparent 70%
      ),
      radial-gradient(
        50% 50% at 6% 100%,
        color-mix(in srgb, var(--m3c-primary) 8%, transparent),
        transparent 70%
      );
    pointer-events: none;
  }

  .nb-marquesina-texto {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    min-width: 0;
  }

  .nb-etiqueta {
    display: inline-flex;
    align-items: center;
    gap: var(--nb-space-2);
    min-height: 2rem;
    padding: 0 var(--nb-space-3);
    border-radius: var(--nb-radius-sm);
    background: color-mix(in srgb, var(--m3c-primary) 16%, transparent);
    color: var(--m3c-primary);
    font-size: var(--nb-text-sm);
    font-weight: 600;
  }

  .nb-marquesina-titulo {
    margin: var(--nb-space-4) 0 var(--nb-space-3);
    font-family: var(--nb-font-display);
    font-size: clamp(var(--nb-text-xl), 4.2vw, calc(3.5rem * var(--nb-font-scale)));
    font-weight: 600;
    line-height: 1.05;
    letter-spacing: -0.02em;
    text-wrap: balance;
    overflow-wrap: anywhere;
    color: var(--m3c-on-surface);
  }

  .nb-marquesina-datos {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-2) var(--nb-space-3);
    margin: 0;
    font-size: var(--nb-text-md);
    font-weight: 500;
    color: var(--m3c-on-surface-variant);
  }

  .nb-punto {
    width: 4px;
    height: 4px;
    border-radius: 2px;
    background: var(--m3c-outline);
  }

  .nb-cita {
    max-width: 36rem;
    margin: var(--nb-space-5) 0 0;
    padding-left: var(--nb-space-4);
    border-left: 3px solid var(--m3c-primary);
    font-family: var(--nb-font-lectura);
    font-size: var(--nb-text-lg);
    font-style: italic;
    line-height: 1.5;
    color: var(--m3c-on-surface);
  }

  .nb-avance {
    display: flex;
    align-items: center;
    gap: var(--nb-space-3);
    width: min(100%, 32rem);
    margin-top: var(--nb-space-5);
  }

  .nb-avance-barra {
    display: block;
    flex: 1 1 auto;
    height: 6px;
    border-radius: var(--nb-radius-full);
    background: color-mix(in srgb, var(--m3c-on-surface) 12%, transparent);
    overflow: hidden;
  }

  .nb-avance-barra > span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--m3c-primary);
  }

  .nb-avance-texto {
    flex: 0 0 auto;
    font-size: var(--nb-text-sm);
    font-variant-numeric: tabular-nums;
    color: var(--m3c-on-surface-variant);
  }

  .nb-avance-texto strong {
    font-weight: 600;
    color: var(--m3c-on-surface);
  }

  .nb-marquesina-acciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-3);
    margin-top: var(--nb-space-5);
  }

  .nb-accion {
    display: inline-flex;
    align-items: center;
    gap: var(--nb-space-2);
    min-height: 3.5rem;
    padding: 0 var(--nb-space-5) 0 var(--nb-space-4);
    border: none;
    border-radius: var(--nb-radius-full);
    background: color-mix(in srgb, var(--m3c-on-surface) 10%, transparent);
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-md);
    font-weight: 600;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-accion:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 16%, transparent);
  }

  .nb-accion-principal {
    background: var(--m3c-primary);
    color: var(--m3c-on-primary);
    box-shadow: var(--m3-elevation-1);
  }

  .nb-accion-principal:hover {
    background: color-mix(in srgb, var(--m3c-on-primary) 8%, var(--m3c-primary));
  }

  .nb-cifras {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-2) var(--nb-space-3);
    margin: var(--nb-space-5) 0 0;
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }

  .nb-cifras strong {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--m3c-on-surface);
  }

  /*
   * La pila mide lo que le deja la columna, y sus portadas se colocan en
   * proporción a ese ancho: con medidas fijas, al encoger la columna se salían de
   * ella y tapaban el título.
   */
  .nb-pila {
    position: relative;
    container-type: inline-size;
    aspect-ratio: 22 / 24;
  }

  .nb-pila-portada {
    position: absolute;
    width: 50cqw;
  }

  .nb-pila-portada[data-lugar='0'] {
    top: 16cqw;
    left: 50cqw;
    opacity: 0.6;
    transform: rotate(8deg);
  }

  .nb-pila-portada[data-lugar='1'] {
    top: 14cqw;
    left: 0;
    opacity: 0.6;
    transform: rotate(-7deg);
  }

  .nb-pila-portada[data-lugar='frente'] {
    top: 2cqw;
    left: 19cqw;
    width: 61cqw;
  }

  .nb-pila-portada[data-lugar='frente'] :global(.nb-portada) {
    box-shadow: var(--m3-elevation-3);
  }

  .nb-filas {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-7);
    padding: var(--nb-space-2) clamp(var(--nb-space-4), 4vw, var(--nb-space-8)) 0;
  }

  .nb-empezado {
    display: flex;
    gap: var(--nb-space-4);
    width: 100%;
    min-height: 8.5rem;
    padding: var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-lg);
    background: var(--m3c-surface-container);
    color: var(--m3c-on-surface);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-empezado:hover {
    background: var(--m3c-surface-container-high);
  }

  .nb-empezado-texto {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    flex: 1 1 auto;
    min-width: 0;
    padding: var(--nb-space-1) 0;
  }

  .nb-empezado-titulo {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
    font-family: var(--nb-font-display);
    font-size: var(--nb-text-md);
    font-weight: 600;
    line-height: 1.25;
  }

  .nb-empezado-autor {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }

  .nb-empezado-pie {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    margin-top: auto;
  }

  .nb-empezado-cifra {
    flex: 0 0 auto;
    font-size: var(--nb-text-xs);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .nb-obra {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    width: 100%;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--m3c-on-surface);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .nb-obra:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  /* Dos líneas y no una, como en la rejilla de la biblioteca. */
  .nb-obra-titulo {
    margin-top: var(--nb-space-2);
    overflow: hidden;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    min-height: 2lh;
    font-size: var(--nb-text-sm);
    font-weight: 600;
    transition: color var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-obra:hover:not(:disabled) .nb-obra-titulo {
    color: var(--m3c-primary);
  }

  .nb-obra-autor {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
  }

  /* La clase existe para contarlas: son las obras de las filas, no las de la marquesina. */
  .nb-obra-de-fila {
    position: relative;
  }

  .nb-lista {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
  }

  .nb-lista-cabecera {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--nb-space-1) var(--nb-space-3);
    min-height: 2.5rem;
  }

  .nb-lista-titulo {
    margin: 0;
    font-size: var(--nb-text-lg);
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .nb-lista-porque {
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }

  /*
   * Dos columnas donde caben, una donde no: las diez se leen en orden de columna,
   * que es como se lee una lista numerada, y nada se desplaza en horizontal.
   */
  .nb-lista-obras {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 22rem), 1fr));
    gap: var(--nb-space-2) var(--nb-space-5);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-lista-obra {
    display: flex;
    align-items: center;
    gap: var(--nb-space-4);
    width: 100%;
    padding: var(--nb-space-2) var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-lg);
    background: transparent;
    color: var(--m3c-on-surface);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-lista-obra:hover:not(:disabled) {
    background: var(--m3c-surface-container);
  }

  .nb-lista-obra:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .nb-lista-puesto {
    flex: 0 0 2.5rem;
    font-family: var(--nb-font-display);
    font-size: var(--nb-text-xl);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    text-align: right;
    color: var(--m3c-primary);
  }

  .nb-lista-texto {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    min-width: 0;
  }

  .nb-nota {
    margin: 0;
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }

  .nb-materias {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
  }

  .nb-materias-titulo {
    margin: 0;
    font-size: var(--nb-text-lg);
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .nb-materias-rejilla {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
    gap: var(--nb-space-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-materia {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    width: 100%;
    min-height: 7rem;
    padding: var(--nb-space-4);
    border: none;
    border-radius: var(--nb-radius-lg);
    background: var(--m3c-surface-container);
    color: var(--m3c-on-surface);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-materia:hover {
    background: var(--m3c-surface-container-high);
  }

  .nb-materia-icono {
    display: flex;
    margin-bottom: auto;
    color: var(--m3c-primary);
  }

  .nb-materia-nombre {
    overflow-wrap: anywhere;
    font-family: var(--nb-font-display);
    font-size: var(--nb-text-md);
    font-weight: 600;
    line-height: 1.2;
  }

  .nb-materia-cuenta {
    font-size: var(--nb-text-xs);
    font-variant-numeric: tabular-nums;
    color: var(--m3c-on-surface-variant);
  }
</style>
