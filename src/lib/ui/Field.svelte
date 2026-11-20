<!--
  @component
  Etiqueta, control y, si hace falta, descripción y error, ya asociados.

  Existe para que la asociación no dependa de que alguien se acuerde. Un
  `<label>` suelto junto a un `<input>` sin `for` se ve idéntico y no dice nada
  al lector de pantalla; aquí el identificador lo genera el componente y se lo
  entrega al control, así que no puede faltar.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    /** Texto de la etiqueta. Se traduce. */
    etiqueta: string;
    /** Recibe los atributos que el control debe llevar puestos. */
    children: Snippet<[{ id: string; 'aria-describedby': string | undefined }]>;
    /** Explicación permanente del campo. */
    descripcion?: string;
    /** Mensaje de error. Su presencia marca el campo como inválido. */
    error?: string;
  }

  let { etiqueta, children, descripcion, error }: Props = $props();

  const id = $props.id();
  const idDescripcion = `${id}-descripcion`;
  const idError = `${id}-error`;

  // El error va primero: si hay error, es lo que hay que oír antes.
  const describedBy = $derived(
    [error ? idError : null, descripcion ? idDescripcion : null].filter(Boolean).join(' ') ||
      undefined
  );
</script>

<div class="nb-campo" data-invalido={error ? 'true' : undefined}>
  <label class="nb-campo-etiqueta" for={id}>{etiqueta}</label>
  {@render children({ id, 'aria-describedby': describedBy })}
  {#if descripcion}
    <p class="nb-campo-descripcion" id={idDescripcion}>{descripcion}</p>
  {/if}
  {#if error}
    <p class="nb-campo-error" id={idError} role="alert">{error}</p>
  {/if}
</div>

<style>
  .nb-campo {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
  }

  .nb-campo-etiqueta {
    font-size: var(--nb-text-sm);
    color: var(--m3c-on-surface-variant);
  }

  .nb-campo-descripcion {
    margin: 0;
    font-size: var(--nb-text-xs);
    color: var(--m3c-on-surface-variant);
  }

  .nb-campo-error {
    margin: 0;
    font-size: var(--nb-text-xs);
    color: var(--m3c-error);
  }
</style>
