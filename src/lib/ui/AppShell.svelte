<!--
  @component
  La carcasa: barra lateral, barra superior, área principal y paneles.

  Contrato: docs/architecture/ui-components.md, secciones 1 y 2, y
  docs/design/ui-conventions.md.

  Cada zona es un punto de referencia estándar de accesibilidad. No es una
  elección de estilo: es el mecanismo por el que el lector de pantalla y el
  teclado funcionan sin emular nada, que es lo que compró ADR-012 al descartar
  el escritorio virtual.

  **La barra lateral es de alto completo y se pliega a solo iconos.** De alto
  completo porque es lo que la hace el marco de la aplicación y no un cajón
  dentro de una pantalla; plegable porque en una ventana estrecha o en una
  lectura larga, cada píxel de ancho es del contenido. Plegada sigue siendo la
  misma barra con los mismos elementos en el mismo orden: solo desaparece el
  texto, nunca un destino.

  **Ahora la jerarquía la dan las capas, no los bordes.** La barra
  lateral y la superior están sobre la superficie de la ventana, y el contenido
  va en un lienzo de la capa de encima, con las esquinas redondeadas. Es la
  gramática de Material 3: lo que separa dos zonas es que una está más arriba que
  la otra, y un trazo entre ellas sobraba.

  La clase de ancho la calcula JavaScript sobre el tamaño del propio elemento y
  no una consulta de medios, por dos motivos: la carcasa puede no ocupar toda la
  ventana, y así los puntos de corte tienen una sola fuente (`anchos.ts`) en
  lugar de estar repetidos en la hoja de estilos.
-->
<script lang="ts">
  import { textos } from '$lib/i18n';
  import { type Snippet, tick, untrack } from 'svelte';

  import { clasificarAncho, laNavegacionVaAbajo, type ClaseDeAncho } from './anchos';
  import { claseInicial, proveerCarcasa } from './carcasa.svelte';

  interface Props {
    /** La barra superior de la vista activa. */
    banner?: Snippet;
    /** La marca, arriba del todo de la barra lateral. */
    marca?: Snippet;
    /** Los destinos. Van dentro de la barra lateral. */
    navegacion?: Snippet;
    /**
     * Las tarjetas vivas de la barra lateral: lo que se está leyendo, cómo va el
     * índice, lo que necesita atención.
     *
     * Son **dinámicas**: aparecen cuando dicen algo y se van cuando dejan de
     * decirlo. Una tarjeta fija que casi siempre está vacía es un hueco con
     * marco. Plegada la barra no caben, y en la barra inferior tampoco.
     */
    tarjetas?: Snippet;
    /** Las acciones del pie (ajustes, etc.). Van al fondo de la barra. */
    pie?: Snippet;
    /** La vista activa. Es el único contenido obligatorio. */
    principal: Snippet;
    /** Uno o dos `Panel`. Cuántos se acoplan lo decide la clase de ancho. */
    paneles?: Snippet;
    /** Etiqueta accesible de la navegación primaria. Se traduce. */
    etiquetaNavegacion?: string;
    /** Texto del enlace de salto. Se traduce. */
    etiquetaSalto?: string;
    /**
     * Lectura inmersiva: la carcasa se retira **entera** y solo queda el
     * contenido.
     *
     * Es el compromiso 2 de ADR-012 y un modo de primera clase, no una opción
     * de menú: quien lo activa lo hace con una tecla. Las zonas dependen de
     * este único valor a propósito; con una condición por zona, olvidar una
     * deja la barra superior encima del texto y no se nota hasta verlo.
     */
    inmersivo?: boolean;
    /** Qué se anuncia al entrar en inmersión. Se traduce. */
    etiquetaInmersion?: string;
    /** Barra lateral plegada a solo iconos: lo que se ha pedido. */
    plegada?: boolean;
    /**
     * Si la barra **se está pintando** plegada, que durante el cambio no es lo
     * mismo que lo pedido: el contenido de la barra cambia de forma mientras es
     * invisible, a mitad de la transición. Quien pinta los destinos tiene que
     * seguir esto y no `plegada`, o cambiarían de forma a la vista.
     */
    plegadaVisible?: boolean;
  }

  let {
    banner,
    marca,
    navegacion,
    tarjetas,
    pie,
    principal,
    paneles,
    etiquetaNavegacion,
    etiquetaSalto,
    inmersivo = false,
    etiquetaInmersion,
    plegada = $bindable(false),
    plegadaVisible = $bindable(false)
  }: Props = $props();

  // Los textos de la carcasa salen del diccionario, y las props quedan como
  // anulación para quien monte la carcasa en una prueba con otros nombres.
  const t = $derived(textos().carcasa);
  const nombreNavegacion = $derived(etiquetaNavegacion ?? t.navegacion);
  const nombreSalto = $derived(etiquetaSalto ?? t.salto);
  const avisoInmersion = $derived(etiquetaInmersion ?? t.inmersion);

  let raiz: HTMLDivElement | undefined = $state();
  let clase = $state<ClaseDeAncho>(claseInicial());
  let columna: HTMLDivElement | undefined = $state();
  let claseDelContenido = $state<ClaseDeAncho>(claseInicial());

  // El contexto se provee una sola vez con un objeto que lee `clase` al
  // consultarse: si se pasara el valor, los paneles no se enterarían de que la
  // ventana cambió de tamaño.
  proveerCarcasa({
    get clase() {
      return clase;
    },
    get claseDelContenido() {
      return claseDelContenido;
    }
  });

  // En la clase compacta la barra lateral está siempre plegada: desplegada no
  // cabría junto al contenido, y superponerla la convertiría en un cajón, que
  // es otra cosa.
  // Y en la clase media también: a 800 px la barra con nombres se comía un tercio
  // de la ventana y se desbordaba, con Ajustes cortado y una barra de
  // desplazamiento propia. Los nombres siguen en la etiqueta emergente.
  const soloIconos = $derived(plegadaVisible || clase !== 'amplio');

  /*
   * # Plegar la barra sin que tiemble lo de al lado
   *
   * Observación de uso del 2026-09-12: "tosco, brusco, se ven rebotes, se siente un bajón de
   * fotogramas". Tres causas, las tres leídas en el código y la segunda medida:
   *
   * 1. **Los rebotes eran literales.** El ancho se animaba con `--nb-easing`,
   *    que era `cubic-bezier(0.34, 1.56, 0.64, 1)`: una curva que se pasa del
   *    final y vuelve. La barra crecía más de lo debido y retrocedía.
   * 2. **Cada fotograma de la animación recomponía la biblioteca entera.** Al
   *    cambiar el ancho de la barra cambia el de la columna, y con él cuántas
   *    tarjetas caben por fila.
   * 3. **Todo cambiaba de forma a la vista y a destiempo**: el texto de los
   *    destinos desaparecía de golpe, la marca pasaba de 7,5 a 2,25 rem sin
   *    transición, y las tarjetas se iban antes de que la barra encogiera.
   *
   * # Lo que se hace ahora: no se anima ningún ancho
   *
   * La barra tiene el mismo color que la ventana, así que **no se ve dónde
   * acaba**: lo que el ojo sigue es el borde del lienzo de al lado. Por eso basta
   * con mover el lienzo, y moverlo es una transformación, que el compositor hace
   * sin recomponer nada.
   *
   * 1. El contenido de la barra se funde a transparente.
   * 2. Con la barra invisible, cambia de forma **de una vez**: su ancho, sus
   *    destinos y la vista de al lado se recomponen una sola vez. En el mismo
   *    fotograma, la columna se desplaza exactamente lo que se ha movido su
   *    borde, así que a la vista sigue donde estaba.
   * 3. La columna vuelve a su sitio con la curva estándar de Material 3, que no
   *    se pasa del final, y el contenido de la barra reaparece.
   *
   * Se descartó animar el ancho congelando la vista de al lado —una caja de
   * medidas fijas mientras dura—: medido en Chromium con la CPU a un cuarto y
   * cien obras, bajaba la disposición de 455 a 250 ms en seis cambios pero
   * seguía recomponiendo la columna en cada fotograma, y los fotogramas de más
   * de 20 ms pasaban de 50 a 90. Ver `docs/design/ui-conventions.md`, sección 7.
   *
   * Con movimiento reducido no hay tiempos: el cambio es inmediato, que es lo
   * que se pidió.
   */
  type Fase = 'quieta' | 'saliendo' | 'entrando';
  let fase = $state<Fase>('quieta');
  let columnaDesplazada = $state<number | null>(null);
  let lateral: HTMLElement | undefined = $state();
  let interior: HTMLElement | undefined = $state();
  let turno = 0;

  /** Cuánto dura la transición de un elemento, leída de su estilo: el tema la escala. */
  function duracion(elemento: HTMLElement | undefined): number {
    if (!elemento) return 0;
    const valor = getComputedStyle(elemento).transitionDuration.split(',')[0]?.trim() ?? '0s';
    const numero = Number.parseFloat(valor);
    if (!Number.isFinite(numero)) return 0;
    return valor.endsWith('ms') ? numero : numero * 1000;
  }

  function sinMovimiento(): boolean {
    return (
      globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true ||
      document.documentElement.dataset.motion === 'reduced'
    );
  }

  const esperar = (ms: number) => new Promise((resolver) => setTimeout(resolver, ms));
  const fotograma = () => new Promise((resolver) => requestAnimationFrame(resolver));

  async function cambiarDeForma(destino: boolean, mio: number) {
    if (!columna || sinMovimiento() || clase === 'compacto') {
      plegadaVisible = destino;
      return;
    }

    fase = 'saliendo';
    await esperar(duracion(interior));
    if (mio !== turno) return;

    const antes = columna.getBoundingClientRect().left;
    plegadaVisible = destino;
    await tick();
    const despues = columna.getBoundingClientRect().left;
    // Donde estaba, sin transición: a la vista no se ha movido nada todavía.
    columnaDesplazada = antes - despues;
    await fotograma();
    await fotograma();
    if (mio !== turno) return;

    // Y a su sitio, con transición.
    fase = 'entrando';
    columnaDesplazada = 0;
    await tick();
    await esperar(duracion(columna));
    if (mio !== turno) return;
    columnaDesplazada = null;
    fase = 'quieta';
  }

  // Solo lo pedido dispara el cambio. Lo que se pinta y la fase cambian **dentro**
  // del cambio, y si también lo dispararan, cada tiempo reiniciaría el anterior.
  $effect(() => {
    const pedida = plegada;
    untrack(() => {
      if (pedida === plegadaVisible && fase === 'quieta') return;
      turno += 1;
      void cambiarDeForma(pedida, turno);
    });
  });

  $effect(() => {
    if (!raiz || typeof ResizeObserver === 'undefined') return;

    // El cambio va al siguiente fotograma: escribirlo dentro del aviso cambia el
    // tamaño de lo observado en la misma pasada, y WebKit lo publica como error
    // ("ResizeObserver loop completed…") en cada cierre de documento.
    const observador = new ResizeObserver(([entrada]) => {
      if (entrada) {
        const ancho = entrada.contentRect.width;
        requestAnimationFrame(() => (clase = clasificarAncho(ancho)));
      }
    });
    observador.observe(raiz);

    return () => observador.disconnect();
  });

  /*
   * La columna se mide aparte porque el panel no compite con la barra lateral:
   * compite con el contenido. Un solo número para las dos decisiones acoplaba
   * un panel de 288 px junto a 208 px de contenido en una ventana de 720.
   */
  $effect(() => {
    if (!columna || typeof ResizeObserver === 'undefined') return;

    const observador = new ResizeObserver(([entrada]) => {
      if (entrada) {
        const ancho = entrada.contentRect.width;
        requestAnimationFrame(() => (claseDelContenido = clasificarAncho(ancho)));
      }
    });
    observador.observe(columna);

    return () => observador.disconnect();
  });
</script>

<div
  class="nb-carcasa"
  bind:this={raiz}
  data-ancho={clase}
  data-inmersivo={inmersivo}
  data-plegada={soloIconos}
>
  <a class="nb-skip-link" href="#nb-principal">{nombreSalto}</a>

  <!--
    Que la carcasa se haya ido es información: sin anunciarlo, quien usa un
    lector de pantalla solo percibe que la mitad de la aplicación dejó de
    existir. El anuncio dice además por dónde se vuelve.
  -->
  <p class="nb-anuncio" role="status">{inmersivo ? avisoInmersion : ''}</p>

  {#if navegacion && !inmersivo}
    <nav
      class="nb-lateral"
      bind:this={lateral}
      aria-label={nombreNavegacion}
      data-abajo={laNavegacionVaAbajo(clase)}
      data-fase={fase}
    >
      <!--
        El interior es lo que se desplaza, y la barra lo que pinta el fondo.

        Van separados porque no pueden ser el mismo elemento: una caja
        transformada cuenta para el área desplazable de quien la contiene, y con
        las dos cosas en la barra, la barra enseñaba una barra de desplazamiento
        **permanente** que no llevaba a ninguna parte. Separados, la barra recorta
        y el interior solo se desplaza cuando los destinos de verdad no caben.
      -->
      <div class="nb-lateral-interior" bind:this={interior}>
        {#if marca}
          <div class="nb-marca" data-sola={soloIconos}>{@render marca()}</div>
        {/if}

        <div class="nb-lateral-destinos">{@render navegacion()}</div>

        <div class="nb-lateral-hueco"></div>

        {#if tarjetas && !soloIconos}
          <div class="nb-lateral-tarjetas">{@render tarjetas()}</div>
        {/if}

        {#if pie}
          <div class="nb-lateral-pie">{@render pie()}</div>
        {/if}
      </div>
    </nav>
  {/if}

  <div
    class="nb-columna"
    bind:this={columna}
    data-fase={fase}
    style:transform={columnaDesplazada === null ? undefined : `translateX(${columnaDesplazada}px)`}
  >
    {#if banner && !inmersivo}
      <header class="nb-banner">{@render banner()}</header>
    {/if}

    <div class="nb-cuerpo">
      <!--
        `tabindex="-1"` es lo que hace que el enlace de salto **mueva el foco de
        verdad**. Con un destino no enfocable, varios navegadores desplazan la
        vista y dejan el foco donde estaba, así que el siguiente tabulador vuelve
        al principio de la barra y el enlace no sirve para nada. Es una violación
        seria de WCAG 2.1.1.

        Negativo y no cero: esta zona desplaza, y una región que desplaza tiene
        que ser alcanzable por teclado, pero se alcanza **por su contenido**. La
        superficie de lectura deja una parada de tabulación en el bloque del
        cursor, y con eso la zona ya tiene acceso por teclado sin añadir una
        parada más que no lleva a ninguna parte.
      -->
      <main class="nb-principal" id="nb-principal" tabindex="-1">
        {@render principal()}
      </main>

      {#if paneles && !inmersivo}
        <div class="nb-paneles">{@render paneles()}</div>
      {/if}
    </div>
  </div>
</div>

<style>
  /*
   * Dos columnas: la barra lateral, de alto completo, y todo lo demás. Que sea
   * de alto completo es lo que la hace el marco de la aplicación en lugar de un
   * cajón dentro de una pantalla.
   */
  .nb-carcasa {
    display: grid;
    grid-template-columns: auto 1fr;
    height: 100%;
    min-height: 0;
    /*
     * Mientras la columna vuelve a su sitio al plegar la barra, su borde derecho
     * pasa por fuera de la ventana. Se recorta aquí para que eso no aparezca como
     * una barra de desplazamiento horizontal de un instante.
     */
    overflow: hidden;
    background: var(--m3c-surface);
  }

  /*
   * Fuera de la vista pero disponible para el lector de pantalla. No se usa
   * `display: none`, que lo dejaría también fuera del árbol de accesibilidad y
   * entonces el anuncio no se anunciaría.
   */
  .nb-anuncio {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  /*
   * **El fondo vivo se retiró, y esta es su lápida.**
   *
   * Era un degradado que se movía despacio detrás de la barra lateral y del
   * banner. Medido el 2026-09-02 con un desplazamiento continuo sobre la
   * biblioteca: con el fondo animado, 13 fotogramas de los 77 pasaban de 20 ms;
   * sin él, 3 de 87. Competía por el mismo fotograma que el desplazamiento, y
   * `performance-budgets.md` ya decía qué hacer: una animación que no cabe se
   * elimina, no se recorta. Queda la capa de textura del tema, que es estática.
   */

  .nb-lateral {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    width: var(--nb-lateral-ancho);
    padding: var(--nb-space-4) var(--nb-space-3);

    /*
     * **Opaco, y sobre la misma superficie que la ventana.** La barra no es una
     * capa: es el marco. Lo que está encima es el lienzo del contenido.
     *
     * Opaco por lo: con el fondo en transparente la barra se
     * componía contra lo que hubiera detrás, y el contraste de sus destinos
     * dejaba de ser el que la paleta garantiza —el destino en reposo llegó a
     * 1,1:1—.
     */
    background-color: var(--m3c-surface);
    /* Recorta y no se desplaza: lo que se desplaza es `.nb-lateral-interior`. */
    overflow: hidden;
    /*
     * Sin transición de ancho: lo que se anima es el desplazamiento de la
     * columna de al lado. Ver `cambiarDeForma`.
     */
    /*
     * Sin `will-change`. Anunciar un cambio permanente fija una capa de
     * composición para siempre, y aquí no había nada que anunciar: el ancho
     * cambia al plegarse, que es una vez.
     */
    transform: translateZ(0);
  }

  /*
   * La textura del tema, si la tiene. Va aquí y no en el fondo de la página
   * porque es donde se nota sin estorbar a la lectura, y detrás de todo para
   * que ningún texto se apoye en ella.
   */
  .nb-lateral::before {
    content: '';
    position: absolute;
    inset: 0;
    background: var(--nb-textura, none);
    opacity: var(--nb-textura-opacidad, 0);
    pointer-events: none;
  }

  /* La marca preside la barra y respira antes de dar paso a los destinos. */
  .nb-marca {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--nb-space-2);
    min-height: var(--nb-banner-alto);
    padding: 0 var(--nb-space-2);
    margin-top: var(--nb-space-2);
    margin-bottom: var(--nb-space-5);
  }

  .nb-lateral-interior {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    flex: 1 1 auto;
    min-height: 0;
    min-width: 0;
    /*
     * `auto` y no `scroll`: la barra aparece cuando los destinos no caben, que
     * en una ventana normal es nunca. Son siete.
     */
    overflow-y: auto;
    overflow-x: hidden;
  }

  /*
   * El contenido de la barra se funde mientras la barra cambia de ancho, y cambia
   * de forma cuando no se ve. Ver el comentario de `cambiarDeForma`.
   */
  .nb-lateral-interior {
    transition: opacity var(--nb-motion-fast) var(--nb-easing-estandar);
  }

  .nb-lateral[data-fase='saliendo'] .nb-lateral-interior {
    opacity: 0;
  }

  .nb-columna[data-fase='entrando'] {
    transition: transform var(--nb-motion-lateral) var(--nb-easing-estandar);
  }

  .nb-lateral-hueco {
    flex: 1 1 auto;
    min-height: 16px;
  }

  /*
   * **Plegada, todo va en un solo eje.** Antes medía 3,5 rem con
   * doce píxeles de margen a cada lado, así que por dentro quedaban 32 px para
   * destinos de 48: se salían cuatro por la derecha, el activo quedaba recortado
   * contra el lienzo y ningún icono estaba en el eje de la marca. Medido sobre la
   * compilación. Ahora la barra es algo más ancha, el margen más estrecho y los
   * destinos algo más pequeños (`NavItem`), y caben con aire a los dos lados.
   */
  .nb-carcasa[data-plegada='true'] .nb-lateral {
    width: var(--nb-lateral-ancho-plegada);
    padding: 0 var(--nb-space-2) var(--nb-space-4);
  }

  /*
   * La marca, a la altura de la barra superior. Plegada queda al lado del botón
   * que despliega la barra, y a distinta altura los dos parecían descolocados.
   */
  .nb-carcasa[data-plegada='true'] .nb-marca {
    min-height: var(--nb-banner-alto);
    margin: 0;
    padding: 0;
  }

  /*
   * Y los destinos, centrados en vertical entre la marca y el pie, que es donde
   * Material 3 pone el grupo de un riel. Desplegada no: debajo van las tarjetas, y
   * el grupo tiene que quedarse arriba.
   */
  .nb-carcasa[data-plegada='true'] .nb-lateral-hueco {
    display: none;
  }

  .nb-carcasa[data-plegada='true'] .nb-lateral-destinos {
    gap: var(--nb-space-2);
    margin-block: auto;
  }

  .nb-lateral-destinos,
  .nb-lateral-pie {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .nb-lateral-tarjetas {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    min-width: 0;
    margin-bottom: var(--nb-space-3);
  }

  .nb-columna {
    /*
     * Solo mientras vuelve a su sitio. Fuera de la fase de entrada no hay
     * transición, y así colocarla donde estaba no se ve moverse.
     */
    transition: none;
    display: grid;
    /*
     * La columna se declara explícita y con `minmax(0, 1fr)`. Sin ella, la
     * retícula crea una columna implícita de tamaño `auto`, que es
     * `min-content..max-content`: el banner la estiraba hasta caber entero y
     * empujaba la ventana a desplazarse en horizontal. Aquí no se desplaza
     * nada: lo que no cabe se recorta dentro de su propio bloque.
     */
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto 1fr;
    min-width: 0;
    min-height: 0;
  }

  .nb-banner {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--nb-space-2);
    /* La misma altura que toma la marca plegada, que se alinea con esta barra. */
    min-height: var(--nb-banner-alto);
    /*
     * **Esto es lo que hace que escribir en el buscador no cueste cien
     * milisegundos por letra.**
     *
     * Medido en el motor real —WebKitGTK, con la biblioteca de verdad dentro— el
     * 2026-09-18: cada letra tardaba 133 ms en Biblioteca y 109 en Inicio, y de
     * ellos entre 95 y 120 eran **disposición**. El mismo campo suelto costaba
     * 17 ms y cero de disposición; puesto dentro de esta barra, volvía a costar
     * 110. No era el buscador ni el marco —sin avisar a Svelte costaba lo
     * mismo—: era que un cambio de texto aquí dentro ensucia la disposición de
     * todo lo que hay debajo, y debajo está la biblioteca entera.
     *
     * De todas las contenciones probadas **solo esta lo corta**: `layout` sola,
     * `content` e `inline-size` seguían costando 110 ms, y contener el cuerpo en
     * lugar de la barra, también. Con `size` la altura de la barra deja de
     * depender de su contenido, que es cierto por construcción: la fija
     * `min-height` y dentro no hay nada más alto —el título no envuelve y el
     * campo mide menos—. Medido: la geometría no cambia, 844x72 antes y después.
     */
    contain: layout size;
    padding: 0 var(--nb-space-4) 0 var(--nb-space-1);
    margin: 0;
    /*
     * **Opaco.** Ni desenfoque ni translucidez: debajo pasa la biblioteca
     * entera, y mezclar un color con lo que se mueve obliga a rehacer la mezcla
     * en cada fotograma. Medido con el fondo animado ya retirado, el peor
     * fotograma de un desplazamiento continuo bajaba de 25,1 a 22,7 ms al
     * hacerlo opaco.
     *
     * Y **sin recortar**: los menús de la barra nacen debajo de su botón y
     * tienen que poder salirse de ella. La capa de textura no lo necesita, porque
     * ya ocupa exactamente la caja.
     */
    background-color: var(--m3c-surface);
    z-index: 10;
  }

  .nb-banner::before {
    content: '';
    position: absolute;
    inset: 0;
    background: var(--nb-textura, none);
    opacity: var(--nb-textura-opacidad, 0);
    pointer-events: none;
    z-index: -1;
  }

  /*
   * El lienzo: la capa de encima, con sus esquinas. Recorta lo que lleva dentro
   * —la biblioteca y los paneles—, y por eso los menús de la barra, que no son
   * suyos, viven fuera de él.
   */
  .nb-cuerpo {
    position: relative;
    display: flex;
    min-height: 0;
    margin: 0 var(--nb-space-4) var(--nb-space-4) 0;
    border-radius: var(--nb-radius-xl);
    background: var(--m3c-surface-container-low);
    overflow: hidden;
  }

  .nb-principal {
    flex: 1 1 auto;
    min-width: 0;
    overflow: auto;
  }

  .nb-paneles {
    display: flex;
    min-height: 0;
  }

  /*
   * Compacto: la barra lateral pasa al borde inferior, en fila. Va abajo y no
   * arriba porque en una ventana estrecha el borde inferior es el que queda al
   * alcance sin recolocar la mano.
   */
  .nb-carcasa[data-ancho='compacto'] {
    grid-template-columns: 1fr;
    grid-template-rows: 1fr auto;
  }

  .nb-carcasa[data-ancho='compacto'] .nb-lateral {
    grid-row: 2;
    width: auto;
    padding: var(--nb-space-2);
  }

  /*
   * En una barra inferior los destinos van en fila, y no hay nada que
   * desplazar: caben o no caben, y si no caben lo que sobra es un destino, no
   * recorrido.
   */
  .nb-carcasa[data-ancho='compacto'] .nb-lateral-interior {
    flex-direction: row;
    align-items: center;
    overflow: hidden;
  }

  .nb-carcasa[data-ancho='compacto'] .nb-lateral-destinos,
  .nb-carcasa[data-ancho='compacto'] .nb-lateral-pie {
    flex-direction: row;
    justify-content: space-around;
    flex: 1 1 auto;
  }

  /* En una barra inferior no cabe ni la marca ni el hueco. */
  .nb-carcasa[data-ancho='compacto'] .nb-marca,
  .nb-carcasa[data-ancho='compacto'] .nb-lateral-hueco {
    display: none;
  }

  .nb-carcasa[data-ancho='compacto'] .nb-columna {
    grid-row: 1;
  }

  .nb-carcasa[data-ancho='compacto'] .nb-cuerpo {
    margin: 0 var(--nb-space-2);
  }

  .nb-carcasa[data-ancho='compacto'] :global(.nb-destino) {
    justify-content: center;
    padding: 0 var(--nb-space-2);
  }

  /*
   * **En inmersión la retícula pasa a tener una sola columna.**
   *
   * Sin esto, la carcasa seguía declarando `auto 1fr` mientras la barra lateral
   * ya no existía: la columna del contenido caía en la celda `auto`, que se
   * dimensiona por el contenido, y el lector quedaba de 550 píxeles pegado a la
   * izquierda en una ventana de 1920. Medido: `main` a 550x222 en inmersión
   * contra 1696x1024 fuera de ella.
   *
   * Las filas, por lo mismo: sin banner ni paneles, `auto 1fr` dejaba la fila
   * del contenido con la altura de su texto.
   */
  .nb-carcasa[data-inmersivo='true'] {
    grid-template-columns: minmax(0, 1fr);
  }

  .nb-carcasa[data-inmersivo='true'] .nb-columna {
    grid-template-rows: minmax(0, 1fr);
  }

  /*
   * En inmersión no queda marco del que separarse, así que el lienzo ocupa la
   * ventana entera, sin márgenes ni esquinas. "El lector ocupa la ventana
   * entera" es exactamente el criterio que se comprueba, y un margen lo haría
   * falso por dieciséis píxeles.
   */
  .nb-carcasa[data-inmersivo='true'] .nb-cuerpo {
    margin: 0;
    border-radius: 0;
    background: transparent;
  }

  .nb-carcasa[data-ancho='compacto'] .nb-paneles {
    display: contents;
  }
</style>
