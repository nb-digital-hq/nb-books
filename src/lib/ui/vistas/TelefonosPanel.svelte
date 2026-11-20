<!--
  @component
  Ajustes > Teléfonos: el servidor para NB Books Mobile (ADR-016).

  Encenderlo es la decisión de que la aplicación escuche en la red local, y por
  eso viene apagado y se dice siempre en qué dirección escucha. Emparejar lo
  enciende, porque emparejar sin escuchar no tiene sentido. Contrato:
  `docs/architecture/mobile-contract.md`.
-->
<script lang="ts">
  import { NbError, onDomainEvent } from '$core/ipc';
  import {
    mobileDevices,
    mobilePair,
    mobileReleaseControl,
    mobileRevoke,
    mobileSetEnabled,
    mobileState
  } from '$core/ipc/movil';
  import { idioma } from '$lib/i18n';
  import type { MobileDevice, MobilePairing, MobileServerState } from '$lib/bindings';
  import Button from '../Button.svelte';
  import Switch from '../Switch.svelte';

  interface Props {
    /** Los textos de Ajustes en el idioma activo. */
    texto: (clave: string) => string;
  }

  let { texto }: Props = $props();

  let servidor = $state<MobileServerState>({ activo: false, direccion: null });
  let telefonos = $state<MobileDevice[]>([]);
  let emparejar = $state<MobilePairing | null>(null);
  let aviso = $state<string | null>(null);
  let ocupado = $state(false);

  const fecha = $derived(
    new Intl.DateTimeFormat(idioma(), { dateStyle: 'medium', timeStyle: 'short' })
  );

  function explicar(error: unknown): string {
    const codigo = error instanceof NbError ? error.code : '';
    return codigo === 'COMPONENT_UNAVAILABLE' ? texto('movilSinPuerto') : String(codigo);
  }

  async function releer() {
    try {
      const antes = telefonos.length;
      [servidor, telefonos] = await Promise.all([mobileState(), mobileDevices()]);
      if (!servidor.activo) emparejar = null;
      // El código vale una vez: si con él se acaba de emparejar un teléfono, ya
      // no sirve y no se sigue enseñando como si sirviera.
      if (emparejar && telefonos.length > antes) emparejar = null;
    } catch (error) {
      aviso = explicar(error);
    }
  }

  // Lo que cambia sin que se pulse nada aquí -un teléfono que se empareja, otro
  // que toma el lector- lo anuncia el runtime.
  $effect(() => {
    void releer();
    const bajas = [
      onDomainEvent('MobileDevicesChanged', () => void releer()),
      onDomainEvent('RemoteControllerChanged', () => void releer())
    ];
    return () => {
      for (const baja of bajas) void baja.then((f) => f());
    };
  });

  // El código caduca: pasado su tiempo se retira en lugar de enseñar uno que ya
  // no vale.
  $effect(() => {
    if (!emparejar) return;
    const plazo = setTimeout(() => (emparejar = null), emparejar.segundos * 1000);
    return () => clearTimeout(plazo);
  });

  async function encender(activo: boolean) {
    ocupado = true;
    aviso = null;
    try {
      servidor = await mobileSetEnabled(activo);
      if (!activo) emparejar = null;
    } catch (error) {
      aviso = explicar(error);
    } finally {
      ocupado = false;
    }
  }

  async function pedirCodigo() {
    ocupado = true;
    aviso = null;
    try {
      emparejar = await mobilePair();
      servidor = await mobileState();
    } catch (error) {
      aviso = explicar(error);
    } finally {
      ocupado = false;
    }
  }

  async function revocar(id: string) {
    try {
      await mobileRevoke(id);
    } catch (error) {
      aviso = explicar(error);
    }
    await releer();
  }
</script>

<p class="nb-ayuda">{texto('movilAyuda')}</p>

<div class="nb-fila">
  <span class="nb-fila-texto">
    {texto('movilServidor')}
    <span class="nb-ayuda" role="status">
      {servidor.activo && servidor.direccion
        ? texto('movilEscuchando').replace('{direccion}', servidor.direccion)
        : texto('movilApagado')}
    </span>
  </span>
  <Switch
    etiqueta={texto('movilServidor')}
    activo={servidor.activo}
    deshabilitado={ocupado}
    alCambiar={(v) => void encender(v)}
  />
</div>
{#if aviso}
  <p class="nb-ayuda" role="alert">{aviso}</p>
{/if}

<div class="nb-acciones">
  <Button onclick={() => void pedirCodigo()} deshabilitado={ocupado}
    >{texto('movilEmparejar')}</Button
  >
</div>

{#if emparejar}
  <div class="nb-emparejar">
    <svg
      class="nb-qr"
      role="img"
      aria-label={texto('movilQr')}
      viewBox={`-2 -2 ${emparejar.qr.lado + 4} ${emparejar.qr.lado + 4}`}
      shape-rendering="crispEdges"
    >
      <rect
        x="-2"
        y="-2"
        width={emparejar.qr.lado + 4}
        height={emparejar.qr.lado + 4}
        fill="#fff"
      />
      <path d={emparejar.qr.trazado} fill="#000" />
    </svg>
    <div>
      <p class="nb-ayuda">{texto('movilEmparejarAyuda')}</p>
      <p>
        {texto('movilCodigo')}:
        <strong class="nb-codigo">{emparejar.codigo}</strong>
      </p>
      <p class="nb-ayuda">{emparejar.direccion}</p>
    </div>
  </div>
{/if}

<h3 class="nb-subtitulo">{texto('movilTelefonos')}</h3>
{#if telefonos.length === 0}
  <p class="nb-ayuda">{texto('movilSinTelefonos')}</p>
{:else}
  <ul class="nb-telefonos">
    {#each telefonos as telefono (telefono.id)}
      <li class="nb-fila">
        <span class="nb-fila-texto">
          {telefono.nombre}
          <span class="nb-ayuda">
            {telefono.vistoEn
              ? texto('movilVisto').replace('{fecha}', fecha.format(new Date(telefono.vistoEn)))
              : texto('movilNuncaVisto')}
            {#if telefono.controlando}
              &middot; {texto('movilControlando')}
            {/if}
          </span>
        </span>
        {#if telefono.controlando}
          <Button onclick={() => void mobileReleaseControl()}>{texto('movilCortar')}</Button>
        {/if}
        <Button onclick={() => void revocar(telefono.id)}>{texto('revocar')}</Button>
      </li>
    {/each}
  </ul>
  <p class="nb-ayuda">{texto('movilRevocarAyuda')}</p>
{/if}

<style>
  /* Las mismas filas y ayudas que el resto de Ajustes, que no llegan aquí por
     estar acotadas a su vista. */
  .nb-fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-4);
    min-height: 3rem;
    padding: var(--nb-space-3) var(--nb-space-4);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    font-size: var(--nb-text-sm);
  }

  .nb-fila-texto {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--nb-space-1);
    font-weight: 500;
  }

  .nb-ayuda {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    line-height: 1.5;
  }

  .nb-subtitulo {
    margin: var(--nb-space-3) 0 var(--nb-space-2);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    font-weight: 600;
  }

  .nb-acciones {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
    margin-top: var(--nb-space-2);
  }

  .nb-emparejar {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-4);
    align-items: center;
    margin-block: var(--nb-space-3);
  }

  /* El QR va siempre en negro sobre blanco, con el tema que sea: es lo que leen
     las cámaras sin fallar. */
  .nb-qr {
    inline-size: 12rem;
    block-size: 12rem;
    flex: none;
    border-radius: var(--nb-radius-sm);
  }

  .nb-codigo {
    font-family: monospace;
    font-size: var(--nb-text-md);
    letter-spacing: 0.15em;
  }

  .nb-telefonos {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    margin: 0 0 var(--nb-space-2);
    padding: 0;
    list-style: none;
  }
</style>
