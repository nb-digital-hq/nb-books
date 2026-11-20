<!--
  @component
  La marca de la aplicación: la silueta, sobre un disco del color de acento.

  # Por qué el disco, y por qué la silueta no es una imagen

  El dibujo es **una silueta plana sobre nada**: fuera del trazo no hay fondo,
  es transparente. Puesta tal cual sobre un fondo del mismo tono desaparece, y
  con nueve acentos entre `#00402f` y `#7fe3c0` no hay un solo tono que sirva
  para todos: el que se lee sobre el acento oscuro se pierde sobre el claro.

  Por eso el archivo **no se monta como imagen sino como máscara**, y el color
  lo pone `--nb-on-accent`, que es el mismo par que ya usan `Button` y `Switch`
  para escribir sobre el acento. La marca deja de tener un color propio: hereda
  el que el tema garantiza que contrasta con el disco de debajo. Comprobado
  componiendo la silueta sobre los nueve pares de acento y contraste que define
  `app.css`, a 40, 64 y 160 px.

  Se descartó pintarla siempre clara sobre el disco: se lee sobre los cuatro
  acentos oscuros y desaparece sobre los cinco claros.

  La silueta va al 62 % del disco. Por debajo se queda pequeña dentro de su
  propio aro; por encima de 70 las puntas alcanzan el borde y el disco deja de
  leerse como disco.

  # Por qué el archivo y no un SVG en línea

  La silueta tiene curvas largas que en trazado pesarían más que el PNG a los
  tamaños en que se usa, y cargarían el marcado en cada pintado. Como máscara
  solo cuenta su canal alfa, así que el archivo va en gris con alfa: dos
  canales donde había cuatro.
-->
<script lang="ts">
  import { asset } from '$app/paths';

  interface Props {
    /**
     * Diámetro del disco.
     *
     * Cualquier medida CSS, no solo píxeles: en la barra se pasa una que sale
     * del ancho de la propia barra, para que la marca crezca con ella en lugar
     * de quedarse en un número escrito a mano que se desajusta en cuanto la
     * barra cambia.
     */
    tamano?: string | number;
    /** Nombre accesible. Es lo único que anuncia la aplicación, así que no falta. */
    etiqueta?: string;
  }

  let { tamano = '6rem', etiqueta = 'NB Books' }: Props = $props();

  const medida = $derived(typeof tamano === 'number' ? `${tamano}px` : tamano);
</script>

<span
  class="nb-logo"
  style:--nb-logo-tamano={medida}
  style:--nb-logo-fuente={`url("${asset('logo.png')}")`}
  role="img"
  aria-label={etiqueta}
>
  <span class="nb-logo-silueta"></span>
</span>

<style>
  .nb-logo {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: var(--nb-logo-tamano);
    height: var(--nb-logo-tamano);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-primary);
  }

  /*
   * El color es del tema y la forma del archivo: la máscara recorta el fondo
   * con el alfa del PNG, así que lo que se ve pintado es `--nb-on-accent` y no
   * el tono con el que se dibujó la silueta.
   */
  .nb-logo-silueta {
    width: 62%;
    height: 62%;
    background: var(--m3c-on-primary);
    -webkit-mask: var(--nb-logo-fuente) center / contain no-repeat;
    mask: var(--nb-logo-fuente) center / contain no-repeat;
  }
</style>
