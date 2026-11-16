<!--
  @component
  Botón sin texto visible.

  `etiqueta` es obligatoria y no tiene valor por defecto a propósito: un botón
  de solo icono sin nombre accesible es invisible para un lector de pantalla, y
  es el defecto de accesibilidad más frecuente en interfaces de escritorio. Al
  ser obligatoria en el tipo, olvidarla no compila.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    /** Nombre accesible. Obligatorio. Se traduce. */
    etiqueta: string;
    /** El icono. Se marca como decorativo: el nombre lo pone `etiqueta`. */
    children: Snippet;
    deshabilitado?: boolean;
    /** Para controles que expresan un estado abierto o cerrado. */
    expandido?: boolean;
    /**
     * Para controles que expresan una elección vigente.
     *
     * No es lo mismo que `expandido`, y la diferencia importa: "cuadrícula" no
     * despliega nada, **es** el modo en curso. Antes los dos se
     * expresaban con `aria-expanded` y ninguno de los dos tenía estilo, así que
     * el modo elegido y el que no se veían exactamente igual. Un control
     * seleccionable sin estado seleccionado es un control que no se puede usar
     * sin probarlo.
     */
    activo?: boolean;
    /**
     * Para controles cuya función **está ocurriendo ahora**.
     *
     * Distinto de `activo`, y la diferencia se ve: un panel abierto y una voz
     * narrando no son lo mismo. Se pidió el 2026-09-18 al mirar la barra
     * del lector: sin esto, leer en voz alta y tener el panel de voz abierto se
     * veían idénticos, y parar lo que suena exigía abrir el panel para
     * comprobarlo. Lleva su propio color, no un matiz del mismo.
     */
    funcionando?: boolean;
    onclick?: (evento: MouseEvent) => void;
  }

  let {
    etiqueta,
    children,
    deshabilitado = false,
    expandido,
    activo,
    funcionando = false,
    onclick
  }: Props = $props();
</script>

<button
  class="nb-boton-icono"
  type="button"
  aria-label={etiqueta}
  aria-expanded={expandido}
  aria-pressed={activo}
  data-funcionando={funcionando ? 'true' : undefined}
  title={etiqueta}
  disabled={deshabilitado}
  {onclick}
>
  <span class="nb-icono" aria-hidden="true">{@render children()}</span>
</button>

<style>
  .nb-boton-icono {
    display: inline-flex;
    align-items: center;
    justify-content: center;

    width: calc(2.5rem * var(--nb-font-scale));
    height: calc(2.5rem * var(--nb-font-scale));
    padding: 0;

    border: none;
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    cursor: pointer;
    transition: none;
  }

  .nb-boton-icono:hover:not(:disabled),
  .nb-boton-icono:focus-visible:not(:disabled) {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
    color: var(--m3c-on-surface);
  }

  /*
   * Pulsado. Es el acuse de recibo del clic, y no existía en
   * ningún control de la aplicación: sobre un puente lento —el escenario 2 de
   * `chaos.md`— no había forma de saber si la pulsación había llegado.
   */
  .nb-boton-icono:active:not(:disabled) {
    background: color-mix(in srgb, var(--m3c-on-surface) 12%, transparent);
    color: var(--m3c-on-surface);
  }

  /*
   * Abierto y elegido se ven igual porque significan lo mismo para quien mira:
   * este es el que está. Lo que cambia es lo que anuncia el lector de pantalla,
   * y eso lo decide el atributo, no el color.
   */
  .nb-boton-icono[aria-expanded='true'],
  .nb-boton-icono[aria-pressed='true'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }

  /*
   * Funcionando: el color de acento, que en el sistema de diseño es el que
   * significa "esto es lo principal que está pasando". Gana al de abierto porque
   * describe algo que ocurre ahora mismo, no un estado de la pantalla.
   */
  .nb-boton-icono[data-funcionando='true'],
  .nb-boton-icono[data-funcionando='true'][aria-expanded='true'],
  .nb-boton-icono[data-funcionando='true'][aria-pressed='true'] {
    background: var(--m3c-primary-container);
    color: var(--m3c-on-primary-container);
  }

  .nb-boton-icono:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .nb-icono {
    display: flex;
    align-items: center;
    justify-content: center;
    line-height: 1;
  }
</style>
