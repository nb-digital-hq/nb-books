<!--
  @component
  Lo que se abre debajo de un control de la barra superior: importar, buscar,
  estanterías y colecciones.

  Contrato: docs/design/ui-conventions.md, sección 4.1..

  Es una zona `complementary` y no un diálogo, por lo mismo que `Panel`: no es
  modal, así que no atrapa el foco. Lo que cambia respecto al panel del borde
  derecho es **dónde nace**: debajo del control que lo abrió, que es donde
  Material 3 pone un menú y donde el ojo lo busca después de pulsar. Antes se
  pulsaba arriba en el centro y la respuesta aparecía en el borde de la ventana.

  Se va con Escape y al pulsar fuera. Quien lo monta decide qué hacer con el foco,
  porque no es lo mismo cerrarlo con la tecla —el foco vuelve al control— que
  pulsando en otra parte, donde robarle el foco a lo que se acaba de pulsar sería
  deshacer el gesto.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    /** Nombre accesible de la zona. */
    titulo: string;
    children: Snippet;
    /** Por qué se cierra: con la tecla, o pulsando fuera. */
    alCerrar: (motivo: 'tecla' | 'fuera') => void;
    /** Lo que no cuenta como fuera: el control que lo abrió y lo que lo rodea. */
    ancla?: HTMLElement;
    /** Con qué borde del control se alinea. */
    alineacion?: 'inicio' | 'fin';
    /** Ancho que pide. Nunca más que la ventana. */
    ancho?: string;
    id?: string;
  }

  let {
    titulo,
    children,
    alCerrar,
    ancla,
    alineacion = 'fin',
    ancho = '26rem',
    id
  }: Props = $props();

  let raiz: HTMLElement | undefined = $state();

  /** Separación mínima con los bordes de la ventana, en píxeles. */
  const MARGEN = 16;

  /*
   * **No se sale de la ventana**, por estrecha que sea.
   *
   * Alineado con su control, un emergente ancho que nace de un botón cercano al
   * borde izquierdo se saldría por ese lado: fuera del alcance del ratón y del
   * tabulador. El ancho ya se acota en CSS; lo que CSS no sabe es dónde está el
   * control, así que el desvío se mide al abrir. Ocurre una vez y bajo la
   * animación de entrada, así que no se ve saltar.
   */
  $effect(() => {
    if (!raiz) return;
    /*
     * Se mide la caja **de disposición** y no la compuesta: al abrir, la
     * animación de entrada lo tiene a escala 0,97, y medido así el desvío salía
     * corto por ese 3 %. El borde que no se mueve al escalar es el del origen de
     * la transformación, así que se toma ese y se suma o resta el ancho real.
     */
    const caja = raiz.getBoundingClientRect();
    const ancho = raiz.offsetWidth;
    const izquierda = alineacion === 'inicio' ? caja.left : caja.right - ancho;
    const derecha = izquierda + ancho;
    const anchoDeLaVentana = globalThis.innerWidth ?? derecha;
    let desvio = 0;
    if (izquierda < MARGEN) desvio = MARGEN - izquierda;
    else if (derecha > anchoDeLaVentana - MARGEN) desvio = anchoDeLaVentana - MARGEN - derecha;
    if (desvio !== 0) raiz.style.setProperty('--nb-emergente-desvio', `${desvio}px`);
  });

  function alPulsarTecla(evento: KeyboardEvent) {
    if (evento.key !== 'Escape') return;
    evento.stopPropagation();
    alCerrar('tecla');
  }

  function alPulsarFuera(evento: MouseEvent) {
    const destino = evento.target;
    if (!(destino instanceof Node) || !raiz) return;
    // Un nodo que ya no está en el documento estaba dentro y desapareció al
    // repintar: es el mismo caso que describe `Panel`, y cerrar por él haría
    // que usar un control del emergente lo cerrara.
    if (!destino.isConnected) return;
    if (raiz.contains(destino) || ancla?.contains(destino)) return;
    alCerrar('fuera');
  }
</script>

<svelte:window onkeydown={alPulsarTecla} onclick={alPulsarFuera} />

<aside
  class="nb-emergente"
  bind:this={raiz}
  {id}
  aria-label={titulo}
  data-alineacion={alineacion}
  style:--nb-emergente-ancho={ancho}
>
  {@render children()}
</aside>

<style>
  .nb-emergente {
    position: absolute;
    top: calc(100% + var(--nb-space-2));
    z-index: 40;
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
    width: min(var(--nb-emergente-ancho), calc(100vw - 2 * var(--nb-space-4)));
    max-height: min(40rem, calc(100vh - 6rem));
    padding: var(--nb-space-4);
    overflow: auto;
    border-radius: var(--nb-radius-lg);
    /*
     * Opaco y sin desenfoque: se apoya sobre la biblioteca, que se desplaza, y
     * sin borde, porque lo separa la capa tonal y la sombra de su altura.
     */
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    box-shadow: var(--m3-elevation-3);
    translate: var(--nb-emergente-desvio, 0) 0;
    transform-origin: top right;
    animation: nb-emerger var(--nb-motion-fast) var(--nb-easing);
  }

  .nb-emergente[data-alineacion='fin'] {
    right: 0;
  }

  .nb-emergente[data-alineacion='inicio'] {
    left: 0;
    transform-origin: top left;
  }

  /*
   * Aparecer es respuesta a un gesto y dura lo que dura la transición rápida:
   * no es movimiento en reposo, y con movimiento reducido la red de seguridad
   * de `app.css` la deja en nada.
   */
  @keyframes nb-emerger {
    from {
      opacity: 0;
      transform: scale(0.97);
    }
  }
</style>
