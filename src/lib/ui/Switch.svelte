<!--
  @component
  Interruptor de dos estados.

  Es un `<button>` con `role="switch"`, no una casilla de verificación
  disfrazada: un interruptor aplica su cambio al momento y una casilla suele
  esperar a que se confirme un formulario. Anunciar como casilla algo que actúa
  al instante hace que la persona que lo oye espere un botón de guardar que no
  existe.

  El estado visible no es solo el color: la posición del pulsador cambia. Un
  interruptor cuyo único indicio es el color no dice nada a quien no lo
  distingue, y es el criterio 1.4.1 de WCAG.
-->
<script lang="ts">
  interface Props {
    /** Nombre accesible. Obligatorio. Se traduce. */
    etiqueta: string;
    activo: boolean;
    deshabilitado?: boolean;
    alCambiar?: (activo: boolean) => void;
  }

  let { etiqueta, activo, deshabilitado = false, alCambiar }: Props = $props();
</script>

<button
  class="nb-interruptor"
  type="button"
  role="switch"
  aria-checked={activo}
  aria-label={etiqueta}
  disabled={deshabilitado}
  onclick={() => alCambiar?.(!activo)}
>
  <span class="nb-interruptor-pista" aria-hidden="true">
    <span class="nb-interruptor-pulsador"></span>
  </span>
</button>

<style>
  .nb-interruptor {
    display: inline-flex;
    align-items: center;
    padding: var(--nb-space-1);
    border: none;
    background: transparent;
    cursor: pointer;
  }

  .nb-interruptor:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  /*
   * El interruptor no tenía **ningún** estado de puntero: ni realce al pasar ni
   * acuse al pulsar. En una pantalla de ajustes llena de ellos, eso significa
   * que la única forma de saber si has dado a uno es mirar si el pulsador se ha
   * movido, y con movimiento reducido ni siquiera se mueve.
   */
  .nb-interruptor:hover:not(:disabled) .nb-interruptor-pista {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, var(--m3c-surface-container-highest));
  }

  .nb-interruptor-pista {
    display: flex;
    align-items: center;
    box-sizing: border-box;
    width: calc(3.25rem * var(--nb-font-scale));
    height: calc(2rem * var(--nb-font-scale));
    padding: 0 calc(0.375rem * var(--nb-font-scale));
    border: 2px solid var(--m3c-outline);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-highest);
    transition: none;
  }

  .nb-interruptor-pulsador {
    width: calc(1rem * var(--nb-font-scale));
    height: calc(1rem * var(--nb-font-scale));
    border-radius: var(--nb-radius-full);
    background: var(--m3c-outline);
    transition: none;
  }

  /*
   * Encendido, el pulsador crece y pasa al otro lado. Se coloca con la alineación
   * de la pista y no con un desplazamiento calculado: con la escala tipográfica
   * subida, un desplazamiento fijo lo sacaba de la pista.
   */
  .nb-interruptor[aria-checked='true'] .nb-interruptor-pista {
    justify-content: flex-end;
    padding: 0 calc(0.25rem * var(--nb-font-scale));
    border-color: var(--m3c-primary);
    background: var(--m3c-primary);
  }

  .nb-interruptor[aria-checked='true'] .nb-interruptor-pulsador {
    width: calc(1.5rem * var(--nb-font-scale));
    height: calc(1.5rem * var(--nb-font-scale));
    background: var(--m3c-on-primary);
  }
</style>
