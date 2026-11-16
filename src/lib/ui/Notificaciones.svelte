<!--
  @component
  Los avisos, apilados en una esquina. Es la respuesta a "¿ha pasado algo?".

  Contrato: docs/design/ui-conventions.md, sección 9.

  # Por qué en una esquina y no en el sitio de la acción

  Porque las acciones que hay que confirmar están repartidas por toda la
  aplicación —la barra del lector, un panel, una ficha— y un sitio distinto por
  acción obliga a mirar a otra parte cada vez. Una esquina fija se aprende una
  vez. Es lo que hace `AdwToastOverlay` en Foliate y es la única parte de su
  interfaz que se copia aquí tal cual, porque el problema es idéntico.

  # Dos regiones y no una

  Una confirmación se anuncia cuando el lector de pantalla termine lo que está
  diciendo; un fallo interrumpe. Son dos niveles de urgencia distintos y una
  sola región solo puede tener uno, así que hay dos: la educada para lo que fue
  bien y la que interrumpe para lo que no.

  # Lo que no hace

  No roba el foco, no bloquea nada y no exige respuesta. Un aviso que hay que
  cerrar para seguir trabajando es un diálogo disfrazado.
-->
<script lang="ts">
  import { textos } from '$lib/i18n';

  import Icon from './Icon.svelte';
  import type { NombreDeIcono } from './iconos';
  import { notificaciones } from './notificaciones.svelte';
  import type { ClaseDeNotificacion } from './notificaciones';

  const t = $derived(textos().avisos);

  const ICONO: Readonly<Record<ClaseDeNotificacion, NombreDeIcono>> = {
    exito: 'correcto',
    error: 'error',
    informacion: 'informacion'
  };

  const lista = $derived(notificaciones.lista);
  const educadas = $derived(lista.filter((n) => n.clase !== 'error'));
  const urgentes = $derived(lista.filter((n) => n.clase === 'error'));
</script>

<!--
  Mientras el puntero está encima o el foco dentro, la cuenta atrás se detiene.
  Es lo que permitió que un fallo deje de ser eterno sin perder lo que lo hacía
  eterno: poder volver a leerlo y copiar su código.

  `onfocusin` y `onfocusout` y no `onfocus`: el foco cae en el botón de cerrar,
  que es un hijo, y `focus` no burbujea.
-->
<div class="nb-avisos" data-vacio={lista.length === 0}>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="nb-pila"
    role="status"
    aria-live="polite"
    aria-label={t.region}
    onmouseenter={() => notificaciones.pausar(true)}
    onmouseleave={() => notificaciones.pausar(false)}
    onfocusin={() => notificaciones.pausar(true)}
    onfocusout={() => notificaciones.pausar(false)}
  >
    {#each educadas as aviso (aviso.id)}
      <div class="nb-aviso" data-clase={aviso.clase}>
        <span class="nb-aviso-icono" aria-hidden="true"><Icon nombre={ICONO[aviso.clase]} /></span>
        <span class="nb-aviso-texto">
          <span class="nb-aviso-mensaje">
            {aviso.mensaje}
            {#if aviso.veces > 1}
              <!-- La cuenta va aquí y no en otra línea: es parte de la frase. -->
              <span class="nb-aviso-veces">{aviso.veces} {t.veces}</span>
            {/if}
          </span>
          {#if aviso.detalle}
            <span class="nb-aviso-detalle nb-selectable">{aviso.detalle}</span>
          {/if}
        </span>
        <button
          class="nb-aviso-cerrar"
          type="button"
          aria-label={t.cerrar}
          onclick={() => notificaciones.cerrar(aviso.id)}
        >
          <Icon nombre="cerrar" tamano={14} />
        </button>
      </div>
    {/each}
  </div>

  <!--
    Los fallos interrumpen: `log` con `aria-live` explícito, y no `role="alert"`
    en cada aviso. Un contenedor con nombre y sin papel es un `generic` con
    `aria-label`, que axe rechaza con razón: nadie puede nombrar lo que no es
    nada.
  -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="nb-pila"
    role="log"
    aria-live="assertive"
    aria-label={t.regionFallos}
    onmouseenter={() => notificaciones.pausar(true)}
    onmouseleave={() => notificaciones.pausar(false)}
    onfocusin={() => notificaciones.pausar(true)}
    onfocusout={() => notificaciones.pausar(false)}
  >
    {#each urgentes as aviso (aviso.id)}
      <div class="nb-aviso" data-clase="error">
        <span class="nb-aviso-icono" aria-hidden="true"><Icon nombre="error" /></span>
        <span class="nb-aviso-texto">
          <span class="nb-aviso-mensaje">
            {aviso.mensaje}
            {#if aviso.veces > 1}
              <span class="nb-aviso-veces">{aviso.veces} {t.veces}</span>
            {/if}
          </span>
          {#if aviso.detalle}
            <span class="nb-aviso-detalle nb-selectable">{aviso.detalle}</span>
          {/if}
        </span>
        <button
          class="nb-aviso-cerrar"
          type="button"
          aria-label={t.cerrar}
          onclick={() => notificaciones.cerrar(aviso.id)}
        >
          <Icon nombre="cerrar" tamano={14} />
        </button>
      </div>
    {/each}
  </div>
</div>

<style>
  /*
   * Fijo respecto a la ventana y no a la carcasa: en lectura inmersiva la
   * carcasa no existe, y un aviso que viviera dentro de ella desaparecería justo
   * cuando el usuario acaba de anotar algo.
   *
   * `pointer-events: none` en el contenedor y `auto` en cada aviso: sin eso, la
   * columna vacía de la esquina se comería los clics del contenido de debajo.
   */
  .nb-avisos {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 60;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--nb-space-2);
    max-width: min(26rem, calc(100vw - 32px));
    pointer-events: none;
  }

  .nb-pila {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    width: 100%;
  }

  .nb-aviso {
    display: flex;
    align-items: flex-start;
    gap: var(--nb-space-3);
    padding: var(--nb-space-3);
    border: 1px solid var(--m3c-outline);
    border-radius: var(--nb-radius-sm);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    box-shadow: var(--m3-elevation-1);
    pointer-events: auto;
    animation: nb-aviso-entra var(--nb-motion-base) var(--nb-easing);
  }

  /*
   * El color va en el borde de la izquierda y en el icono, nunca en el fondo: un
   * fondo de color obliga a un texto propio para cada clase, y en alto contraste
   * las cuatro combinaciones dejan de estar medidas. Así el contraste del texto
   * es el mismo que en el resto de la aplicación, siempre.
   */
  .nb-aviso[data-clase='exito'] {
    border-left: 3px solid var(--m3c-primary);
  }

  .nb-aviso[data-clase='error'] {
    border-left: 3px solid var(--m3c-error);
  }

  .nb-aviso[data-clase='informacion'] {
    border-left: 3px solid var(--m3c-outline);
  }

  .nb-aviso-icono {
    display: flex;
    flex: 0 0 auto;
    color: var(--m3c-on-surface-variant);
  }

  .nb-aviso[data-clase='exito'] .nb-aviso-icono {
    color: var(--m3c-primary);
  }

  .nb-aviso[data-clase='error'] .nb-aviso-icono {
    color: var(--m3c-error);
  }

  .nb-aviso-texto {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    min-width: 0;
    flex: 1 1 auto;
  }

  .nb-aviso-mensaje {
    font-size: var(--nb-text-sm);
    line-height: 1.4;
    overflow-wrap: anywhere;
  }

  .nb-aviso-veces {
    margin-left: var(--nb-space-1);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
    font-variant-numeric: tabular-nums;
  }

  .nb-aviso-detalle {
    color: var(--m3c-on-surface-variant);
    font-family: monospace;
    font-size: var(--nb-text-xs);
    overflow-wrap: anywhere;
  }

  .nb-aviso-cerrar {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    width: 1.5rem;
    height: 1.5rem;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--nb-radius-xs);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    cursor: pointer;
    transition: none;
  }

  .nb-aviso-cerrar:hover {
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface);
  }

  .nb-aviso-cerrar:active {
    background: var(--nb-accent-soft);
  }

  /*
   * Entra desde abajo y con opacidad. Con movimiento reducido la animación dura
   * lo que dura un token puesto a cero, así que aparece sin transición: el aviso
   * sigue estando, que es lo que importa.
   */
  @keyframes nb-aviso-entra {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
