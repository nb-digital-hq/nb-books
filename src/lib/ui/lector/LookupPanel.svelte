<!--
  @component
  Consulta sobre la selección: definición, enciclopedia y traducción.

  Contrato: `docs/architecture/integrations.md` y
  docs/architecture/reader-contracts.md, sección 5.2.

  Tres cosas que este componente **no** hace, y son las que hacen comprobable su
  criterio de aceptación:

  1. **No consulta.** Recibe el resultado. Quien pregunta es el lector, y quien
     responde es el registro de proveedores del runtime. Así el panel no sabe si
     contesta un diccionario local o un traductor en línea, que es la razón
     entera de que exista el contrato de consulta.
  2. **No mueve la lectura.** No tiene con qué: no recibe ni el locator ni
     ninguna forma de cambiarlo. El criterio dice que consultar no mueve la
     posición ni un locator, y aquí eso es una imposibilidad, no una promesa.
  3. **No atrapa el foco.** Nada se enfoca solo al abrirse, así que el foco se
     queda en el párrafo desde el que se consultó y volver es no hacer nada. Es
     la regla de paneles de ADR-012, y aquí además es lo que hace cierto el
     "devuelve el foco exactamente donde estaba".

  Lo que sí hace, y es la mitad que se olvida: **decir quién no contestó y por
  qué**. Un proveedor omitido en silencio se lee como "esta palabra no existe",
  y no es lo mismo que "no tienes instalado el diccionario de este idioma" ni
  que "esto se quedó fuera porque estás sin red".
-->
<script lang="ts">
  import { textos } from '$lib/i18n';

  import type { LookupOutcome, LookupSkipped } from '$core/ipc';

  import Spinner from '../Spinner.svelte';
  import Icon from '../Icon.svelte';
  import { palabrasDe } from './consulta';

  interface Props {
    /** La palabra consultada. */
    termino: string;
    /**
     * El párrafo del que salió.
     *
     * De él salen las demás palabras que se ofrecen. Se selecciona por bloques
     * —decisión—, así que la selección es casi siempre un párrafo, y
     * un párrafo no está en ningún diccionario: quien consulta sabe cuál es la
     * palabra que no entiende, y el programa no.
     */
    seleccion?: string;
    /** Lo que devolvieron los proveedores. `null` mientras no hay respuesta. */
    resultado: LookupOutcome | null;
    cargando?: boolean;
    /**
     * Código canónico del fallo, si lo hubo.
     *
     * `COMPONENT_UNAVAILABLE` es el caso con nombre propio: significa que esta
     * instalación no tiene **ningún** proveedor, y entonces lo que hay que decir
     * no es que falló, sino dónde se instala uno.
     */
    error?: string | null;
    /** Se pide consultar otra palabra del mismo párrafo. */
    alElegirPalabra?: (palabra: string) => void;
    /**
     * Se autoriza a salir a un destino que un proveedor pidió.
     *
     * El anfitrión es **el que vino en el omitido**, no uno que este componente
     * componga: quien decide a dónde sale la aplicación es el runtime.
     */
    alAutorizar?: (host: string) => void;
    /**
     * Qué se envía al consultar, en lenguaje llano.
     *
     * Lo dice el runtime, donde vive junto al propósito. Se enseña **antes** de
     * ofrecer el permiso: el criterio dice que el primer uso diga qué
     * texto va a salir, y un botón de permitir sin eso pide una firma en blanco.
     */
    queSeEnvia?: string | null;
    /** Textos. Anulación, no fuente: el diccionario manda. */
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    termino,
    seleccion = '',
    resultado,
    cargando = false,
    error = null,
    alElegirPalabra,
    alAutorizar,
    queSeEnvia = null,
    etiquetas
  }: Props = $props();

  const t = $derived({ ...textos().consulta, ...etiquetas } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  const entradas = $derived(resultado?.entries ?? []);
  const omitidos = $derived(resultado?.skipped ?? []);

  // Las demás palabras del párrafo. Solo se ofrecen si hay más de una: con una
  // sola, la lista sería un botón que repite lo que ya se está consultando.
  const palabras = $derived(palabrasDe(seleccion));
  const hayQueElegir = $derived(palabras.length > 1);

  /**
   * Por qué se quedó fuera un proveedor, en una frase.
   *
   * La clave es el motivo tal cual lo manda el runtime: un código, no una
   * cadena, por la misma regla que los errores. Traducirlo es trabajo de aquí.
   */
  function motivo(omitido: LookupSkipped): string {
    return texto(`omitido-${omitido.reason}`);
  }

  /** Nombre de la clase de respuesta, para que se distinga de qué es cada cosa. */
  function clase(kind: string): string {
    return texto(`clase-${kind}`);
  }
</script>

<div class="nb-consulta">
  <p class="nb-termino">
    <Icon nombre="diccionario" />
    <span class="nb-palabra">{termino}</span>
  </p>

  {#if cargando}
    <Spinner etiqueta={texto('consultando')} tamano="1.5rem" />
  {:else if error === 'COMPONENT_UNAVAILABLE'}
    <!--
      No es un fallo: es una instalación sin diccionarios. Se dice qué falta y
      dónde se pone, en lugar de un mensaje de error que no lleva a ninguna
      parte.
    -->
    <p class="nb-aviso" role="status">
      <Icon nombre="informacion" />
      {texto('sinProveedores')}
    </p>
  {:else if error}
    <p class="nb-aviso" role="status">
      <Icon nombre="advertencia" />
      {texto('fallo')}
    </p>
  {:else if entradas.length === 0}
    <p class="nb-aviso" role="status">{texto('sinResultados')}</p>
  {/if}

  {#if entradas.length > 0}
    <ul class="nb-entradas">
      {#each entradas as entrada, indice (`${entrada.providerId}-${indice}`)}
        <li class="nb-entrada">
          <p class="nb-cabecera">
            <span class="nb-lema">{entrada.headword}</span>
            <span class="nb-clase">{clase(entrada.kind)}</span>
          </p>
          <p class="nb-cuerpo">{entrada.body}</p>
          {#if entrada.source}
            <!-- La procedencia no es cortesía: distingue lo que dice una obra
                 lexicográfica de lo que dice una máquina. -->
            <p class="nb-fuente">{entrada.source}</p>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if hayQueElegir}
    <section class="nb-palabras" aria-label={texto('otraPalabra')}>
      <h3 class="nb-titulo">{texto('otraPalabra')}</h3>
      <div class="nb-fichas">
        {#each palabras as palabra (palabra)}
          <button
            type="button"
            class="nb-ficha"
            aria-pressed={palabra.toLocaleLowerCase() === termino.toLocaleLowerCase()}
            onclick={() => alElegirPalabra?.(palabra)}
          >
            {palabra}
          </button>
        {/each}
      </div>
    </section>
  {/if}

  {#if omitidos.length > 0}
    <section class="nb-omitidos" aria-label={texto('omitidos')}>
      <h3 class="nb-titulo">{texto('omitidos')}</h3>
      <ul>
        {#each omitidos as omitido (omitido.providerId)}
          <li>
            <span class="nb-nombre">{omitido.label}</span>
            <span class="nb-motivo">{motivo(omitido)}</span>
            {#if omitido.reason === 'networkNotAllowed' && omitido.host}
              {@const destino = omitido.host}
              <!--
                Antes del botón, qué sale. Un permiso que se concede sin saber
                qué se envía no es un consentimiento informado.
              -->
              <span class="nb-motivo">
                {texto('seEnvia')}
                {queSeEnvia ?? texto('seEnviaDesconocido')}
                &middot; {destino}
              </span>
              <button type="button" class="nb-permitir" onclick={() => alAutorizar?.(destino)}>
                {texto('permitir')}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

<style>
  .nb-consulta {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
  }

  .nb-termino {
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    margin: 0;
    color: var(--m3c-on-surface-variant);
  }

  .nb-palabra {
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-lg);
    font-weight: 600;
    /* Una selección larga no puede empujar el panel: se corta y se lee lo que
       cabe, que para saber qué se consultó basta. */
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .nb-entradas,
  .nb-omitidos ul {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-entrada {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    padding-bottom: var(--nb-space-3);
  }

  .nb-entrada:last-child {
    padding-bottom: 0;
  }

  .nb-cabecera {
    display: flex;
    align-items: baseline;
    gap: var(--nb-space-2);
    margin: 0;
  }

  .nb-lema {
    color: var(--m3c-on-surface);
    font-weight: 600;
  }

  .nb-clase {
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .nb-cuerpo {
    margin: 0;
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
    line-height: 1.55;
    /* Una definición trae saltos de línea propios: respetarlos es la diferencia
       entre una entrada legible y un párrafo apelmazado. */
    white-space: pre-wrap;
  }

  .nb-fuente {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }

  .nb-palabras {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding-top: var(--nb-space-3);
  }

  .nb-fichas {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-1);
  }

  .nb-ficha {
    min-height: var(--nb-fila-compacta);
    padding: 0 var(--nb-space-2);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-sm);
    background: color-mix(in srgb, var(--m3c-surface-container-highest) 30%, transparent);
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-sm);
    cursor: pointer;
  }

  .nb-ficha:active {
    background: color-mix(in srgb, var(--m3c-on-surface) 12%, transparent);
  }

  .nb-ficha:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-ficha[aria-pressed='true'] {
    border-color: transparent;
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }

  .nb-omitidos {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding-top: var(--nb-space-3);
  }

  .nb-titulo {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .nb-omitidos li {
    display: flex;
    flex-direction: column;
  }

  .nb-nombre {
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
  }

  .nb-motivo {
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }

  .nb-permitir {
    align-self: flex-start;
    min-height: var(--nb-fila-compacta);
    margin-top: var(--nb-space-1);
    padding: 0 var(--nb-space-2);
    border: 1px solid color-mix(in srgb, var(--m3c-outline-variant) 40%, transparent);
    border-radius: var(--nb-radius-xs);
    background: color-mix(in srgb, var(--m3c-surface-container-highest) 30%, transparent);
    color: var(--m3c-on-surface);
    font: inherit;
    font-size: var(--nb-text-sm);
    cursor: pointer;
  }

  .nb-permitir:active {
    background: color-mix(in srgb, var(--m3c-primary) 15%, transparent);
  }

  .nb-permitir:hover {
    background: color-mix(in srgb, var(--m3c-surface-container-highest) 50%, transparent);
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
