<!--
  @component
  La portada de una obra. Si no hay imagen, se genera.

  Contrato: docs/design/ui-conventions.md.

  **Un hueco gris con un icono dentro no es una portada, es una ausencia.** Con
  152.080 documentos, la mayoría no traerá imagen, y una biblioteca entera de
  rectángulos vacíos es exactamente la pantalla que no queremos. Aquí, cuando no
  hay portada, se dibuja una: color derivado del título, el título compuesto con
  la tipografía de lectura y el lomo a la izquierda.

  El color se deriva **del título** y no al azar: la misma obra tiene siempre la
  misma portada, en esta y en cualquier otra pantalla, y eso es lo que permite
  reconocerla de un vistazo en una rejilla.
-->
<script lang="ts">
  import { porcentajeLeido } from '$core/progreso';
  interface Props {
    titulo: string;
    autor?: string | null;
    /** Ruta de la portada real, si la edición tiene una. */
    imagen?: string | null;
    /** Ancho en píxeles. El alto sale de la proporción de un libro. */
    ancho?: number;
    /**
     * La portada ocupa el ancho de su hueco en lugar de uno fijo.
     *
     * Existe porque una rejilla de columnas elásticas con portadas de ancho
     * fijo deja un margen que crece con la ventana: medido a 1440, la columna
     * medía 193 píxeles y la portada 172, así que cada obra llevaba veintiún
     * píxeles de aire a la derecha que nadie había pedido y la rejilla parecía
     * mal alineada. `ancho` sigue haciendo falta donde el hueco no manda —la
     * ficha, la barra lateral—, y por eso se queda.
     */
    fluida?: boolean;
    /** Progresión de 0 a 1, o nulo si no se ha empezado. */
    progreso?: number | null;
  }

  let {
    titulo,
    autor = null,
    imagen = null,
    ancho = 150,
    fluida = false,
    progreso = null
  }: Props = $props();

  /**
   * Tono derivado del título.
   *
   * Derivado y no al azar: la misma obra tiene siempre la misma portada, en esta
   * y en cualquier otra pantalla, y eso es lo que permite reconocerla de un
   * vistazo. Con un azar, cada repintado cambiaría el color.
   *
   * **Y el reparto importa tanto como el determinismo.** La primera versión hacía
   * `(suma * 31 + código) % 360` en cada letra: tomar el resto en cada paso tira
   * la entropía que acaba de entrar, así que títulos parecidos -y en una
   * biblioteca real hay muchos, del mismo autor o de la misma saga- caían en el
   * mismo color. Medido sobre las tres obras del banco, dos salían del mismo
   * verde. Ahora el título pasa por FNV-1a, que reparte, y el resultado se
   * multiplica por el **ángulo dorado**: dos valores consecutivos del hash caen
   * en lados opuestos del círculo de color en lugar de al lado.
   */
  const tono = $derived.by(() => {
    let hash = 0x811c9dc5;
    for (let i = 0; i < titulo.length; i += 1) {
      hash ^= titulo.charCodeAt(i);
      hash = Math.imul(hash, 0x01000193) >>> 0;
    }
    return Math.round(((hash % 997) * 137.508) % 360);
  });

  // El texto de una portada generada no cabe por debajo de cierto tamaño, y
  // apretarlo lo convierte en una mancha. Una portada elástica siempre lo lleva:
  // su hueco lo fija una rejilla que ya tiene un mínimo por columna.
  const compacta = $derived(!fluida && ancho < 60);
</script>

<div
  class="nb-portada"
  style:width={fluida ? null : `${ancho}px`}
  style:--ancho={ancho}
  style:--nb-portada-tono={tono}
  data-fluida={fluida}
  data-generada={imagen === null}
>
  {#if imagen}
    <img src={imagen} alt="" decoding="async" loading="lazy" />
  {:else}
    <span class="nb-lomo" aria-hidden="true"></span>
    {#if !compacta}
      <span class="nb-portada-texto">
        <span class="nb-portada-titulo">{titulo}</span>
        {#if autor}
          <span class="nb-portada-autor">{autor}</span>
        {/if}
      </span>
    {/if}
  {/if}

  {#if progreso !== null && progreso > 0}
    <span class="nb-portada-progreso" aria-hidden="true">
      <span style:width="{porcentajeLeido(progreso)}%"></span>
    </span>
  {/if}
</div>

<style>
  .nb-portada {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: var(--nb-radius-sm);
    overflow: hidden;
    background: var(--m3c-surface-container);
    flex-shrink: 0;
    box-shadow: var(--m3-elevation-2);
  }

  /*
   * Del título sale el **tono** y nada más: la luminosidad y el croma los pone
   * el tema en `app.css`, así que en un tema claro las portadas son claras. Sin
   * esa separación, una rejilla llena desentonaría con el resto de la interfaz
   * en cuanto alguien cambiara de tema.
   */
  /*
   * Elástica: el ancho lo pone el hueco y la proporción hace el resto. El
   * tamaño del texto pasa a depender del propio ancho, porque un cuerpo fijo
   * dentro de una portada que crece se ve cada vez más pequeño.
   */
  .nb-portada[data-fluida='true'] {
    width: 100%;
  }

  .nb-portada[data-fluida='true'] .nb-portada-titulo {
    font-size: clamp(0.9rem, 9cqw, 1.35rem);
  }

  .nb-portada[data-fluida='true'] .nb-portada-autor {
    font-size: clamp(0.6rem, 5.2cqw, 0.85rem);
  }

  .nb-portada {
    container-type: inline-size;
  }

  .nb-portada img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  /* El canto: es lo que hace que un rectángulo parezca un libro. */
  .nb-lomo {
    position: absolute;
    inset: 0 auto 0 0;
    width: 6%;
    min-width: 4px;
    background: var(--nb-portada-lomo, var(--m3c-outline-variant));
  }

  /*
   * El bloque de texto va **centrado en vertical** ahora. Anclado
   * arriba, con el autor abajo, dejaba un hueco muerto en el medio que es lo que
   * hacía que una portada generada pareciera un rectángulo con una etiqueta en la
   * esquina en lugar de una portada.
   */
  .nb-portada-texto {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.5em;
    padding: 10% 9% 10% 14%;
    color: var(--nb-portada-texto, var(--m3c-on-surface));
  }

  .nb-portada-titulo {
    font-family: var(--nb-font-lectura);
    /*
     * La letra se ajusta al ancho de la portada: a tamaño fijo, una
     * miniatura de 72 px partía "TALLER4." en "TAL / LER / 4." y cortaba el
     * resto. La portada declara su ancho como contenedor para poder medirlo.
     */
    font-size: clamp(0.55rem, 13cqw, var(--nb-text-lg));
    font-weight: 600;
    line-height: 1.25;
    /*
     * Un título sin espacios es **una sola palabra** para el navegador, así que
     * ni se parte ni se recorta: se sale de su caja. Y no es un caso raro —
     * medido sobre una biblioteca real, cuatro de cada cinco archivos se llaman
     * `Algo_Asi_Con_Guiones`, y ese nombre es el título hasta que se leen los
     * metadatos—. `anywhere` permite partir dentro de la palabra cuando no hay
     * otro sitio por donde hacerlo.
     *
     * No se desplaza girando sobre sí mismo: eso sería movimiento que no
     * transporta significado, y además solo se activaría con el ratón encima,
     * que es un gesto que esta interfaz no admite como única vía. El título
     * entero está en la ficha, a un foco de distancia.
     */
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  /*
   * El autor **debajo del título y con una línea entre los dos**, no pegado al
   * borde inferior en letra diminuta: a `--nb-text-xs` con opacidad 0,72 sobre un
   * fondo de color no se leía, y un dato que no se lee no está.
   */
  .nb-portada-autor {
    padding-top: 0.5em;
    border-top: 1px solid currentcolor;
    font-size: var(--nb-text-sm);
    letter-spacing: 0.03em;
    opacity: 0.85;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .nb-portada-progreso {
    position: absolute;
    inset: auto 0 0 0;
    height: 3px;
    background: color-mix(in srgb, var(--m3c-shadow) 10%, transparent);
  }

  .nb-portada-progreso span {
    display: block;
    height: 100%;
    background: var(--m3c-primary);
  }
</style>
