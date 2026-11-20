<!--
  @component
  Los ajustes del plano del libro: cómo se compone el texto.

  Contrato: docs/architecture/ui-components.md, sección 4, y el
  compromiso 4 de ADR-012..

  Es el plano del **libro**, no el de la aplicación: aquí no hay tema, ni acento,
  ni densidad. Cambiar algo de aquí no puede alterar cómo se ve la aplicación, y
  al revés; la separación la hace cumplir `libro.ts`, que solo lee las
  preferencias que el runtime marca como del libro.

  Los siete controles se pintan recorriendo `CLAVES`, así que no puede haber un
  control sin preferencia detrás ni una preferencia sin control.
-->
<script lang="ts">
  import { textos } from '$lib/i18n';
  import { Field, Range, Select, Switch } from '$ui';

  import { ALINEACIONES, type Composicion, LIMITES } from './libro';

  interface Props {
    composicion: Composicion;
    /** Se avisa con el campo cambiado; guardarlo es de quien monta el panel. */
    alCambiar: (campo: keyof Composicion, valor: Composicion[keyof Composicion]) => void;
    /** Textos. Se traducen. */
    etiquetas?: Readonly<Record<string, string>>;
  }

  let { composicion, alCambiar, etiquetas }: Props = $props();

  /** Un texto que falte se queda en su clave, que es visible y se corrige. */
  // El diccionario del idioma en uso, con las anulaciones de quien monta el
  // componente encima. `$derived` y no una constante: cambiar de idioma tiene
  // que repintar, y una constante capturaría el diccionario de un solo momento.
  const t = $derived({ ...textos().libro, ...etiquetas } as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  const FAMILIAS = ['system', 'serif', 'sans-serif'] as const;

  const DESLIZADORES = [
    { campo: 'cuerpo', limite: LIMITES.cuerpo, unidad: 'px' },
    { campo: 'interlineado', limite: LIMITES.interlineado, unidad: '' },
    { campo: 'margen', limite: LIMITES.margen, unidad: 'px' },
    // La unidad de texto sale del diccionario: escrita aquí, quedaba en español en
    // cualquier idioma.
    { campo: 'ancho', limite: LIMITES.ancho, unidad: 'unidadCaracteres' }
  ] as const;
</script>

<div class="nb-ajustes">
  <Field etiqueta={texto('familia')}>
    {#snippet children({ id })}
      <Select
        {id}
        etiqueta={texto('familia')}
        valor={composicion.familia}
        opciones={FAMILIAS.map((f) => [f, texto(f === 'system' ? 'sistema' : f)] as const)}
        alCambiar={(v) => alCambiar('familia', v)}
      />
    {/snippet}
  </Field>

  {#each DESLIZADORES as { campo, limite, unidad } (campo)}
    <Field
      etiqueta="{texto(campo)}: {composicion[campo]}{unidad === 'unidadCaracteres'
        ? texto(unidad)
        : unidad}"
    >
      {#snippet children({ id })}
        <Range
          {id}
          etiqueta={texto(campo)}
          valor={composicion[campo]}
          minimo={limite.min}
          maximo={limite.max}
          paso={limite.paso}
          alCambiar={(v) => alCambiar(campo, v)}
        />
      {/snippet}
    </Field>
  {/each}

  <Field etiqueta={texto('alineacion')}>
    {#snippet children({ id })}
      <Select
        {id}
        etiqueta={texto('alineacion')}
        valor={composicion.alineacion}
        opciones={ALINEACIONES.map((nombre) => [nombre, texto(`alineacion-${nombre}`)])}
        alCambiar={(valor) => alCambiar('alineacion', valor)}
      />
    {/snippet}
  </Field>
  <div class="nb-fila">
    <span>{texto('paginado')}</span>
    <Switch
      etiqueta={texto('paginado')}
      activo={composicion.paginado}
      alCambiar={(valor) => alCambiar('paginado', valor)}
    />
  </div>
</div>

<style>
  .nb-ajustes {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-4);
  }

  .nb-fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-3);
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }
</style>
