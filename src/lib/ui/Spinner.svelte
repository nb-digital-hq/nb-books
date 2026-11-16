<!--
  @component
  El único indicador de carga de la aplicación.

  # Por qué espera antes de aparecer

  El innegociable dice que **nada se anima
  con la aplicación en reposo**, y un indicador de carga se mueve. No hay
  contradicción formal -cargando no se está en reposo- pero faltaba decir qué se
  permite mientras se espera, y sobre todo **cuándo**.

  Desde que los comandos salieron del hilo de la ventana y las consultas
  van acotadas, casi todo lo que se pide contesta antes de que
  nadie lo note: abrir el panel de estanterías sobre 55.740 autores cuesta 6 ms.
  Poner un indicador ahí no informa de nada: **parpadea**, que es peor que el
  silencio, porque un destello llama la atención y no dice nada.

  Así que espera. Por debajo del umbral no se pinta nada; por encima aparece,
  porque entonces el silencio sí parece que se ha roto algo.

  # De dónde sale el umbral

  De lo medido en este proyecto, no de una costumbre. Un cambio de página de la
  biblioteca cuesta entre 90 y 233 ms, y el panel de estanterías 6: un indicador
  con umbral por debajo de eso destellaría en las dos operaciones más frecuentes
  de la aplicación. Y el presupuesto de "time to interactive" es de 800 ms, así
  que a partir de ahí ya hay que decir algo. 300 ms cae entre las dos cosas: por
  debajo, el destello cuesta más que la espera; por encima, el silencio empieza a
  parecer un fallo.

  # Lo que hace que la regla no dependa de acordarse

  Que la espera vive **aquí dentro** y no en cada vista. Una vista que quiera
  decir que está cargando pone este componente y ya está esperando; no hay forma
  de poner un indicador sin umbral sin escribir otro componente, y eso lo rechaza
  `check-ui.mjs`, que no admite ninguna animación en bucle fuera de este archivo.
-->
<script lang="ts">
  interface Props {
    etiqueta?: string;
    tamano?: string;
    /**
     * Cuánto se espera antes de aparecer, en milisegundos.
     *
     * Se puede bajar donde se sepa que la espera va a ser larga -abrir un PDF
     * pesado- pero no subir sin decir por qué: el umbral existe para no
     * destellar, no para esconder.
     */
    umbral?: number;
    /**
     * En una fila, con la etiqueta al lado y sin márgenes: para decirlo junto a
     * lo que espera -las pestañas de la búsqueda- sin ocupar el sitio de un
     * estado de carga de vista entera.
     */
    enLinea?: boolean;
  }
  let { etiqueta, tamano = '2rem', umbral = 300, enLinea = false }: Props = $props();

  /*
   * Se monta invisible y se enseña al pasar el umbral. No al revés -montarlo
   * tarde- porque el componente ya está donde tiene que estar y solo cambia si
   * se ve: así no hay un salto de disposición cuando aparece.
   */
  let visible = $state(false);

  $effect(() => {
    const temporizador = setTimeout(() => {
      visible = true;
    }, umbral);
    return () => clearTimeout(temporizador);
  });
</script>

<div
  class="nb-spinner-contenedor"
  aria-label={etiqueta}
  role="status"
  data-visible={visible}
  data-en-linea={enLinea}
>
  {#if visible}
    <div class="nb-spinner" style:--nb-spinner-tamano={tamano}></div>
    {#if etiqueta}
      <span class="nb-spinner-etiqueta">{etiqueta}</span>
    {/if}
  {/if}
</div>

<style>
  .nb-spinner-contenedor {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--nb-space-3);
    padding: var(--nb-space-6);
    color: var(--m3c-on-surface-variant);
  }

  .nb-spinner-contenedor[data-en-linea='true'] {
    flex-direction: row;
    gap: var(--nb-space-2);
    padding: 0 var(--nb-space-2);
  }

  .nb-spinner-contenedor[data-en-linea='true'] .nb-spinner {
    border-width: 2px;
  }

  .nb-spinner {
    width: var(--nb-spinner-tamano);
    height: var(--nb-spinner-tamano);
    border: 3px solid color-mix(in srgb, var(--m3c-primary) 20%, transparent);
    border-top-color: var(--m3c-primary);
    border-radius: var(--nb-radius-full);
    animation: nb-giro 0.8s linear infinite;
  }

  .nb-spinner-etiqueta {
    font-size: var(--nb-text-sm);
    font-weight: 500;
  }

  /*
   * Quien pide menos movimiento no quiere que se lo quiten todo: quiere que no
   * se mueva. El indicador sigue estando y sigue diciendo lo mismo, quieto.
   */
  @media (prefers-reduced-motion: reduce) {
    .nb-spinner {
      animation: none;
      border-top-color: var(--m3c-primary);
      border-right-color: var(--m3c-primary);
    }
  }

  @keyframes nb-giro {
    0% {
      transform: rotate(0deg);
    }
    100% {
      transform: rotate(360deg);
    }
  }
</style>
