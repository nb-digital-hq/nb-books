/**
 * El runtime, simulado dentro del navegador.
 *
 * # Por qué se simula
 *
 * Estas pruebas existen para comprobar que **la interfaz responde**: que una
 * tecla llega, que el foco se mueve, que un panel se abre. Que SQLite escribe ya
 * lo comprueban las 521 pruebas del runtime contra bases de datos reales, y
 * arrastrar una base entera hasta aquí haría la suite lenta y frágil sin
 * comprobar nada nuevo.
 *
 * Lo que sí importa es que el simulador **responda lo mismo que el runtime**:
 * los tipos vienen de `src/lib/bindings`, generados desde Rust, así que un
 * cambio de contrato rompe esto igual que rompería la aplicación.
 */

import { deflateSync } from 'node:zlib';

import type { Page } from '@playwright/test';

/** Obras con las que se puebla la biblioteca simulada. */
const OBRAS = [
  { titulo: 'Cien Años de Soledad', autor: 'Gabriel García Márquez', progreso: 0.42 },
  { titulo: 'Pedro Páramo', autor: 'Juan Rulfo', progreso: null },
  { titulo: 'Rayuela', autor: 'Julio Cortázar', progreso: 0.1 }
];

/**
 * Instala el simulador antes de que la aplicación monte.
 *
 * Va en `addInitScript` y no en una ruta interceptada porque el puente de Tauri
 * no es HTTP: es un objeto que el webview inyecta, y aquí se inyecta igual.
 */
/** Un PNG de un píxel transparente, para que una imagen del libro cargue. */
const PIXEL = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=',
  'base64'
);

/**
 * Una portada **distinta por obra**, del tamaño al que se sirven de verdad.
 *
 * # Por qué no vale una imagen para todas
 *
 * Porque el motor descodifica **por URL**: cincuenta `<img>` apuntando al mismo
 * sitio son una descodificación y un mapa de bits, y cincuenta obras con su
 * portada son cincuenta. Medido el 2026-09-06 en WebKitGTK con cincuenta por
 * página, pasar de página cuesta 2 ms con una portada repetida y unos 100 con
 * cincuenta distintas. Servir una sola aquí no hacía la prueba optimista: la
 * hacía **incapaz de fallar**, y con ella se llegó a concluir que la caché de la
 * interfaz no hacía falta.
 *
 * Son 400x600, que es lo que `covers.rs` genera, así que cada una ocupa 960 KB
 * descodificada y una página de cincuenta pesa lo que pesa de verdad.
 *
 * # Por qué se genera aquí en lugar de traer archivos
 *
 * Porque las portadas reales están en un corpus con derechos vigentes que no
 * entra en el repositorio (`AGENTS.md`, sección 5). Lo que hace falta de ellas es
 * que sean **distintas**, no que sean bonitas.
 */
const ANCHO_DE_PORTADA = 400;
const ALTO_DE_PORTADA = 600;

function crc32(datos: Buffer): number {
  let c = ~0;
  for (const byte of datos) {
    c ^= byte;
    for (let i = 0; i < 8; i += 1) c = (c >>> 1) ^ (0xed_b8_83_20 & -(c & 1));
  }
  return ~c >>> 0;
}

function trozo(tipo: string, datos: Buffer): Buffer {
  const cuerpo = Buffer.concat([Buffer.from(tipo, 'latin1'), datos]);
  const largo = Buffer.alloc(4);
  largo.writeUInt32BE(datos.length);
  const suma = Buffer.alloc(4);
  suma.writeUInt32BE(crc32(cuerpo));
  return Buffer.concat([largo, cuerpo, suma]);
}

const portadasGeneradas = new Map<number, Buffer>();

function portadaDistinta(indice: number): Buffer {
  const guardada = portadasGeneradas.get(indice);
  if (guardada) return guardada;

  // Un degradado que depende del índice: bytes distintos por obra, para que
  // ninguna capa de por medio pueda servir una en lugar de otra.
  const base = (indice * 2_654_435_761) % 0xff_ff_ff;
  const filas: Buffer[] = [];
  for (let y = 0; y < ALTO_DE_PORTADA; y += 1) {
    const fila = Buffer.alloc(1 + ANCHO_DE_PORTADA * 3);
    for (let x = 0; x < ANCHO_DE_PORTADA; x += 1) {
      fila[1 + x * 3] = (base + x) & 0xff;
      fila[2 + x * 3] = ((base >> 8) + y) & 0xff;
      fila[3 + x * 3] = ((base >> 16) + x + y) & 0xff;
    }
    filas.push(fila);
  }

  const cabecera = Buffer.alloc(13);
  cabecera.writeUInt32BE(ANCHO_DE_PORTADA, 0);
  cabecera.writeUInt32BE(ALTO_DE_PORTADA, 4);
  cabecera[8] = 8; // bits por canal
  cabecera[9] = 2; // color verdadero, sin alfa
  const png = Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    trozo('IHDR', cabecera),
    trozo('IDAT', deflateSync(Buffer.concat(filas))),
    trozo('IEND', Buffer.alloc(0))
  ]);
  portadasGeneradas.set(indice, png);
  return png;
}

/**
 * El bus de eventos y los modelos de IA del simulador, en un guion aparte.
 *
 * Aparte y no dentro de `montarPuente` porque no dependen de ninguna opción de
 * montaje, y así se leen solas. **No** es por la complejidad de esa función: su
 * aviso de complejidad ya estaba, y quitarlo no era el motivo.
 *
 * Se carga **antes** que el guion principal, que es quien las usa: los guiones de
 * arranque se ejecutan en el orden en que se registran.
 */
async function simularModelosYEventos(page: Page) {
  await page.addInitScript(() => {
    const global = globalThis as Record<string, unknown>;

    /*
     * Los oyentes del bus de eventos, para poder **emitir** desde una prueba.
     *
     * Antes el simulador contestaba a `listen` y tiraba el oyente, así que
     * ningún recorrido podía comprobar nada que llegara por evento: el avance de
     * una importación, el de una descarga. `transformCallback` devuelve el propio
     * cierre, así que lo que llega aquí es la función que hay que llamar.
     */
    type Oyente = (evento: { payload: unknown }) => void;
    const oyentes: Array<{ evento: string; oyente: Oyente }> = [];
    global.__nbEscuchar = (args: { event: string; handler: Oyente }) =>
      oyentes.push({ evento: args.event, oyente: args.handler });
    const emitir = (evento: string, payload: unknown) => {
      for (const o of oyentes.filter((o) => o.evento === evento)) o.oyente({ payload });
    };
    global.__nbEmitir = emitir;

    /*
     * Los modelos de IA. La descarga simula lo que hace la de verdad
     * contra Hugging Face: el primer anfitrión se autoriza al confirmar, y la
     * redirección a la red de distribución **pide su propio permiso** antes de
     * traer nada.
     */
    const RED_DE_DISTRIBUCION = 'cdn.ejemplo';
    const permisos = new Set<string>();
    let estado: 'available' | 'installed' = 'available';
    const terminar = (modelId: unknown) => {
      if (!permisos.has(RED_DE_DISTRIBUCION)) {
        emitir('AiModelFinished', {
          modelId,
          outcome: 'needsConsent',
          host: RED_DE_DISTRIBUCION,
          reasonCode: null
        });
        return;
      }
      emitir('AiModelProgress', { modelId, done: 100, total: 223_082_311 });
      estado = 'installed';
      emitir('AiModelFinished', { modelId, outcome: 'installed', host: null, reasonCode: null });
    };

    /*
     * El presupuesto de la IA, que el simulador de `update_setting`
     * apunta aquí. El tope de memoria decide si el modelo cabe, igual que en el
     * runtime: con 512 MB, el de 768 no.
     */
    const presupuesto = { hilos: 0, memoria: 1024 };
    global.__nbPresupuestoDeIa = presupuesto;

    global.__nbRespuestasDeModelos = {
      ai_models: () => ({
        machineSupported: true,
        machineMemoryBytes: 16 * 1024 ** 3,
        installing: null,
        cores: 8,
        automaticThreads: 4,
        memoryLimitBytes: presupuesto.memoria * 1024 ** 2,
        backgroundPause: null,
        accelerator: 'cpu',
        fastBuild: true,
        models: [
          {
            id: 'granite-embedding-107m-multilingual',
            name: 'Granite Embedding 107M Multilingual',
            author: 'IBM',
            license: 'Apache-2.0',
            languages: ['en', 'de', 'es', 'fr', 'ja', 'pt', 'ar', 'cs', 'it', 'ko', 'nl', 'zh'],
            diskBytes: 223_082_311,
            memoryBytes: 768 * 1024 ** 2,
            status: estado,
            host: 'huggingface.co',
            whatIsSent:
              'nada de tu biblioteca; solo se pide el archivo del modelo, y quien lo sirve sabe que lo descargas',
            fits: true,
            freeBytes: 100 * 1024 ** 3,
            exceedsMemoryLimit: 768 + 26 > presupuesto.memoria,
            generates: false,
            traitMeasured: 'vectores',
            recommended: false,
            inUse: false,
            needsMemoryBytes: 4 * 1024 ** 3,
            needsCores: 2,
            slowBuild: false,
            estimatedSeconds: null,
            warnings: [],
            suggestedLimitMb: 768 + 26 > presupuesto.memoria ? 1024 : null
          },
          // Un modelo que **genera**, para el recorrido de la recomendación
          // por máquina: el catálogo lleva varios y la aplicación señala uno.
          {
            id: 'qwen25-15b-instruct-q4km',
            name: 'Qwen2.5 1.5B Instruct',
            author: 'Qwen (Alibaba Cloud)',
            license: 'Apache-2.0',
            languages: ['es', 'en', 'pt'],
            diskBytes: 1_117_320_736,
            memoryBytes: 1_920 * 1024 ** 2,
            status: 'installed',
            host: 'huggingface.co',
            whatIsSent:
              'nada de tu biblioteca; solo se pide el archivo del modelo, y quien lo sirve sabe que lo descargas',
            fits: true,
            freeBytes: 100 * 1024 ** 3,
            exceedsMemoryLimit: 1_920 + 96 > presupuesto.memoria,
            generates: true,
            traitMeasured: 'fiel',
            recommended: true,
            inUse: (global.__nbModeloLocal as string) === 'qwen25-15b-instruct-q4km',
            needsMemoryBytes: 6 * 1024 ** 3,
            needsCores: 4,
            slowBuild: false,
            estimatedSeconds: 120,
            warnings: [],
            suggestedLimitMb: 1_920 + 96 > presupuesto.memoria ? 2048 : null
          },
          // Uno que este equipo **no** debería elegir: pide más memoria de la que
          // tiene. Ahora se ofrece igual, con la confirmación fuerte,
          // y el recorrido comprueba que no se elige sin ella.
          {
            id: 'qwen3-4b-q4km',
            name: 'Qwen3 4B',
            author: 'Qwen (Alibaba Cloud)',
            license: 'Apache-2.0',
            languages: ['es', 'en', 'pt'],
            diskBytes: 2_500_000_000,
            memoryBytes: 4_032 * 1024 ** 2,
            status: 'installed',
            host: 'huggingface.co',
            whatIsSent:
              'nada de tu biblioteca; solo se pide el archivo del modelo, y quien lo sirve sabe que lo descargas',
            fits: true,
            freeBytes: 100 * 1024 ** 3,
            exceedsMemoryLimit: 4_032 + 672 > presupuesto.memoria,
            generates: true,
            traitMeasured: 'prosa',
            recommended: false,
            inUse: (global.__nbModeloLocal as string) === 'qwen3-4b-q4km',
            needsMemoryBytes: 12 * 1024 ** 3,
            needsCores: 8,
            slowBuild: false,
            estimatedSeconds: 300,
            warnings: ['EXCEEDS_MACHINE_MEMORY', 'BELOW_RECOMMENDED', 'CITES_WORSE'],
            suggestedLimitMb: 4_032 + 672 > presupuesto.memoria ? 8192 : null
          }
        ]
      }),
      // La voz neuronal se instala por la misma puerta que los modelos, y
      // por eso su respuesta vive aquí al lado. De fábrica **no está descargada**,
      // que es lo que se encuentra quien acaba de instalar la aplicación.
      neural_voice: () => ({
        id: 'kokoro-82m',
        name: 'Kokoro 82M',
        author: 'hexgrad',
        licence: 'Apache-2.0',
        host: 'huggingface.co',
        downloadBytes: 327_212_226,
        memoryBytes: 600 * 1024 ** 2,
        // `instalada` narra en vivo; `lenta` está instalada y el equipo no llega, que
        // ahora se ofrece igual avisando de que se cortará.
        installed: ['instalada', 'lenta'].includes(global.__nbVozNeuronal as string),
        ready: (global.__nbVozNeuronal as string) === 'instalada',
        reason:
          (global.__nbVozNeuronal as string) === 'instalada'
            ? null
            : (global.__nbVozNeuronal as string) === 'lenta'
              ? 'DEMASIADO_LENTA'
              : 'NO_INSTALADA',
        factor:
          (global.__nbVozNeuronal as string) === 'instalada'
            ? 0.42
            : (global.__nbVozNeuronal as string) === 'lenta'
              ? 1.67
              : null,
        voices: [
          { id: 'ef_dora', name: 'Dora', language: 'es' },
          { id: 'em_alex', name: 'Alex', language: 'es' },
          { id: 'em_santa', name: 'Santa', language: 'es' }
        ]
      }),
      allow_model_host: (args: { host: string }) => {
        permisos.add(args.host);
        return null;
      },
      install_ai_model: (args: { id: string }) => {
        setTimeout(() => terminar(args.id), 50);
        return false;
      },
      cancel_ai_model_install: () => null,
      uninstall_ai_model: () => {
        estado = 'available';
        return null;
      }
    };
  });
}

/*
 * Las funciones de estudio con IA. De fábrica **no hay quien genere**, como
 * en una instalación recién hecha, y el lector no ofrece estudiar; una prueba lo
 * enciende con `__nbEstudio`. Lo generado trae a propósito un marcado con código y
 * un punto sin citas: es lo que el panel tiene que enseñar como texto y como no
 * verificable.
 */
async function simularEstudioConIa(page: Page) {
  await page.addInitScript(() => {
    const global = globalThis as Record<string, unknown>;
    const estado = () =>
      (global.__nbEstudio as { listo: boolean; externo: boolean; autorizado: boolean }) ?? {
        listo: false,
        externo: false,
        autorizado: false
      };
    const fuente = (href: string) => ({
      version: 1,
      href,
      mediaType: 'application/xhtml+xml',
      locations: { fragments: ['#char=0'], progression: 0, totalProgression: 0 },
      text: { highlight: 'Muchos años después' }
    });
    const guardadas: Array<Record<string, unknown>> = [];

    Object.assign(global.__nbRespuestasDeModelos as Record<string, unknown>, {
      study_availability: () => ({
        generator: estado().externo ? 'external' : 'local',
        ready: estado().listo,
        reason: estado().listo ? null : 'AI_DISABLED',
        providerLabel: estado().externo ? 'un-modelo en ia.example' : 'Qwen2.5 1.5B Instruct',
        leftDevice: estado().externo,
        host: estado().externo ? 'ia.example' : null,
        authorized: estado().autorizado,
        whatIsSent: 'los fragmentos del documento que la función necesite',
        // Las cuatro con los dos proveedores ahora. Con el local, las
        // preguntas **se ofrecen desaconsejadas**: medido, salen con la clave en la
        // opción equivocada, y eso es lo que se dice antes de generarlas.
        functions: ['summary', 'flashcards', 'questions', 'concepts'],
        discouraged: estado().externo
          ? []
          : [{ function: 'questions', reason: 'LOCAL_QUESTIONS_UNRELIABLE' }],
        estimatedSeconds: estado().externo ? null : 120,
        warnings: []
      }),
      estimate_study_scope: (args: { scope?: Record<string, unknown> }) => {
        const text = (args?.scope?.text as string) ?? '';
        const chars = text.length > 0 ? text.length : 3200;
        return {
          characters: chars,
          approximateTokens: Math.ceil(chars / 4),
          sampled: false
        };
      },
      generate_study: (args: { function: string; scope: Record<string, unknown> }) => {
        if (estado().externo && !estado().autorizado) throw { code: 'CONSENT_REQUIRED' };
        const comun = {
          function: args.function,
          editionId: args.scope.editionId,
          templateVersion: 1,
          providerId: estado().externo ? 'external' : 'local-generativo',
          providerLabel: estado().externo ? 'un-modelo en ia.example' : 'Qwen2.5 1.5B Instruct',
          leftDevice: estado().externo,
          sources: [
            {
              locator: fuente('OEBPS/c1.xhtml'),
              excerpt: 'Muchos años después, frente al pelotón'
            },
            { locator: fuente('OEBPS/c2.xhtml'), excerpt: 'Macondo era entonces una aldea' }
          ],
          sampled: false
        };
        if (args.function === 'flashcards') {
          return {
            ...comun,
            templateId: 'estudio.tarjetas',
            reading: {
              discarded: 1,
              invalidCitations: 0,
              items: {
                kind: 'tarjetas',
                items: [
                  { question: '¿Qué recordó el coronel?', answer: 'El hielo', citations: [0] },
                  { question: '¿Cómo era Macondo?', answer: 'Una aldea', citations: [1] }
                ]
              }
            }
          };
        }
        return {
          ...comun,
          templateId: 'estudio.resumen',
          reading: {
            discarded: 0,
            invalidCitations: 2,
            items: {
              kind: 'resumen',
              items: [
                { text: 'El coronel recuerda el hielo.', citations: [0, 1] },
                { text: '<img src=x onerror="window.__nbInyectado=1">', citations: [1] },
                { text: 'Una afirmación sin fuente.', citations: [] }
              ]
            }
          }
        };
      },
      cancel_study: () => null,
      save_study_cards: (args: { cards: Array<Record<string, unknown>> }) =>
        args.cards.map((c, i) => {
          const id = `t${guardadas.length + i}`;
          guardadas.push({
            ...c,
            id,
            title: 'Cien Años de Soledad',
            fileId: 'f0',
            template: `${String(c.templateId)}@${String(c.templateVersion)}`,
            editedAt: null,
            createdAt: '2026-09-17T10:00:00.000Z',
            dueAt: '2026-09-17T10:00:00.000Z',
            intervalDays: 0,
            easeFactor: 2.5,
            repetitions: 0,
            lastReviewedAt: null
          });
          return id;
        }),
      study_cards: (args?: { dueOnly?: boolean }) => {
        const todas = [...guardadas].reverse();
        if (args?.dueOnly) {
          const ahora = new Date().toISOString();
          return todas.filter((t) => (t.dueAt as string) <= ahora);
        }
        return todas;
      },
      review_study_card: (args: { id: string; rating: string }) => {
        const tarjeta = guardadas.find((g) => g.id === args.id);
        if (tarjeta) {
          tarjeta.repetitions = ((tarjeta.repetitions as number) || 0) + 1;
          tarjeta.intervalDays =
            args.rating === 'again' ? 1 : Math.max(1, ((tarjeta.intervalDays as number) || 0) * 2);
          tarjeta.lastReviewedAt = new Date().toISOString();
          const d = new Date();
          d.setDate(d.getDate() + (tarjeta.intervalDays as number));
          tarjeta.dueAt = d.toISOString();
          return tarjeta;
        }
        return null;
      },
      study_card_counts: () => {
        const ahora = new Date().toISOString();
        const due = guardadas.filter((t) => (t.dueAt as string) <= ahora).length;
        return { total: guardadas.length, due };
      },
      edit_study_card: () => null,
      delete_study_card: (args: { id: string }) => {
        const i = guardadas.findIndex((g) => g.id === args.id);
        if (i >= 0) guardadas.splice(i, 1);
        return null;
      },
      save_concepts: () => [],
      concepts: () => [],
      edit_concept: () => null,
      delete_concept: () => null
    });
    // Con `lento`, generar tarda: es lo que pasa con el modelo local -entre uno y
    // seis minutos, medido- y lo que hace falta para cerrar el panel a mitad.
    {
      const respuestas = global.__nbRespuestasDeModelos as Record<
        string,
        (args: { function: string; scope: Record<string, unknown> }) => unknown
      >;
      const inmediato = respuestas.generate_study;
      respuestas.generate_study = (args) => {
        const generado = inmediato?.(args);
        return (estado() as { lento?: boolean }).lento
          ? new Promise((resolver) => setTimeout(() => resolver(generado), 1200))
          : generado;
      };
    }
  });
}

/*
 * El proveedor de IA externo, con el mismo contrato que el runtime: la
 * primera prueba **no envía nada** y devuelve el mensaje que enviaría; solo tras
 * autorizar sale, y lo que sale queda en el registro tal cual. Se declara aparte
 * de `simularModelosYEventos` para que su complejidad no se sume a la de aquella.
 */
/*
 * La preparación de la búsqueda por significado, con el contrato del
 * runtime: estimar no prepara nada; empezar emite el avance y el final, y después
 * la estimación dice que no queda nada pendiente.
 */
async function simularPreparacionPorSignificado(page: Page) {
  await page.addInitScript(() => {
    const global = globalThis as Record<string, unknown>;
    const emitir = global.__nbEmitir as (evento: string, payload: unknown) => void;
    const preparados = new Set<string>();
    const excluidos = new Set<string>();
    const FRAGMENTOS = 240;
    const clave = (alcance: Record<string, string>) =>
      alcance.scope === 'document' ? alcance.editionId : alcance.collectionId;

    Object.assign(global.__nbRespuestasDeModelos as Record<string, unknown>, {
      estimate_vectors: (args: { scope: Record<string, string> }) => {
        const listo = preparados.has(clave(args.scope));
        return {
          documents: 1,
          excluded: 0,
          notIndexed: 0,
          fragments: FRAGMENTOS,
          pending: listo ? 0 : FRAGMENTOS,
          bytes: listo ? 0 : FRAGMENTOS * 620,
          seconds: listo ? 0 : FRAGMENTOS * 0.6,
          measuredRate: false,
          fits: true,
          modelReady: true
        };
      },
      start_vectors: (args: { scope: Record<string, string> }) => {
        const id = clave(args.scope);
        setTimeout(() => {
          emitir('VectorJobProgress', {
            editionId: id,
            done: 120,
            total: FRAGMENTOS,
            queued: 0,
            pauseReason: null
          });
        }, 50);
        setTimeout(() => {
          preparados.add(id);
          emitir('VectorJobFinished', { outcome: 'done', reasonCode: null });
        }, 400);
        return null;
      },
      pause_vectors: () => null,
      cancel_vectors: () => null,
      vector_job: () => ({
        running: false,
        paused: false,
        pauseReason: null,
        editionId: null,
        done: 0,
        total: 0,
        queued: 0
      }),
      reader_activity: () => null,
      vector_exclusion: (args: { id: string }) => excluidos.has(args.id),
      set_vector_exclusion: (args: { id: string; excluded: boolean }) => {
        if (args.excluded) excluidos.add(args.id);
        else excluidos.delete(args.id);
        return null;
      }
    });
  });
}

/** Un WAV de silencio: PCM de 16 bits, mono, 8 kHz. */
function silencioWav(segundos: number): Buffer {
  const muestras = 8000 * segundos;
  const datos = muestras * 2;
  const wav = Buffer.alloc(44 + datos);
  wav.write('RIFF', 0);
  wav.writeUInt32LE(36 + datos, 4);
  wav.write('WAVE', 8);
  wav.write('fmt ', 12);
  wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20);
  wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(8000, 24);
  wav.writeUInt32LE(16000, 28);
  wav.writeUInt16LE(2, 32);
  wav.writeUInt16LE(16, 34);
  wav.write('data', 36);
  wav.writeUInt32LE(datos, 40);
  return wav;
}

/**
 * Preparar audio. **Antes el simulador no tenía
 * ninguno de estos comandos**, así que ningún recorrido abría el panel de preparar
 * el capítulo, y por eso nadie vio que solo aparecía después de abrir Estudiar con
 * IA. Una prueba lo fija con `__nbAudio`: `{ capitulo, libro, falla }`.
 *
 * El audio que sirve es un WAV de silencio de tres segundos hecho aquí, con tres
 * marcas de un segundo en tres recursos distintos: lo que hay que comprobar es que
 * el lector siga el texto, y para eso basta con que el tiempo avance.
 */
async function simularAudioPreparado(page: Page) {
  await page.addInitScript(() => {
    const global = globalThis as Record<string, unknown>;
    const estado = () =>
      (global.__nbAudio as { capitulo?: boolean; libro?: boolean; falla?: string } | undefined) ??
      {};
    const emitir = global.__nbEmitir as (evento: string, payload: unknown) => void;
    const marca = (href: string, desde: number) => ({
      startMs: desde,
      endMs: desde + 1000,
      locator: {
        version: 1,
        href,
        mediaType: 'application/xhtml+xml',
        locations: { fragments: ['#char=0'], progression: 0, totalProgression: 0 },
        text: { highlight: '' }
      }
    });
    const preparado = (titulo: string) => {
      const cfg = global.__nbAudio as
        | { voz?: string; modelRev?: string; matchesCurrent?: boolean }
        | undefined;
      return {
        editionId: 'e0',
        title: titulo,
        voice: cfg?.voz ?? 'ef_dora',
        modelRev: cfg?.modelRev ?? 'rev-1',
        matchesCurrent: cfg?.matchesCurrent ?? true,
        path: '/audio/preparado.wav',
        durationMs: 3000,
        bytes: 48_000,
        status: 'ready',
        marks: [
          marca('OEBPS/c1.xhtml', 0),
          marca('OEBPS/c2.xhtml', 1000),
          marca('OEBPS/c1.xhtml', 2000)
        ]
      };
    };
    const alcanceValido = (scope: { kind: string }) => {
      if (scope.kind !== 'chapter' && scope.kind !== 'book') {
        throw `invalid args \`scope\`: unknown variant \`${scope.kind}\`, expected \`chapter\` or \`book\``;
      }
      return {};
    };
    Object.assign(global.__nbRespuestasDeModelos as Record<string, unknown>, {
      // El simulador valida el alcance como lo valida serde en el runtime. Antes
      // aceptaba `Book`, que el runtime rechaza, y por eso el audio preparado
      // estuvo roto en el binario con estas pruebas en verde.
      estimate_audio: (args: { scope: { kind: string } }) => ({
        ...alcanceValido(args.scope),
        fragments: 12,
        characters: 9000,
        audioSeconds: 530,
        bytes: 1_600_000,
        workSeconds: 880,
        already: false,
        fits: true,
        freeBytes: 900_000_000
      }),
      prepared_audio: (args: { scope: { kind: string } }) => {
        alcanceValido(args.scope);
        return (args.scope.kind === 'book' ? estado().libro : estado().capitulo)
          ? preparado(args.scope.kind === 'book' ? 'El libro' : 'El capítulo')
          : null;
      },
      audio_job: () => null,
      audio_queue: () => [],
      start_audio: () => {
        setTimeout(() => {
          emitir('AudioJobProgress', {
            editionId: 'e0',
            done: 1,
            total: 12,
            remainingMs: null,
            seconds: 20,
            queued: 0
          });
          const falla = estado().falla;
          emitir('AudioJobFinished', {
            editionId: 'e0',
            outcome: falla ? 'failed' : 'done',
            reasonCode: falla ?? null
          });
        }, 60);
        return null;
      },
      sample_audio: () => '/audio/muestra.wav',
      cancel_audio: () => null,
      forget_audio: () => null,
      start_audio_collection: () => 2,
      // La narración en vivo con la voz neuronal: cada tramo, un archivo.
      synthesize_neural_speech: () => '/audio/tramo.wav'
    });
  });
}

async function simularProveedorExterno(page: Page) {
  await page.addInitScript(() => {
    const global = globalThis as Record<string, unknown>;
    const proveedor = { protocol: 'none', host: '', model: '', autorizado: false, clave: false };
    global.__nbProveedorDeIa = proveedor;
    const envios: Array<Record<string, unknown>> = [];
    /** Lo marcado como incorrecto, que el simulador guarda de verdad. */
    const reportes: Array<Record<string, unknown>> = [];
    /*
     * Los cuatro comandos de los avisos, en una función **dentro** del script que
     * se inyecta: una de fuera no viaja al navegador -se serializa el cuerpo de
     * este callback y nada más-, y los recorridos se llenaron de "no está
     * definida". Aquí dentro sirve para lo que hacía falta, que era no pasar del
     * límite de complejidad del registro de comandos.
     */
    const comandosDeReportes = () => ({
      report_ai_output: (args: { report: Record<string, unknown> }) => {
        reportes.unshift({
          id: reportes.length + 1,
          createdAt: '2026-09-19T10:00:00.000Z',
          function: String(args.report.function),
          templateId: String(args.report.templateId),
          templateVersion: Number(args.report.templateVersion),
          providerId: String(args.report.providerId),
          providerLabel: String(args.report.providerLabel),
          leftDevice: Boolean(args.report.leftDevice),
          editionId: args.report.editionId as string | null,
          reason: String(args.report.reason),
          note: (args.report.note as string) ?? '',
          output: String(args.report.output)
        });
        return reportes.length;
      },
      ai_reports: () => reportes,
      forget_ai_report: (args: { id: number }) => {
        const donde = reportes.findIndex((r) => r.id === args.id);
        if (donde >= 0) reportes.splice(donde, 1);
        return donde >= 0;
      },
      clear_ai_reports: () => {
        const cuantos = reportes.length;
        reportes.length = 0;
        return cuantos;
      },
      save_ai_reports: () => null
    });
    const QUE_SALE =
      'los fragmentos del documento que la función necesite y lo que pidas, nunca el documento entero; quien lo recibe puede guardarlo según su propia política';
    const configurado = () =>
      proveedor.protocol !== 'none' && proveedor.host !== '' && proveedor.model !== '';
    const cuerpo = () =>
      JSON.stringify(
        {
          model: proveedor.model,
          max_tokens: 2048,
          system:
            'Esto es una prueba de conexión de NB Books. Responde únicamente con la palabra: listo.',
          messages: [{ role: 'user', content: 'Sin fragmentos del documento.' }]
        },
        null,
        2
      );

    Object.assign(global.__nbRespuestasDeModelos as Record<string, unknown>, {
      ai_provider_status: () => ({
        protocol: proveedor.protocol,
        configured: configurado(),
        host: proveedor.host || null,
        endpoint: configurado() ? `https://${proveedor.host}/v1/messages` : null,
        model: proveedor.model,
        invalidAddress: false,
        authorized: proveedor.autorizado,
        keyringAvailable: true,
        credentialStored: proveedor.clave,
        whatIsSent: QUE_SALE
      }),
      probe_ai_provider: () => {
        if (!configurado()) return { outcome: 'notConfigured' };
        if (!proveedor.autorizado) {
          return {
            outcome: 'needsConsent',
            host: proveedor.host,
            whatIsSent: QUE_SALE,
            body: cuerpo()
          };
        }
        envios.unshift({
          id: envios.length + 1,
          sentAt: '2026-09-16T12:00:00.000Z',
          protocol: proveedor.protocol,
          host: proveedor.host,
          model: proveedor.model,
          capability: 'summary',
          body: cuerpo(),
          bytes: cuerpo().length,
          status: 200
        });
        return { outcome: 'answered', host: proveedor.host, model: proveedor.model, text: 'listo' };
      },
      allow_ai_provider: () => {
        proveedor.autorizado = true;
        return null;
      },
      set_ai_credential: () => {
        proveedor.clave = true;
        return null;
      },
      clear_ai_credential: () => {
        proveedor.clave = false;
        return null;
      },
      ai_sends: () => envios,
      clear_ai_sends: () => {
        const cuantos = envios.length;
        envios.length = 0;
        return cuantos;
      },
      // Lo marcado como incorrecto, que el simulador guarda de verdad para
      // que el recorrido pueda comprobar que **lo marcado aparece**: un botón que
      // anota en una tabla que nadie enseña es lo mismo que ningún botón.
      ...comandosDeReportes()
    });
  });
}

export async function montarPuente(
  page: Page,
  opciones: {
    /** Comandos que el simulador rechaza, para probar los modos degradados. */
    rechaza?: string[];
    /** Biblioteca sin ninguna obra, para probar los estados vacíos. */
    vacia?: boolean;
    /**
     * El tablero devuelve sus cuarenta y ocho celdas ocupadas.
     *
     * Es lo que se cuenta contra el presupuesto de nodos: un tablero a medias
     * no diría nada sobre el caso que importa, que es el lleno.
     */
    tableroLleno?: boolean;
    /** La biblioteca no se pudo abrir y se arrancó recuperando. */
    recuperada?: boolean;
    /**
     * Las estanterías contestan **al revés de como se piden**: cuanto más corta
     * la búsqueda, más tarda.
     *
     * Es el defecto de la regla 3 del contrato de carga, reproducido a
     * propósito. Al teclear se dispara una petición por pulsación, y con la
     * biblioteca real unas tardan segundos y otras milisegundos, así que la de
     * dos letras contestando después de la de seis no es improbable: es lo
     * normal. Sin guarda, lo que se queda en pantalla es el resultado de algo
     * que ya nadie escribió.
     */
    estanteriasDesordenadas?: boolean;
    /**
     * Ninguna edición trae portada, así que se pinta la generada.
     *
     * Existe porque el banco **siempre** servía una portada por obra -que es lo
     * que arregló J8- y con eso la portada generada no se ejercitaba nunca en la
     * suite, aunque sea lo que ve la mayoría de una biblioteca real: el
     * 2026-09-12, con el corpus entero dentro, 982 obras de 152.080 no tenían
     * imagen y esas son las que la usan. Un banco que no puede enseñar la mitad
     * de lo que se ve no sirve para juzgarla.
     */
    sinPortadas?: boolean;
    /**
     * Hay un catálogo OPDS añadido.
     *
     * De fábrica no hay ninguno, y eso es lo que hace que Descubrir no se
     * anuncie. Con esto se prueba el destino cuando sí existe.
     */
    conCatalogo?: boolean;
    /**
     * Hay una fuente de metadatos aportada por una extensión.
     *
     * Sin esto, `search_online_metadata` rechaza con `CONSENT_REQUIRED`, que es
     * el estado de partida. Con esto responde dos fichas: una del catálogo del
     * núcleo y otra de una extensión, que es lo que hace falta para comprobar
     * que la procedencia se ve.
     */
    conFuenteDeExtension?: boolean;
    /**
     * El catálogo está autorizado y responde con feeds navegables.
     *
     * Sin esto, `browse_catalog` rechaza siempre con `CONSENT_REQUIRED`, que es
     * el estado de partida y el que hace falta para probar el aviso. Con esto se
     * puede recorrer el catálogo, que es lo otro que hay que probar.
     */
    catalogoAbierto?: boolean;
    /**
     * Cuántas obras tiene la biblioteca, en lugar de las tres de siempre.
     *
     * Existe para poder comprobar la virtualización contra un catálogo del
     * tamaño del real. Con tres obras, una lista que pinta todo y una que pinta
     * una ventana se ven exactamente igual, y el presupuesto de nodos se puede
     * incumplir sin que ninguna prueba lo note.
     */
    obras?: number;
    /**
     * Milisegundos que tarda **cada** respuesta del runtime.
     *
     * Es el escenario 2 de `chaos.md`: latencia de disco. Lo que se comprueba
     * con esto no es que la interfaz sea rápida, sino que el puente es de verdad
     * asíncrono: con un puente síncrono la ventana se congela y no hay estado de
     * carga que enseñar.
     */
    lento?: number;
    /**
     * Hay una importación que quedó a medias.
     *
     * De fábrica no: el estado normal es que no haya ninguna, y ofrecer continuar
     * algo que no existe sería exactamente el aviso que la gente aprende a
     * ignorar. Lo enciende el recorrido que comprueba que se ofrece.
     */
    aMedias?: boolean;
    /**
     * El documento abierto es un PDF, con su capa de texto.
     *
     * Existe porque hasta entonces **el camino de PDF no lo
     * ejercitaba nada**: `text_layer` y `page_size` lanzaban formato no
     * soportado, así que ninguna prueba podía tocar la mitad del producto que
     * lee documentos paginados.
     */
    pdf?: boolean;
    /**
     * Cuántos párrafos tiene el capítulo, en lugar de los tres de siempre.
     *
     * Existe ahora para medir lo que crece con el capítulo: mover el
     * cursor rehacía trabajo sobre el capítulo entero con cada flecha, y con tres
     * párrafos eso no se puede ver. Los tres de siempre van delante, así que las
     * anotaciones siguen cayendo donde caen.
     */
    parrafos?: number;
    /**
     * El marcado del capítulo, en lugar del de siempre.
     *
     * Para lo que tiene que pintarse tal y como lo deja el saneador del runtime -una
     * fórmula, una ilustración vectorial-: la prueba lo lee de la salida esperada que
     * fija `src-tauri/tests/epub_lector.rs`, así que lo que se pinta aquí es lo que
     * el runtime entrega y no una copia escrita a mano que pueda quedarse atrás.
     */
    capitulo?: string;
    /**
     * Lo que el capítulo trae y el saneador quitó, como lo dice el runtime
     *: `audio`, `video`, `embedded`, `form`.
     */
    omitidos?: string[];
    /**
     * El libro trae su propia narración: una superposición de EPUB 3 para
     * el primer capítulo, con un tramo de un segundo por párrafo.
     */
    narrado?: boolean;
    /**
     * El capítulo entero dentro de una cita, como los diarios de algunas
     * ediciones: un bloque anclable que envuelve a todos los demás.
     */
    envuelto?: boolean;
    /**
     * Cuántos resaltados más, repartidos por el libro fuera del capítulo que se lee
     *: para comprobar que el mapa no crece con lo anotado.
     */
    anotacionesExtra?: number;
    /** El runtime espera la señal de la prueba antes de estar listo. */
    runtimeEnEspera?: boolean;
    /**
     * Buscar dentro de los libros tarda un segundo y medio, y buscar por título
     * contesta enseguida.
     *
     * Es la proporción que se midió sobre la base real el 2026-09-12 (entre 30 y
     * 180 ms contra entre 0,45 y 1,7 s), y es la que destapó que cambiar de
     * alcance seguía enseñando los resultados del anterior: con las dos al mismo
     * ritmo, el hueco en el que se veía lo que no tocaba no llegaba a existir.
     */
    busquedaLenta?: boolean;
    /**
     * El índice se paró por el tope de disco, con las cifras de la instalación
     * real del 2026-09-12: 28.552 de 152.080 obras buscables y lo derivado en el
     * 20 % de lo que ocupan los libros.
     */
    indiceSinSitio?: boolean;
    /**
     * Cuántas de las primeras obras son posibles duplicados: el recuento
     * del filtro y lo que trae al pulsarlo.
     */
    duplicados?: number;
  } = {}
): Promise<void> {
  // El anfitrión de los assets. En la aplicación de verdad lo atiende el
  // protocolo de Tauri contra el archivo de la caché; aquí lo que importa es que
  // la imagen **se pida ahí** y no al servidor de la aplicación.
  //
  // Una portada `pN.png` devuelve **la portada N y no otra**: es lo que hace que
  // una página de la biblioteca cueste lo que cuesta. Lo demás sigue siendo un
  // píxel, porque no es una portada y su tamaño no significa nada.
  await page.route('http://asset.localhost/**', (ruta) => {
    // El audio preparado y la muestra: tres segundos de silencio en WAV, hechos
    // aquí. Sin audio que avance no se puede comprobar que el lector siga el texto.
    if (/\.wav(?:[?#]|$)/.test(decodeURIComponent(ruta.request().url()))) {
      ruta.fulfill({ status: 200, contentType: 'audio/wav', body: silencioWav(3) });
      return;
    }
    const portada = /\/p(\d+)\.png(?:[?#]|$)/.exec(ruta.request().url());
    ruta.fulfill({
      status: 200,
      contentType: 'image/png',
      body: portada ? portadaDistinta(Number(portada[1])) : PIXEL
    });
  });

  await simularModelosYEventos(page);
  await simularProveedorExterno(page);
  await simularPreparacionPorSignificado(page);
  await simularEstudioConIa(page);
  await simularAudioPreparado(page);

  await page.addInitScript(
    ({
      obras,
      cuantas,
      rechaza,
      lento,
      recuperada,
      conCatalogo,
      conFuenteDeExtension,
      catalogoAbierto,
      aMedias,
      pdf,
      parrafos,
      capitulo,
      omitidos,
      narrado,
      envuelto,
      anotacionesExtra,
      runtimeEnEspera,
      tableroLleno,
      estanteriasDesordenadas,
      sinPortadas,
      busquedaLenta,
      indiceSinSitio,
      duplicados
    }) => {
      /*
       * Las obras generadas se añaden detrás de las de siempre, no las
       * sustituyen: las demás pruebas buscan "Cien Años de Soledad" por su
       * nombre, y una biblioteca grande tiene que seguir siendo la misma
       * biblioteca con más cosas dentro.
       */
      const generadas = Array.from({ length: Math.max(0, cuantas - obras.length) }, (_, i) => ({
        titulo: `Obra Generada ${i + 1}`,
        autor: `Autor ${i % 500}`,
        progreso: null as number | null
      }));

      /**
       * Lo que un ajuste de IA cambia en el simulador.
       *
       * Aparte de `update_setting` porque son cinco ramas de un solo asunto, y
       * dentro hacían de aquella función una de las dos más enrevesadas del
       * archivo. El runtime hace lo mismo: cambiar el tope suelta el modelo,
       * cambiar el elegido suelta el cargado, y cambiar la dirección reconfigura
       * el proveedor externo.
       */
      function anotarAjusteDeIa(clave: string, valor: unknown) {
        const global = globalThis as Record<string, unknown>;
        const presupuesto = global.__nbPresupuestoDeIa as
          | { hilos: number; memoria: number }
          | undefined;
        if (presupuesto && clave === 'ai_threads') presupuesto.hilos = Number(valor);
        if (presupuesto && clave === 'ai_memory_limit_mb') presupuesto.memoria = Number(valor);
        if (clave === 'ai_model_local') global.__nbModeloLocal = String(valor);
        const proveedor = global.__nbProveedorDeIa as Record<string, unknown> | undefined;
        const campo = { ai_provider: 'protocol', ai_host: 'host', ai_model: 'model' }[clave];
        if (proveedor && campo) proveedor[campo] = String(valor);
      }

      /** Si la raíz de biblioteca ya se reapuntó en esta sesión. */
      let mudada = false;

      /** Si hay una importación que quedó a medias, para el recorrido. */
      let importacionAMedias = aMedias;

      /*
       * Cada obra con **su** portada, no con la de todas.
       *
       * Sin esto ninguna edición traía `coverPath`, así que los recorridos
       * pintaban la portada generada por CSS y no cargaban ni una imagen: el
       * coste de descodificar portadas distintas no podía aparecer en la suite
       * ni queriendo. Es lo que dejó pasar el defecto que J8 buscaba.
       */
      /*
       * Mutables: el simulador escribe lo que escribiría el runtime -un
       * progreso, un título corregido, un documento importado- y lo **anuncia**
       * igual que él, para que un recorrido pueda comprobar que lo que se ve en
       * una pantalla cambia en las demás sin recargar.
       */
      const ediciones = [...obras, ...generadas].map((obra, indice) => ({
        editionId: `e${indice}`,
        title: obra.titulo,
        authors: obra.autor,
        seriesTitle: null,
        seriesIndex: null,
        availability: 'available',
        progression: obra.progreso,
        addedAt: '2026-08-01T10:00:00Z',
        copies: 1,
        fileId: `f${indice}`,
        coverPath: sinPortadas ? null : `p${indice}.png`,
        mediaType: 'application/epub+zip',
        byteSize: 1_500_000 + indice * 1000
      }));

      /**
       * El capítulo, por párrafos.
       *
       * Van en constantes y no incrustados en el marcado porque el simulador
       * tiene que poder **calcular** dónde cae cada anotación: un desplazamiento
       * escrito a mano se queda desfasado en cuanto alguien toca una coma, y
       * entonces la prueba de que las marcas se pintan en su sitio pasaría a
       * comprobar que se pintan en el sitio equivocado.
       */
      /*
       * Los fragmentos de la página de PDF, con su sitio normalizado de 0 a 1.
       *
       * Texto propio y corriente: lo que se comprueba es la geometría y el
       * anclaje, no el contenido. Están partidos **a mitad de frase** a
       * propósito, que es como los parte un extractor de PDF de verdad, y por eso
       * la cita de la prueba cruza dos: si los fragmentos no separaran, el texto
       * proyectado diría "anualresume" y esa cita no se encontraría nunca.
       */
      const FRAGMENTOS = [
        { text: 'El informe anual', x: 0.1, y: 0.1, width: 0.3, height: 0.04 },
        { text: 'resume los datos', x: 0.42, y: 0.1, width: 0.3, height: 0.04 },
        { text: 'de cada trimestre.', x: 0.1, y: 0.16, width: 0.32, height: 0.04 }
      ];

      /** El texto de la página tal y como lo normaliza el proveedor de PDF. */
      const TEXTO_DE_LA_PAGINA = FRAGMENTOS.map((f) => f.text).join(' ');

      const PARRAFOS = [
        'Muchos años después, frente al pelotón de fusilamiento.',
        'Macondo era entonces una aldea de veinte casas de barro.',
        'El mundo era tan reciente que muchas cosas carecían de nombre.'
      ];

      /**
       * El texto plano del capítulo, con las reglas del runtime.
       *
       * Cerrar un bloque separa y el espacio se colapsa, que es lo que hace
       * `texto_plano` en `src-tauri/lector/src/reader/epub.rs`. Sobre esto se cuentan
       * los desplazamientos, igual que allí.
       */
      const TEXTO_DEL_CAPITULO = PARRAFOS.join(' ');

      /** Lo que va detrás de los tres párrafos de siempre: texto propio, de largos distintos. */
      const PALABRAS =
        'la casa del camino llevaba años esperando a que alguien volviera con una carta sin abrir y una lámpara que nadie había encendido desde el invierno'.split(
          ' '
        );
      const PARRAFOS_EXTRA = Array.from(
        { length: Math.max(0, parrafos - PARRAFOS.length) },
        (_, i) =>
          `${Array.from(
            { length: 30 + ((i * 37) % 90) },
            (_, k) => PALABRAS[(i * 7 + k * 3) % PALABRAS.length]
          ).join(' ')}.`
      );

      /** Si se autorizó el catálogo de metadatos desde la ficha. */
      let metadatosAutorizados = false;

      /** Lo último que guardó `save_progress`, con lo que se vuelve a abrir. */
      let guardadoEnSesion: typeof locator | null = null;

      const locator = {
        version: 1,
        href: 'OEBPS/c1.xhtml',
        mediaType: 'application/xhtml+xml',
        title: 'Capítulo 1',
        locations: { fragments: ['#char=0'], progression: 0, totalProgression: 0 },
        text: { highlight: 'Muchos años después' }
      };

      /**
       * Publica un hecho por el bus simulado, como lo publica el runtime: con el
       * discriminante dentro de la carga, que es como lo serializa `DomainEvent`.
       */
      const anunciar = (tipo: string, carga: Record<string, unknown> = {}) =>
        (
          (globalThis as Record<string, unknown>).__nbEmitir as
            | ((evento: string, payload: unknown) => void)
            | undefined
        )?.(tipo, { type: tipo, ...carga });

      /**
       * Dónde cae una cita del capítulo largo, como fracción del libro.
       *
       * Solo para los párrafos de detrás de los tres de siempre: las anotaciones de
       * las demás pruebas se anclan en esos tres, y su locator no cambia. En un
       * capítulo largo, desplazarse **mueve** la lectura, que es lo que un recorrido
       * de progreso necesita poder ver.
       */
      const progresionDe = (cita: string): number | null => {
        // Igualdad entera y no por el principio: los párrafos generados empiezan
        // igual cada cuatro, y un prefijo los confundía.
        const indice = PARRAFOS_EXTRA.indexOf(cita.trim());
        if (indice < 0) return null;
        const total = PARRAFOS.length + PARRAFOS_EXTRA.length;
        return (PARRAFOS.length + indice + 1) / total;
      };

      /** Lo empezado y sin terminar, con el umbral del runtime (`browse.rs`). */
      const empezada = (e: { progression: number | null }) =>
        e.progression !== null && e.progression < 0.99;

      /** Los documentos que entran por `import_path`, para darles otro nombre. */
      let importados = 0;

      /*
       * Una carpeta con copias exactas. Con `__nbCopias`, elegir una carpeta
       * la importa como el runtime: responde enseguida que sigue en marcha, avisa
       * del avance y del fin por el bus, y la ejecución guardada dice sus copias.
       */
      const conCopias = Boolean((globalThis as Record<string, unknown>).__nbCopias);
      let copiasImportadas = false;

      /*
       * El tema de extensión, instalado de fábrica salvo que la prueba lo quite.
       *
       * Con `__nbSinTemaDeExtension` no está, y `extensions_install` lo instala:
       * es lo que permite comprobar que un tema aparece en Ajustes al instalarlo y
       * desaparece al quitarlo, sin reiniciar.
       */
      const ID_DEL_TEMA = 'lat.nbdigitallabs.books.temas.crepusculo';
      const sinTema = Boolean((globalThis as Record<string, unknown>).__nbSinTemaDeExtension);
      let temaInstalado = !sinTema;
      /** Si el tema está en la lista de instaladas, que solo pasa al instalarlo aquí. */
      let temaEnLaLista = false;
      const filaDelTema = () => ({
        id: ID_DEL_TEMA,
        name: 'Tema Crepúsculo',
        version: '1.0.0',
        enabled: temaInstalado,
        readable: true,
        missingModules: [],
        commands: 0,
        key: null,
        permissions: [],
        providers: []
      });

      /**
       * El sitio de la página de PDF, y la marca que lo cita.
       *
       * La cita **cruza dos fragmentos** a propósito: es lo que distingue una
       * proyección que separa de una que los pega. Sin separación el texto diría
       * "anualresume" y esta cita no se encontraría, así que la marca aparecería
       * como no colocada en lugar de pintarse.
       */
      const CITA_DEL_PDF = 'anual resume';

      /** Dónde cae una cita, según el texto al que pertenezca su ancla. */
      const desplazamientoDeLaCita = (href: string, cita: string): number => {
        if (href === 'OEBPS/c1.xhtml') return Math.max(0, TEXTO_DEL_CAPITULO.indexOf(cita));
        if (href === '1') return Math.max(0, TEXTO_DE_LA_PAGINA.indexOf(cita));
        return 0;
      };

      const locatorDelPdf = {
        version: 1,
        href: '1',
        mediaType: 'application/pdf',
        title: 'Página 1',
        locations: {
          fragments: [`#char=${TEXTO_DE_LA_PAGINA.indexOf(CITA_DEL_PDF)}`],
          progression: 0,
          totalProgression: 0
        },
        text: { highlight: CITA_DEL_PDF }
      };

      /**
       * El ancla de la que ya no queda rastro en el documento.
       *
       * Con su propia cita, y no compartiendo la del principio: dos anotaciones
       * con el mismo texto no se distinguen en una lista, y entonces la prueba
       * no comprueba lo que dice comprobar.
       */
      const anclaPerdida = {
        version: 1,
        href: 'OEBPS/c9.xhtml',
        mediaType: 'application/xhtml+xml',
        title: 'Capítulo 9',
        locations: { fragments: ['#char=900'], progression: 0.9, totalProgression: 0.9 },
        text: { highlight: 'Una cita que desapareció' }
      };

      /** Otro punto del documento, para que saltar signifique moverse. */
      const otroLocator = {
        version: 1,
        href: 'OEBPS/c2.xhtml',
        mediaType: 'application/xhtml+xml',
        title: 'Capítulo 2',
        locations: { fragments: ['#char=120'], progression: 0.4, totalProgression: 0.4 },
        text: { highlight: 'El coronel Aureliano Buendía' }
      };

      const anotacion = (
        id: string,
        kind: string,
        body: string | null,
        target: typeof locator
      ) => ({
        id,
        editionId: 'e0',
        kind,
        body,
        target,
        createdAt: '2026-08-28T00:00:00Z',
        supersedes: null,
        deletedAt: null
      });

      /**
       * Las anotaciones del documento, con estado.
       *
       * Editar y borrar tienen que **verse** en la siguiente consulta: un
       * simulador que devolviera siempre la misma lista dejaría pasar un panel
       * que llama al puente y no se entera de la respuesta.
       */
      /**
       * Un resaltado **en el capítulo que se está leyendo**.
       *
       * Los otros anclan en el capítulo 2 o han perdido su ancla, así que
       * ninguno se pinta sobre lo que hay en pantalla y la prueba no tendría nada
       * que comprobar. Va con una cita que existe de verdad en el texto: el
       * simulador calcula su desplazamiento, y la marca solo aparece si la
       * proyección coincide con la del runtime.
       */
      const enElSegundoParrafo = {
        version: 1,
        href: 'OEBPS/c1.xhtml',
        mediaType: 'application/xhtml+xml',
        title: 'Capítulo 1',
        locations: { fragments: ['#char=56'], progression: 0.2, totalProgression: 0.2 },
        text: { highlight: 'una aldea de veinte casas' }
      };

      /**
       * Un marcador **en el tercer párrafo del capítulo que se lee**.
       *
       * Antes apuntaba al capítulo 2, así que no había forma de comprobar que
       * poner un marcador se ve en algún sitio. Su cita existe en el texto, y el
       * simulador calcula su desplazamiento igual que el de los demás.
       */
      const enElTercerParrafo = {
        version: 1,
        href: 'OEBPS/c1.xhtml',
        mediaType: 'application/xhtml+xml',
        title: 'Capítulo 1',
        locations: { fragments: ['#char=113'], progression: 0.6, totalProgression: 0.6 },
        text: { highlight: 'El mundo era tan reciente' }
      };

      let anotaciones = [
        anotacion('a1', 'note', 'Lo importante es el hielo', locator),
        anotacion('a2', 'highlight', null, otroLocator),
        anotacion('a3', 'bookmark', null, enElTercerParrafo),
        anotacion('a5', 'highlight', null, enElSegundoParrafo),
        anotacion('a4', 'note', 'Esta ya no ancla', anclaPerdida),
        // Solo tiene sentido con `pdf`, y en un EPUB no estorba: su href no es
        // el de ningún capítulo, así que no se pinta.
        anotacion('a6', 'highlight', null, locatorDelPdf),
        ...Array.from({ length: anotacionesExtra ?? 0 }, (_, i) =>
          anotacion(`extra-${i}`, 'highlight', null, {
            ...otroLocator,
            href: 'OEBPS/c9.xhtml',
            locations: {
              fragments: ['#char=0'],
              progression: 0,
              totalProgression: i / (anotacionesExtra ?? 1)
            }
          })
        )
      ];

      /** La última no ancla: es la que comprueba que una perdida no ofrece salto. */
      const PERDIDA = 'a4';

      /**
       * Notas del usuario y los pasajes que citan.
       *
       * Con estado de verdad y no con respuestas fijas: lo que se comprueba es
       * el ciclo entero —crear, citar, verlo desde los dos lados, quitarlo— y
       * con una lista inmutable eso no se puede ver. Antes el
       * simulador devolvía siempre cero notas, así que Estudio no se probaba con
       * ninguna dentro.
       */
      let notas = [
        {
          id: 'n1',
          title: 'Sobre el tiempo circular',
          body: 'Macondo vuelve sobre sí mismo.',
          createdAt: '2026-08-01T10:00:00Z',
          updatedAt: '2026-08-01T10:00:00Z'
        }
      ];
      let citas: Array<{ noteId: string; annotationId: string }> = [];

      type TarjetaSimulada = {
        id: string;
        noteId: string | null;
        title: string | null;
        preview: string | null;
        linksTo: string | null;
        linksToTitle: string | null;
      };
      let siguienteTarjeta = 1;
      /*
       * Con `tableroLleno`, las ocho columnas con quince tarjetas cada una: las
       * ciento veinte que caben, que es lo que se cuenta contra el presupuesto de
       * nodos.
       */
      let tablerosSimulados = [
        {
          id: 'brd-1',
          title: 'Capítulo tres',
          createdAt: '2026-09-08T10:00:00Z',
          columnas: tableroLleno
            ? Array.from({ length: 8 }, (_, c) => ({
                id: `col-${c}`,
                title: `Columna ${c + 1}`,
                cards: Array.from({ length: 15 }, (_, t) => ({
                  id: `crd-lleno-${c}-${t}`,
                  noteId: `nota-${c}-${t}`,
                  title: `Nota número ${c * 15 + t}`,
                  preview: 'un anticipo del cuerpo que ocupa un par de líneas en la tarjeta…',
                  linksTo: null,
                  linksToTitle: null
                })) as TarjetaSimulada[]
              }))
            : [
                {
                  id: 'col-1',
                  title: 'Por Revisar',
                  cards: [
                    {
                      id: 'crd-a',
                      noteId: 'n1',
                      title: 'Sobre el tiempo circular',
                      preview: 'Macondo vuelve sobre sí mismo.',
                      linksTo: null,
                      linksToTitle: null
                    },
                    {
                      id: 'crd-b',
                      noteId: 'nota-2',
                      title: 'La segunda nota',
                      preview: null,
                      linksTo: null,
                      linksToTitle: null
                    }
                  ] as TarjetaSimulada[]
                },
                {
                  id: 'col-2',
                  title: 'En Curso',
                  cards: [
                    {
                      id: 'crd-c',
                      noteId: null,
                      title: null,
                      preview: null,
                      linksTo: 'brd-2',
                      linksToTitle: 'Personajes'
                    }
                  ] as TarjetaSimulada[]
                },
                { id: 'col-3', title: 'Hecho', cards: [] as TarjetaSimulada[] }
              ]
        },
        {
          id: 'brd-2',
          title: 'Personajes',
          createdAt: '2026-09-07T10:00:00Z',
          columnas: [] as Array<{ id: string; title: string; cards: TarjetaSimulada[] }>
        }
      ];
      const columnasDe = (tablero: string) =>
        tablerosSimulados.find((t) => t.id === tablero)?.columnas ?? [];
      const ponerTarjeta = (tablero: string, columna: string, tarjeta: TarjetaSimulada) => {
        const columnas = columnasDe(tablero);
        // Una nota está una sola vez en cada tablero, como en `boards.rs`.
        for (const c of columnas) {
          c.cards = c.cards.filter((t) => !tarjeta.noteId || t.noteId !== tarjeta.noteId);
        }
        columnas.find((c) => c.id === columna)?.cards.push(tarjeta);
      };

      /**
       * Las colecciones del usuario, con estado.
       *
       * Crear, renombrar, borrar y añadir tienen que verse en la siguiente
       * consulta: con una lista fija, un menú que no se enterara de la respuesta
       * pasaría igual.
       */
      let colecciones: Array<{
        id: string;
        name: string;
        kind: string;
        origin: string;
        miembros: string[];
      }> = [
        {
          id: 'col-1',
          name: 'Para la Tesis',
          kind: 'manual',
          origin: 'personal',
          miembros: ['e0']
        },
        { id: 'col-2', name: 'Relecturas', kind: 'manual', origin: 'personal', miembros: [] }
      ];
      let siguienteColeccion = 3;

      /** Una anotación con el libro del que salió, como la da `library`. */
      const conContexto = (a: (typeof anotaciones)[number]) => ({
        annotation: a,
        title: 'Cien Años de Soledad',
        authors: 'Gabriel García Márquez',
        // La perdida no tiene archivo disponible: es la que comprueba que sin
        // copia no se ofrece un salto que fallaría.
        fileId: a.id === PERDIDA ? null : 'f0'
      });

      /** Lo que la interfaz ha pedido, para poder afirmar sobre ello. */
      const llamadas: Array<{ comando: string; args: unknown }> = [];
      (globalThis as Record<string, unknown>).__nbLlamadas = llamadas;

      let runtimeLiberado = !runtimeEnEspera;
      window.addEventListener(
        'nb-test-runtime-listo',
        () => {
          runtimeLiberado = true;
        },
        { once: true }
      );
      const respuestas: Record<string, (args: Record<string, unknown>) => unknown> = {
        // Los modelos de IA vienen de `simularModelosYEventos`, en su propio guion.
        ...((globalThis as Record<string, unknown>).__nbRespuestasDeModelos as Record<
          string,
          (args: Record<string, unknown>) => unknown
        >),
        runtime_ready: () => runtimeLiberado,
        update_check: () => null,
        update_managed_by_launcher: () => false,
        update_install: () => undefined,
        edit_edition_field: () => undefined,
        retired_audio_notice: () => null,
        browse_letter_anchors: () => [
          { letter: 'A', offset: 0 },
          { letter: 'C', offset: 60 },
          { letter: 'M', offset: 300 }
        ],
        delete_document: () => ({ fileDeleted: false, workRemoved: true }),
        health_check: () => ({ version: '1.0.0', schemaVersion: 13 }),

        // Extensiones. Una instalada y activa, con **dos** permisos
        // concedidos y uno sin conceder.
        //
        // Dos y no uno a propósito: con un solo permiso concedido, "el primero
        // de la lista" y "el de esta fila" son el mismo anfitrión, así que una
        // fila que revocara siempre el primero pasaría la prueba. Lo destapó
        // una mutación.
        // El simulador devuelve lo mismo que el anfitrión: la API 1.0.0, estable.
        extensions_api: () => ({ current: 1, oldest: 0, stable: true }),
        extensions_list: () => [
          ...(temaEnLaLista ? [filaDelTema()] : []),
          {
            id: 'org.ejemplo.metadatos',
            name: 'Metadatos Académicos',
            version: '1.0.0',
            enabled: true,
            readable: true,
            missingModules: [],
            commands: 2,
            key: 'a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90',
            // Declara que revisa metadatos por lotes: sale del
            // manifiesto, así que la biblioteca lo ofrece sin cargar su código.
            providers: ['bulk-metadata'],
            permissions: [
              {
                capability: 'network:fetch',
                host: 'datos.ejemplo.org',
                implies: 'Podrá enviar y recibir datos de los servidores que se listan.',
                whatIsSent: 'lo que la extensión decida enviar, que no podemos acotar',
                granted: true
              },
              {
                capability: 'network:fetch',
                host: 'terceros.ejemplo.org',
                implies: 'Podrá enviar y recibir datos de los servidores que se listan.',
                whatIsSent: 'lo que la extensión decida enviar, que no podemos acotar',
                granted: true
              },
              {
                capability: 'network:fetch',
                host: 'otros.ejemplo.org',
                implies: 'Podrá enviar y recibir datos de los servidores que se listan.',
                whatIsSent: 'lo que la extensión decida enviar, que no podemos acotar',
                granted: false
              }
            ]
          },
          // La segunda existe para que las dos formas de la firma se vean en la
          // misma pantalla, y se diferencia de la primera **solo** en eso: sin
          // permisos y sin órdenes, para no tocar lo que miden las otras
          // pruebas de esta lista.
          {
            id: 'org.ejemplo.sin-firmar',
            name: 'Extensión Sin Firmar',
            version: '1.0.0',
            // Apagada porque el registro la retiró por maliciosa: es lo que
            // hace la comprobación, y la fila tiene que decir por qué.
            enabled: false,
            readable: true,
            missingModules: [],
            commands: 0,
            permissions: [],
            providers: [],
            key: null
          }
        ],
        extensions_generations: () => [1, 2],
        // Lo que el registro ha retirado y está instalado aquí. Sale
        // de lo guardado en la última comprobación, no de la red.
        extensions_withdrawals: () => [
          {
            extension: 'org.ejemplo.sin-firmar',
            version: '1.0.0',
            reason: 'malicious',
            detail: 'Enviaba lo que leías a un tercero',
            date: '2026-09-08',
            disabled: true
          }
        ],
        extensions_check_withdrawals: () => [],
        // Lo que una extensión aporta sobre un documento. Sale del
        // manifiesto, así que está antes de que se cargue código.
        extensions_context_commands: () => [
          {
            extension: 'lat.nbdigitallabs.books.banco.usa-el-mundo',
            id: 'usa-el-mundo.consultar',
            title: 'Consultar la Fuente de Prueba',
            category: 'Metadatos'
          }
        ],
        extensions_run_command: () => 'Respondió 200',
        // Una vista aportada a Estudio. La pestaña sale del
        // manifiesto; componerla es lo que cargaría el código.
        // Un tema aportado por una extensión. No trae código: es un
        // manifiesto con colores y nada más.
        extensions_themes: () =>
          temaInstalado
            ? [
                {
                  extension: ID_DEL_TEMA,
                  name: 'Crepúsculo',
                  tokens: {
                    '--nb-surface': '#161a22',
                    '--nb-text': '#e8ecf2',
                    '--nb-accent': '#7fb2e5',
                    '--nb-on-accent': '#0d1420',
                    '--nb-focus': '#7fb2e5'
                  }
                }
              ]
            : [],
        // Un modo de lectura aportado por un paquete de personalización
        //. No trae código: es un manifiesto con colores y números.
        extensions_reading_modes: () => [
          {
            id: 'lat.nbdigitallabs.books.personalizacion.papel/pergamino',
            extension: 'lat.nbdigitallabs.books.personalizacion.papel',
            name: 'Pergamino',
            background: '#f2e8d5',
            text: '#33291b',
            brightness: 96,
            warmth: 45,
            monochrome: false
          }
        ],
        // Fondos sonoros aportados por un paquete. Sin audio: el
        // manifiesto dice qué clase de ruido es y el núcleo lo sintetiza.
        // Formatos de cita aportados por una extensión.
        review_metadata_in_bulk: () => ({
          reviewed: 50,
          suggested: 3,
          recorded: 3,
          failedBatches: 0
        }),
        // Tableros de notas, por columnas. Con estado: mover una
        // tarjeta tiene que verse en la respuesta, o una interfaz que no se
        // enterara de ella pasaría igual.
        board_limits: () => ({ columns: 8, cards: 120 }),
        boards_list: () =>
          tablerosSimulados.map(({ columnas, ...tablero }) => ({
            ...tablero,
            filled: columnas.reduce((suma, c) => suma + c.cards.length, 0)
          })),
        boards_create: (args) => {
          const nuevo = {
            id: `brd-${tablerosSimulados.length + 1}`,
            title: String(args.title).trim(),
            createdAt: '2026-09-12T11:00:00Z',
            columnas: (args.columns as string[]).map((title, i) => ({
              id: `col-nueva-${i}`,
              title,
              cards: [] as TarjetaSimulada[]
            }))
          };
          tablerosSimulados = [nuevo, ...tablerosSimulados];
          return { id: nuevo.id, title: nuevo.title, createdAt: nuevo.createdAt, filled: 0 };
        },
        boards_delete: (args) => {
          const habia = tablerosSimulados.some((t) => t.id === args.id);
          tablerosSimulados = tablerosSimulados.filter((t) => t.id !== args.id);
          return habia;
        },
        board_columns: (args) => columnasDe(String(args.boardId)),
        board_add_column: (args) => {
          const columnas = columnasDe(String(args.boardId));
          columnas.push({
            id: `col-${String(args.boardId)}-${columnas.length}`,
            title: String(args.title).trim(),
            cards: []
          });
          return columnas;
        },
        board_rename_column: (args) => {
          const columnas = columnasDe(String(args.boardId));
          const columna = columnas.find((c) => c.id === args.columnId);
          if (columna) columna.title = String(args.title).trim();
          return columnas;
        },
        board_delete_column: (args) => {
          const tablero = tablerosSimulados.find((t) => t.id === args.boardId);
          if (tablero) tablero.columnas = tablero.columnas.filter((c) => c.id !== args.columnId);
          return columnasDe(String(args.boardId));
        },
        board_move_column: (args) => {
          const columnas = columnasDe(String(args.boardId));
          const desde = columnas.findIndex((c) => c.id === args.columnId);
          const [columna] = columnas.splice(desde, 1);
          if (columna)
            columnas.splice(Math.min(Number(args.position), columnas.length), 0, columna);
          return columnas;
        },
        board_add_note: (args) => {
          const nota = notas.find((n) => n.id === args.noteId);
          ponerTarjeta(String(args.boardId), String(args.columnId), {
            id: `crd-${siguienteTarjeta++}`,
            noteId: String(args.noteId),
            title: nota?.title ?? null,
            preview: nota?.body ?? null,
            linksTo: null,
            linksToTitle: null
          });
          return columnasDe(String(args.boardId));
        },
        board_add_link: (args) => {
          ponerTarjeta(String(args.boardId), String(args.columnId), {
            id: `crd-${siguienteTarjeta++}`,
            noteId: null,
            title: null,
            preview: null,
            linksTo: String(args.linksTo),
            linksToTitle: tablerosSimulados.find((t) => t.id === args.linksTo)?.title ?? null
          });
          return columnasDe(String(args.boardId));
        },
        board_create_note: (args) => {
          const nueva = {
            id: `n${notas.length + 1}`,
            title: String(args.title),
            body: String(args.body),
            createdAt: '2026-09-12T10:00:00Z',
            updatedAt: '2026-09-12T10:00:00Z'
          };
          notas = [...notas, nueva];
          ponerTarjeta(String(args.boardId), String(args.columnId), {
            id: `crd-${siguienteTarjeta++}`,
            noteId: nueva.id,
            title: nueva.title,
            preview: nueva.body || null,
            linksTo: null,
            linksToTitle: null
          });
          return columnasDe(String(args.boardId));
        },
        board_move_card: (args) => {
          const columnas = columnasDe(String(args.boardId));
          let tarjeta: TarjetaSimulada | undefined;
          for (const columna of columnas) {
            const i = columna.cards.findIndex((t) => t.id === args.cardId);
            if (i >= 0) [tarjeta] = columna.cards.splice(i, 1);
          }
          const destino = columnas.find((c) => c.id === args.columnId);
          if (tarjeta && destino) {
            destino.cards.splice(Math.min(Number(args.position), destino.cards.length), 0, tarjeta);
          }
          return columnas;
        },
        board_remove_card: (args) => {
          for (const columna of columnasDe(String(args.boardId))) {
            columna.cards = columna.cards.filter((t) => t.id !== args.cardId);
          }
          return columnasDe(String(args.boardId));
        },
        find_note: (args) => notas.find((n) => n.id === args.id) ?? null,
        study_counts: () => ({ cards: 3, concepts: 2 }),
        annotation_counts: () => {
          const cuenta = (clase: string) => anotaciones.filter((a) => a.kind === clase).length;
          return {
            all: anotaciones.length,
            note: cuenta('note'),
            highlight: cuenta('highlight'),
            bookmark: cuenta('bookmark'),
            underline: cuenta('underline'),
            drawing: cuenta('drawing')
          };
        },
        collections_list: () =>
          colecciones.map(({ miembros, ...c }) => ({ ...c, members: miembros.length })),
        collections_create: (args) => {
          const nueva = {
            id: `col-${siguienteColeccion++}`,
            name: String(args.name).trim(),
            kind: 'manual',
            origin: 'personal',
            miembros: [] as string[]
          };
          colecciones = [...colecciones, nueva];
          anunciar('CollectionsChanged', { collectionId: null });
          return { id: nueva.id, name: nueva.name, kind: 'manual', origin: 'personal', members: 0 };
        },
        collections_rename: (args) => {
          colecciones = colecciones.map((c) =>
            c.id === args.id ? { ...c, name: String(args.name).trim() } : c
          );
          anunciar('CollectionsChanged', { collectionId: args.id });
          return null;
        },
        collections_delete: (args) => {
          const habia = colecciones.some((c) => c.id === args.id);
          colecciones = colecciones.filter((c) => c.id !== args.id);
          if (habia) anunciar('CollectionsChanged', { collectionId: null });
          return habia;
        },
        collections_add: (args) => {
          const pedidas = args.editionIds as string[];
          let nuevas = 0;
          colecciones = colecciones.map((c) => {
            if (c.id !== args.id) return c;
            const faltan = pedidas.filter((e) => !c.miembros.includes(e));
            nuevas = faltan.length;
            return { ...c, miembros: [...c.miembros, ...faltan] };
          });
          if (nuevas > 0) anunciar('CollectionsChanged', { collectionId: args.id });
          return nuevas;
        },
        collections_remove: (args) => {
          colecciones = colecciones.map((c) =>
            c.id === args.id
              ? { ...c, miembros: c.miembros.filter((e) => e !== args.editionId) }
              : c
          );
          anunciar('CollectionsChanged', { collectionId: args.id });
          return null;
        },
        collections_containing: (args) =>
          colecciones.filter((c) => c.miembros.includes(String(args.editionId))).map((c) => c.id),
        extensions_citation_formats: () => [
          {
            id: 'lat.nbdigitallabs.books.citas.bibliografia/bibtex',
            extension: 'lat.nbdigitallabs.books.citas.bibliografia',
            name: 'BibTeX',
            fileExtension: 'bib'
          }
        ],
        save_citations: () => null,
        extensions_soundscapes: () => [
          {
            id: 'lat.nbdigitallabs.books.sonido.concentracion/lluvia',
            extension: 'lat.nbdigitallabs.books.sonido.concentracion',
            name: 'Lluvia',
            kind: 'lluvia'
          },
          {
            id: 'lat.nbdigitallabs.books.sonido.concentracion/oleaje',
            extension: 'lat.nbdigitallabs.books.sonido.concentracion',
            name: 'Oleaje',
            kind: 'oleaje'
          }
        ],
        extensions_study_views: () => [
          {
            extension: 'lat.nbdigitallabs.books.banco.vista-de-estudio',
            id: 'vista-de-estudio.resumen',
            title: 'Resumen de Prueba'
          }
        ],
        extensions_render_view: () => ({
          title: 'Resumen de prueba',
          blocks: [
            { clase: 'heading', valor: 'Lo que hay' },
            { clase: 'paragraph', valor: 'Esta vista la compone una extensión.' },
            { clase: 'items', valor: ['Primero', 'Segundo'] },
            { clase: 'field', valor: { label: 'Origen', value: 'Extensión de prueba' } }
          ]
        }),
        // Un paquete incompleto contesta lo suyo, que es lo que la prueba de
        // abajo comprueba: `__nbExtensionIncompleta` lo enciende.
        extensions_install: () => {
          if (sinTema && !temaEnLaLista) {
            temaInstalado = true;
            temaEnLaLista = true;
            anunciar('ExtensionsChanged');
            return { estado: 'installed', detalle: filaDelTema() };
          }
          return ((globalThis as Record<string, unknown>).__nbExtensionIncompleta as boolean)
            ? { estado: 'incomplete', detalle: { modules: ['citas.wasm'] } }
            : {
                estado: 'installed',
                detalle: {
                  id: 'org.ejemplo.metadatos',
                  name: 'Metadatos Académicos',
                  version: '1.0.0',
                  enabled: true,
                  readable: true,
                  missingModules: [],
                  commands: 2,
                  key: 'a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90',
                  permissions: []
                }
              };
        },
        extensions_grant: () => null,
        extensions_revoke: () => null,
        extensions_set_enabled: () => [],
        extensions_uninstall: (args) => {
          if (args.id === ID_DEL_TEMA && temaEnLaLista) {
            temaInstalado = false;
            temaEnLaLista = false;
            anunciar('ExtensionsChanged');
          }
          return [];
        },
        extensions_rollback: () => [],
        // Los componentes que el runtime publica de verdad. Un simulador que omite
        // la dimensión que se mide convierte cualquier medición en una confirmación.
        component_states: () =>
          (globalThis as Record<string, unknown>).__nbEstadosDeComponente ?? [
            { component: 'database', state: 'READY', reasonCode: null },
            { component: 'reader', state: 'READY', reasonCode: null },
            { component: 'search', state: 'READY', reasonCode: null },
            { component: 'metadata', state: 'READY', reasonCode: null },
            { component: 'ai', state: 'STOPPED', reasonCode: 'AI_DISABLED' }
          ],
        settings_snapshot: () => [
          { key: 'theme', value: 'gabinete', affectsFirstPaint: true, plane: 'application' },
          { key: 'accent_color', value: 'theme', affectsFirstPaint: false, plane: 'application' },
          { key: 'high_contrast', value: false, affectsFirstPaint: true, plane: 'application' },
          { key: 'reduced_motion', value: false, affectsFirstPaint: true, plane: 'application' },
          { key: 'font_scale', value: 1, affectsFirstPaint: true, plane: 'application' },
          { key: 'language', value: 'es', affectsFirstPaint: true, plane: 'application' },
          // La voz sale de aquí de fábrica: local, y sin servicio configurado.
          // Es el estado de partida y el que la interfaz tiene que saber pintar.
          {
            key: 'voice_provider',
            value:
              ((globalThis as Record<string, unknown>).__nbProveedorDeVoz as string) ?? 'local',
            affectsFirstPaint: false,
            plane: 'application'
          },
          { key: 'speech_host', value: '', affectsFirstPaint: false, plane: 'application' },
          { key: 'speech_model', value: '', affectsFirstPaint: false, plane: 'application' },
          { key: 'speech_voice', value: '', affectsFirstPaint: false, plane: 'application' },
          { key: 'book_font_size', value: 18, affectsFirstPaint: false, plane: 'book' },
          { key: 'book_reading_mode', value: 'sepia', affectsFirstPaint: false, plane: 'book' }
        ],
        update_setting: (args) => {
          anotarAjusteDeIa(String(args.key), args.value);
          return {
            key: args.key,
            value: args.value,
            // Lo declara el catálogo del runtime, no quien escribe: devolver
            // siempre `false` haría que la caché de arranque nunca se reescriba y
            // la prueba de que el tema sobrevive al reinicio pasaría en falso.
            affectsFirstPaint: ['theme', 'high_contrast', 'reduced_motion', 'font_scale'].includes(
              String(args.key)
            ),
            // El plano, por el mismo motivo, y **este simulador lo tenía mal**:
            // devolvía siempre `application`, y la composición del libro solo lee
            // lo que viene marcado como `book`. Con eso, ningún ajuste del libro
            // ni del kit accesible se aplicaba jamás en estas pruebas, así que la
            // suite daba por buenos dos paneles enteros que no podía comprobar.
            // El runtime lo decide por el prefijo de la clave —`SettingKey::plane`
            // en `settings.rs`— y aquí se replica igual.
            plane: String(args.key).startsWith('book_') ? 'book' : 'application'
          };
        },
        /*
         * Las raíces de la biblioteca, y la mudanza.
         *
         * El simulador recuerda si se reapuntó, porque el criterio no es que la
         * llamada no falle: es que después de reapuntar los documentos vuelvan.
         */
        library_roots: () =>
          mudada
            ? [
                {
                  id: 'r1',
                  name: 'Biblioteca',
                  kind: 'referenced',
                  path: '/otro/disco/biblioteca',
                  files: 3,
                  missing: 0
                }
              ]
            : [
                {
                  id: 'r1',
                  name: 'Biblioteca',
                  kind: 'referenced',
                  path: null,
                  files: 3,
                  missing: 3
                }
              ],
        relocate_library_root: () => {
          mudada = true;
          return { checked: 3, available: 3, missing: 0, unreadable: 0, managedMissing: 0 };
        },
        refresh_availability: () =>
          mudada
            ? { checked: 3, available: 3, missing: 0, unreadable: 0, managedMissing: 0 }
            : { checked: 3, available: 0, missing: 3, unreadable: 0, managedMissing: 0 },
        /*
         * Una clase por petición y con búsqueda, como el runtime. El simulador
         * filtra de verdad porque lo que hay que poder probar es justo eso: que
         * una estantería se encuentra escribiendo parte de su nombre.
         */
        library_shelves: (args) => {
          const q = args.query as { kind: string; search?: string; limit?: number };
          const todas: Record<
            string,
            { kind: string; key: string; label: string; count: number }[]
          > = {
            author: [
              {
                kind: 'author',
                key: 'Gabriel García Márquez',
                label: 'Gabriel García Márquez',
                count: 2
              },
              { kind: 'author', key: 'Jorge Luis Borges', label: 'Jorge Luis Borges', count: 1 }
            ],
            series: [],
            subject: [{ kind: 'subject', key: 'sub-1', label: 'Realismo Mágico', count: 2 }],
            language: [{ kind: 'language', key: 'es', label: 'es', count: 3 }],
            format: [
              { kind: 'format', key: 'application/pdf', label: 'application/pdf', count: 1 }
            ],
            year: [{ kind: 'year', key: '1967', label: '1967', count: 1 }]
          };
          const texto = (q.search ?? '').trim().toLowerCase();
          const filtradas = (todas[q.kind] ?? []).filter(
            (s) => texto === '' || s.label.toLowerCase().includes(texto)
          );
          const pagina = { shelves: filtradas.slice(0, q.limit ?? 20), total: filtradas.length };

          if (!estanteriasDesordenadas) return pagina;
          // Cuanto más corta la búsqueda, más tarda: la primera pulsación
          // contesta la última.
          const espera = Math.max(0, 400 - texto.length * 100);
          return new Promise((resolver) => setTimeout(() => resolver(pagina), espera));
        },
        browse_library: (args) => {
          const consulta = args.query as {
            filter?: { kind?: string; value?: string };
            limit?: number;
            offset?: number;
          };
          const filtradas =
            consulta.filter?.kind === 'inProgress'
              ? ediciones.filter(empezada)
              : consulta.filter?.kind === 'duplicates'
                ? ediciones.slice(0, duplicados)
                : consulta.filter?.kind === 'collection'
                  ? ediciones.filter((e) =>
                      colecciones.some(
                        (c) => c.id === consulta.filter?.value && c.miembros.includes(e.editionId)
                      )
                    )
                  : ediciones;
          // El desplazamiento se respeta, como en `browse.rs`. Ignorarlo hacía
          // que la vista recibiera siempre la primera página y pareciera que la
          // virtualización funcionaba aunque pidiera la ventana equivocada.
          const desde = Math.max(0, consulta.offset ?? 0);
          return {
            total: filtradas.length,
            items: filtradas.slice(desde, desde + (consulta.limit ?? 60))
          };
        },
        indexing_progress: () =>
          indiceSinSitio
            ? {
                pending: 123528,
                indexed: 0,
                skipped: 0,
                failed: 0,
                paused: true,
                pauseReason: 'STORAGE_BUDGET_REACHED',
                searchable: 28552,
                total: 152080,
                storage: {
                  documentsBytes: 162_319_958_712,
                  derivedBytes: 32_496_000_000,
                  percentOfDocuments: 20.02,
                  budgetPercent: 20
                }
              }
            : null,
        // Como la biblioteca de verdad sana: nada duplicado y nada ausente, y lo
        // empezado que haya. Es el caso en que dos de los cuatro filtros no
        // llevan a ninguna obra y no deben ofrecerse.
        library_facets: () => ({
          all: ediciones.length,
          inProgress: ediciones.filter(empezada).length,
          duplicates: duplicados,
          unavailable: 0
        }),
        reading_stats: () => ({
          minutesToday: 47,
          minutesWeek: 310,
          streakDays: 5,
          inProgress: ediciones.filter(empezada).length,
          documents: ediciones.length
        }),
        /*
         * Las filas de Inicio, con obras propias y no las de la biblioteca: como en
         * el runtime, ninguna obra se repite entre filas, y cada fila llega al
         * mínimo de ocho. La semilla elige la materia, para que se pueda comprobar
         * que otra visita pinta otra pantalla.
         */
        home_rows: (args) => {
          const semilla = Number(args.seed ?? 0);
          const materias = ['Novela', 'Poesía', 'Ensayo', 'Historia'];
          const materia = materias[semilla % materias.length] ?? 'Novela';
          const obrasDe = (fila: string, cuantas: number, autor: string | null) =>
            Array.from({ length: cuantas }, (_, i) => ({
              editionId: `${fila}-${i}`,
              title: `${fila === 'lista' ? 'Para Hoy' : materia} ${i + 1}`,
              authors: autor,
              seriesTitle: null,
              seriesIndex: null,
              availability: 'available',
              progression: null,
              addedAt: '2026-08-01T10:00:00Z',
              copies: 1,
              fileId: i === 1 ? null : `f-${fila}-${i}`,
              coverPath: null
            }));
          return [
            {
              id: 'porque-e0',
              kind: {
                kind: 'porqueLees',
                value: { title: 'Cien Años de Soledad', author: 'Gabriel García Márquez' }
              },
              layout: 'portadas',
              items: obrasDe('porque', 9, 'Gabriel García Márquez')
            },
            {
              id: `materia-${materia}`,
              kind: { kind: 'materia', value: { key: `m-${materia}`, label: materia } },
              layout: 'grandes',
              items: obrasDe('materia', 10, null)
            },
            {
              id: 'al-azar',
              kind: { kind: 'alAzar' },
              layout: 'lista',
              items: obrasDe('lista', 10, 'Autor al Azar')
            }
          ];
        },
        list_notes: () => notas,
        create_note: (args) => {
          const nueva = {
            id: `n${notas.length + 1}`,
            title: args.title as string,
            body: args.body as string,
            createdAt: '2026-09-01T10:00:00Z',
            updatedAt: '2026-09-01T10:00:00Z'
          };
          notas = [...notas, nueva];
          return nueva;
        },
        update_note: (args) => {
          const editada = {
            ...(notas.find((n) => n.id === args.id) ?? notas[0]),
            id: args.id as string,
            title: args.title as string,
            body: args.body as string,
            updatedAt: '2026-09-01T11:00:00Z'
          };
          notas = notas.map((n) => (n.id === args.id ? editada : n));
          return editada;
        },
        delete_note: (args) => {
          notas = notas.filter((n) => n.id !== args.id);
          citas = citas.filter((c) => c.noteId !== args.id);
          return null;
        },
        note_backlinks: () => [],
        // Arranque normal: nada que recuperar. La prueba que comprueba el aviso
        // monta su propio puente con un informe dentro.
        recovery_report: () =>
          recuperada
            ? {
                reasonCode: 'STORAGE_FAILURE',
                isolatedAs: 'nb-books.sqlite.corrupted-1787970000'
              }
            : null,
        diagnostics_preview: () => ({
          version: '1.0.0',
          schemaVersion: 13,
          platform: 'linux x86_64',
          components: [
            { component: 'database', state: 'READY', reasonCode: null },
            { component: 'reader', state: 'FAILED', reasonCode: 'ENGINE_CRASH' }
          ],
          // Ya redactado: es lo que el runtime entrega, no lo que hay en el
          // archivo. Que la redacción funcione lo comprueban las pruebas de Rust.
          log: [
            '2026-08-28T09:00:00Z  INFO nb_books_lib: arrancando',
            '<línea retirada: ruta-de-usuario>',
            '2026-08-28T09:00:06Z  WARN nb_books_lib::reader: code=DOCUMENT_CORRUPTED'
          ],
          omittedLines: 1200
        }),
        export_diagnostics: () => '{}',
        // La voz del runtime: en el navegador de las pruebas no hay
        // sintetizador, y eso es lo que el panel tiene que saber decir.
        voice_status: () => ({ available: false, reason: 'sin-sintetizador', voices: [] }),
        voice_speak: () => null,
        voice_stop: () => null,
        save_diagnostics: () => null,
        save_library_export: () => 3,
        open_document: (args) => ({
          fileId: args.fileId,
          editionId: 'e0',
          title: 'Cien Años de Soledad',
          mediaType: pdf ? 'application/pdf' : 'application/epub+zip',
          // Un PDF no recompone ni deja cambiar la tipografía: su página es una
          // imagen con una capa de texto encima. Decirlo aquí es lo que hace que
          // la interfaz no ofrezca controles que no harían nada.
          capabilities: {
            textSelection: true,
            internalSearch: true,
            reflow: !pdf,
            typography: !pdf,
            synchronizedAudio: Boolean(narrado),
            textExtraction: true
          },
          navigation: pdf
            ? { entries: [{ title: 'Página 1', locator: locatorDelPdf, children: [] }] }
            : { entries: [{ title: 'Capítulo 1', locator, children: [] }] },
          // Se abre donde se guardó, como el runtime: sin esto ninguna prueba podía
          // ver que reabrir un libro no volvía a la posición guardada.
          locator: pdf ? locatorDelPdf : (guardadoEnSesion ?? locator),
          approximate: false,
          language: 'es',
          fresh: false
        }),
        close_document: () => null,
        render_document: () =>
          pdf
            ? {
                // La página va como **ruta**, no como bytes: es lo que manda
                // `data-flows.md`, y por eso se sirve por el protocolo de assets.
                kind: 'image',
                path: '/tmp/nb-books/paginas/1.png',
                media_type: 'application/pdf'
              }
            : {
                kind: 'markup',
                html:
                  capitulo ??
                  (envuelto ? '<blockquote>' : '') +
                    `<p>${PARRAFOS[0]}</p>` +
                    // Una ilustración, **como la devuelve el runtime de verdad**: con la
                    // ruta del archivo que extrajo a su caché, porque el webview no
                    // puede leer dentro del EPUB. Sin una imagen aquí, ninguna prueba
                    // podía ver que esa ruta se pedía tal cual al servidor de desarrollo
                    // y respondía 404. Va dentro de un `figure`, que no es anclable, así
                    // que el documento sigue teniendo tres bloques.
                    '<figure><img src="/tmp/nb-books/recursos/abc123.jpg" alt="Macondo"/></figure>' +
                    `<p>${PARRAFOS[1]}</p>` +
                    `<p>${PARRAFOS[2]}</p>` +
                    PARRAFOS_EXTRA.map((parrafo) => `<p>${parrafo}</p>`).join('') +
                    (envuelto ? '</blockquote>' : ''),
                media_type: 'application/xhtml+xml',
                omitted: omitidos ?? []
              },
        page_size: () => {
          if (!pdf) throw { code: 'FORMAT_UNSUPPORTED' };
          return { width: 612, height: 792 };
        },
        text_layer: () => {
          if (!pdf) throw { code: 'FORMAT_UNSUPPORTED' };
          return { page: 1, status: 'ready', spans: FRAGMENTOS };
        },
        locator_for_selection: (args) => {
          const progresion = progresionDe(String(args.quote ?? ''));
          if (progresion === null) return locator;
          return {
            ...locator,
            locations: {
              ...locator.locations,
              progression: progresion,
              totalProgression: progresion
            },
            text: { highlight: String(args.quote) }
          };
        },
        // La consulta sobre la selección. El diccionario de estas
        // pruebas responde a una palabra y omite un proveedor, que es lo que
        // permite comprobar las dos mitades del panel: lo que se encontró y
        // quién se quedó fuera.
        lookup_selection: (args) => ({
          entries:
            String(args.term).toLowerCase() === 'macondo'
              ? []
              : [
                  {
                    providerId: 'es-basico',
                    kind: 'dictionary',
                    headword: String(args.term).toLowerCase(),
                    body: 'Definición de prueba.',
                    source: 'Diccionario de prueba'
                  }
                ],
          skipped: [
            {
              providerId: 'wikipedia',
              label: 'Wikipedia',
              reason: 'networkNotAllowed',
              host: 'es.wikipedia.org'
            }
          ]
        }),
        list_dictionaries: () => [
          {
            id: 'es-basico',
            name: 'Diccionario de prueba',
            language: 'es',
            wordCount: 1200,
            bytes: 2048,
            attribution: null,
            installedAt: 1787900000
          }
        ],
        // Con `__nbVocabulario`, una palabra consultada; de fábrica ninguna.
        list_vocabulary: () =>
          (globalThis as Record<string, unknown>).__nbVocabulario
            ? [
                {
                  entry: {
                    id: 'v1',
                    headword: 'aldea',
                    term: 'aldea',
                    editionId: 'e0',
                    providerId: 'es-basico',
                    locator: enElSegundoParrafo,
                    createdAt: '2026-09-20T10:00:00Z'
                  },
                  title: 'Cien Años de Soledad',
                  fileId: 'f0'
                }
              ]
            : [],
        save_vocabulary: () => 1,
        // El ritmo de lectura. De fábrica no hay bastantes sesiones, como en
        // una instalación nueva, y no se dice nada; `__nbRitmo` pone uno.
        reading_pace: () => (globalThis as Record<string, unknown>).__nbRitmo ?? null,
        vocabulary_to_cards: () => ({ created: 1, already: 0, withoutDefinition: 0 }),
        clear_reading_history: () => null,
        clear_annotations_and_notes: () => null,
        clear_library: () => null,
        // Los catálogos OPDS. De fábrica no hay ninguno, que es lo
        // que hace que Descubrir no se anuncie: un destino cuya única pantalla
        // sería un formulario de configuración no es un destino.
        list_catalogs: () =>
          conCatalogo
            ? [
                {
                  id: 'c1',
                  title: 'Biblioteca de ejemplo',
                  url: 'https://catalogo.example/opds/raiz.xml',
                  addedAt: '2026-08-29T00:00:00Z'
                }
              ]
            : [],
        add_catalog: (args) => ({
          id: 'c1',
          title: String(args.title),
          url: String(args.url),
          addedAt: '2026-08-29T00:00:00Z'
        }),
        remove_catalog: () => null,
        /*
         * Dos feeds y no uno: uno de navegación y la sección a la que lleva.
         * Con un solo feed no se puede comprobar lo que importa —que al entrar
         * en una sección se siga sabiendo en qué catálogo estás, y que haya
         * vuelta—, porque no hay a dónde entrar.
         */
        /*
         * Contesta como el runtime: una página que es un feed o que
         * dice **qué anfitrión** falta autorizar. El catálogo de ejemplo redirige
         * a `espejo.example`, como hacen casi todos los reales, y el permiso que
         * abre el catálogo es el de ese espejo y no el suyo: es lo que hace
         * capaz de fallar a la prueba del bucle de autorización.
         */
        browse_catalog: (args) => {
          const espejoAutorizado = llamadas.some(
            (l) => l.comando === 'allow_catalog_host' && l.args.host === 'espejo.example'
          );
          if (!catalogoAbierto && !espejoAutorizado) {
            return {
              estado: 'needsConsent',
              detalle: { host: 'espejo.example', cleartext: false }
            };
          }
          const url = String(args.url);
          if (url.includes('novedades')) {
            return {
              estado: 'feed',
              detalle: {
                title: 'Novedades',
                entries: [
                  {
                    title: 'Obra Descargable',
                    authors: 'Alguien',
                    summary: null,
                    navigation: null,
                    download: 'https://catalogo.example/obras/o.epub',
                    mediaType: 'application/epub+zip'
                  }
                ],
                next: null
              }
            };
          }
          return {
            estado: 'feed',
            detalle: {
              title: 'Biblioteca de ejemplo',
              entries: [
                {
                  title: 'Novedades',
                  authors: null,
                  summary: 'Las últimas publicaciones.',
                  navigation: 'https://catalogo.example/opds/novedades.xml',
                  download: null,
                  mediaType: null
                }
              ],
              next: null
            }
          };
        },
        /*
         * El registro de importaciones.
         *
         * De fábrica no hay ninguna a medias, que es el estado normal: ofrecer
         * continuar algo que no existe sería el aviso que la gente aprende a
         * ignorar. `importacionAMedias` lo cambia para el recorrido que lo prueba.
         */
        pending_import: () =>
          importacionAMedias
            ? {
                id: 'run-1',
                root: '/una/carpeta/con/libros',
                storageMode: 'referenced',
                startedAt: '2026-09-05T10:00:00Z',
                finishedAt: null,
                processed: 120,
                imported: 90,
                alreadyInLibrary: 20,
                rejected: 10,
                cancelled: false
              }
            : null,
        last_import: () =>
          copiasImportadas
            ? {
                id: 'run-copias',
                root: '/otro/disco/biblioteca',
                storageMode: 'referenced',
                startedAt: '2026-09-24T10:00:00Z',
                finishedAt: '2026-09-24T10:01:00Z',
                processed: 6,
                imported: 4,
                alreadyInLibrary: 0,
                rejected: 0,
                duplicates: 2,
                cancelled: false
              }
            : {
                id: 'run-0',
                root: '/una/carpeta/con/libros',
                storageMode: 'referenced',
                startedAt: '2026-09-04T10:00:00Z',
                finishedAt: '2026-09-04T11:00:00Z',
                processed: 300,
                imported: 280,
                alreadyInLibrary: 18,
                rejected: 2,
                duplicates: 0,
                cancelled: false
              },
        import_duplicates: (args) =>
          args.runId === 'run-copias'
            ? [
                {
                  path: 'descargas/cien años.epub',
                  originalTitle: 'Cien años de soledad',
                  originalPath: 'garcia-marquez/cien-anos.epub',
                  createdAt: '2026-09-24T10:00:30Z'
                },
                {
                  path: 'copias/rayuela (1).epub',
                  originalTitle: 'Rayuela',
                  originalPath: 'cortazar/rayuela.epub',
                  createdAt: '2026-09-24T10:00:40Z'
                }
              ]
            : [],
        import_rejections: () => [
          {
            path: 'roto/documento.epub',
            code: 'DOCUMENT_CORRUPTED',
            createdAt: '2026-09-04T10:30:00Z'
          },
          {
            path: 'otro/archivo.bin',
            code: 'FORMAT_UNSUPPORTED',
            createdAt: '2026-09-04T10:31:00Z'
          }
        ],
        discard_pending_import: () => {
          importacionAMedias = false;
          return null;
        },
        download_from_catalog: () => ({
          estado: 'imported',
          detalle: {
            processed: 1,
            imported: 1,
            alreadyInLibrary: 0,
            rejected: [],
            cancelled: false,
            running: false
          }
        }),
        // El catálogo abierto. De fábrica no hay permiso, y eso es lo
        // que la ficha tiene que saber decir: no es un fallo, falta autorizar.
        /*
         * La búsqueda, que **antes no estaba aquí**.
         *
         * Su ausencia es la razón por la que nadie vio el defecto que arregla
         * pulsar un resultado no abría nada -el manejador recibía la
         * edición y la descartaba- y ningún recorrido tocaba nunca el panel de
         * búsqueda, así que el recorrido no podía verlo. Un simulador que omite
         * una pantalla entera hace que esa pantalla no exista para la
         * verificación.
         *
         * Se sirven **dos resultados y no uno**, y el segundo sin copia
         * disponible: con uno solo, "el primero" y "el que se puede abrir"
         * serían el mismo, y una interfaz que ofreciera abrir lo que no está
         * pasaría la prueba igual.
         */
        /*
         * La búsqueda por significado: un fragmento que también encuentra
         * la léxica —el mismo locator— y otro que solo encuentra ella. Con
         * `__nbSignificadoLento` tarda tres segundos, para comprobar que lo léxico no
         * la espera.
         */
        semantic_search: () => {
          const hits = [
            {
              editionId: 'e0',
              fileId: 'f0',
              locator,
              snippet: 'Muchos años después, frente al pelotón',
              similarity: 0.82
            },
            {
              editionId: 'e0',
              fileId: 'f0',
              locator: { ...locator, href: 'c2.xhtml' },
              snippet: 'Macondo era entonces una aldea de veinte casas',
              similarity: 0.71
            }
          ];
          return (globalThis as Record<string, unknown>).__nbSignificadoLento
            ? new Promise((resolver) => setTimeout(() => resolver(hits), 3000))
            : hits;
        },
        search: (args) => {
          const alcance = args.scope as { scope: string };
          if (alcance.scope === 'catalog') {
            return {
              kind: 'catalog',
              hits: [
                {
                  editionId: 'e0',
                  fileId: 'f0',
                  title: 'Cien Años de Soledad',
                  authors: 'Gabriel García Márquez',
                  snippet: 'Cien Años de Soledad',
                  rank: -1.2
                },
                {
                  editionId: 'e9',
                  fileId: null,
                  title: 'Obra en un Volumen Desconectado',
                  authors: '',
                  snippet: 'Obra en un Volumen Desconectado',
                  rank: -0.4
                }
              ]
            };
          }
          return {
            kind: 'text',
            hits: [
              {
                editionId: 'e0',
                fileId: 'f0',
                locator,
                snippet: 'Muchos años después, frente al pelotón',
                rank: -1.1,
                title: 'Cien Años de Soledad'
              },
              {
                editionId: 'e9',
                fileId: null,
                locator,
                snippet: 'un pasaje de la obra que no está a mano',
                rank: -0.3,
                title: 'Obra en un Volumen Desconectado'
              }
            ]
          };
        },

        edition_details: () => ({
          publisher: 'Editorial de la Ficha',
          publishedDate: '1967-05-30',
          language: 'es',
          description: 'La historia de los Buendía en Macondo.',
          subjects: ['Novela', 'Realismo mágico'],
          files: [
            {
              mediaType: 'application/epub+zip',
              byteSize: 734003,
              relativePath: 'Gabriel García Márquez/Cien años de soledad.epub',
              root: 'biblioteca',
              available: true,
              managed: false
            }
          ]
        }),
        allow_metadata_host: () => {
          metadatosAutorizados = true;
          return null;
        },
        search_online_metadata: () => {
          if (!conFuenteDeExtension && !metadatosAutorizados) throw { code: 'CONSENT_REQUIRED' };
          return [
            {
              title: 'Cien años de soledad',
              authors: 'Gabriel García Márquez',
              publishedDate: '1967',
              publisher: 'Sudamericana',
              language: 'es',
              coverUrl: null
            },
            {
              title: 'Cien años de soledad',
              authors: 'Gabriel García Márquez',
              publishedDate: '1967',
              publisher: 'Editorial Académica',
              language: 'es',
              coverUrl: null,
              // La de la extensión lleva su procedencia. La del núcleo no
              // lleva ninguna, y esa diferencia es justo lo que se comprueba.
              source: 'lat.nbdigitallabs.books.banco.fuente-de-metadatos'
            }
          ];
        },
        // Aplicar un candidato escribe su título si es otro, y lo anuncia: es lo que
        // hace el runtime cuando el campo no tenía una decisión del usuario.
        apply_online_metadata: (args) => {
          const candidato = args.candidate as { title?: string };
          const obra = ediciones.find((e) => e.editionId === args.editionId);
          if (!obra || !candidato.title || candidato.title === obra.title) {
            return { applied: 0, suggested: 0, unchanged: 1 };
          }
          obra.title = candidato.title;
          anunciar('MetadataUpdated', { editionId: obra.editionId });
          return { applied: 1, suggested: 0, unchanged: 0 };
        },
        fetch_online_cover: () => 'e0.jpg',
        pending_suggestions: () => [],
        accept_suggestion: () => null,
        reject_suggestion: () => null,
        // La red. De fábrica no hay ningún permiso
        // concedido y no ha salido nada, que es exactamente el estado inicial
        // real: sin consentimiento no sale nada aunque el interruptor esté
        // apagado.
        network_report: () => ({
          offline: false,
          consents: [],
          log: [],
          // Un catálogo añadido y sin permiso: es el estado de partida real, y
          // el que la vista de Ajustes tiene que saber enseñar.
          pending: [
            {
              host: 'catalogo.example',
              label: 'Catálogo de Ejemplo',
              whatIsSent: 'la dirección del catálogo y lo que busques en él'
            }
          ]
        }),
        allow_catalog_host: () => null,
        allow_speech_host: () => null,
        // Sin servidor escrito, que es como arranca: no hay a dónde enseñar.
        speech_endpoint: () => null,
        // Devuelve una ruta como el runtime; el proveedor de la nube la
        // convierte en una dirección de asset y la reproduce.
        synthesize_speech: () => '/cache/voz/tramo.mp3',
        what_is_sent: () => 'el texto que selecciones',
        allow_lookup_host: () => null,
        revoke_network_consent: () => null,
        delete_vocabulary: () => null,
        clear_vocabulary: () => 0,
        create_annotation: () => ({
          id: 'a1',
          editionId: 'e0',
          kind: 'note',
          body: 'una nota',
          target: locator,
          createdAt: '2026-08-28T00:00:00Z',
          supersedes: null,
          deletedAt: null
        }),
        create_bookmark: () => ({
          id: 'a2',
          editionId: 'e0',
          kind: 'bookmark',
          body: null,
          target: locator,
          createdAt: '2026-08-28T00:00:00Z',
          supersedes: null,
          deletedAt: null
        }),
        save_progress: (args) => {
          if (!pdf) guardadoEnSesion = args.locator as typeof locator;
          // El documento abierto es siempre la primera obra (`open_document`). El
          // runtime escribe con antirrebote y anuncia lo escrito; aquí se escribe
          // siempre, que es lo que vería quien espera cinco segundos.
          const progresion = (args.locator as { locations?: { totalProgression?: number } })
            ?.locations?.totalProgression;
          const obra = ediciones.find((e) => e.editionId === 'e0');
          if (obra && typeof progresion === 'number' && obra.progression !== progresion) {
            obra.progression = progresion;
            anunciar('ProgressSaved', { editionId: obra.editionId, progression: progresion });
          }
          return {
            position: { kind: 'fragment', value: { href: 'c1', offset: 0 } },
            precision: 'exact'
          };
        },
        flush_progress: () => null,
        // Los nombres del libro, como los da el runtime: sacados del texto,
        // con sus sitios en el orden del libro.
        // Y los sitios de uno, según el índice: el primero del recorrido es otro, para
        // que un recorrido pueda ver que se usan los del índice al desplegarlo.
        entity_places: (args) => ({
          fromIndex: true,
          places: args.name === 'Macondo' ? [enElTercerParrafo, enElSegundoParrafo] : [locator]
        }),
        book_entities: () => ({
          entities: [
            {
              name: 'Macondo',
              mentions: 3,
              places: [enElSegundoParrafo]
            },
            { name: 'Aureliano Buendía', mentions: 12, places: [locator, enElTercerParrafo] }
          ]
        }),
        // La narración del libro, como la da el runtime: el audio ya extraído a la
        // caché y un tramo por `par`, con el Locator del párrafo que lee.
        synchronized_audio: (args) => {
          if (!narrado || args.href !== 'OEBPS/c1.xhtml') return null;
          return {
            href: 'OEBPS/c1.xhtml',
            path: '/tmp/nb-books/recursos/narracion-c1.wav',
            durationMs: 3000,
            marks: PARRAFOS.map((parrafo, i) => {
              const inicio = desplazamientoDeLaCita('OEBPS/c1.xhtml', parrafo);
              return {
                startMs: i * 1000,
                endMs: (i + 1) * 1000,
                locator: {
                  ...locator,
                  locations: {
                    fragments: [`#char=${inicio}`],
                    progression: inicio / TEXTO_DEL_CAPITULO.length,
                    totalProgression: inicio / TEXTO_DEL_CAPITULO.length
                  },
                  text: { highlight: parrafo.slice(0, 40) }
                }
              };
            })
          };
        },
        /*
         * El repaso de las carpetas. Con `__nbAparecioEnLaCarpeta` hay un
         * documento nuevo en una carpeta referenciada, y el repaso lo importa y lo
         * anuncia como el runtime; sin él no encuentra nada.
         */
        rescan_roots: () => {
          const global = globalThis as Record<string, unknown>;
          if (!global.__nbAparecioEnLaCarpeta) return null;
          global.__nbAparecioEnLaCarpeta = false;
          const nueva = {
            editionId: 'aparecida',
            title: 'Documento que Apareció en la Carpeta',
            authors: null,
            seriesTitle: null,
            seriesIndex: null,
            availability: 'available',
            progression: null,
            addedAt: '2026-09-23T12:00:00Z',
            copies: 1,
            fileId: 'f-aparecida',
            coverPath: null
          };
          ediciones.unshift(nueva);
          anunciar('DocumentImported', {
            editionId: nueva.editionId,
            fileId: nueva.fileId,
            mediaType: 'application/pdf'
          });
          return null;
        },
        /*
         * El índice dice dónde ha quedado cada anotación **de verdad**.
         *
         * Antes devolvía siempre `c1` con desplazamiento cero, que no es el
         * href de ningún capítulo de este documento —el suyo es
         * `OEBPS/c1.xhtml`— ni el sitio de ninguna cita. Servía para el panel,
         * que solo lista, y no habría servido para nada que pinte: con eso,
         * la prueba no habría podido fallar aunque estuviera rota.
         */
        // Cada formato resuelve contra su propio texto: el capítulo en un EPUB y
        // la página en un PDF. Es lo mismo que hace el runtime, y va aparte para
        // que la función de arriba siga leyéndose.
        annotation_outline: () =>
          anotaciones.map((a) => {
            const perdida = a.id === PERDIDA;
            const cita = a.target.text?.highlight ?? '';
            const desplazamiento = desplazamientoDeLaCita(a.target.href, cita);
            return {
              annotation: a,
              position: perdida
                ? null
                : {
                    kind: 'fragment',
                    value: { href: a.target.href, offset: Math.max(0, desplazamiento) }
                  },
              precision: perdida ? null : 'exact',
              relocated: false,
              warn: false,
              lost: perdida
            };
          }),
        edit_annotation: (args) => {
          const previa = anotaciones.find((a) => a.id === args.id);
          if (!previa) throw { code: 'DOCUMENT_NOT_FOUND' };
          // Editar da de baja y crea la sucesora, con identificador nuevo. Es lo
          // que hace el runtime, y el panel tiene que quedarse con el nuevo.
          const sucesora = {
            ...previa,
            id: `${previa.id}-b`,
            body: (args.body as string | null) ?? null,
            supersedes: previa.id
          };
          anotaciones = anotaciones.map((a) => (a.id === previa.id ? sucesora : a));
          return sucesora;
        },
        delete_annotation: (args) => {
          anotaciones = anotaciones.filter((a) => a.id !== args.id);
          return null;
        },
        library_annotations: (args) =>
          anotaciones.filter((a) => !args.kind || a.kind === args.kind).map(conContexto),

        /*
         * Los pasajes que una nota cita, y quién cita un pasaje.
         *
         * El simulador guarda las citas de verdad, en lugar de responder una
         * lista fija: lo que se comprueba es el ciclo entero —citar, verlo desde
         * los dos lados, quitarlo—, y con respuestas fijas eso no se puede ver.
         */
        cite_annotation: (args) => {
          const par = { noteId: args.noteId as string, annotationId: args.annotationId as string };
          if (!citas.some((c) => c.noteId === par.noteId && c.annotationId === par.annotationId)) {
            citas.push(par);
          }
          return null;
        },
        uncite_annotation: (args) => {
          citas = citas.filter(
            (c) => !(c.noteId === args.noteId && c.annotationId === args.annotationId)
          );
          return null;
        },
        cited_annotations: (args) =>
          citas
            .filter((c) => c.noteId === args.noteId)
            .map((c) => anotaciones.find((a) => a.id === c.annotationId))
            .filter((a) => a !== undefined)
            .map(conContexto),
        notes_citing: (args) =>
          citas
            .filter((c) => c.annotationId === args.annotationId)
            .map((c) => notas.find((n) => n.id === c.noteId))
            .filter((n) => n !== undefined),

        // El puente de eventos de Tauri pasa por `invoke` como cualquier otro
        // comando: sin esto, suscribirse rechaza y la aplicación se queda sin
        // enterarse de lo que ocurre por detrás.
        'plugin:event|listen': (args) =>
          ((globalThis as Record<string, unknown>).__nbEscuchar as (a: unknown) => number)(args),
        'plugin:event|unlisten': () => null,

        // El diálogo del sistema, que aquí contesta siempre con la misma
        // carpeta. Lo que se comprueba con él no es que el diálogo funcione
        // —eso es del sistema— sino que la interfaz hace algo con lo que
        // devuelve, que es lo que faltaba.
        'plugin:dialog|open': () => '/otro/disco/biblioteca',
        // Un archivo suelto entra de una vez y se anuncia, como en el runtime; una
        // carpeta seguiría en marcha y avisaría por su propio avance.
        import_path: (args) => {
          if (conCopias && args.path === '/otro/disco/biblioteca') {
            const cifras = { processed: 6, imported: 4, skipped: 0, rejected: 0, duplicates: 2 };
            setTimeout(() => anunciar('BulkImportProgress', { ...cifras, total: 6 }), 100);
            setTimeout(() => {
              copiasImportadas = true;
              anunciar('BulkImportFinished', { ...cifras, cancelled: false });
            }, 900);
            return {
              processed: 0,
              imported: 0,
              alreadyInLibrary: 0,
              duplicates: 0,
              rejected: [],
              cancelled: false,
              running: true
            };
          }
          importados += 1;
          const nueva = {
            editionId: `importada-${importados}`,
            title: `Documento Importado ${importados}`,
            authors: null,
            seriesTitle: null,
            seriesIndex: null,
            availability: 'available',
            progression: null,
            addedAt: '2026-09-23T10:00:00Z',
            copies: 1,
            fileId: `fi${importados}`,
            coverPath: null
          };
          // Lo último que entra va delante, que es como ordena "por fecha".
          ediciones.unshift(nueva);
          anunciar('DocumentImported', {
            editionId: nueva.editionId,
            fileId: nueva.fileId,
            mediaType: 'application/epub+zip'
          });
          return {
            processed: 1,
            imported: 1,
            alreadyInLibrary: 0,
            duplicates: 0,
            rejected: [],
            cancelled: false,
            running: false
          };
        },
        'plugin:dialog|save': () => '/tmp/anotaciones.json',
        save_annotations: () => null,
        legal_notices: () => ({
          license: 'Apache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/',
          notice: 'NB Books\nCopyright 2026 The NB Books Authors',
          thirdParty:
            '# Avisos de Terceros\n\n| Componente | Versión | Licencia | Origen |\n|---|---|---|---|'
        })
      };

      if (busquedaLenta) {
        const deSiempre = respuestas.search;
        respuestas.search = (args) =>
          (args.scope as { scope: string }).scope === 'catalog'
            ? deSiempre(args)
            : new Promise((resolver) => setTimeout(() => resolver(deSiempre(args)), 1500));
      }

      // Darse de baja de un evento pasa por aquí en Tauri. Antes el
      // simulador no lo tenía porque ningún recorrido cerraba un panel suscrito a
      // eventos; el de preparar audio lo hace.
      (globalThis as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: () => {}
      };
      (globalThis as Record<string, unknown>).__TAURI_INTERNALS__ = {
        invoke: (comando: string, args: Record<string, unknown> = {}) => {
          llamadas.push({ comando, args });

          const responder = () => {
            if (rechaza.includes(comando)) return Promise.reject({ code: 'INTERNAL' });
            const respuesta = respuestas[comando];
            if (!respuesta) return Promise.reject({ code: 'INTERNAL' });
            try {
              // La respuesta cruza el puente **como JSON**, igual que en Tauri. Sin
              // esto la interfaz recibía los mismos objetos que guarda el
              // simulador, y un título corregido aquí aparecía en pantalla sin que
              // nadie lo volviera a pedir: un banco en el que "la vista no se
              // enteró" no podía fallar nunca.
              return Promise.resolve(respuesta(args)).then((valor) =>
                valor === undefined ? valor : JSON.parse(JSON.stringify(valor))
              );
            } catch (error) {
              return Promise.reject(error);
            }
          };

          // La latencia se añade **después** de apuntar la llamada: lo que
          // interesa es que la interfaz siga viva mientras espera, no que tarde
          // en pedir.
          if (lento > 0) {
            return new Promise((resolver, rechazar) => {
              setTimeout(() => responder().then(resolver, rechazar), lento);
            });
          }
          return responder();
        },
        transformCallback: (callback: unknown) => callback,
        // Como el de verdad: una ruta del sistema no es una dirección, y el
        // puente la convierte en una que el webview sabe pedir. Devolverla tal
        // cual —como hacía este simulador— hacía que ninguna prueba pudiera
        // notar la diferencia entre convertir y no convertir, y por eso pasó
        // desapercibido que las imágenes de los libros se pedían al servidor de
        // desarrollo, que respondía 404 a cada una.
        //
        // Se usa la forma de Windows —`http://asset.localhost/...`— y no la de
        // Linux —`asset://localhost/...`— porque las dos son reales y esta se
        // puede servir en el navegador de las pruebas. Con un esquema propio,
        // cada imagen quedaría sin cargar y el navegador registraría un error de
        // consola por cada una, que es justo lo que la suite prohíbe.
        convertFileSrc: (ruta: string) => `http://asset.localhost/${encodeURIComponent(ruta)}`
      };
    },
    {
      obras: opciones.vacia ? [] : OBRAS,
      cuantas: opciones.vacia ? 0 : (opciones.obras ?? OBRAS.length),
      rechaza: opciones.rechaza ?? [],
      lento: opciones.lento ?? 0,
      recuperada: opciones.recuperada ?? false,
      conCatalogo: opciones.conCatalogo ?? false,
      conFuenteDeExtension: opciones.conFuenteDeExtension ?? false,
      catalogoAbierto: opciones.catalogoAbierto ?? false,
      aMedias: opciones.aMedias ?? false,
      pdf: opciones.pdf ?? false,
      parrafos: opciones.parrafos ?? 0,
      capitulo: opciones.capitulo,
      omitidos: opciones.omitidos,
      narrado: opciones.narrado,
      envuelto: opciones.envuelto ?? false,
      anotacionesExtra: opciones.anotacionesExtra,
      runtimeEnEspera: opciones.runtimeEnEspera ?? false,
      tableroLleno: opciones.tableroLleno ?? false,
      estanteriasDesordenadas: opciones.estanteriasDesordenadas ?? false,
      sinPortadas: opciones.sinPortadas ?? false,
      busquedaLenta: opciones.busquedaLenta ?? false,
      indiceSinSitio: opciones.indiceSinSitio ?? false,
      duplicados: opciones.duplicados ?? 0
    }
  );
}

/** Los comandos que la interfaz ha pedido, en orden. */
export async function comandosPedidos(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    ((globalThis as Record<string, unknown>).__nbLlamadas as Array<{ comando: string }>).map(
      (l) => l.comando
    )
  );
}

/**
 * Un destino de Estudio en su navegación de la izquierda.
 *
 * Antes eran pestañas. `first` porque el nombre de un destino puede
 * repetirse en un subdestino -"Notas" es una vista y una clase de anotación-, y
 * el destino va antes en el orden del documento.
 */
export function vistaDeEstudio(page: Page, nombre: string | RegExp) {
  return page
    .getByRole('navigation', { name: 'Secciones de Estudio' })
    .getByRole('button', { name: nombre })
    .first();
}

/**
 * Los argumentos con los que se pidió un comando, en orden.
 *
 * `comandosPedidos` dice **que** se llamó; esto dice **con qué**. Saltar a una
 * anotación se distingue de no saltar justamente en el argumento: el locator.
 */
export async function argumentosDe(page: Page, comando: string): Promise<unknown[]> {
  return page.evaluate((buscado) => {
    const llamadas = (globalThis as Record<string, unknown>).__nbLlamadas as Array<{
      comando: string;
      args: unknown;
    }>;
    return llamadas.filter((l) => l.comando === buscado).map((l) => l.args);
  }, comando);
}
