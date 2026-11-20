<!--
  @component
  El panel de anotaciones del documento abierto.

  Contrato: `docs/architecture/reader-contracts.md` y
  docs/architecture/accessibility.md, sección 4..

  Tres cosas que este componente **no** hace, y son las que hacen comprobable su
  criterio de aceptación:

  1. **No interpreta teclas.** Se las pasa a `teclaDelPanel`, que es pura, y
     aplica lo que devuelve `aplicarEnPanel`. Por eso el recorrido completo
     —recorrer, saltar, editar y borrar— se prueba en `anotaciones.test.ts` sin
     navegador.
  2. **No decide el orden.** La lista llega ya en orden de lectura desde
     `annotation_outline`, con las que no se pudieron colocar al final. Ordenar
     aquí sería una segunda fuente del mismo orden, y dos fuentes divergen.
  3. **No llama al puente.** Devuelve los efectos por sus callbacks, igual que
     `ReaderView`: editar da de baja la anotación y crea su sucesora con
     identificador nuevo, y quien puede quedarse con el nuevo es quien manda.

  **Tabulación móvil.** La lista es una sola parada de tabulación: solo los
  botones de la fila activa son alcanzables con el tabulador, y las flechas
  mueven entre filas. Con tres acciones por fila, cuarenta anotaciones serían
  ciento veinte paradas, y el entregable dice "orden de tabulación predecible".
-->
<script lang="ts">
  import { idioma, textos } from '$lib/i18n';
  import { TEXTOS_DEL_LECTOR } from '$lib/i18n/locales/lector';
  import { onMount, untrack } from 'svelte';

  import type { CitationFormat, Locator, OutlineEntry } from '$lib/bindings';

  import Button from '../Button.svelte';
  import Icon from '../Icon.svelte';
  import type { NombreDeIcono } from '../iconos';
  import {
    type AccionDelPanel,
    aplicarEnPanel,
    conCursorEn,
    conEntradas,
    type EfectoDelPanel,
    escribir,
    type ModoDelPanel,
    panelInicial,
    sePuedeEditar,
    sePuedeSaltar,
    teclaDelPanel
  } from './anotaciones';

  interface Props {
    /** En orden de lectura, con las perdidas al final. Lo da el runtime. */
    entradas: OutlineEntry[];
    /**
     * Las que el runtime colocó y aun así no se pudieron pintar en la página.
     *
     * Es un estado distinto de "perdida": el runtime sabe dónde va, y lo que
     * falla es la proyección al texto compuesto. Se dice en lugar de callarse,
     * porque desde fuera "no la veo en el libro" y "no existe" se parecen
     * demasiado.
     */
    sinPintar?: ReadonlySet<string>;
    /** Si el motor no sabe pintar marcas, se dice una vez y no por entrada. */
    sinSoporteDeMarcas?: boolean;
    /**
     * Anotación a la que apuntar, si se llegó aquí señalándola en la página.
     *
     * Cambiar el valor mueve el cursor; volver a ponerlo a nulo, no. Es el mismo
     * patrón que `palabraPedida` en el lector, y por el mismo motivo: quien
     * llama no tiene forma de hablar con el estado interno de este componente.
     */
    destacada?: string | null;
    alSaltar: (locator: Locator) => void;
    /** Editar crea una sucesora: quien escucha se queda con la nueva. */
    alEditar: (id: string, cuerpo: string | null) => void;
    alBorrar: (id: string) => void;
    /**
     * Se pide llevarse las anotaciones de este documento.
     *
     * Va aquí y no en Estudio porque exportar es del **documento**: el formato
     * del W3C ancla cada anotación a su obra, y una exportación de la biblioteca
     * entera es otra cosa, que es la. Sin esta acción, el comando de
     * la función existía y el archivo no se podía producir.
     */
    alExportar?: (formato?: 'markdown' | 'text') => void;
    /**
     * Los formatos de cita que aportan las extensiones instaladas.
     *
     * Vacío mientras no haya ninguna: exportar citas es una contribución, no
     * una función del núcleo, y sin extensión no hay a qué formato exportar.
     */
    formatosDeCita?: readonly CitationFormat[];
    alExportarCitas?: (formato: string) => void;
    /**
     * Si el panel se lleva el foco al abrirse.
     *
     * Cierto en el lector: el panel se abre con una tecla, y dejar el foco donde
     * estaba obligaría a tabular un número desconocido de veces para llegar a lo
     * que acabas de abrir. No es atrapar el foco: se sale con el tabulador y con
     * Escape, y al cerrarse lo devuelve donde estaba.
     */
    tomarFoco?: boolean;
    /** Textos. Se traducen. */
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    entradas,
    sinPintar,
    sinSoporteDeMarcas = false,
    destacada = null,
    alSaltar,
    alEditar,
    alBorrar,
    alExportar,
    formatosDeCita = [],
    alExportarCitas,
    tomarFoco = true,
    etiquetas
  }: Props = $props();

  // El diccionario del idioma en uso, con las anulaciones de quien monta el
  // componente encima. `$derived` y no una constante: cambiar de idioma tiene
  // que repintar, y una constante capturaría el diccionario de un solo momento.
  const t = $derived({ ...textos().anotaciones, ...etiquetas } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;
  const tl = $derived(TEXTOS_DEL_LECTOR[idioma()]);

  // La lectura de la prop va dentro de una función, como en `ReaderView`: leerla
  // suelta en la inicialización captura solo su primer valor y el compilador lo
  // avisa. Quien mantiene la lista al día después es el efecto de abajo.
  function entradasIniciales() {
    return entradas;
  }

  let panel = $state(panelInicial(entradasIniciales()));

  let raiz: HTMLElement | undefined = $state();
  let campo: HTMLTextAreaElement | undefined = $state();
  let confirmacion: HTMLButtonElement | undefined = $state();

  // La lista se recarga con cada anotación, edición y borrado. `conEntradas`
  // conserva el cursor; leer `panel` sin `untrack` convertiría este efecto en
  // dependencia de sí mismo y se repetiría sin fin.
  $effect(() => {
    const lista = entradas;
    panel = conEntradas(
      untrack(() => panel),
      lista
    );
  });

  /*
   * Señalar una marca en la página deja el panel **en esa anotación**.
   *
   * Va después del efecto que recarga la lista y no dentro: si se llega aquí
   * señalando, la lista ya está; y si llegan las dos cosas a la vez, recargar
   * primero y apuntar después es el orden que deja el cursor donde se pidió.
   */
  $effect(() => {
    const id = destacada;
    if (id === null) return;
    panel = conCursorEn(
      untrack(() => panel),
      id
    );
  });

  /**
   * Primer botón de una fila. Es a donde va el foco cuando el cursor se mueve.
   *
   * Se busca en el DOM en lugar de guardarse en un array de referencias porque
   * cuál es el primer botón depende de la fila —una anotación perdida no ofrece
   * salto y un marcador no ofrece edición—, y un array indexado por posición
   * tendría que saber esa regla por segunda vez.
   */
  function manijaDe(indice: number): HTMLButtonElement | null {
    return raiz?.querySelector<HTMLButtonElement>(`[data-fila="${indice}"] .nb-accion`) ?? null;
  }

  /**
   * Si ya se colocó el foco una vez.
   *
   * Distingue abrir el panel de recargarlo. Al abrirlo el foco entra; después
   * solo se mueve si ya estaba dentro, porque recargar la lista mientras el
   * usuario está leyendo no puede robarle el foco de donde lo tenga.
   */
  let colocado = false;

  /**
   * El modo de la pasada anterior.
   *
   * No es estado reactivo a propósito: lo lee y lo escribe el mismo efecto, y
   * hacerlo reactivo lo convertiría en dependencia de sí mismo. Es el mismo
   * recurso que usa `ReaderView` para avisar del cambio de modo y no de la
   * pasada.
   */
  let modoAnterior: ModoDelPanel = 'lista';

  $effect(() => {
    const indice = panel.cursor;
    const modo = panel.modo;
    // Guardar una edición o confirmar un borrado **destruye** el elemento que
    // tenía el foco. Sin devolverlo a la fila, el foco cae en `body` y quien
    // navega por teclado se queda fuera del panel que estaba usando, con la
    // siguiente tecla sin efecto y sin nada que se lo explique.
    const volviendo = modoAnterior !== 'lista' && modo === 'lista';
    modoAnterior = modo;

    if (modo !== 'lista' || indice < 0) return;
    const destino = manijaDe(indice);
    if (!destino) return;

    const dentro = raiz?.contains(document.activeElement) ?? false;
    if (colocado ? dentro || volviendo : tomarFoco) destino.focus();
    colocado = true;
  });

  // El campo y la confirmación reciben el foco al aparecer: si no, escribir o
  // confirmar exigiría alcanzarlos con el ratón, que es lo que el criterio
  // prohíbe.
  $effect(() => {
    if (panel.modo === 'editando') campo?.focus();
    if (panel.modo === 'confirmando') confirmacion?.focus();
  });

  onMount(() => {
    const previo = document.activeElement as HTMLElement | null;
    return () => {
      // Al cerrarse, el foco vuelve. Sin esto queda en `body` y el siguiente
      // tabulador empieza por el principio de la pantalla, que es la forma
      // silenciosa de dejar perdido a quien navega por teclado.
      if (document.activeElement === document.body) previo?.focus?.();
    };
  });

  function ejecutar(efecto: EfectoDelPanel) {
    switch (efecto.tipo) {
      case 'saltar':
        alSaltar(efecto.locator);
        return;
      case 'editar':
        alEditar(efecto.id, efecto.cuerpo);
        return;
      case 'borrar':
        alBorrar(efecto.id);
    }
  }

  function despachar(accion: AccionDelPanel | null): boolean {
    if (!accion) return false;
    const transicion = aplicarEnPanel(panel, accion);
    panel = transicion.estado;
    if (transicion.efecto) ejecutar(transicion.efecto);
    return true;
  }

  function alPulsarTecla(evento: KeyboardEvent) {
    if (!despachar(teclaDelPanel(evento, panel.modo))) return;

    evento.preventDefault();
    // **Detener la propagación es lo que impide que una tecla haga dos cosas.**
    // El lector escucha en `window`: sin esto, la flecha abajo movería la fila
    // del panel y además el párrafo del texto, y Enter saltaría y además abriría
    // una nota sobre lo que hubiera seleccionado.
    evento.stopPropagation();
  }

  /** Acciones de una fila, en el orden en que se pintan. */
  interface Accion {
    id: 'ir' | 'editar' | 'borrar';
    etiqueta: string;
    icono: NombreDeIcono;
  }

  const ACCION_DE_BOTON: Record<Accion['id'], AccionDelPanel> = {
    ir: 'saltar',
    editar: 'editar',
    borrar: 'borrar'
  };

  function accionesDe(entrada: OutlineEntry): Accion[] {
    const lista: Accion[] = [];
    if (sePuedeSaltar(entrada)) lista.push({ id: 'ir', etiqueta: texto('ir'), icono: 'lector' });
    if (sePuedeEditar(entrada)) {
      lista.push({ id: 'editar', etiqueta: texto('editar'), icono: 'editar' });
    }
    // Borrar siempre: una anotación perdida sigue siendo suya, y quitarla de en
    // medio es lo único que se puede hacer con ella.
    lista.push({ id: 'borrar', etiqueta: texto('borrar'), icono: 'borrar' });
    return lista;
  }

  /**
   * Actúa sobre una fila desde su botón.
   *
   * Mueve el cursor antes de actuar: sin eso, pulsar el botón de una fila que no
   * es la activa actuaría sobre la activa, que es un borrado en el sitio
   * equivocado esperando a ocurrir.
   */
  function activar(indice: number, accion: Accion['id']) {
    const transicion = aplicarEnPanel({ ...panel, cursor: indice }, ACCION_DE_BOTON[accion]);
    panel = transicion.estado;
    if (transicion.efecto) ejecutar(transicion.efecto);
  }

  const ICONO_DE_CLASE: Record<string, NombreDeIcono> = {
    note: 'nota',
    highlight: 'resaltar',
    bookmark: 'marcador'
  };

  const NOMBRE_DE_CLASE: Record<string, string> = {
    note: 'nota',
    highlight: 'resaltado',
    bookmark: 'marcador'
  };

  /** Lo que se lee de una entrada: la cita, y el título cuando no hay cita. */
  function citaDe(entrada: OutlineEntry): string {
    const cita = entrada.annotation.target.text?.highlight?.trim();
    if (cita) return cita;
    return entrada.annotation.target.title?.trim() || texto('sinTexto');
  }

  /** Avisos de una entrada. Perdida sustituye a los demás: es el más grave. */
  function avisosDe(entrada: OutlineEntry): string[] {
    if (entrada.lost) return [texto('perdida')];
    const avisos: string[] = [];
    if (entrada.warn) avisos.push(texto('aproximada'));
    if (entrada.relocated) avisos.push(texto('reanclada'));
    if (sinPintar?.has(entrada.annotation.id)) avisos.push(texto('sinPintar'));
    return avisos;
  }
</script>

<div bind:this={raiz}>
  {#if entradas.length === 0}
    <p class="nb-vacio">{texto('vacio')}</p>
  {:else}
    {#if sinSoporteDeMarcas}
      <p class="nb-sin-marcas" role="status">{texto('sinSoporteDeMarcas')}</p>
    {/if}
    {#if alExportar}
      <div class="nb-acciones">
        <Button onclick={() => alExportar?.()}>{texto('exportar')}</Button>
        <!--
          Los demás formatos, plegados: cinco botones seguidos ocupaban la
          cabecera del panel antes de la primera anotación. Markdown y
          texto plano se leen sin NB Books; el de arriba es el JSON de
          W3C. Los de cita los aporta una extensión.
        -->
        <details class="nb-otros-formatos">
          <summary>{tl.anotacionesOtrosFormatos}</summary>
          <div class="nb-formatos">
            <Button onclick={() => alExportar?.('markdown')}>{tl.anotacionesMarkdown}</Button>
            <Button onclick={() => alExportar?.('text')}>{tl.anotacionesTexto}</Button>
            {#each formatosDeCita as formato (formato.id)}
              <Button onclick={() => alExportarCitas?.(formato.id)}>
                {texto('exportarCitas')}
                {formato.name}
              </Button>
            {/each}
          </div>
        </details>
      </div>
    {/if}
    <ul class="nb-lista" aria-label={texto('lista')}>
      <!--
        Se indexa por **posición y no por identificador**, igual que el índice
        del documento. No es descuido: editar una anotación devuelve una
        sucesora con identificador nuevo, así que con la clave puesta en el
        identificador la fila se destruye y se vuelve a crear, y el foco de
        quien acababa de guardar se cae a `body`. Por posición, la fila se
        reutiliza y el foco sobrevive. Al borrar, además, es lo que se quiere:
        el cursor se queda en su sitio y pasa a señalar a la siguiente.
      -->
      {#each entradas as entrada, indice (indice)}
        {@const activa = indice === panel.cursor}
        <li class="nb-fila" data-fila={indice} data-activa={activa} data-perdida={entrada.lost}>
          <div class="nb-cabecera">
            <span class="nb-clase">
              <Icon nombre={ICONO_DE_CLASE[entrada.annotation.kind] ?? 'nota'} />
              {texto(NOMBRE_DE_CLASE[entrada.annotation.kind] ?? 'nota')}
            </span>
            <div class="nb-acciones">
              {#each accionesDe(entrada) as accion (accion.id)}
                <button
                  type="button"
                  class="nb-accion"
                  aria-label={accion.etiqueta}
                  title={accion.etiqueta}
                  tabindex={activa ? 0 : -1}
                  onkeydown={alPulsarTecla}
                  onclick={() => activar(indice, accion.id)}
                >
                  <span class="nb-glifo" aria-hidden="true">
                    <Icon nombre={accion.icono} />
                  </span>
                </button>
              {/each}
            </div>
          </div>

          <p class="nb-cita">{citaDe(entrada)}</p>

          {#if entrada.annotation.body}
            <p class="nb-cuerpo">{entrada.annotation.body}</p>
          {/if}

          {#each avisosDe(entrada) as aviso (aviso)}
            <p class="nb-aviso">
              <Icon nombre={entrada.lost ? 'error' : 'advertencia'} />
              {aviso}
            </p>
          {/each}

          {#if activa && panel.modo === 'editando'}
            <div class="nb-edicion">
              <textarea
                bind:this={campo}
                class="nb-campo"
                aria-label={texto('cuerpo')}
                value={panel.borrador}
                onkeydown={alPulsarTecla}
                oninput={(e) => (panel = escribir(panel, e.currentTarget.value))}></textarea>
              <div class="nb-botones">
                <button
                  type="button"
                  class="nb-boton"
                  onkeydown={alPulsarTecla}
                  onclick={() => despachar('guardar')}
                >
                  {texto('guardar')}
                </button>
                <button
                  type="button"
                  class="nb-boton"
                  onkeydown={alPulsarTecla}
                  onclick={() => despachar('cancelar')}
                >
                  {texto('cancelar')}
                </button>
              </div>
            </div>
          {/if}

          {#if activa && panel.modo === 'confirmando'}
            <div class="nb-edicion" role="group" aria-label={texto('confirmar')}>
              <p class="nb-seguro">{texto('seguro')}</p>
              <div class="nb-botones">
                <button
                  bind:this={confirmacion}
                  type="button"
                  class="nb-boton nb-peligro"
                  onkeydown={alPulsarTecla}
                  onclick={() => despachar('confirmar-borrado')}
                >
                  {texto('confirmar')}
                </button>
                <button
                  type="button"
                  class="nb-boton"
                  onkeydown={alPulsarTecla}
                  onclick={() => despachar('cancelar')}
                >
                  {texto('cancelar')}
                </button>
              </div>
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  /* La acción del panel entero, separada de la lista y antes que ella. */
  .nb-acciones {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--nb-space-2);
    padding: 0 var(--nb-space-3) var(--nb-space-2);
  }

  .nb-otros-formatos {
    flex-basis: 100%;
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }
  .nb-otros-formatos summary {
    cursor: pointer;
    text-align: end;
  }
  .nb-formatos {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--nb-space-2);
    padding-top: var(--nb-space-2);
  }

  .nb-lista {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-fila {
    padding: var(--nb-space-2) var(--nb-space-3);
    border: 1px solid transparent;
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
  }

  /*
   * La fila activa se marca con borde y con fondo, no solo con fondo: en alto
   * contraste el fondo suave desaparece, y quedarse sin ninguna señal de dónde
   * está el cursor es justo lo que rompe la navegación por teclado.
   */
  .nb-fila[data-activa='true'] {
    border-color: var(--m3c-primary);
    background: color-mix(in srgb, var(--m3c-primary) 12%, var(--m3c-surface-container-high));
  }

  .nb-fila[data-perdida='true'] .nb-cita {
    color: var(--m3c-on-surface-variant);
  }

  .nb-cabecera {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-2);
  }

  .nb-clase {
    display: inline-flex;
    align-items: center;
    gap: var(--nb-space-1);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }

  .nb-acciones {
    display: flex;
    gap: var(--nb-space-1);
  }

  .nb-accion {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: calc(2rem * var(--nb-font-scale));
    height: calc(2rem * var(--nb-font-scale));
    padding: 0;
    border: none;
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    cursor: pointer;
  }

  .nb-accion:active {
    background: color-mix(in srgb, var(--m3c-primary) 15%, transparent);
  }

  .nb-accion:hover {
    background: color-mix(in srgb, var(--m3c-surface-container-highest) 50%, transparent);
    color: var(--m3c-on-surface);
  }

  .nb-glifo {
    display: flex;
    align-items: center;
    justify-content: center;
    line-height: 1;
  }

  .nb-cita {
    margin: var(--nb-space-1) 0 0;
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
    /* La cita puede ser larga; recortarla no la pierde, solo la enseña. */
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .nb-cuerpo {
    margin: var(--nb-space-1) 0 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }

  .nb-aviso {
    display: flex;
    align-items: center;
    gap: var(--nb-space-1);
    margin: var(--nb-space-1) 0 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }

  .nb-edicion {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    margin-top: var(--nb-space-2);
  }

  .nb-campo {
    min-height: 4rem;
    padding: var(--nb-space-2);
    border: 1px solid color-mix(in srgb, var(--m3c-outline-variant) 40%, transparent);
    border-radius: var(--nb-radius-xs);
    background: color-mix(in srgb, var(--m3c-surface-container-highest) 30%, transparent);
    color: var(--m3c-on-surface);
    font-family: var(--nb-font-ui);
    font-size: var(--nb-text-sm);
    resize: vertical;
  }

  .nb-seguro {
    margin: 0;
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
  }

  .nb-botones {
    display: flex;
    gap: var(--nb-space-2);
  }

  .nb-boton {
    padding: var(--nb-space-1) var(--nb-space-2);
    border: 1px solid color-mix(in srgb, var(--m3c-outline-variant) 40%, transparent);
    border-radius: var(--nb-radius-xs);
    background: color-mix(in srgb, var(--m3c-surface-container-highest) 30%, transparent);
    color: var(--m3c-on-surface);
    font-family: inherit;
    font-size: var(--nb-text-sm);
    cursor: pointer;
  }

  .nb-boton:active {
    background: color-mix(in srgb, var(--m3c-primary) 15%, transparent);
  }

  .nb-boton:hover {
    background: color-mix(in srgb, var(--m3c-surface-container-highest) 50%, transparent);
  }

  .nb-peligro {
    border-color: var(--m3c-error);
    color: var(--m3c-error);
  }

  .nb-vacio {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }

  /*
   * El aviso de que el motor no sabe marcar el texto. Va arriba y una sola vez:
   * no es de ninguna anotación, es de todas.
   */
  .nb-sin-marcas {
    margin: 0 0 var(--nb-space-3);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }
</style>
