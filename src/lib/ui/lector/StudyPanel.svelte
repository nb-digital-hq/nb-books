<!--
  @component
  Estudiar con IA el capítulo abierto o la selección: resumir, hacer estudio.tarjetas y
  estudio.preguntas, y sacar estudio.conceptos.

  Contratos: docs/architecture/ai/prompts.md y docs/architecture/ai/privacy.md.

  Cuatro cosas que este panel garantiza y que son criterio, no estilo:

  1. **Quién genera está siempre a la vista**, y si el texto sale del equipo, a
     dónde (`ai/privacy.md` sección 6). Antes de generar y al lado de lo generado.
  2. **Lo generado se pinta como texto.** Nada de lo que devuelve un modelo pasa
     por `{@html}` ni se convierte en enlace: una salida que escriba marcado o una
     dirección la enseña como los caracteres que son.
  3. **Cada afirmación dice de dónde sale**, y abrir la fuente lleva al fragmento
     exacto. Lo que no cita nada se marca como no verificable.
  4. **Nada se guarda solo.** Las estudio.tarjetas, las estudio.preguntas y los estudio.conceptos son una
     propuesta que se edita y se descarta aquí; entra en Estudio al pulsar guardar
    .

  Llama al puente desde aquí, como `PrepararSignificado`: la carcasa está en el
  paquete de arranque, y esto solo se carga al abrir el panel.
-->
<script lang="ts">
  import {
    allowAiProvider,
    cancelStudy,
    estimateStudyScope,
    generateStudy,
    reportAiOutput,
    saveConcepts,
    saveStudyCards,
    studyAvailability
  } from '$core/ipc';
  import { idioma } from '$lib/i18n';
  import { codigoDeError, mensajeDeError } from '$lib/i18n/errores';
  import { TEXTOS_DE_ESTUDIO } from '$lib/i18n/locales/estudio';
  import type {
    AiReportReason,
    ConceptKind,
    GeneratedCard,
    GeneratedConcept,
    GeneratedQuestion,
    Locator,
    NavigationEntry,
    StudyAvailability,
    StudyFunction,
    StudyGeneration,
    StudyScope,
    StudyScopeEstimate
  } from '$lib/bindings';

  import { notificar } from '../notificaciones.svelte';

  import { capituloDe } from './capitulo';
  import { estudioDelLector as estudio, estudioPara } from './estudio.svelte';

  import Button from '../Button.svelte';
  import Field from '../Field.svelte';
  import Select from '../Select.svelte';
  import Spinner from '../Spinner.svelte';

  interface Props {
    editionId: string;
    /** El índice del documento, para saber dónde acaba el capítulo. */
    entradas: NavigationEntry[];
    /** Dónde se está leyendo. */
    actual: Locator;
    /** Lo seleccionado, si hay algo. */
    seleccion: { locator: Locator; texto: string } | null;
    /** Abrir una fuente. No mueve nada más que la lectura. */
    alSaltar: (locator: Locator) => void;
  }

  let { editionId, entradas, actual, seleccion, alSaltar }: Props = $props();

  const capitulo = $derived(capituloDe(entradas, actual));

  const t = $derived(TEXTOS_DE_ESTUDIO[idioma()] as Record<string, string>);
  const texto = (clave: string): string => t[clave] ?? clave;

  /** El nombre de cada función. Cuáles se ofrecen lo dice el runtime, no esta lista. */
  const NOMBRES: Record<StudyFunction, string> = {
    summary: 'resumir',
    flashcards: 'tarjetas',
    questions: 'preguntas',
    concepts: 'conceptos'
  };
  const TIPOS: ConceptKind[] = ['persona', 'lugar', 'obra', 'organizacion', 'fecha', 'concepto'];

  let disponible = $state<StudyAvailability | null>(null);
  let sobre = $state<'capitulo' | 'seleccion'>('capitulo');
  let estimacion = $state<StudyScopeEstimate | null>(null);

  /**
   * Marcar una salida como incorrecta.
   *
   * Es una acción **discreta**: un enlace al lado de quién generó, no un botón
   * grande. Lo que se quiere es que esté cuando haga falta, no empujar a usarla; y
   * lo que se anota es qué plantilla y qué modelo la hicieron, porque un estudio.fallo que
   * se repite en veinte avisos apunta a una plantilla concreta y un pulgar hacia
   * abajo no apunta a nada.
   */
  const MOTIVOS: { valor: AiReportReason; clave: string }[] = [
    { valor: 'falso', clave: 'marcarFalso' },
    { valor: 'no_viene_a_cuento', clave: 'marcarNoVieneACuento' },
    { valor: 'incompleto', clave: 'marcarIncompleto' },
    { valor: 'mal_escrito', clave: 'marcarMalEscrito' },
    { valor: 'otro', clave: 'marcarOtro' }
  ];
  let marcando = $state(false);
  let motivo = $state<AiReportReason>('falso');
  let notaDelAviso = $state('');
  let marcada = $state(false);

  async function marcarIncorrecta() {
    if (!estudio.resultado) return;
    try {
      await reportAiOutput({
        function: estudio.resultado.function,
        templateId: estudio.resultado.templateId,
        templateVersion: estudio.resultado.templateVersion,
        providerId: estudio.resultado.providerId,
        providerLabel: estudio.resultado.providerLabel,
        leftDevice: estudio.resultado.leftDevice,
        editionId,
        reason: motivo,
        note: notaDelAviso.trim() || null,
        // Lo que se guarda es **la salida del modelo**, no el texto del libro: los
        // fragmentos de `sources` se quedan aquí a propósito.
        output: JSON.stringify(estudio.resultado.reading.items)
      });
      marcando = false;
      marcada = true;
      notaDelAviso = '';
    } catch (e) {
      estudio.fallo = mensajeDeError(e);
    }
  }

  async function consultar() {
    try {
      disponible = await studyAvailability();
    } catch (e) {
      estudio.fallo = mensajeDeError(e);
    }
  }

  $effect(() => {
    void consultar();
  });

  // Sin selección no se ofrece "la selección"; con ella, se ofrece primero.
  $effect(() => {
    sobre = seleccion ? 'seleccion' : 'capitulo';
  });

  /**
   * Lo que se puede pedir con quien genera ahora.
   *
   * El modelo local no hace estudio.preguntas de autoevaluación —las devuelve con la clave
   * en la opción equivocada—, así que no se ofrecen; con un proveedor externo sí.
   * Ofrecer una función rota es peor que no ofrecerla.
   */
  const funciones = $derived(disponible?.functions ?? []);

  /** El motivo por el que una función se ofrece sin recomendarla, si lo hay. */
  function desaconsejada(id: StudyFunction): string | undefined {
    return disponible?.discouraged.find((d) => d.function === id)?.reason;
  }

  /** Una espera, dicha como se dice: "unos 2 minutos", "menos de un minuto". */
  function minutos(segundos: number): string {
    if (segundos < 60) return texto('menosDeUnMinuto');
    return `${texto('unos')} ${Math.round(segundos / 60)} ${texto('minutos')}`;
  }

  $effect(() => {
    const primera = funciones[0];
    if (primera && !funciones.includes(estudio.funcion)) estudio.funcion = primera;
  });

  const alcance = $derived<StudyScope | null>(
    sobre === 'seleccion' && seleccion
      ? { kind: 'selection', editionId, locator: seleccion.locator, text: seleccion.texto }
      : capitulo.hrefs.length > 0
        ? {
            kind: 'section',
            editionId,
            hrefs: capitulo.hrefs,
            fromPosition: capitulo.desde,
            toPosition: capitulo.hasta
          }
        : null
  );

  $effect(() => {
    if (disponible?.leftDevice && alcance) {
      let cancelado = false;
      void estimateStudyScope(alcance)
        .then((est) => {
          if (!cancelado) estimacion = est;
        })
        .catch(() => {
          if (!cancelado) estimacion = null;
        });
      return () => {
        cancelado = true;
      };
    } else {
      estimacion = null;
    }
  });

  /** Si el panel sigue abierto. Lo que termina con él cerrado se avisa aparte. */
  let abierto = true;
  $effect(() => {
    abierto = true;
    return () => {
      abierto = false;
    };
  });

  // Lo que había generado para este libro sigue aquí al volver a abrir el panel; y
  // si cambia de libro, empieza de cero.
  $effect(() => {
    estudioPara(editionId);
  });

  async function generar() {
    if (!alcance) return;
    estudio.generando = true;
    estudio.fallo = null;
    estudio.sinPermiso = false;
    estudio.guardado = false;
    estudio.resultado = null;
    cancelada = false;
    try {
      const generado = await generateStudy(estudio.funcion, alcance);
      if (cancelada) return;
      estudio.resultado = generado;
      const items = generado.reading.items;
      estudio.tarjetas = items.kind === 'tarjetas' ? structuredClone(items.items) : [];
      estudio.preguntas = items.kind === 'preguntas' ? structuredClone(items.items) : [];
      estudio.conceptos = items.kind === 'conceptos' ? structuredClone(items.items) : [];
      // Terminó con el panel cerrado: se dice, porque quien siguió leyendo no lo ve.
      if (!abierto) notificar.exito(texto('listoEnSegundoPlano'));
    } catch (e) {
      const codigo = codigoDeError(e);
      if (codigo === 'CONSENT_REQUIRED') estudio.sinPermiso = true;
      else if (codigo !== 'CANCELLED') estudio.fallo = mensajeDeError(e);
    } finally {
      estudio.generando = false;
      void consultar();
    }
  }

  /**
   * Cancelar se ve al momento.
   *
   * Antes el panel seguía en "Generando" hasta que el runtime soltaba el modelo,
   * que en un procesador sin AVX2 eran minutos: parecía que cancelar no hacía
   * nada. Se deja de esperar ya; lo que el runtime termine después se descarta,
   * porque `generar` comprueba esta marca antes de enseñar nada.
   */
  let cancelada = false;
  function cancelar() {
    cancelada = true;
    estudio.generando = false;
    void cancelStudy();
  }

  async function autorizarYGenerar() {
    try {
      await allowAiProvider();
      await generar();
    } catch (e) {
      estudio.fallo = mensajeDeError(e);
    }
  }

  function fuentes(citas: number[]): Locator[] {
    return citas.flatMap((i) =>
      estudio.resultado?.sources[i] ? [estudio.resultado.sources[i].locator] : []
    );
  }

  async function guardarTodo() {
    if (!estudio.resultado) return;
    const comun = {
      editionId: estudio.resultado.editionId,
      provider: estudio.resultado.providerId,
      templateId: estudio.resultado.templateId,
      templateVersion: estudio.resultado.templateVersion
    };
    try {
      if (estudio.tarjetas.length > 0) {
        await saveStudyCards(
          estudio.tarjetas.map((c) => ({
            ...comun,
            front: c.question,
            back: c.answer,
            options: null,
            correct: null,
            sources: fuentes(c.citations)
          }))
        );
      }
      if (estudio.preguntas.length > 0) {
        await saveStudyCards(
          estudio.preguntas.map((p) => ({
            ...comun,
            front: p.prompt,
            back: p.explanation,
            options: p.options,
            correct: p.correct,
            sources: fuentes(p.citations)
          }))
        );
      }
      if (estudio.conceptos.length > 0) {
        await saveConcepts(
          estudio.conceptos.map((c) => ({
            ...comun,
            name: c.name,
            kind: c.kind,
            description: c.description,
            sources: fuentes(c.citations)
          }))
        );
      }
      estudio.guardado = true;
      estudio.fallo = null;
    } catch (e) {
      estudio.fallo = mensajeDeError(e);
    }
  }

  const quedaAlgo = $derived(
    estudio.tarjetas.length + estudio.preguntas.length + estudio.conceptos.length > 0
  );
</script>

{#snippet citas(lista: number[])}
  {#if lista.length === 0}
    <span class="nb-estudio-sin-cita" title={texto('noVerificableAyuda')}
      >{texto('noVerificable')}</span
    >
  {:else}
    <span class="nb-estudio-citas">
      {#each lista as i (i)}
        {@const fuente = estudio.resultado?.sources[i]}
        {#if fuente}
          <button
            type="button"
            class="nb-estudio-cita"
            title={fuente.excerpt}
            aria-label="{texto('abrirFuente')} {i + 1}"
            onclick={() => alSaltar(fuente.locator)}>{texto('fuente')} {i + 1}</button
          >
        {/if}
      {/each}
    </span>
  {/if}
{/snippet}

<section class="nb-estudio" aria-label={texto('titulo')} aria-busy={estudio.generando}>
  <!-- Quién genera, siempre a la vista: ai/privacy.md sección 6. -->
  {#if disponible?.ready}
    <p class="nb-estudio-quien" data-sale={disponible.leftDevice}>
      {texto('quien')}
      <strong>{disponible.providerLabel}</strong>{#if disponible.leftDevice}. {texto('saleHacia')}
        <strong>{disponible.host}</strong>.{:else}, {texto('enEsteEquipo')}{/if}
    </p>
  {:else if disponible}
    <p class="nb-estudio-texto">
      {texto(disponible.reason ? `motivo-${disponible.reason}` : 'sinGenerador')}
    </p>
  {/if}

  {#if disponible?.ready}
    <div class="nb-estudio-controles">
      <Select
        etiqueta={texto('funcion')}
        valor={estudio.funcion}
        opciones={funciones.map(
          (id) =>
            [
              id,
              desaconsejada(id)
                ? `${texto(NOMBRES[id])} (${texto('noRecomendada')})`
                : texto(NOMBRES[id])
            ] as const
        )}
        alCambiar={(valor) => (estudio.funcion = valor as StudyFunction)}
      />
      {#if seleccion}
        <Select
          etiqueta={texto('sobre')}
          valor={sobre}
          opciones={[
            ['seleccion', texto('seleccion')],
            ['capitulo', capitulo.titulo || texto('capitulo')]
          ]}
          alCambiar={(valor) => (sobre = valor as 'capitulo' | 'seleccion')}
        />
      {/if}
    </div>

    <!--
      Lo que es probable que pase, **antes** de pulsar: recomendar no es obligar
      (2026-09-22), así que nada de esto quita el botón. Lo dice.
    -->
    {#if desaconsejada(estudio.funcion)}
      <p class="nb-estudio-aviso-previo" role="status">
        {texto(`desaconsejada-${desaconsejada(estudio.funcion)}`)}
      </p>
    {/if}
    {#if disponible.estimatedSeconds !== null && !disponible.leftDevice}
      <p class="nb-estudio-texto">
        {texto('tardaraAqui')}
        {minutos(disponible.estimatedSeconds)}{#each disponible.warnings as aviso (aviso)}. {texto(
            `prevision-${aviso}`
          )}{/each}.
      </p>
    {/if}
    {#if disponible.leftDevice && estimacion && !estudio.generando}
      <p class="nb-estudio-texto" data-testid="estudio-estimacion-coste">
        {texto('costeTextoSalida')
          .replace('{caracteres}', String(estimacion.characters))
          .replace('{tokens}', String(estimacion.approximateTokens))}
        {#if estimacion.sampled}
          {' '}{texto('costeTextoMuestreo')}
        {/if}
      </p>
    {/if}

    {#if estudio.generando}
      <div class="nb-estudio-fila" role="status">
        <Spinner etiqueta={texto('generando')} enLinea />
        <span class="nb-estudio-texto">
          {texto('generando')}.
          {#if !disponible.leftDevice}{texto('generandoLocal')}{/if}
        </span>
      </div>
      <div class="nb-estudio-acciones">
        <Button onclick={cancelar}>{texto('cancelar')}</Button>
      </div>
    {:else if estudio.sinPermiso}
      <p class="nb-estudio-texto" role="alert">
        {texto('sinPermiso')} <strong>{disponible.host}</strong>. {disponible.whatIsSent}
      </p>
      <div class="nb-estudio-acciones">
        <Button variante="primario" onclick={() => void autorizarYGenerar()}
          >{texto('autorizarYGenerar')}</Button
        >
      </div>
    {:else}
      <div class="nb-estudio-acciones">
        <Button variante="primario" deshabilitado={!alcance} onclick={() => void generar()}>
          {estudio.resultado ? texto('repetir') : texto('generar')}
        </Button>
      </div>
    {/if}
  {/if}

  {#if estudio.fallo}
    <p class="nb-estudio-fallo" role="alert">{estudio.fallo}</p>
  {/if}

  {#if estudio.resultado && !estudio.generando}
    <div class="nb-estudio-resultado">
      <p class="nb-estudio-marca">
        {texto('generado')}
        {estudio.resultado.providerLabel}
        {#if !marcada && !marcando}
          · <button class="nb-estudio-marcar" type="button" onclick={() => (marcando = true)}>
            {texto('marcarIncorrecta')}
          </button>
        {/if}
      </p>
      {#if marcada}
        <p class="nb-estudio-texto" role="status">{texto('marcarGuardada')}</p>
      {/if}
      {#if marcando}
        <div class="nb-estudio-aviso">
          <Select
            etiqueta={texto('marcarTitulo')}
            valor={motivo}
            opciones={MOTIVOS.map((m) => [m.valor, texto(m.clave)] as const)}
            alCambiar={(v) => (motivo = v as AiReportReason)}
          />
          <Field etiqueta={texto('marcarNota')}>
            {#snippet children({ id })}
              <input {id} class="nb-campo" type="text" bind:value={notaDelAviso} />
            {/snippet}
          </Field>
          <p class="nb-estudio-texto">{texto('marcarAyuda')}</p>
          <div class="nb-estudio-acciones">
            <Button variante="primario" onclick={() => void marcarIncorrecta()}>
              {texto('marcarGuardar')}
            </Button>
            <Button onclick={() => (marcando = false)}>{texto('marcarCancelar')}</Button>
          </div>
        </div>
      {/if}
      {#if estudio.resultado.sampled}
        <p class="nb-estudio-texto">{texto('recortado')}</p>
      {/if}
      {#if estudio.resultado.reading.discarded > 0}
        <p class="nb-estudio-texto">
          {estudio.resultado.reading.discarded}
          {texto(estudio.resultado.reading.discarded === 1 ? 'descartado' : 'descartados')}
        </p>
      {/if}
      {#if estudio.resultado.reading.invalidCitations > 0}
        <p class="nb-estudio-texto">
          {estudio.resultado.reading.invalidCitations}
          {texto(
            estudio.resultado.reading.invalidCitations === 1 ? 'citaInvalida' : 'citasInvalidas'
          )}
        </p>
      {/if}

      {#if estudio.resultado.reading.items.items.length === 0}
        <p class="nb-estudio-texto" role="alert">
          {texto(estudio.resultado.reading.discarded > 0 ? 'nadaUtil' : 'ilegible')}
        </p>
      {:else if estudio.resultado.reading.items.kind === 'resumen'}
        <ol class="nb-estudio-lista">
          {#each estudio.resultado.reading.items.items as punto, i (i)}
            <li class="nb-estudio-punto">
              <span>{punto.text}</span>
              {@render citas(punto.citations)}
            </li>
          {/each}
        </ol>
      {:else}
        <ul class="nb-estudio-lista">
          {#each estudio.tarjetas as tarjeta, i (tarjeta)}
            <li class="nb-estudio-propuesta">
              <Field etiqueta={texto('pregunta')}>
                {#snippet children({ id })}
                  <textarea {id} class="nb-campo" rows="2" bind:value={tarjeta.question}></textarea>
                {/snippet}
              </Field>
              <Field etiqueta={texto('respuesta')}>
                {#snippet children({ id })}
                  <textarea {id} class="nb-campo" rows="2" bind:value={tarjeta.answer}></textarea>
                {/snippet}
              </Field>
              <div class="nb-estudio-fila">
                {@render citas(tarjeta.citations)}
                <Button onclick={() => estudio.tarjetas.splice(i, 1)}>{texto('descartar')}</Button>
              </div>
            </li>
          {/each}
          {#each estudio.preguntas as pregunta, i (pregunta)}
            <li class="nb-estudio-propuesta">
              <Field etiqueta={texto('enunciado')}>
                {#snippet children({ id })}
                  <textarea {id} class="nb-campo" rows="2" bind:value={pregunta.prompt}></textarea>
                {/snippet}
              </Field>
              {#each pregunta.options as _, j (j)}
                <Field etiqueta="{texto('opcion')} {j + 1}">
                  {#snippet children({ id })}
                    <input {id} class="nb-campo" type="text" bind:value={pregunta.options[j]} />
                  {/snippet}
                </Field>
              {/each}
              <Select
                etiqueta={texto('correcta')}
                valor={String(pregunta.correct)}
                opciones={pregunta.options.map(
                  (_, j) => [String(j), `${texto('opcion')} ${j + 1}`] as const
                )}
                alCambiar={(valor) => (pregunta.correct = Number(valor))}
              />
              <Field etiqueta={texto('explicacion')}>
                {#snippet children({ id })}
                  <textarea {id} class="nb-campo" rows="2" bind:value={pregunta.explanation}
                  ></textarea>
                {/snippet}
              </Field>
              <div class="nb-estudio-fila">
                {@render citas(pregunta.citations)}
                <Button onclick={() => estudio.preguntas.splice(i, 1)}>{texto('descartar')}</Button>
              </div>
            </li>
          {/each}
          {#each estudio.conceptos as concepto, i (concepto)}
            <li class="nb-estudio-propuesta">
              <Field etiqueta={texto('nombre')}>
                {#snippet children({ id })}
                  <input {id} class="nb-campo" type="text" bind:value={concepto.name} />
                {/snippet}
              </Field>
              <Select
                etiqueta={texto('clase')}
                valor={concepto.kind}
                opciones={TIPOS.map((tipo) => [tipo, texto(`tipo-${tipo}`)] as const)}
                alCambiar={(valor) => (concepto.kind = valor as ConceptKind)}
              />
              <Field etiqueta={texto('descripcion')}>
                {#snippet children({ id })}
                  <textarea {id} class="nb-campo" rows="2" bind:value={concepto.description}
                  ></textarea>
                {/snippet}
              </Field>
              <div class="nb-estudio-fila">
                {@render citas(concepto.citations)}
                <Button onclick={() => estudio.conceptos.splice(i, 1)}>{texto('descartar')}</Button>
              </div>
            </li>
          {/each}
        </ul>
        {#if estudio.guardado}
          <p class="nb-estudio-texto" role="status">{texto('guardadas')}</p>
        {:else if quedaAlgo}
          <div class="nb-estudio-acciones">
            <Button variante="primario" onclick={() => void guardarTodo()}
              >{texto('guardarTodo')}</Button
            >
          </div>
        {:else}
          <p class="nb-estudio-texto">{texto('nadaQueGuardar')}</p>
        {/if}
      {/if}
    </div>
  {/if}
</section>

<style>
  .nb-estudio {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
  }

  /*
   * Un enlace y no un botón: marcar una salida como incorrecta tiene que estar
   * cuando haga falta y no competir por la atención con "guardar", que es lo que
   * la mayoría va a pulsar.
   */
  .nb-estudio-marcar {
    padding: 0;
    border: 0;
    background: none;
    color: var(--m3c-primary);
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }

  .nb-estudio-aviso-previo {
    margin: 0;
    padding: var(--nb-space-2);
    border-inline-start: 3px solid var(--m3c-tertiary);
    color: var(--m3c-on-surface);
    font-size: var(--nb-text-sm);
    line-height: 1.5;
  }

  .nb-estudio-aviso {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding: var(--nb-space-2);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-md);
  }

  .nb-estudio-quien,
  .nb-estudio-texto,
  .nb-estudio-marca {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-sm);
    line-height: 1.5;
  }

  .nb-estudio-quien {
    padding: var(--nb-space-2) var(--nb-space-3);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container);
  }

  .nb-estudio-quien[data-sale='true'] {
    background: var(--m3c-tertiary-container);
    color: var(--m3c-on-tertiary-container);
  }

  .nb-estudio-marca {
    font-weight: 600;
  }

  .nb-estudio-controles,
  .nb-estudio-acciones,
  .nb-estudio-fila {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--nb-space-2);
  }

  .nb-estudio-fila {
    justify-content: space-between;
  }

  .nb-estudio-fallo {
    margin: 0;
    color: var(--m3c-error);
    font-size: var(--nb-text-sm);
  }

  .nb-estudio-resultado {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
  }

  .nb-estudio-lista {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-3);
    margin: 0;
    padding-inline-start: var(--nb-space-4);
  }

  .nb-estudio-punto {
    line-height: 1.5;
    white-space: pre-wrap;
  }

  .nb-estudio-propuesta {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-2);
    padding: var(--nb-space-3);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-low);
    list-style: none;
  }

  .nb-estudio-citas {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--nb-space-1);
    margin-inline-start: var(--nb-space-2);
  }

  .nb-estudio-cita {
    padding: 0 var(--nb-space-2);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-full);
    background: transparent;
    color: var(--m3c-primary);
    font: inherit;
    font-size: var(--nb-text-xs);
    cursor: pointer;
  }

  .nb-estudio-cita:hover {
    background: var(--m3c-surface-container-high);
  }

  .nb-estudio-sin-cita {
    margin-inline-start: var(--nb-space-2);
    padding: 0 var(--nb-space-2);
    border-radius: var(--nb-radius-full);
    background: var(--m3c-error-container);
    color: var(--m3c-on-error-container);
    font-size: var(--nb-text-xs);
  }
</style>
