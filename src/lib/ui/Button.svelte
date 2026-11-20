<!--
  @component
  Botón. Siempre un `<button>` real: nunca un `div` con un manejador de clic.

  Un `div` con `onclick` no recibe el foco, no responde a Espacio ni a Intro y
  el lector de pantalla no lo anuncia como accionable. `scripts/check-ui.mjs`
  y las reglas de accesibilidad del compilador de Svelte lo rechazan.

  Tiene la forma de Material 3: una píldora. El primario va relleno
  con el acento, el secundario es tonal —el contenedor de la secundaria, que en
  Grafito es gris— y el de peligro, relleno con el color de error.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    children: Snippet;
    variante?: 'primario' | 'secundario' | 'peligro';
    tipo?: 'button' | 'submit' | 'reset';
    deshabilitado?: boolean;
    onclick?: (evento: MouseEvent) => void;
  }

  let {
    children,
    variante = 'secundario',
    tipo = 'button',
    deshabilitado = false,
    onclick
  }: Props = $props();
</script>

<button class="nb-boton" data-variante={variante} type={tipo} disabled={deshabilitado} {onclick}>
  {@render children()}
</button>

<style>
  .nb-boton {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--nb-space-2);

    /*
     * 2,5rem con la escala a 1, que es la altura de un botón de Material. Crece
     * con `--nb-font-scale`, porque un objetivo táctil fijo con la tipografía
     * aumentada deja el texto fuera del botón.
     */
    min-height: calc(2.5rem * var(--nb-font-scale));
    /*
     * Parte el texto antes que salirse. Con `nowrap`, dos botones en el ancho de
     * un panel de dieciocho unidades se salían por la izquierda y el que quedaba
     * fuera no se podía pulsar: lo tapaba el texto del libro.
     */
    max-width: 100%;
    padding: var(--nb-space-1) var(--nb-space-5);

    border: none;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font: inherit;
    font-size: var(--nb-text-sm);
    font-weight: 600;
    line-height: 1.25;
    text-align: center;
    cursor: pointer;
    transition:
      background var(--nb-motion-fast) var(--nb-easing),
      box-shadow var(--nb-motion-fast) var(--nb-easing);
  }

  /*
   * Cada realce va acotado con su variante, y no suelto, por especificidad:
   * `.nb-boton:hover:not(:disabled)` son tres selectores y
   * `.nb-boton[data-variante='primario']` son dos, así que un realce suelto le
   * ganaba al fondo de la variante. El resultado fue un botón primario que al
   * pasar el ratón se volvía casi negro conservando su texto oscuro: invisible, y
   * en el sitio donde más se pulsa.
   *
   * El realce es la capa de estado de Material: el color del texto mezclado sobre
   * el fondo, no un tono nuevo que el alto contraste no cubriría.
   */
  .nb-boton[data-variante='secundario']:hover:not(:disabled),
  .nb-boton[data-variante='secundario']:focus-visible:not(:disabled) {
    background: color-mix(
      in srgb,
      var(--m3c-on-secondary-container) 8%,
      var(--m3c-secondary-container)
    );
  }

  .nb-boton[data-variante='secundario']:active:not(:disabled) {
    background: color-mix(
      in srgb,
      var(--m3c-on-secondary-container) 12%,
      var(--m3c-secondary-container)
    );
  }

  .nb-boton:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .nb-boton[data-variante='primario'] {
    background: var(--m3c-primary);
    color: var(--m3c-on-primary);
  }

  .nb-boton[data-variante='primario']:hover:not(:disabled),
  .nb-boton[data-variante='primario']:focus-visible:not(:disabled) {
    background: color-mix(in srgb, var(--m3c-on-primary) 8%, var(--m3c-primary));
    box-shadow: var(--m3-elevation-1);
  }

  .nb-boton[data-variante='primario']:active:not(:disabled) {
    background: color-mix(in srgb, var(--m3c-on-primary) 12%, var(--m3c-primary));
    box-shadow: none;
  }

  .nb-boton[data-variante='peligro'] {
    background: var(--m3c-error);
    color: var(--m3c-on-error);
  }

  .nb-boton[data-variante='peligro']:hover:not(:disabled),
  .nb-boton[data-variante='peligro']:focus-visible:not(:disabled) {
    background: color-mix(in srgb, var(--m3c-on-error) 8%, var(--m3c-error));
  }
</style>
