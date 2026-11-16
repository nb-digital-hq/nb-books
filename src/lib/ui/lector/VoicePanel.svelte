<!--
  @component
  Lectura en voz alta: los controles y, cuando no la hay, el motivo.

  Contrato: `docs/architecture/voice-services.md`.

  **Este componente no habla.** Recibe el estado de la narración y devuelve lo
  que el usuario pide; quien habla es el proveedor, detrás de su contrato, y
  quien lo conduce es el lector. Es lo que permite que el motor neuronal entre
  sin tocar nada de aquí.

  La mitad más importante es la de abajo: **cuando no hay voz, se dice por qué**.
  Un botón que no hace nada al pulsarlo es peor que no ofrecer ninguno, y en
  accesibilidad esa diferencia se paga entera.
-->
<script lang="ts">
  import { idioma, textos } from '$lib/i18n';
  import type { DeclaracionDeVoz, VozDisponible } from '$lib/voz/contrato';
  import { catalogoLegible } from '$lib/voz/eleccion';
  import { type EstadoDeNarracion, VELOCIDADES } from '$lib/voz/narracion';

  import Icon from '../Icon.svelte';
  import Select from '../Select.svelte';

  interface Props {
    /** Lo que el proveedor dice de sí mismo. */
    declaracion: DeclaracionDeVoz;
    /**
     * Nombre del servicio de síntesis, cuando la voz viene de fuera.
     *
     * Se enseña **siempre** que la narración salga del dispositivo. No es un
     * dato técnico de más: es la mitad del consentimiento. Autorizar una vez en
     * Ajustes y no volver a ver a dónde va el texto del libro sería un permiso
     * que se concede una vez y se olvida, y el que se olvida deja de ser
     * informado.
     */
    servicio?: string | null;
    voces: VozDisponible[];
    narracion: EstadoDeNarracion;
    alAlternar: () => void;
    alParar: () => void;
    alElegirVoz: (id: string) => void;
    alCambiarVelocidad: (valor: number) => void;
    /** Textos. Anulación, no fuente: el diccionario manda. */
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    declaracion,
    servicio = null,
    voces,
    narracion,
    alAlternar,
    alParar,
    alElegirVoz,
    alCambiarVelocidad,
    etiquetas
  }: Props = $props();

  const t = $derived({ ...textos().voz, ...etiquetas } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  const hablando = $derived(narracion.estado === 'hablando');

  /** Si lo que narra sale del dispositivo. Lo declara el proveedor, no se deduce. */
  const desdeFuera = $derived(declaracion.necesitaRed);

  /*
   * La voz remota **sí pausa**. El proveedor local corta y vuelve a empezar el
   * tramo, porque la síntesis del sistema no expone pausa; lo remoto es un
   * archivo de audio, así que pausar es pausar y reanudar sigue por donde iba.
   * El botón lo dice, porque no es lo mismo y quien lo pulsa lo nota.
   */
  const pausaDeVerdad = $derived(desdeFuera);

  /*
   * El catálogo, con el idioma en la lengua de quien mira y ordenado por él.
   *
   * Son ciento treinta entradas: `speech-dispatcher` ofrece una voz por idioma
   * más miles de variantes con nombre de fantasía, y el runtime ya se queda solo
   * con las primeras. Enseñadas como venían —`Afrikaans (af)`, `Bishnupriya
   * (bpy)`— la lista no se podía recorrer buscando la propia, porque el nombre
   * de la voz es el del idioma **en inglés** y el orden era el suyo.
   */
  const opcionesDeVoz = $derived(
    catalogoLegible(voces, idioma()).map((v) => [v.id, v.etiqueta] as const)
  );

  const opcionesDeVelocidad = $derived(
    VELOCIDADES.map((valor) => [String(valor), `${valor}x`] as const)
  );

  /**
   * La voz que hay que enseñar elegida.
   *
   * Sale del narrador, que ya la ha deducido del idioma del documento. Aquí no
   * se vuelve a decidir: hacerlo dejaba el desplegable enseñando una voz —la
   * primera del catálogo— distinta de la que sonaba.
   */
  const vozActual = $derived(narracion.voz ?? '');
</script>

<div class="nb-voz">
  {#if !declaracion.disponible}
    <!--
      Ni un control. Se explica qué falta y, cuando tiene arreglo, cuál es: "no
      hay voces instaladas" se resuelve instalando una, y "esta plataforma no
      trae síntesis" no, y son dos mensajes distintos porque son dos situaciones
      distintas para quien lo lee.
    -->
    <p class="nb-sin-voz" role="status">
      <Icon nombre="informacion" />
      {texto(declaracion.motivo ?? 'sinSintesis')}
    </p>
  {:else}
    {#if declaracion.advertencia === 'lenta'}
      <!--
        Se ofrece y **se dice lo que va a pasar**: recomendar no es obligar, lo
        se decidió el 2026-09-22. El factor es el medido en esta máquina, no
        uno supuesto, y lo recomendado aquí es preparar el capítulo antes.
      -->
      <p class="nb-sin-voz" role="status">
        <Icon nombre="advertencia" />
        {texto('lentaEnVivo')}
        {#if declaracion.factor !== undefined}
          ({texto('factorMedido')}
          {declaracion.factor.toLocaleString(idioma(), {
            maximumFractionDigits: 1
          })}).
        {/if}
        {texto('lentaRecomendado')}
      </p>
    {/if}
    {#if desdeFuera}
      <!--
        De dónde sale la voz, cada vez que se abre el panel.

        Es la mitad del consentimiento: autorizarlo una vez en Ajustes y no
        volver a verlo sería un permiso que se concede y se olvida, y el que se
        olvida deja de ser informado. Va arriba del todo, antes de los controles,
        porque es lo que hay que saber antes de pulsar.
      -->
      <p class="nb-procedencia" role="status">
        <Icon nombre="red" tamano={15} />
        <span
          >{texto('vieneDe')}
          {servicio ?? texto('servicioSinNombre')}. {texto('loPermitiste')}</span
        >
      </p>
    {/if}

    <div class="nb-controles" role="group" aria-label={texto('controles')}>
      <button type="button" class="nb-boton nb-principal" onclick={alAlternar}>
        <Icon nombre={hablando ? 'pausa' : 'reproducir'} />
        {#if hablando}
          {texto(pausaDeVerdad ? 'pausar' : 'pausarReiniciando')}
        {:else}
          {texto('leer')}
        {/if}
      </button>
      <button
        type="button"
        class="nb-boton"
        disabled={narracion.estado === 'parada'}
        onclick={alParar}
      >
        {texto('parar')}
      </button>
    </div>

    <label class="nb-campo">
      <span class="nb-etiqueta">{texto('velocidad')}</span>
      <Select
        etiqueta={texto('velocidad')}
        valor={String(narracion.velocidad)}
        opciones={opcionesDeVelocidad}
        alCambiar={(valor) => alCambiarVelocidad(Number(valor))}
      />
    </label>

    {#if !desdeFuera}
      <label class="nb-campo">
        <span class="nb-etiqueta">{texto('voz')}</span>
        <Select
          etiqueta={texto('voz')}
          valor={vozActual}
          opciones={opcionesDeVoz}
          alCambiar={alElegirVoz}
        />
      </label>
    {/if}

    <!--
      La voz remota no lista sus voces aquí. Cada servicio nombra las suyas a su
      manera y no hay forma estándar de preguntárselas, así que se escribe en
      Ajustes tal y como el servicio la llame; un desplegable con nombres
      inventados sería peor que ninguno.
    -->
    <p class="nb-nota">{texto(desdeFuera ? 'remota' : 'local')}</p>
  {/if}
</div>

<style>
  .nb-voz {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
  }

  .nb-controles {
    display: flex;
    gap: var(--nb-space-2);
  }

  .nb-boton {
    display: inline-flex;
    align-items: center;
    gap: var(--nb-space-1);
    min-height: 32px;
    padding: 0 var(--nb-space-3);
    border: none;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font: inherit;
    font-size: var(--nb-text-sm);
    cursor: pointer;
  }

  .nb-boton:hover:not(:disabled) {
    background: color-mix(
      in srgb,
      var(--m3c-on-secondary-container) 8%,
      var(--m3c-secondary-container)
    );
  }

  .nb-boton:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .nb-principal {
    background: var(--m3c-primary);
    color: var(--m3c-on-primary);
  }

  .nb-campo {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
  }

  .nb-etiqueta {
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }

  /*
   * La procedencia se marca con el color de acento y no con el de aviso: no es
   * un problema ni una advertencia, es información sobre algo que el usuario
   * autorizó a propósito.
   */
  .nb-procedencia {
    display: flex;
    align-items: flex-start;
    gap: var(--nb-space-2);
    margin: 0;
    padding: var(--nb-space-2);
    border: none;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-tertiary-container);
    color: var(--m3c-on-tertiary-container);
    font-size: var(--nb-text-xs);
  }

  .nb-sin-voz {
    display: flex;
    align-items: flex-start;
    gap: var(--nb-space-2);
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
  }

  .nb-nota {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }
</style>
