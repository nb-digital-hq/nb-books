<!--
  @component
  Ajustes > Acerca de > Actualizaciones. Nunca se busca sola: pulsar el botón es
  el consentimiento para consultar las publicaciones del proyecto en GitHub, y
  con el modo sin red puesto no sale nada. Lo que se instala lleva la firma de
  la publicación, y el runtime la comprueba con la clave compilada antes de
  tocar el disco.
-->
<script lang="ts">
  import { NbError } from '$core/ipc';
  import {
    actualizacionBuscar,
    actualizacionDelLauncher,
    actualizacionInstalar
  } from '$core/ipc/ficha';
  import type { Actualizacion } from '$lib/bindings';
  import Button from '../Button.svelte';
  import Spinner from '../Spinner.svelte';

  interface Props {
    /** Los textos de Ajustes en el idioma activo. */
    texto: (clave: string) => string;
  }

  let { texto }: Props = $props();

  let ocupado = $state(false);
  let nueva = $state<Actualizacion | null>(null);
  let mensaje = $state<string | null>(null);
  // Una copia que instaló NB Launcher se actualiza desde el Launcher: aquí no se
  // ofrece buscar, y se dice quién lo hace.
  let delLauncher = $state(false);
  $effect(() => {
    actualizacionDelLauncher()
      .then((v) => (delLauncher = v))
      .catch(() => (delLauncher = false));
  });

  function explicar(error: unknown): string {
    const codigo = error instanceof NbError ? error.code : '';
    if (codigo === 'NETWORK_DISABLED') return texto('actualizacionSinRed');
    if (codigo === 'COMPONENT_UNAVAILABLE') return texto('actualizacionGestor');
    if (codigo === 'DOCUMENT_CORRUPTED') return texto('actualizacionFirma');
    return texto('actualizacionError');
  }

  async function buscar() {
    ocupado = true;
    mensaje = texto('actualizacionBuscando');
    try {
      nueva = await actualizacionBuscar();
      mensaje = nueva
        ? `${texto('actualizacionDisponible')} ${nueva.version}`
        : texto('actualizacionAlDia');
    } catch (error) {
      mensaje = explicar(error);
    } finally {
      ocupado = false;
    }
  }

  async function instalar() {
    ocupado = true;
    mensaje = texto('actualizacionInstalando');
    try {
      await actualizacionInstalar();
    } catch (error) {
      mensaje = explicar(error);
    } finally {
      ocupado = false;
    }
  }
</script>

<p class="nb-ayuda">{texto('actualizacionAyuda')}</p>

{#if delLauncher}
  <p class="nb-ayuda" role="status">{texto('actualizacionLauncher')}</p>
{:else}
  <div class="nb-actualizacion">
    {#if nueva?.instalable}
      <Button variante="primario" deshabilitado={ocupado} onclick={instalar}>
        {texto('actualizacionInstalar')}
      </Button>
    {:else}
      <Button deshabilitado={ocupado} onclick={buscar}>{texto('actualizacionBuscar')}</Button>
    {/if}
    {#if ocupado}
      <Spinner />
    {/if}
  </div>

  {#if mensaje}
    <p class="nb-ayuda" role="status">{mensaje}</p>
  {/if}
  {#if nueva && !nueva.instalable}
    <p class="nb-ayuda">{texto('actualizacionGestor')}</p>
  {/if}
  {#if nueva?.notas}
    <p class="nb-actualizacion-notas nb-selectable">{nueva.notas}</p>
  {/if}
{/if}

<style>
  .nb-actualizacion {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-2);
  }

  .nb-actualizacion-notas {
    margin: 0;
    white-space: pre-line;
  }
</style>
