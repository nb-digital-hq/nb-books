<!--
  @component
  Búsqueda: la misma caja para las tres preguntas.

  Contrato: architecture/search.md y ADR-013, que la declara **transversal**: no
  es un destino, aparece dentro de los que la necesitan.

  Los tres alcances devuelven cosas distintas a propósito, y por eso el resultado
  viene etiquetado en lugar de ser una lista a secas: el catálogo devuelve
  **documentos** y los otros dos, **párrafos**. Buscar un libro y buscar dentro
  de un libro son dos preguntas que ni siquiera devuelven la misma clase de cosa.

  El fragmento con la coincidencia marcada **lo genera SQL**, no esta capa:
  marcarlo aquí exigiría saber cómo tokenizó el índice —sin tildes, sin
  mayúsculas y por raíces—, y un resaltado hecho a mano no marcaría nada al
  buscar "analisis" sobre un texto que dice "análisis".

  **Y busca también por significado**, en lo que se haya preparado.
  Aparte y después: la búsqueda léxica se pide y se pinta como siempre, y la de
  significado se pide con una pausa algo mayor y, cuando contesta, se funde con la
  léxica marcando de dónde sale cada resultado. Si no contesta —sin modelo, con la
  IA caída—, la lista léxica se queda como estaba. Es el criterio de la iteración.

  La caja vive en la barra superior y los resultados se abren debajo
  de ella. Por eso lo escrito se puede atar desde fuera y la caja propia es
  opcional: buscar es lo mismo en los dos sitios, y lo que cambia es dónde se
  escribe. Y se busca **mientras se escribe**, con una pausa corta, que es lo que
  `search.md` pide para el catálogo: pulsar Intro para cada intento trata un
  buscador como un formulario.
-->
<script lang="ts">
  import { guardaDeRespuesta } from '$core/carga';
  import { codigoDeError, idioma, textos } from '$lib/i18n';
  import { TEXTOS_DE_SIGNIFICADO } from '$lib/i18n/locales/significado';
  import type {
    Locator,
    NavigationEntry,
    Results,
    Scope,
    SearchStatus,
    SemanticHit
  } from '$lib/bindings';
  import { mensajeDeBusqueda, sePuedeBuscar } from '$core/search';
  import { capitulosPorRecurso } from '../lector/posicion';

  import Icon from '../Icon.svelte';
  import Spinner from '../Spinner.svelte';
  import EmptyState from './EmptyState.svelte';
  import PrepararSignificado from './PrepararSignificado.svelte';
  import { fusionar, trozosDelPasaje, type Origen } from './fusion';

  type Alcance = 'catalog' | 'corpus' | 'document';

  interface Props {
    /** Lo ejecuta quien monta el panel: aquí no se llama al puente. */
    alBuscar: (consulta: string, alcance: Scope) => Promise<Results>;
    /**
     * La búsqueda por significado. Opcional: sin ella, el panel es el de
     * siempre.
     */
    alBuscarPorSignificado?: (consulta: string, alcance: Scope) => Promise<SemanticHit[]>;
    /** Si hay un documento abierto, se puede buscar dentro de él. */
    /**
     * La navegación permite resolver el nombre de cada capítulo al cargar este panel. Lo
     * que se indexó antes de leer el índice de los EPUB 2 guarda "Sección 14"
     * donde el libro dice "Escena I", y reindexar la biblioteca por un rótulo no
     * es razonable: se traduce al pintar.
     */
    documentoAbierto?: {
      editionId: string;
      titulo: string;
      navegacion?: readonly NavigationEntry[];
    } | null;
    /**
     * Si el documento abierto se puede buscar ya, y por qué si no.
     *
     * Va como propiedad y no se consulta aquí, como todo lo demás del panel.
     * Existe porque sin ello buscar dentro de un libro recién importado devuelve
     * cero coincidencias **sin decir que todavía se está preparando**, y cero
     * coincidencias es una respuesta que parece definitiva. Los ayudantes que lo
     * interpretan viven en `$core/search` y no los llamaba nadie.
     */
    estadoDeBusqueda?: SearchStatus | null;
    /** Lo escrito. Se ata desde fuera cuando la caja está en la barra superior. */
    consulta?: string;
    /** Si el panel pinta su propia caja. Sin ella, la pone quien lo monta. */
    conCaja?: boolean;
    /** Por dónde se empieza a buscar. Desde el lector, dentro del libro abierto. */
    alcanceInicial?: Alcance;

    alAbrirEdicion?: (fileId: string | null) => void;
    alSaltar?: (fileId: string | null, locator: Locator) => void;
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    alBuscar,
    alBuscarPorSignificado,
    documentoAbierto = null,
    estadoDeBusqueda = null,
    consulta = $bindable(''),
    conCaja = true,
    alcanceInicial = 'catalog',
    alAbrirEdicion,
    alSaltar,
    etiquetas
  }: Props = $props();

  // El diccionario del idioma en uso, con las anulaciones de quien monta el
  // componente encima. `$derived` y no una constante: cambiar de idioma tiene
  // que repintar, y una constante capturaría el diccionario de un solo momento.
  const t = $derived({ ...textos().buscar, ...etiquetas } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  /**
   * Cuánto se espera a que se deje de escribir antes de preguntar, por alcance.
   *
   * **No es la misma pausa para los tres, y está medido.** Buscar por título
   * cuesta entre 30 y 180 ms sobre la base real de 152.080 obras; buscar dentro
   * de los libros, entre 0,45 y 1,7 s más lo que tarde en releer los pasajes.
   * Con 220 ms para todo -lo de antes-, quien escribe a ritmo normal lanzaba una
   * búsqueda en el texto **por letra**, y cada una ocupaba una de las cuatro
   * conexiones de la base mientras nadie iba a mirar su respuesta.
   *
   * La pausa ahorra trabajo; lo que corta el que ya salió es el runtime, que
   * interrumpe la búsqueda anterior al llegar la siguiente.
   */
  const PAUSA: Record<Alcance, number> = { catalog: 300, corpus: 550, document: 400 };

  /**
   * Cuántas letras hacen falta antes de buscar.
   *
   * Por debajo, el índice no puede contestar nada útil: una o dos letras no son
   * una palabra, y dentro de los libros "de" aparece en millones de fragmentos.
   * Se dice en lugar de callar, para que no parezca que la búsqueda no responde.
   */
  const MINIMO: Record<Alcance, number> = { catalog: 2, corpus: 3, document: 3 };

  // Solo se lee al montar, y a propósito: el alcance lo cambia quien busca.
  // svelte-ignore state_referenced_locally
  let alcance = $state<Alcance>(alcanceInicial);

  /**
   * La última respuesta, **con la pregunta que contesta**.
   *
   * Antes se guardaban solo los resultados, y por eso cambiar de
   * "Títulos" a "Dentro de los Libros" seguía enseñando los títulos hasta que
   * llegaba lo otro: la lista no sabía de qué búsqueda era. Con la pregunta al
   * lado, lo que no corresponde al alcance que se está mirando no se pinta.
   */
  let respuesta = $state<{ consulta: string; alcance: Alcance; resultados: Results } | null>(null);
  let fallo = $state<{ consulta: string; alcance: Alcance } | null>(null);
  const guarda = guardaDeRespuesta();

  const limpia = $derived(consulta.trim());
  const faltanLetras = $derived(limpia.length > 0 && limpia.length < MINIMO[alcance]);
  const buscable = $derived(limpia.length >= MINIMO[alcance]);

  /** Lo que contesta exactamente a lo escrito, en el alcance elegido. */
  const vigentes = $derived(
    respuesta && respuesta.consulta === limpia && respuesta.alcance === alcance
      ? respuesta.resultados
      : null
  );
  /**
   * Lo que se enseña mientras llega lo nuevo: la respuesta anterior **del mismo
   * alcance**, atenuada. Mientras se escribe, borrar la lista en cada letra la
   * haría parpadear; lo de otro alcance, en cambio, no se enseña nunca.
   */
  const mostrados = $derived(
    vigentes ?? (buscable && respuesta?.alcance === alcance ? respuesta.resultados : null)
  );
  const fallido = $derived(
    fallo !== null && fallo.consulta === limpia && fallo.alcance === alcance
  );
  /** Hay una pregunta sin contestar: esperando la pausa o esperando al runtime. */
  const esperando = $derived(buscable && vigentes === null && !fallido);

  /*
   * El aviso solo tiene sentido buscando dentro del libro abierto: en el
   * catálogo se busca por título, que no necesita índice de ninguna clase.
   */
  const aviso = $derived(
    alcance === 'document' && estadoDeBusqueda && !sePuedeBuscar(estadoDeBusqueda)
      ? mensajeDeBusqueda(estadoDeBusqueda)
      : null
  );
  // `busqueda.disponible` no está en el diccionario a propósito: es el mensaje
  // del caso en el que no se enseña ninguno, y una clave que nunca se pide
  // sería una cadena vacía que la prueba de diccionarios rechaza con razón.

  /**
   * Los alcances que se ofrecen ahora mismo.
   *
   * "En este libro" solo existe con un libro abierto: ofrecerlo sin él sería un
   * control que no puede hacer nada, y aquí se omite en lugar de atenuarse.
   */
  const alcances = $derived<Array<[Alcance, string]>>(
    documentoAbierto
      ? [
          ['catalog', 'catalogo'],
          ['corpus', 'corpus'],
          ['document', 'documento']
        ]
      : [
          ['catalog', 'catalogo'],
          ['corpus', 'corpus']
        ]
  );

  const capitulos = $derived(capitulosPorRecurso(documentoAbierto?.navegacion ?? []));

  /** En qué parte del libro cae un resultado, con el nombre que le da su índice. */
  function parteDe(hit: { editionId: string; locator: Locator }): string {
    const propio =
      hit.editionId === documentoAbierto?.editionId ? capitulos[hit.locator.href] : undefined;
    return propio ?? hit.locator.title ?? '';
  }

  function alcanceReal(cual: Alcance): Scope {
    if (cual === 'catalog') return { scope: 'catalog' };
    if (cual === 'corpus') return { scope: 'corpus', collectionId: null };
    return { scope: 'document', editionId: documentoAbierto?.editionId ?? '' };
  }

  async function buscar(pregunta: string, donde: Alcance) {
    try {
      const recibida = await guarda(() => alBuscar(pregunta, alcanceReal(donde)));
      // Solo pinta la última pedida. Buscar en el texto tarda lo que tarde el
      // índice y buscar por título son milisegundos, así que una consulta de
      // tres letras podía contestar después de otra de cinco.
      if (!recibida.vigente) return;
      respuesta = { consulta: pregunta, alcance: donde, resultados: recibida.valor };
      fallo = null;
    } catch {
      fallo = { consulta: pregunta, alcance: donde };
    }
  }

  $effect(() => {
    const pregunta = limpia;
    const donde = alcance;
    if (pregunta.length < MINIMO[donde]) return;
    const pausa = setTimeout(() => void buscar(pregunta, donde), PAUSA[donde]);
    return () => clearTimeout(pausa);
  });

  /**
   * Cuánto se espera antes de preguntar por significado: **700 ms**, más que a la
   * léxica. Calcular el vector de la consulta cuesta unas décimas de segundo, y un
   * segundo y medio más si hay que cargar el modelo; preguntarlo con cada letra
   * sería ocupar el procesador con consultas que nadie va a ver.
   */
  const PAUSA_POR_SIGNIFICADO = 700;

  let porSignificado = $state<{ consulta: string; alcance: Alcance; hits: SemanticHit[] } | null>(
    null
  );
  /**
   * Si el runtime ya dijo que no hay búsqueda por significado —sin modelo, con la
   * IA caída—. No se vuelve a preguntar mientras el panel siga abierto.
   */
  let sinSignificado = $state(false);
  const guardaDeSignificado = guardaDeRespuesta();
  const ts = $derived(TEXTOS_DE_SIGNIFICADO[idioma()] as Record<string, string>);

  async function buscarPorSignificado(pregunta: string, donde: Alcance) {
    if (!alBuscarPorSignificado) return;
    try {
      const recibida = await guardaDeSignificado(() =>
        alBuscarPorSignificado(pregunta, alcanceReal(donde))
      );
      if (!recibida.vigente) return;
      porSignificado = { consulta: pregunta, alcance: donde, hits: recibida.valor };
    } catch (e) {
      // No se enseña: la lista léxica sigue ahí, que es lo que importa. Solo se
      // deja de preguntar si lo que falta es la IA entera.
      if (codigoDeError(e) === 'COMPONENT_UNAVAILABLE') sinSignificado = true;
    }
  }

  $effect(() => {
    const pregunta = limpia;
    const donde = alcance;
    if (donde === 'catalog' || sinSignificado || pregunta.length < MINIMO[donde]) return;
    const pausa = setTimeout(
      () => void buscarPorSignificado(pregunta, donde),
      PAUSA_POR_SIGNIFICADO
    );
    return () => clearTimeout(pausa);
  });

  const semanticos = $derived(
    porSignificado && porSignificado.consulta === limpia && porSignificado.alcance === alcance
      ? porSignificado.hits
      : []
  );

  /** La lista de párrafos, fundida con lo que haya por significado. */
  const coincidencias = $derived(
    mostrados && mostrados.kind === 'text' ? fusionar(mostrados.hits, semanticos) : []
  );

  /** Si la lista mezcla lo exacto con lo encontrado por significado. */
  const hayMezcla = $derived(coincidencias.some((c) => c.origen !== 'exacta'));

  const ETIQUETA_DE_ORIGEN: Record<Origen, string> = {
    exacta: 'origenExacta',
    significado: 'origenSignificado',
    ambas: 'origenAmbas'
  };
</script>

<div class="nb-busqueda" aria-label={texto('region')}>
  {#if conCaja}
    <div class="nb-caja">
      <Icon nombre="buscar" />
      <input
        class="nb-consulta"
        type="search"
        aria-label={texto('consulta')}
        placeholder={texto('consulta')}
        bind:value={consulta}
        onkeydown={(e) => {
          if (e.key === 'Enter' && buscable) void buscar(limpia, alcance);
        }}
      />
    </div>
  {/if}

  <div class="nb-alcances-fila">
    <div class="nb-alcances" role="tablist" aria-label={texto('region')}>
      {#each alcances as [valor, clave] (valor)}
        <button
          type="button"
          role="tab"
          aria-selected={alcance === valor}
          class="nb-alcance"
          onclick={() => (alcance = valor)}
        >
          {texto(clave)}
        </button>
      {/each}
    </div>
    {#if esperando}
      <!--
        Se dice que está buscando mientras haya una pregunta sin contestar, se
        esté escribiendo o esperando al runtime, y el propio indicador espera su
        umbral: una búsqueda por título que contesta en 40 ms no llega a
        enseñarlo, y una dentro de los libros que tarda un segundo sí. No se
        vuelve a montar con cada letra, que lo tendría siempre por debajo del
        umbral. Antes no había nada, y escribir parecía no hacer
        nada.
      -->
      <Spinner etiqueta={texto('buscando')} tamano="1rem" enLinea />
    {/if}
  </div>

  {#if aviso}
    <!--
      Se dice **antes** que el resultado y no en lugar de él: buscar sigue
      funcionando, lo que pasa es que todavía no encuentra todo. Ocultar la
      búsqueda mientras se prepara la haría parecer rota.
    -->
    <p class="nb-nota" role="status">
      <Icon nombre="informacion" tamano={16} />
      {texto(aviso)}
    </p>
  {/if}

  {#if fallido}
    <p class="nb-fallo" role="alert">{texto('error')}</p>
  {:else if limpia.length === 0}
    <EmptyState icono="buscar" mensaje={texto('vacio')} />
  {:else if faltanLetras}
    <p class="nb-estado" role="status">
      {texto('faltanLetras').replace('{n}', String(MINIMO[alcance]))}
    </p>
  {:else if !mostrados}
    <!-- Sin nada que enseñar todavía: el hueco lo ocupa el indicador de arriba. -->
  {:else if mostrados.hits.length === 0 && coincidencias.length === 0}
    {#if !esperando}
      <EmptyState icono="buscar" mensaje={texto('sinResultados')} />
    {/if}
  {:else if mostrados.kind === 'catalog'}
    <ul class="nb-resultados" aria-busy={esperando} data-desfasados={esperando}>
      {#each mostrados.hits as hit (hit.editionId)}
        <li>
          <!--
            Sin archivo disponible el resultado se enseña y **no se pulsa**: el
            volumen puede estar desconectado, y ofrecer un salto que va a fallar
            es peor que decir que ese libro no está a mano ahora.
          -->
          <button type="button" disabled={!hit.fileId} onclick={() => alAbrirEdicion?.(hit.fileId)}>
            <Icon nombre="biblioteca" tamano={18} />
            <span class="nb-pasaje">
              <span class="nb-fragmento">{hit.title}</span>
              {#if hit.authors}
                <!-- Quién la firma: sin esto, tres "Cervantes" no se distinguían. -->
                <span class="nb-pasaje-obra">{hit.authors}</span>
              {/if}
            </span>
            {#if !hit.fileId}
              <span class="nb-sin-copia">{texto('sinCopia')}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <ul class="nb-resultados" aria-busy={esperando} data-desfasados={esperando}>
      {#each coincidencias as hit, indice (indice)}
        <li>
          <button
            type="button"
            disabled={!hit.fileId}
            onclick={() => alSaltar?.(hit.fileId, hit.locator)}
          >
            <Icon nombre={hit.origen === 'significado' ? 'ia' : 'lector'} tamano={18} />
            <span class="nb-pasaje">
              {#if hit.titulo || hit.locator.title}
                <!-- De qué libro y de qué parte: sin esto eran frases sin dueño. -->
                <span class="nb-pasaje-obra">
                  {hit.titulo}{#if hit.titulo && parteDe(hit)}{' · '}{/if}{parteDe(hit)}
                  {#if hit.otras > 0}
                    <span class="nb-pasaje-otras">{texto('yEnOtras')} {hit.otras}</span>
                  {/if}
                </span>
              {/if}
              <span class="nb-fragmento nb-fragmento-de-libro"
                >{#each trozosDelPasaje(hit.snippet) as trozo, i (i)}{#if trozo.marcado}<mark
                      >{trozo.texto}</mark
                    >{:else}{trozo.texto}{/if}{/each}</span
              >
            </span>
            <!--
              De dónde sale, solo cuando hay de dónde elegir: si todo es exacto,
              poner "Exacta" en cada fila no dice nada. En cuanto hay algo por
              significado, todas lo dicen, porque una cercanía de sentido no merece
              la misma confianza que una coincidencia (embeddings.md, 4).
            -->
            {#if hayMezcla}
              <span class="nb-origen" data-origen={hit.origen}
                >{ts[ETIQUETA_DE_ORIGEN[hit.origen]]}</span
              >
            {/if}
            {#if !hit.fileId}
              <span class="nb-sin-copia">{texto('sinCopia')}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if alcance === 'document' && documentoAbierto}
    <!--
      La búsqueda por significado del libro abierto: nada se prepara sin
      pedirlo, y el coste va delante del botón.
    -->
    <PrepararSignificado alcance={{ scope: 'document', editionId: documentoAbierto.editionId }} />
  {/if}
</div>

<style>
  .nb-busqueda {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    min-height: 0;
  }

  .nb-caja {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    min-height: 3rem;
    padding: 0 var(--nb-space-4);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface-variant);
  }

  .nb-consulta {
    flex: 1 1 auto;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--m3c-on-surface);
    font-family: inherit;
    font-size: var(--nb-text-sm);
  }

  .nb-alcances-fila {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-2);
  }

  .nb-alcances {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-1);
  }

  /*
   * Lo de la pregunta anterior mientras llega la nueva: se ve, para que la lista
   * no parpadee en cada letra, pero se nota que ya no es la respuesta.
   */
  .nb-resultados[data-desfasados='true'] {
    opacity: 0.55;
  }

  /*
   * Pestañas de Material: el texto y un trazo debajo del elegido. Sin caja
   * alrededor, porque lo que se elige es dónde buscar y no un botón que se pulsa.
   */
  .nb-alcance {
    position: relative;
    min-height: 3rem;
    padding: 0 var(--nb-space-4);
    border: none;
    background: transparent;
    color: var(--m3c-on-surface-variant);
    font-family: inherit;
    font-size: var(--nb-text-sm);
    font-weight: 500;
    cursor: pointer;
    transition: color var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-alcance:hover {
    color: var(--m3c-on-surface);
  }

  .nb-alcance[aria-selected='true'] {
    color: var(--m3c-primary);
    font-weight: 600;
  }

  .nb-alcance[aria-selected='true']::after {
    content: '';
    position: absolute;
    right: var(--nb-space-3);
    bottom: 0;
    left: var(--nb-space-3);
    height: 3px;
    border-radius: var(--nb-radius-xs) var(--nb-radius-xs) 0 0;
    background: var(--m3c-primary);
  }

  .nb-resultados {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
    overflow: auto;
  }

  .nb-resultados button {
    display: flex;
    align-items: flex-start;
    gap: var(--nb-space-3);
    width: 100%;
    padding: var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-md);
    background: transparent;
    color: var(--m3c-on-surface);
    font-family: inherit;
    font-size: var(--nb-text-sm);
    text-align: left;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-resultados button:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-resultados button:active {
    background: color-mix(in srgb, var(--m3c-on-surface) 12%, transparent);
  }

  .nb-fragmento {
    min-width: 0;
    line-height: 1.5;
  }

  /* Lo que sale de dentro de un libro se lee con la letra de leer. */
  .nb-pasaje {
    display: grid;
    gap: var(--nb-space-1);
    min-width: 0;
    flex: 1 1 auto;
  }

  .nb-pasaje-obra {
    font-size: var(--nb-text-xs);
    font-weight: 600;
    color: var(--m3c-on-surface-variant);
  }

  .nb-pasaje-otras {
    font-weight: 400;
  }

  .nb-fragmento-de-libro mark {
    background: var(--m3c-primary-container);
    color: var(--m3c-on-primary-container);
    border-radius: 2px;
  }

  .nb-fragmento-de-libro {
    font-family: var(--nb-font-display);
    font-size: var(--nb-text-md);
  }

  /*
   * El motivo, junto al resultado que no se puede abrir.
   *
   * Un botón en gris y nada más sería "una promesa a medias", que es justo lo
   * que `salud.ts` describe y prohíbe: el usuario lo intenta, no pasa nada y
   * concluye que el problema es suyo. Con el motivo al lado, el resultado sigue
   * apareciendo -la obra coincide, y ocultarla sería mentir sobre la búsqueda- y
   * además se sabe por qué no se abre.
   */
  .nb-sin-copia {
    flex: none;
    margin-inline-start: auto;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }

  .nb-origen {
    flex: none;
    align-self: center;
    padding: 2px var(--nb-space-2);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-full);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
    white-space: nowrap;
  }

  .nb-origen[data-origen='significado'],
  .nb-origen[data-origen='ambas'] {
    border-color: transparent;
    background: var(--m3c-tertiary-container);
    color: var(--m3c-on-tertiary-container);
  }

  .nb-nota {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    margin: 0;
    padding: var(--nb-space-3);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
    line-height: 1.45;
  }

  .nb-estado,
  .nb-fallo {
    margin: 0;
    padding: var(--nb-space-2) var(--nb-space-3);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }

  .nb-fallo {
    color: var(--m3c-error);
  }
</style>
