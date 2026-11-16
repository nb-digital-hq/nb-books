<!--
  @component
  Las tarjetas, las preguntas y los conceptos guardados, en Estudio.

  Contrato: ADR-013 —la IA no es un destino: aparece dentro del Lector, de la
  Búsqueda y del Estudio— y docs/architecture/ai/philosophy.md, regla 4.

  **Todo lo de aquí dice que lo generó una máquina**, y quién, también después de
  editarlo: editar una tarjeta la hace más tuya, pero no la convierte en algo que
  se escribió a mano, y confundirlo es lo que la regla 4 prohíbe. Todo se puede
  editar y borrar, y cada fuente abre el libro en el fragmento del que salió.

  Llama al puente desde aquí por lo mismo que `PrepararSignificado`: Estudio se
  carga a demanda, y la carcasa, que está en el paquete de arranque, no tiene por
  qué cargar con esto.
-->
<script lang="ts">
  import {
    concepts as listarConceptos,
    deleteConcept,
    deleteStudyCard,
    editConcept,
    editStudyCard,
    reviewStudyCard,
    studyCardCounts,
    studyCards
  } from '$core/ipc';
  import { idioma } from '$lib/i18n';
  import { mensajeDeError } from '$lib/i18n/errores';
  import { TEXTOS_DE_ESTUDIO } from '$lib/i18n/locales/estudio';
  import type {
    Concept,
    ConceptKind,
    Locator,
    ReviewRating,
    StudyCard,
    StudyCardCounts
  } from '$lib/bindings';

  import Button from '../Button.svelte';
  import Field from '../Field.svelte';
  import Select from '../Select.svelte';
  import EmptyState from './EmptyState.svelte';
  import Seccion from './Seccion.svelte';

  interface Props {
    vista: 'tarjetas' | 'conceptos';
    /** Abrir el libro en una fuente. */
    alAbrirFuente: (fileId: string, locator: Locator) => void;
  }

  let { vista, alAbrirFuente }: Props = $props();

  const t = $derived(TEXTOS_DE_ESTUDIO[idioma()] as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  /** Cuántas se piden de cada vez. */
  const PAGINA = 50;
  const TIPOS: ConceptKind[] = ['persona', 'lugar', 'obra', 'organizacion', 'fecha', 'concepto'];

  let tarjetas = $state<StudyCard[]>([]);
  let conceptos = $state<Concept[]>([]);
  let hayMas = $state(false);
  let fallo = $state<string | null>(null);

  let soloPendientes = $state(true);
  let filtroInicializado = $state(false);
  let cuentas = $state<StudyCardCounts | null>(null);

  /** Lo que se ve de cada tarjeta: la respuesta descubierta o la opción elegida. */
  let descubiertas = $state<Record<string, boolean>>({});
  let elegidas = $state<Record<string, number | undefined>>({});
  let editando = $state<string | null>(null);
  let borrador = $state<{
    front: string;
    back: string;
    options: string[] | null;
    correct: number | null;
    kind: ConceptKind;
  } | null>(null);

  async function cargar(desde = 0) {
    try {
      if (vista === 'tarjetas') {
        const c = await studyCardCounts();
        cuentas = c;
        if (!filtroInicializado) {
          filtroInicializado = true;
          if (c.due === 0 && c.total > 0) {
            soloPendientes = false;
          }
        }
        const nuevas = await studyCards(null, PAGINA, desde, soloPendientes);
        tarjetas = desde === 0 ? nuevas : [...tarjetas, ...nuevas];
        hayMas = nuevas.length === PAGINA;
      } else {
        const nuevos = await listarConceptos(null, PAGINA, desde);
        conceptos = desde === 0 ? nuevos : [...conceptos, ...nuevos];
        hayMas = nuevos.length === PAGINA;
      }
      fallo = null;
    } catch (e) {
      fallo = mensajeDeError(e);
    }
  }

  function cambiarFiltro(soloHoy: boolean) {
    if (soloPendientes === soloHoy) return;
    soloPendientes = soloHoy;
    void cargar(0);
  }

  async function responder(id: string, calificacion: ReviewRating) {
    try {
      await reviewStudyCard(id, calificacion);
      delete descubiertas[id];
      delete elegidas[id];
      await cargar(0);
    } catch (e) {
      fallo = mensajeDeError(e);
    }
  }

  $effect(() => {
    void vista;
    editando = null;
    void cargar();
  });

  async function hacer(accion: () => Promise<void>) {
    try {
      await accion();
      editando = null;
      await cargar();
    } catch (e) {
      fallo = mensajeDeError(e);
    }
  }

  function editarTarjeta(tarjeta: StudyCard) {
    editando = tarjeta.id;
    borrador = {
      front: tarjeta.front,
      back: tarjeta.back,
      options: tarjeta.options ? [...tarjeta.options] : null,
      correct: tarjeta.correct,
      kind: 'concepto'
    };
  }

  function editarConcepto(concepto: Concept) {
    editando = concepto.id;
    borrador = {
      front: concepto.name,
      back: concepto.description,
      options: null,
      correct: null,
      kind: concepto.kind
    };
  }
</script>

{#snippet procedencia(provider: string, editada: string | null, titulo: string)}
  <p class="nb-cuaderno-marca">
    {texto('generado')}
    {t[`proveedor-${provider}`] ?? provider} · <span class="nb-cuaderno-libro">{titulo}</span>
    {#if editada}· {texto('editada')}{/if}
  </p>
{/snippet}

{#snippet fuentes(fileId: string | null, lista: Locator[])}
  {#if lista.length === 0}
    <span class="nb-cuaderno-sin-cita" title={texto('noVerificableAyuda')}
      >{texto('noVerificable')}</span
    >
  {:else if fileId}
    {@const archivo = fileId}
    <span class="nb-cuaderno-fuentes">
      {#each lista as locator, i (i)}
        <button
          type="button"
          class="nb-cuaderno-fuente"
          aria-label="{texto('abrirFuente')} {i + 1}"
          onclick={() => alAbrirFuente(archivo, locator)}>{texto('fuente')} {i + 1}</button
        >
      {/each}
    </span>
  {:else}
    <span class="nb-cuaderno-texto">{texto('sinCopia')}</span>
  {/if}
{/snippet}

<Seccion titulo={vista === 'tarjetas' ? texto('vistaTarjetas') : texto('vistaConceptos')}>
  {#if fallo}
    <p class="nb-cuaderno-fallo" role="alert">{fallo}</p>
  {/if}

  {#if vista === 'tarjetas'}
    {#if cuentas && cuentas.total > 0}
      <div class="nb-cuaderno-filtros" role="tablist" aria-label={texto('vistaTarjetas')}>
        <button
          type="button"
          role="tab"
          class="nb-cuaderno-filtro"
          aria-selected={soloPendientes}
          onclick={() => cambiarFiltro(true)}
        >
          {texto('repasarHoy')}
          {#if cuentas.due > 0}
            <span class="nb-cuaderno-cuenta-badge">{cuentas.due}</span>
          {/if}
        </button>
        <button
          type="button"
          role="tab"
          class="nb-cuaderno-filtro"
          aria-selected={!soloPendientes}
          onclick={() => cambiarFiltro(false)}
        >
          {texto('todas')}
          <span class="nb-cuaderno-cuenta-badge">{cuentas.total}</span>
        </button>
      </div>
    {/if}

    {#if tarjetas.length === 0}
      {#if soloPendientes}
        <EmptyState icono="ia" mensaje={texto('sinPendientesHoy')} />
      {:else}
        <EmptyState icono="ia" mensaje={texto('sinTarjetas')} />
      {/if}
    {:else}
      <ul class="nb-cuaderno-lista">
        {#each tarjetas as tarjeta (tarjeta.id)}
          <li class="nb-cuaderno-tarjeta">
            {#if editando === tarjeta.id && borrador}
              {@const b = borrador}
              <Field etiqueta={tarjeta.options ? texto('enunciado') : texto('pregunta')}>
                {#snippet children({ id })}
                  <textarea {id} class="nb-campo" rows="2" bind:value={b.front}></textarea>
                {/snippet}
              </Field>
              {#if b.options}
                {@const opciones = b.options}
                {#each opciones as _, j (j)}
                  <Field etiqueta="{texto('opcion')} {j + 1}">
                    {#snippet children({ id })}
                      <input {id} class="nb-campo" type="text" bind:value={opciones[j]} />
                    {/snippet}
                  </Field>
                {/each}
                <Select
                  etiqueta={texto('correcta')}
                  valor={String(b.correct ?? 0)}
                  opciones={opciones.map(
                    (_, j) => [String(j), `${texto('opcion')} ${j + 1}`] as const
                  )}
                  alCambiar={(valor) => (b.correct = Number(valor))}
                />
              {/if}
              <Field etiqueta={tarjeta.options ? texto('explicacion') : texto('respuesta')}>
                {#snippet children({ id })}
                  <textarea {id} class="nb-campo" rows="2" bind:value={b.back}></textarea>
                {/snippet}
              </Field>
              <div class="nb-cuaderno-acciones">
                <Button
                  variante="primario"
                  onclick={() =>
                    void hacer(() =>
                      editStudyCard(tarjeta.id, {
                        front: b.front,
                        back: b.back,
                        options: b.options,
                        correct: b.options ? b.correct : null
                      })
                    )}>{texto('guardar')}</Button
                >
                <Button onclick={() => (editando = null)}>{texto('cancelar')}</Button>
              </div>
            {:else}
              <div class="nb-cuaderno-cabecera">
                <p class="nb-cuaderno-anverso">{tarjeta.front}</p>
                <span class="nb-cuaderno-progreso-repaso">
                  {#if tarjeta.repetitions > 0}
                    {texto('intervaloActual')}: {tarjeta.intervalDays}d
                  {:else}
                    {texto('nueva')}
                  {/if}
                </span>
              </div>
              {#if tarjeta.options}
                {@const elegida = elegidas[tarjeta.id]}
                {@const comprobada = descubiertas[tarjeta.id] === true}
                <div class="nb-cuaderno-opciones" role="radiogroup" aria-label={tarjeta.front}>
                  {#each tarjeta.options as opcion, j (j)}
                    <button
                      type="button"
                      role="radio"
                      class="nb-cuaderno-opcion"
                      aria-checked={elegida === j}
                      data-correcta={comprobada && j === tarjeta.correct}
                      disabled={comprobada}
                      onclick={() => (elegidas[tarjeta.id] = j)}>{opcion}</button
                    >
                  {/each}
                </div>
                {#if comprobada}
                  <p class="nb-cuaderno-texto" role="status">
                    {elegida === tarjeta.correct ? texto('acierto') : texto('fallo')}
                    {tarjeta.back}
                  </p>
                  <div
                    class="nb-cuaderno-repaso"
                    role="group"
                    aria-label={texto('calificarRepaso')}
                  >
                    <Button onclick={() => void responder(tarjeta.id, 'again')}
                      >{texto('calificacionAgain')}</Button
                    >
                    <Button onclick={() => void responder(tarjeta.id, 'hard')}
                      >{texto('calificacionHard')}</Button
                    >
                    <Button variante="primario" onclick={() => void responder(tarjeta.id, 'good')}
                      >{texto('calificacionGood')}</Button
                    >
                    <Button onclick={() => void responder(tarjeta.id, 'easy')}
                      >{texto('calificacionEasy')}</Button
                    >
                  </div>
                {:else}
                  <div class="nb-cuaderno-acciones">
                    <Button
                      deshabilitado={elegida === undefined}
                      onclick={() => (descubiertas[tarjeta.id] = true)}>{texto('comprobar')}</Button
                    >
                  </div>
                {/if}
              {:else if descubiertas[tarjeta.id]}
                <p class="nb-cuaderno-reverso">{tarjeta.back}</p>
                <div class="nb-cuaderno-repaso" role="group" aria-label={texto('calificarRepaso')}>
                  <Button onclick={() => void responder(tarjeta.id, 'again')}
                    >{texto('calificacionAgain')}</Button
                  >
                  <Button onclick={() => void responder(tarjeta.id, 'hard')}
                    >{texto('calificacionHard')}</Button
                  >
                  <Button variante="primario" onclick={() => void responder(tarjeta.id, 'good')}
                    >{texto('calificacionGood')}</Button
                  >
                  <Button onclick={() => void responder(tarjeta.id, 'easy')}
                    >{texto('calificacionEasy')}</Button
                  >
                </div>
              {:else}
                <div class="nb-cuaderno-acciones">
                  <Button onclick={() => (descubiertas[tarjeta.id] = true)}
                    >{texto('mostrarRespuesta')}</Button
                  >
                </div>
              {/if}
              {@render procedencia(tarjeta.provider, tarjeta.editedAt, tarjeta.title)}
              <div class="nb-cuaderno-pie">
                {@render fuentes(tarjeta.fileId, tarjeta.sources)}
                <span class="nb-cuaderno-acciones">
                  <Button onclick={() => editarTarjeta(tarjeta)}>{texto('editar')}</Button>
                  <Button
                    variante="peligro"
                    onclick={() => void hacer(() => deleteStudyCard(tarjeta.id))}
                    >{texto('borrar')}</Button
                  >
                </span>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {:else if conceptos.length === 0}
    <EmptyState icono="ia" mensaje={texto('sinConceptos')} />
  {:else}
    <ul class="nb-cuaderno-lista">
      {#each conceptos as concepto (concepto.id)}
        <li class="nb-cuaderno-tarjeta">
          {#if editando === concepto.id && borrador}
            {@const b = borrador}
            <Field etiqueta={texto('nombre')}>
              {#snippet children({ id })}
                <input {id} class="nb-campo" type="text" bind:value={b.front} />
              {/snippet}
            </Field>
            <Select
              etiqueta={texto('clase')}
              valor={b.kind}
              opciones={TIPOS.map((tipo) => [tipo, texto(`tipo-${tipo}`)] as const)}
              alCambiar={(valor) => (b.kind = valor as ConceptKind)}
            />
            <Field etiqueta={texto('descripcion')}>
              {#snippet children({ id })}
                <textarea {id} class="nb-campo" rows="2" bind:value={b.back}></textarea>
              {/snippet}
            </Field>
            <div class="nb-cuaderno-acciones">
              <Button
                variante="primario"
                onclick={() => void hacer(() => editConcept(concepto.id, b.front, b.kind, b.back))}
                >{texto('guardar')}</Button
              >
              <Button onclick={() => (editando = null)}>{texto('cancelar')}</Button>
            </div>
          {:else}
            <p class="nb-cuaderno-anverso">
              {concepto.name}
              <span class="nb-cuaderno-clase">{texto(`tipo-${concepto.kind}`)}</span>
            </p>
            {#if concepto.description}
              <p class="nb-cuaderno-reverso">{concepto.description}</p>
            {/if}
            {#if concepto.otherEditions > 0}
              <p class="nb-cuaderno-texto">
                {texto('enOtrosLibros')}
                {concepto.otherEditions}
              </p>
            {/if}
            {@render procedencia(concepto.provider, concepto.editedAt, concepto.title)}
            <div class="nb-cuaderno-pie">
              {@render fuentes(concepto.fileId, concepto.sources)}
              <span class="nb-cuaderno-acciones">
                <Button onclick={() => editarConcepto(concepto)}>{texto('editar')}</Button>
                <Button
                  variante="peligro"
                  onclick={() => void hacer(() => deleteConcept(concepto.id))}
                  >{texto('borrar')}</Button
                >
              </span>
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if hayMas}
    <Button onclick={() => void cargar(vista === 'tarjetas' ? tarjetas.length : conceptos.length)}
      >{texto('cargarMas')}</Button
    >
  {/if}
</Seccion>

<style>
  .nb-cuaderno-filtros {
    display: flex;
    gap: var(--nb-space-2);
    margin-bottom: var(--nb-space-3);
  }

  .nb-cuaderno-filtro {
    display: inline-flex;
    align-items: center;
    gap: var(--nb-space-2);
    padding: var(--nb-space-1) var(--nb-space-3);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-on-surface-variant);
    font: inherit;
    font-size: var(--nb-text-sm);
    cursor: pointer;
  }

  .nb-cuaderno-filtro[aria-selected='true'] {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    border-color: var(--m3c-primary);
    font-weight: 500;
  }

  .nb-cuaderno-cuenta-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0 var(--nb-space-1);
    min-width: 1.25rem;
    height: 1.25rem;
    border-radius: var(--nb-radius-full);
    background: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }

  .nb-cuaderno-filtro[aria-selected='true'] .nb-cuaderno-cuenta-badge {
    background: var(--m3c-primary);
    color: var(--m3c-on-primary);
  }

  .nb-cuaderno-lista {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-cuaderno-tarjeta {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding: var(--nb-space-4);
    border-radius: var(--nb-radius-lg);
    background: var(--m3c-surface-container-low);
  }

  .nb-cuaderno-cabecera {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--nb-space-2);
  }

  .nb-cuaderno-progreso-repaso {
    font-size: var(--nb-text-xs);
    color: var(--m3c-outline);
    white-space: nowrap;
  }

  .nb-cuaderno-repaso {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-2);
    margin-top: var(--nb-space-2);
    padding-top: var(--nb-space-2);
    border-top: 1px dashed var(--m3c-outline-variant);
  }

  .nb-cuaderno-anverso {
    margin: 0;
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-md);
    font-weight: 600;
    line-height: 1.4;
    white-space: pre-wrap;
  }

  .nb-cuaderno-reverso,
  .nb-cuaderno-texto,
  .nb-cuaderno-marca {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    line-height: 1.5;
    white-space: pre-wrap;
  }

  .nb-cuaderno-reverso {
    color: var(--m3c-on-surface);
  }

  .nb-cuaderno-marca {
    font-size: var(--nb-text-xs);
  }

  .nb-cuaderno-libro {
    font-style: italic;
  }

  .nb-cuaderno-clase {
    margin-inline-start: var(--nb-space-2);
    padding: 0 var(--nb-space-2);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
    font-size: var(--nb-text-xs);
    font-weight: 400;
  }

  .nb-cuaderno-opciones {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
  }

  .nb-cuaderno-opcion {
    padding: var(--nb-space-2) var(--nb-space-3);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-md);
    background: transparent;
    color: var(--m3c-on-surface);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }

  .nb-cuaderno-opcion[aria-checked='true'] {
    border-color: var(--m3c-primary);
    background: var(--m3c-secondary-container);
  }

  .nb-cuaderno-opcion[data-correcta='true'] {
    border-color: var(--m3c-primary);
    background: var(--m3c-primary-container);
    color: var(--m3c-on-primary-container);
  }

  .nb-cuaderno-acciones,
  .nb-cuaderno-pie,
  .nb-cuaderno-fuentes {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-2);
  }

  .nb-cuaderno-pie {
    justify-content: space-between;
  }

  .nb-cuaderno-fuente {
    padding: 0 var(--nb-space-2);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-primary);
    font: inherit;
    font-size: var(--nb-text-xs);
    cursor: pointer;
  }

  .nb-cuaderno-sin-cita {
    padding: 0 var(--nb-space-2);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-error-container);
    color: var(--m3c-on-error-container);
    font-size: var(--nb-text-xs);
  }

  .nb-cuaderno-fallo {
    margin: 0;
    color: var(--m3c-error);
    font-size: var(--nb-text-sm);
  }
</style>
