<!--
  @component
  Panel lateral. Es una zona `complementary`, no un diálogo.

  Contrato: docs/architecture/ui-components.md, secciones 1 y 2.

  Dos reglas que vienen de ADR-012 y que este componente hace cumplir:

  1. **Sabe colapsar.** En la clase compacta deja de ocupar hueco y se
     superpone al contenido, descartable. Un panel que no sabe colapsar
     no entra.
  2. **No atrapa el foco.** Un panel no es modal, y atrapar el foco en algo que
     el usuario no percibe como modal es una de las formas más frecuentes de
     dejar encerrado a quien navega por teclado. Se cierra con Escape porque
     eso ayuda, no porque sea un diálogo.
-->
<script lang="ts">
  import { textos } from '$lib/i18n';
  import type { Snippet } from 'svelte';

  import { elPanelSeSuperpone } from './anchos';
  import { usarCarcasa } from './carcasa.svelte';

  interface Props {
    /** Título visible del panel. Es también su nombre accesible. */
    titulo: string;
    children: Snippet;
    /** Si se omite, el panel no se puede cerrar y no muestra el botón. */
    alCerrar?: () => void;
    /**
     * Panel **momentáneo**: se abre encima de lo que haya y se va en cuanto se
     * pulsa fuera.
     *
     * Distingue las dos clases de panel que hay. Uno es parte de la vista —la
     * ficha de la biblioteca, el índice del documento— y ocupa su hueco: está
     * mientras estés ahí. El otro se consulta y se cierra —buscar, importar,
     * estado—, y quitarle sitio al contenido para algo que dura veinte segundos
     * es cobrar caro por poco.
     */
    momentaneo?: boolean;
    /** Texto del botón de cierre. Se traduce. */
    etiquetaCerrar?: string;
  }

  let { titulo, children, alCerrar, momentaneo = false, etiquetaCerrar }: Props = $props();

  const nombreCerrar = $derived(etiquetaCerrar ?? textos().carcasa.cerrarPanel);

  const carcasa = usarCarcasa();
  // La clase del **contenido**, no la de la carcasa: ver `carcasa.svelte.ts`.
  // Solo por el ancho. Ser momentáneo es otra cosa y va en su propio atributo:
  // mezclarlos hacía que un panel momentáneo se creyera siempre en clase
  // compacta y ocupara la vista entera aunque hubiera sitio de sobra.
  const superpuesto = $derived(elPanelSeSuperpone(carcasa.claseDelContenido));

  const idTitulo = $props.id();
  let raiz: HTMLElement | undefined = $state();

  function alPulsarTecla(evento: KeyboardEvent) {
    if (evento.key === 'Escape' && alCerrar) {
      evento.stopPropagation();
      alCerrar();
    }
  }

  /**
   * Pulsar fuera cierra un panel momentáneo.
   *
   * Se escucha el `click` y no el `pointerdown` por el botón que lo abrió: con
   * `pointerdown` este manejador cerraría el panel y el `click` del botón lo
   * volvería a abrir, así que pulsarlo dejaría de cerrarlo. Y se ignora
   * cualquier control con `aria-expanded`, que es justo el que ya alterna por su
   * cuenta.
   */
  function alPulsarFuera(evento: MouseEvent) {
    if (!momentaneo || !alCerrar || !raiz) return;

    const destino = evento.target;
    if (!(destino instanceof Node) || raiz.contains(destino)) return;
    /*
     * Un nodo que ya no está en el documento **no está fuera del panel**: es que
     * estaba dentro y desapareció al repintar.
     *
     * Ocurre siempre que un control del panel cambia algo que rehace su propio
     * marcado. Cambiar la alineación del texto en Ajustes del Libro era
     * exactamente eso: el botón se sustituía, y para cuando el clic llegaba a
     * `window` su nodo ya no tenía padre, así que `raiz.contains` decía que no y
     * el panel se cerraba solo al usarlo. Los eventos suben después de que
     * Svelte haya aplicado el cambio, así que el orden no es una casualidad que
     * se pueda esquivar: hay que preguntarlo.
     */
    if (!destino.isConnected) return;
    if (destino instanceof Element && destino.closest('[aria-expanded]')) return;

    alCerrar();
  }
</script>

<svelte:window onkeydown={alPulsarTecla} onclick={alPulsarFuera} />

<aside
  class="nb-panel"
  bind:this={raiz}
  aria-labelledby={idTitulo}
  data-superpuesto={superpuesto}
  data-momentaneo={momentaneo}
>
  <div class="nb-panel-cabecera">
    <h2 class="nb-panel-titulo" id={idTitulo}>{titulo}</h2>
    {#if alCerrar}
      <button class="nb-panel-cerrar" type="button" onclick={alCerrar} aria-label={nombreCerrar}>
        <span aria-hidden="true">&times;</span>
      </button>
    {/if}
  </div>
  <div class="nb-panel-contenido">{@render children()}</div>
</aside>

<style>
  .nb-panel {
    display: flex;
    flex-direction: column;
    flex: 0 0 var(--nb-panel-width);
    width: var(--nb-panel-width);
    min-width: 0;
    min-height: 0;
    margin: var(--nb-space-3) var(--nb-space-3) var(--nb-space-3) 0;
    border-radius: var(--nb-radius-lg);

    /*
     * Translúcido, sin desenfoque y sin fondo animado.
     *
     * El degradado que había aquí llamaba a `nb-fondo-vivo`, que vive en la
     * carcasa: Svelte acota los fotogramas clave al componente, así que esa
     * animación nunca corrió. Lo que sí hacía el `background-size: 400%` era
     * enseñar una cuarta parte del degradado, es decir, un color plano con tres
     * declaraciones de más.
     *
     * Y el desenfoque se va por lo mismo que en la barra lateral: un panel es
     * cromo permanente al lado de una zona que se desplaza, y desenfocar el
     * fondo de algo así obliga a rehacer el desenfoque en cada fotograma de
     * desplazamiento. En WebKitGTK, que es el motor en Linux, eso se ve como
     * parpadeo.
     *
     * # Por qué opaco, y no translúcido
     *
     * Porque un panel del lector se apoya sobre **la superficie de lectura**, y
     * el fondo de esa superficie no lo elige el tema: lo elige el modo de
     * lectura, que va del sepia al negro. Translúcido, el color resultante era
     * una mezcla con algo que la paleta no controla, y el contraste del texto
     * del panel dejaba de estar garantizado: medido, 4,17:1 sobre un umbral de
     * 4,5. Que nadie lo hubiera visto antes es porque el degradado que había
     * aquí dejaba `background-color` en transparente, y la auditoría no puede
     * medir lo que no sabe calcular.
     */
    background: var(--m3c-surface-container);
    position: relative;
    overflow: hidden;
  }

  .nb-panel::before {
    content: '';
    position: absolute;
    inset: 0;
    background: var(--nb-textura, none);
    opacity: var(--nb-textura-opacidad, 0);
    pointer-events: none;
    z-index: -1;
  }

  .nb-panel-cabecera {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-2);
    padding: var(--nb-space-3) var(--nb-space-3) var(--nb-space-2) var(--nb-space-4);
  }

  .nb-panel-titulo {
    margin: 0;
    font-size: var(--nb-text-md);
    font-weight: 600;
    color: var(--m3c-on-surface);
  }

  .nb-panel-cerrar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 2.5rem;
    height: 2.5rem;
    padding: 0;
    border: none;
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-lg);
    line-height: 1;
    cursor: pointer;
    transition: background var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-panel-cerrar:hover {
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface);
  }

  .nb-panel-contenido {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
    padding: var(--nb-space-3);
  }

  /*
   * Colapsado: deja de ocupar hueco propio y se superpone. Sigue después del
   * contenido en el orden del documento, así que quien navega por teclado o con
   * lector de pantalla lo encuentra donde espera.
   */
  .nb-panel[data-superpuesto='true'] {
    position: absolute;
    inset: 0;
    z-index: 20;
    flex-basis: auto;
    margin: var(--nb-space-2);
  }

  /*
   * Momentáneo en una ventana con sitio: no ocupa el ancho entero como en la
   * clase compacta, se queda en su borde y con su ancho. Así lo que hay debajo
   * sigue a la vista, que es lo que hace evidente que esto se va enseguida y que
   * se puede pulsar ahí para quitarlo.
   */
  .nb-panel[data-momentaneo='true'] {
    position: absolute;
    inset: 0 0 0 auto;
    width: var(--nb-panel-width);
    max-width: calc(100% - 2 * var(--nb-space-3));
    z-index: 20;
    background: var(--m3c-surface-container-high);
    box-shadow: var(--m3-elevation-2);
  }

  /* En compacto vuelve a ocupar todo: ahí no hay debajo que enseñar. */
  .nb-panel[data-momentaneo='true'][data-superpuesto='true'] {
    inset: 0;
  }
</style>
