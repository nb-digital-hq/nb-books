<!--
  @component
  Fondo sonoro para concentrarse.

  Contrato: docs/design/reading-experience, sección 6.

  **Ni un byte de audio.** Lo que una extensión aporta es el nombre y qué clase
  de ruido; el sonido lo sintetiza el núcleo. Con archivos, cada paquete traería
  megabytes al instalador de quien no lo quiere.

  **No se guarda nada.** Ni cuál sonaba ni el volumen: abrir la aplicación y que
  empiece a sonar ruido por su cuenta sería hostil, y de paso hace que
  desinstalar el paquete no deje rastro sin tener que limpiar nada.
-->
<script lang="ts">
  import type { ExtensionSoundscape } from '$lib/bindings';

  import { textos } from '$lib/i18n';
  import Field from '../Field.svelte';
  import Range from '../Range.svelte';
  import Select from '../Select.svelte';

  interface Props {
    /** Los que aportan los paquetes instalados. Sin ninguno, no hay qué sonar. */
    fondos?: readonly ExtensionSoundscape[];
    /** Cuál suena ahora, o nulo. */
    sonando?: string | null;
    volumen?: number;
    /** Minutos del temporizador; 0 es sin temporizador. */
    minutos?: number;
    alSonar?: (id: string) => void;
    alParar?: () => void;
    alCambiarVolumen?: (valor: number) => void;
    alCambiarMinutos?: (valor: number) => void;
  }

  let {
    fondos = [],
    sonando = null,
    volumen = 0.4,
    minutos = 0,
    alSonar,
    alParar,
    alCambiarVolumen,
    alCambiarMinutos
  }: Props = $props();

  const t = $derived(textos().fondoSonoro as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  /** Los minutos que se ofrecen. Cero es "hasta que lo pare". */
  const TIEMPOS = [0, 15, 30, 45, 60] as const;
</script>

<div class="nb-fondo">
  {#if fondos.length === 0}
    <p class="nb-ayuda">{texto('sinFondos')}</p>
  {:else}
    <ul class="nb-lista">
      {#each fondos as fondo (fondo.id)}
        <li>
          <!--
            Botón crudo y no `Button` porque lo que hay que decir es que **está
            puesto**, y eso se dice con `aria-pressed`: un botón que solo cambia
            de color no lo dice a quien usa un lector de pantalla.
          -->
          <button
            type="button"
            class="nb-fondo-boton"
            aria-pressed={sonando === fondo.id}
            onclick={() => (sonando === fondo.id ? alParar?.() : alSonar?.(fondo.id))}
          >
            {fondo.name}
          </button>
        </li>
      {/each}
    </ul>

    <Field etiqueta="{texto('volumen')}: {Math.round(volumen * 100)}%">
      {#snippet children({ id })}
        <Range
          {id}
          etiqueta={texto('volumen')}
          valor={volumen}
          minimo={0}
          maximo={1}
          paso={0.05}
          alCambiar={(valor) => alCambiarVolumen?.(valor)}
        />
      {/snippet}
    </Field>

    <Field etiqueta={texto('temporizador')}>
      {#snippet children({ id })}
        <Select
          {id}
          etiqueta={texto('temporizador')}
          valor={String(minutos)}
          opciones={TIEMPOS.map(
            (m) => [String(m), m === 0 ? texto('sinLimite') : `${m} min`] as const
          )}
          alCambiar={(valor) => alCambiarMinutos?.(Number(valor))}
        />
      {/snippet}
    </Field>

    <p class="nb-ayuda">{texto('ayuda')}</p>
  {/if}
</div>

<style>
  .nb-fondo {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-4);
  }

  .nb-lista {
    display: flex;
    flex-wrap: wrap;
    gap: var(--nb-space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-fondo-boton {
    padding: var(--nb-space-2) var(--nb-space-3);
    border: 1px solid transparent;
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    font: inherit;
    cursor: pointer;
  }

  .nb-fondo-boton[aria-pressed='true'] {
    border-color: var(--m3c-primary);
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }

  .nb-ayuda {
    margin: 0;
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }
</style>
