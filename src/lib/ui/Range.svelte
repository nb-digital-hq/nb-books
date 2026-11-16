<!--
  @component
  Un deslizador con el aspecto de la aplicación.

  Contrato: docs/design/ui-conventions.md, sección 4.

  Es un `<input type="range">` de verdad: el teclado lo mueve con las flechas y
  el lector de pantalla lo anuncia con su valor. Lo único que cambia es cómo se
  pinta, porque el nativo lo dibuja el sistema y no obedece al tema.
-->
<script lang="ts">
  interface Props {
    etiqueta: string;
    valor: number;
    minimo: number;
    maximo: number;
    paso?: number;
    alCambiar: (valor: number) => void;
    id?: string;
    'aria-describedby'?: string;
  }

  let {
    etiqueta,
    valor,
    minimo,
    maximo,
    paso = 1,
    alCambiar,
    id,
    'aria-describedby': describedby
  }: Props = $props();

  const proporcion = $derived(
    maximo === minimo ? 0 : Math.min(1, Math.max(0, (valor - minimo) / (maximo - minimo)))
  );
</script>

<input
  {id}
  class="nb-range"
  type="range"
  aria-label={etiqueta}
  aria-describedby={describedby}
  min={minimo}
  max={maximo}
  step={paso}
  value={valor}
  style:--nb-range-avance="{proporcion * 100}%"
  oninput={(e) => alCambiar(e.currentTarget.valueAsNumber)}
/>

<style>
  /*
   * Alto mínimo de 1,5rem, que a escala 1 son veinticuatro píxeles: es el
   * objetivo de puntero mínimo de WCAG 2.2, y el control medía veinte. Crece con
   * la escala tipográfica por lo mismo que los botones: un objetivo fijo con la
   * letra aumentada deja de estar proporcionado a la mano de quien lo necesita.
   */
  .nb-range {
    /* Las dos medidas del control. Todo lo demás se deriva de ellas. */
    --nb-range-surco: 4px;
    --nb-range-pulgar: 14px;
    width: 100%;
    height: calc(1.5rem * var(--nb-font-scale));
    margin: 0;
    background: transparent;
    appearance: none;
    cursor: pointer;
  }

  /*
   * El relleno es un degradado de dos paradas y no un elemento aparte: así el
   * control sigue siendo un solo `input` y el teclado no tiene nada que rodear.
   */
  .nb-range::-webkit-slider-runnable-track {
    height: var(--nb-range-surco);
    border-radius: 2px;
    background: linear-gradient(
      to right,
      var(--m3c-primary) var(--nb-range-avance),
      var(--m3c-surface-container-highest) var(--nb-range-avance)
    );
  }

  .nb-range::-moz-range-track {
    height: var(--nb-range-surco);
    border-radius: 2px;
    background: linear-gradient(
      to right,
      var(--m3c-primary) var(--nb-range-avance),
      var(--m3c-surface-container-highest) var(--nb-range-avance)
    );
  }

  .nb-range::-webkit-slider-thumb {
    appearance: none;
    width: var(--nb-range-pulgar);
    height: var(--nb-range-pulgar);
    /*
     * Centrar el pulgar sobre el surco es aritmética de las dos medidas, no un
     * número elegido a ojo. Escrito a mano -eran -5px- cualquier cambio de
     * tamaño lo descentraba en silencio.
     */
    margin-top: calc((var(--nb-range-surco) - var(--nb-range-pulgar)) / 2);
    border: none;
    border-radius: 50%;
    background: var(--m3c-primary);
  }

  .nb-range::-moz-range-thumb {
    width: var(--nb-range-pulgar);
    height: var(--nb-range-pulgar);
    border: none;
    border-radius: 50%;
    background: var(--m3c-primary);
  }

  /*
   * Un halo alrededor del pulgar al pasar y al pulsar. Es lo que dice que el
   * control ha recibido el gesto sin mover el valor, y `box-shadow` lo consigue
   * sin cambiar el tamaño del objetivo ni desplazar la pista.
   */
  .nb-range:hover::-webkit-slider-thumb,
  .nb-range:active::-webkit-slider-thumb {
    box-shadow: 0 0 0 4px var(--nb-accent-soft);
  }

  .nb-range:hover::-moz-range-thumb,
  .nb-range:active::-moz-range-thumb {
    box-shadow: 0 0 0 4px var(--nb-accent-soft);
  }
</style>
