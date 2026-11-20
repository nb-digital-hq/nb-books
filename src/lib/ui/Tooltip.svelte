<!--
  @component
  Una etiqueta emergente sobre el elemento que la abre.

  Contrato: docs/design/ui-conventions.md y ADR-014.

  # Por qué el hijo recibe un identificador

  Porque una etiqueta que el lector de pantalla no anuncia no es una etiqueta:
  es un adorno para quien ya ve. `role="tooltip"` por sí solo no anuncia nada;
  lo que la anuncia es que **el elemento con el foco** apunte a ella con
  `aria-describedby`, y ese elemento es el hijo, no esta envoltura. Como el hijo
  lo pone quien usa el componente, se le pasa el identificador para que lo
  coloque.

  # Y por qué se cierra con Escape

  Lo pide WCAG 2.1, criterio 1.4.13: lo que aparece al pasar o al enfocar tiene
  que poder descartarse **sin mover el puntero ni el foco**. Una etiqueta que
  tapa lo que hay debajo y solo se va apartándose deja atrapado justo a quien
  usa ampliación de pantalla.

  # Y por qué se monta fuera del contenedor

  En una barra lateral con `overflow: hidden`, cualquier elemento
  con posición absoluta dentro de la barra queda recortado en su borde derecho.
  Al montarse en `document.body` con posición fija calculada desde el ancla, la
  etiqueta se dibuja libremente fuera del contenedor que la recortaba.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    text,
    position = 'bottom',
    children
  }: {
    text: string;
    position?: 'top' | 'bottom' | 'left' | 'right';
    children: Snippet<[{ describedBy: string }]>;
  } = $props();

  const id = `nb-tooltip-${Math.random().toString(36).slice(2, 9)}`;

  let envoltura: HTMLElement | undefined = $state();
  let coords = $state({ x: 0, y: 0 });
  let visible = $state(false);
  /*
   * Descartada con Escape se queda descartada hasta que el foco o el puntero
   * salen y vuelven. Sin esto, el siguiente `mouseenter` -que llega con mover un
   * píxel- la devolvería, y Escape no habría servido de nada.
   */
  let descartada = $state(false);

  function actualizarPosicion() {
    if (!envoltura || typeof window === 'undefined') return;
    const elemento = (envoltura.firstElementChild as HTMLElement) ?? envoltura;
    const rect = elemento.getBoundingClientRect();
    const separacion = 8;

    switch (position) {
      case 'right': {
        const lateral = envoltura.closest('.nb-lateral');
        const lateralRect = lateral ? lateral.getBoundingClientRect() : null;
        const xBase = lateralRect ? lateralRect.right : rect.right;
        coords = {
          x: xBase + separacion,
          y: rect.top + rect.height / 2
        };
        break;
      }
      case 'left':
        coords = {
          x: rect.left - separacion,
          y: rect.top + rect.height / 2
        };
        break;
      case 'top':
        coords = {
          x: rect.left + rect.width / 2,
          y: rect.top - separacion
        };
        break;
      case 'bottom':
      default:
        coords = {
          x: rect.left + rect.width / 2,
          y: rect.bottom + separacion
        };
        break;
    }
  }

  function mostrar() {
    if (!descartada) {
      actualizarPosicion();
      visible = true;
    }
  }

  function ocultar() {
    visible = false;
    descartada = false;
  }

  function alTeclado(evento: KeyboardEvent) {
    if (evento.key === 'Escape' && visible) {
      visible = false;
      descartada = true;
      // No se propaga: si un panel escucha Escape para cerrarse, descartar la
      // etiqueta cerraría además el panel, que no es lo que se ha pedido.
      evento.stopPropagation();
    }
  }

  function alDesplazar() {
    if (visible) {
      actualizarPosicion();
    }
  }

  function portal(node: HTMLElement) {
    if (typeof document !== 'undefined' && document.body) {
      document.body.appendChild(node);
    }
    return {
      destroy() {
        if (node.parentNode) {
          node.parentNode.removeChild(node);
        }
      }
    };
  }
</script>

<svelte:window onresize={alDesplazar} onscroll={alDesplazar} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="nb-tooltip-envoltura"
  bind:this={envoltura}
  onmouseenter={mostrar}
  onmouseleave={ocultar}
  onfocusin={mostrar}
  onfocusout={ocultar}
  onkeydown={alTeclado}
>
  {@render children({ describedBy: id })}
  {#if visible}
    <div
      class="nb-tooltip nb-tooltip--{position}"
      {id}
      role="tooltip"
      use:portal
      style:--nb-tooltip-x="{coords.x}px"
      style:--nb-tooltip-y="{coords.y}px"
    >
      {text}
    </div>
  {/if}
</div>

<style>
  .nb-tooltip-envoltura {
    position: relative;
    display: inline-flex;
    justify-content: center;
    margin: 0 auto;
  }

  .nb-tooltip {
    position: fixed;
    z-index: 1000;
    left: var(--nb-tooltip-x);
    top: var(--nb-tooltip-y);
    background-color: var(--m3c-inverse-surface);
    color: var(--m3c-inverse-on-surface);
    font-family: var(--nb-font-ui);
    font-size: var(--nb-text-xs);
    font-weight: 500;
    letter-spacing: 0.03333em;
    line-height: 1rem;
    padding: var(--nb-space-1) var(--nb-space-2);
    border-radius: var(--nb-radius-xs);
    white-space: nowrap;
    pointer-events: none;
    box-shadow: var(--m3-elevation-2);
    animation: nb-tooltip-aparecer var(--nb-motion-fast) var(--nb-easing);
  }

  /*
   * Quien pide menos movimiento la ve aparecer sin transición, y no la pierde:
   * lo que molesta es el movimiento, no la información.
   */
  @media (prefers-reduced-motion: reduce) {
    .nb-tooltip {
      animation: none;
    }
  }

  @keyframes nb-tooltip-aparecer {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .nb-tooltip--top {
    transform: translate(-50%, -100%);
  }

  .nb-tooltip--bottom {
    transform: translateX(-50%);
  }

  .nb-tooltip--left {
    transform: translate(-100%, -50%);
  }

  .nb-tooltip--right {
    transform: translateY(-50%);
  }
</style>
