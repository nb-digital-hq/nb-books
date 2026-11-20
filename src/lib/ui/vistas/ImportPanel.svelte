<!--
  @component
  Traer documentos a la biblioteca.

  La ruta **solo puede venir de un gesto del usuario**: se
  suelta sobre la ventana o se escribe. El runtime la canonicaliza y la acota,
  porque el webview no es de fiar.

  Hay dos vías a propósito, y no una: soltar es lo natural con ratón, pero una
  acción exclusiva del ratón está prohibida, así que escribir la ruta es la vía
  de teclado y no un apaño. Las dos acaban en el mismo comando.
-->
<script lang="ts">
  import { idioma, mensajeDeError, textos } from '$lib/i18n';
  import { TEXTOS_DE_COPIAS } from '$lib/i18n/locales/copias';
  import type { ImportDuplicate, ImportRejection, ImportReport, ImportRun } from '$lib/bindings';

  import Button from '../Button.svelte';
  import Field from '../Field.svelte';
  import Icon from '../Icon.svelte';
  import Switch from '../Switch.svelte';

  interface Props {
    /** Lo ejecuta quien monta el panel: aquí no se llama al puente. */
    alImportar: (ruta: string, copiar: boolean) => Promise<ImportReport>;
    /** Cancela la que esté en curso. */
    alCancelar?: () => void;
    /**
     * Abre el selector del sistema. Devuelve las rutas, o `null` si se cancela.
     *
     * Llega de fuera porque el diálogo lo abre el runtime: este panel no habla
     * con el sistema, solo dice qué quiere.
     */
    alElegir?: (carpeta: boolean) => Promise<string[] | null>;
    /**
     * Avance de una importación en marcha, o `null` si no hay ninguna.
     *
     * Llega de fuera porque el evento es del runtime y quien lo escucha es la
     * aplicación: un panel que se suscribe por su cuenta deja de recibir avances
     * en cuanto se cierra, y cerrarlo no para la importación.
     */
    avance?: {
      processed: number;
      imported: number;
      skipped: number;
      rejected: number;
      /** Copias exactas de algo que ya estaba. */
      duplicates?: number;
      total: number;
    } | null;
    /**
     * La importación que quedó a medias, si la hay.
     *
     * Llega de fuera porque se consulta al arrancar y no al abrir el panel: la
     * aplicación tiene que poder **decirlo** sin que nadie vaya a buscarlo. Aquí
     * vive la acción, que es donde importar tiene sentido.
     */
    pendiente?: ImportRun | null;
    alContinuar?: (ejecucion: ImportRun) => void;
    alDescartar?: (ejecucion: ImportRun) => void;
    /**
     * El informe de la última importación, leído del disco.
     *
     * El informe en memoria muere con esta ejecución del panel; este sobrevive a
     * cerrarlo y a cerrar la aplicación, que es lo que hacía falta para poder
     * mirar qué quedó fuera sin haberlo apuntado a mano.
     */
    guardado?: {
      ejecucion: ImportRun;
      rechazos: ImportRejection[];
      copias?: ImportDuplicate[];
    } | null;
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    alImportar,
    alCancelar,
    alElegir,
    avance = null,
    pendiente = null,
    alContinuar,
    alDescartar,
    guardado = null,
    etiquetas
  }: Props = $props();

  // El diccionario del idioma en uso, con las anulaciones de quien monta el
  // componente encima. `$derived` y no una constante: cambiar de idioma tiene
  // que repintar, y una constante capturaría el diccionario de un solo momento.
  const t = $derived({
    ...textos().importar,
    ...TEXTOS_DE_COPIAS[idioma()],
    ...etiquetas
  } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  let ruta = $state('');
  let copiar = $state(false);
  let trabajando = $state(false);
  let informe = $state<ImportReport | null>(null);
  let fallo = $state<string | null>(null);

  /** Pide al sistema una carpeta o unos archivos y los importa. */
  async function elegir(carpeta: boolean) {
    const elegido = await alElegir?.(carpeta);
    // Cancelar no es un error: cambiar de opinión es una respuesta.
    if (!elegido || elegido.length === 0) return;

    for (const camino of elegido) {
      ruta = camino;
      await importar();
      // Una carpeta arranca en su hilo: encadenar la siguiente pisaría el
      // avance de la anterior.
      if (informe?.running) break;
    }
  }

  async function importar() {
    if (ruta.trim().length === 0 || trabajando) return;
    trabajando = true;
    fallo = null;
    try {
      avanceVisto = false;
      informe = await alImportar(ruta.trim(), copiar);
    } catch (error) {
      informe = null;
      fallo = mensajeDeError(error);
    } finally {
      trabajando = false;
    }
  }

  /** Una carpeta sigue en marcha después de responder; un archivo suelto, no. */
  const enMarcha = $derived(informe?.running === true && avance !== null);

  /*
   * Una carpeta terminó cuando, después de verse su avance, el avance desaparece:
   * es lo que hace la aplicación al recibir el fin. Antes el panel seguía
   * diciendo "Importando" después de terminar, y el informe guardado -los
   * rechazos, las copias- no aparecía hasta cerrarlo y volver a abrirlo. Sin haber
   * visto avance no se da por terminada: sin bus, el panel no puede saberlo, y
   * decir "Importando" es lo honrado.
   */
  let avanceVisto = $state(false);
  $effect(() => {
    if (avance) avanceVisto = true;
  });
  const vigente = $derived(
    informe?.running === true && avanceVisto && avance === null ? null : informe
  );
</script>

<div class="nb-importar">
  {#if alElegir}
    <!--
      La vía principal es el diálogo del sistema: la ruta la elige el usuario en
      su escritorio y vuelve firmada por el sistema operativo. Escribirla sigue
      estando debajo, y no es un apaño: es la vía de teclado, y una acción
      exclusiva del ratón está prohibida.
    -->
    <div class="nb-elegir">
      <Button variante="primario" deshabilitado={enMarcha} onclick={() => void elegir(true)}>
        <Icon nombre="carpeta" tamano={15} />
        {texto('elegirCarpeta')}
      </Button>
      <Button deshabilitado={enMarcha} onclick={() => void elegir(false)}>
        <Icon nombre="importar" tamano={15} />
        {texto('elegirArchivos')}
      </Button>
    </div>
  {/if}

  <Field etiqueta={texto('ruta')} descripcion={texto('ayuda')}>
    {#snippet children({ id, 'aria-describedby': describedby })}
      <input
        {id}
        class="nb-campo"
        type="text"
        aria-describedby={describedby}
        bind:value={ruta}
        onkeydown={(e) => {
          if (e.key === 'Enter') void importar();
        }}
      />
    {/snippet}
  </Field>

  <div class="nb-fila">
    <span>{texto('copiar')}</span>
    <Switch etiqueta={texto('copiar')} activo={copiar} alCambiar={(v) => (copiar = v)} />
  </div>
  <p class="nb-ayuda">{texto('copiarAyuda')}</p>

  <div class="nb-acciones">
    <!--
      Deshabilitado mientras hay una en marcha, y no solo mientras se espera la
      respuesta del comando: una importación de carpeta responde enseguida y
      sigue trabajando en su hilo, así que sin esto se podía pulsar otra vez y
      lanzar una segunda contra la misma base.
    -->
    <Button
      variante="primario"
      deshabilitado={trabajando || enMarcha}
      onclick={() => void importar()}
    >
      {trabajando || enMarcha ? texto('trabajando') : texto('importar')}
    </Button>
    {#if enMarcha && alCancelar}
      <Button onclick={alCancelar}>{texto('cancelar')}</Button>
    {/if}
  </div>

  {#if avance}
    <!--
      Barra **y** cifras. La barra se puede pintar porque el runtime recorre la
      carpeta entera antes de importar nada y manda el total desde el primer
      aviso: antes solo mandaba lo procesado, y un contador que
      sube sin decir hasta dónde no responde a la única pregunta que importa
      cuando entran ciento cincuenta mil documentos.
    -->
    <p class="nb-ayuda" role="status">
      {texto('enMarcha')}
      {avance.processed}
      {texto('de')}
      {avance.total}
      {#if avance.total > 0}({Math.floor((avance.processed / avance.total) * 100)} %){/if}
    </p>
    <div
      class="nb-progreso"
      role="progressbar"
      aria-valuemin="0"
      aria-valuemax={avance.total}
      aria-valuenow={avance.processed}
      aria-label={texto('enMarcha')}
    >
      <span
        class="nb-progreso-hecho"
        style:width="{avance.total > 0
          ? Math.min(100, (avance.processed / avance.total) * 100)
          : 0}%"
      ></span>
    </div>
    <dl class="nb-informe">
      <div>
        <dt>{texto('revisados')}</dt>
        <dd>{avance.processed}</dd>
      </div>
      <div>
        <dt>{texto('nuevos')}</dt>
        <dd>{avance.imported}</dd>
      </div>
      <div>
        <dt>{texto('yaEstaban')}</dt>
        <dd>{avance.skipped}</dd>
      </div>
      <!--
        Aparte de "ya estaban": aquello es el mismo archivo visto otra
        vez, y esto, el mismo libro dos veces en el disco. Solo si hay alguna,
        para no añadir un cero a cada importación.
      -->
      {#if avance.duplicates}
        <div>
          <dt>{texto('copias')}</dt>
          <dd>{avance.duplicates}</dd>
        </div>
      {/if}
      <div>
        <dt>{texto('rechazados')}</dt>
        <dd>{avance.rejected}</dd>
      </div>
    </dl>
  {:else if vigente?.running}
    <p class="nb-ayuda" role="status">{texto('enMarcha')}</p>
  {/if}

  {#if pendiente}
    <!--
      La importación que quedó a medias, con sus dos salidas.

      Va arriba y no al final: es lo primero que hay que contestar. Y tiene
      **dos** botones y no uno, porque una oferta que solo se puede aceptar se
      convierte en el aviso que la gente aprende a ignorar; poder decir que no la
      cierra para siempre.
    -->
    <div class="nb-pendiente" role="status">
      <p class="nb-pendiente-texto">
        {texto('quedoAMedias')}
      </p>
      <p class="nb-pendiente-ruta">{pendiente.root}</p>
      <div class="nb-pendiente-acciones">
        <Button variante="primario" onclick={() => alContinuar?.(pendiente)}>
          {texto('continuar')}
        </Button>
        <Button onclick={() => alDescartar?.(pendiente)}>{texto('descartar')}</Button>
      </div>
    </div>
  {/if}

  {#if vigente && !vigente.running}
    <dl class="nb-informe">
      <div>
        <dt>{texto('nuevos')}</dt>
        <dd>{vigente.imported}</dd>
      </div>
      <div>
        <dt>{texto('yaEstaban')}</dt>
        <dd>{vigente.alreadyInLibrary}</dd>
      </div>
      {#if vigente.duplicates > 0}
        <div>
          <dt>{texto('copias')}</dt>
          <dd>{vigente.duplicates}</dd>
        </div>
      {/if}
      <div>
        <dt>{texto('rechazados')}</dt>
        <dd>{vigente.rejected.length}</dd>
      </div>
    </dl>
    {#if vigente.rejected.length > 0}
      <ul class="nb-rechazados">
        {#each vigente.rejected.slice(0, 20) as rechazo, indice (indice)}
          <li>
            <Icon nombre="advertencia" tamano={14} />
            <span class="nb-rechazo-ruta">{rechazo.path}</span>
            <span class="nb-rechazo-motivo" title={rechazo.code}>{mensajeDeError(rechazo)}</span>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  {#if !vigente && guardado && guardado.ejecucion.rejected > 0}
    <!--
      El informe de la última importación, leído del disco.

      Solo cuando no hay uno de esta sesión: enseñar los dos a la vez obligaría a
      distinguir cuál es cuál, y el de ahora siempre gana. Y solo si hubo
      rechazos, porque una importación en la que entró todo no tiene nada que
      contar que el recuento no diga ya.
    -->
    <p class="nb-ayuda">
      {texto('ultimaImportacion')}: {guardado.ejecucion.rejected}
      {texto('rechazados')}
      {#if guardado.rechazos.length < guardado.ejecucion.rejected}
        ({texto('seListanLosPrimeros')} {guardado.rechazos.length})
      {/if}
    </p>
    <ul class="nb-rechazados">
      {#each guardado.rechazos.slice(0, 20) as rechazo, indice (indice)}
        <li>
          <Icon nombre="advertencia" tamano={14} />
          <span class="nb-rechazo-ruta">{rechazo.path}</span>
          <span class="nb-rechazo-motivo" title={rechazo.code}>{mensajeDeError(rechazo)}</span>
        </li>
      {/each}
    </ul>
  {/if}

  {#if !vigente && guardado && guardado.ejecucion.duplicates > 0}
    <!--
      Las copias exactas de la última importación, con de qué es copia
      cada una: el título es como se reconoce qué se tiene dos veces, y la ruta,
      dónde está la que sobra. No entraron, así que no hay nada que deshacer.
    -->
    <p class="nb-ayuda">
      {texto('ultimaImportacion')}: {guardado.ejecucion.duplicates}
      {texto('copias')}
      {#if (guardado.copias?.length ?? 0) < guardado.ejecucion.duplicates}
        ({texto('seListanLosPrimeros')} {guardado.copias?.length ?? 0})
      {/if}
    </p>
    <p class="nb-ayuda">{texto('copiasAyuda')}</p>
    <ul class="nb-rechazados" aria-label={texto('copias')}>
      {#each (guardado.copias ?? []).slice(0, 20) as copia, indice (indice)}
        <li>
          <Icon nombre="duplicado" tamano={14} />
          <span class="nb-rechazo-ruta">{copia.path}</span>
          <span class="nb-copia-de">{texto('copiaDe')} «{copia.originalTitle}»</span>
        </li>
      {/each}
    </ul>
  {/if}

  {#if fallo}
    <p class="nb-fallo" role="alert">{fallo}</p>
  {/if}
</div>

<style>
  .nb-importar {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
  }

  /*
   * Envuelve, como todo lo demás. Con la escala tipográfica subida los dos
   * botones dejan de caber en el ancho del panel, y sin esto el contenido se
   * llevaba una barra de desplazamiento horizontal dentro de la vista: nada se
   * desplaza en horizontal, ver design/ui-conventions.md.
   */
  .nb-elegir {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
  }

  .nb-elegir > :global(*) {
    flex: 1 1 auto;
    min-width: 0;
  }

  .nb-campo {
    box-sizing: border-box;
    width: 100%;
    min-height: 2.5rem;
    padding: var(--nb-space-2) var(--nb-space-3);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface);
    font-family: monospace;
    font-size: var(--nb-text-xs);
  }

  /* La pista reserva su alto para que la barra no empuje al aparecer. */
  .nb-progreso {
    overflow: hidden;
    width: 100%;
    height: 4px;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-highest);
  }

  .nb-progreso-hecho {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--m3c-primary);
    transition: width var(--nb-motion-base) var(--nb-easing);
  }

  .nb-fila {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-2) var(--nb-space-3);
    font-size: var(--nb-text-sm);
  }

  .nb-ayuda {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }

  .nb-pendiente {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding: var(--nb-space-3);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-highest);
  }

  .nb-pendiente-texto {
    margin: 0;
    font-size: var(--nb-text-sm);
  }

  .nb-pendiente-ruta {
    margin: 0;
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
    overflow-wrap: anywhere;
  }

  .nb-pendiente-acciones {
    display: flex;
    gap: var(--nb-space-2);
  }

  .nb-informe {
    display: flex;
    gap: var(--nb-space-4);
    margin: 0;
  }

  .nb-informe dt {
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
  }

  .nb-informe dd {
    margin: 0;
    font-size: var(--nb-text-lg);
    font-variant-numeric: tabular-nums;
  }

  .nb-rechazados {
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 12rem;
    overflow: auto;
  }

  /*
   * La fila se parte antes que desbordarse.
   *
   * El código de error va en monoespaciada y es una sola palabra, así que su
   * ancho mínimo es la palabra entera: no hay forma de encogerlo. Con la escala
   * tipográfica al doble empujaba la fila más ancha que su caja y la lista
   * aparecía con barra de desplazamiento **horizontal**, que es lo que la sección
   * "nada se desplaza en horizontal" de `ui-conventions.md` prohíbe.
   *
   * Con `wrap`, el código baja a la línea siguiente cuando no cabe. Se prefiere a
   * recortarlo con puntos suspensivos porque un código a medias no se puede
   * traducir ni buscar, y es justo el dato que dice por qué el archivo no entró.
   *
   * Estaba desde antes y no se veía: la lista solo aparecía después de importar,
   * así que ningún recorrido llegaba a pintarla. Lo destapó el recorrido al enseñar
   * también el informe guardado, que sí está ahí desde el primer momento.
   */
  /*
   * Ruta arriba y motivo debajo, sin partir: con los dos en una fila que se
   * partía, el código saltaba de línea y el icono se quedaba solo.
   */
  .nb-rechazados li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: var(--nb-space-2);
    min-height: var(--nb-fila-compacta);
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
  }

  .nb-rechazo-ruta {
    /* Sin esto, el tamaño mínimo automático de un elemento flexible es el de su
       contenido, y una ruta larga no dejaría encoger nada. */
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  /* El motivo en palabras, no el código: el código va en el `title`. */
  .nb-rechazo-motivo {
    grid-column: 2;
    color: var(--m3c-on-surface-variant);
  }

  /* El título de lo que ya estaba cede antes que la ruta de la copia. */
  .nb-copia-de {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .nb-fallo {
    margin: 0;
    color: var(--m3c-error);
    font-size: var(--nb-text-sm);
  }

  .nb-acciones {
    display: flex;
    gap: var(--nb-space-2);
  }
</style>
