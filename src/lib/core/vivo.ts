/**
 * Lo que cambió, dicho por quien lo escribió.
 *
 * Contrato: `docs/design/product-philosophy.md`, sección 7,
 * principios 1 y 2, y `docs/architecture/api/events.md`.
 *
 * # Por qué existe
 *
 * Antes cada sitio de la interfaz que escribía algo se encargaba
 * de refrescar lo que creía que dependía de ello: aplicar un candidato de un
 * catálogo recargaba la biblioteca y no Inicio, añadir una obra a una colección
 * releía la lista de colecciones y no la fila de Inicio que la pinta, y avanzar
 * en el lector no movía nada de lo demás hasta cerrarlo. Es exactamente "dos
 * copias que alguien sincroniza", y cada sitio nuevo que escribiera tenía que
 * acordarse de todos los que leen.
 *
 * Ahora **quien escribe es el runtime, y el runtime lo anuncia** después de
 * confirmarlo. Esta capa oye esos anuncios, los agrupa -una importación de cien
 * mil documentos son cien mil anuncios- y le dice a la carcasa qué hay que
 * volver a leer. Quien pinta no necesita saber quién escribió, y quien escribe no
 * necesita saber quién pinta.
 *
 * # Lo que no hace
 *
 * No guarda ningún dato. Una caché aquí sería la segunda copia que esto viene a
 * quitar: lo único que sale de esta capa es **qué** releer, y lo que se relee
 * se le pregunta al runtime, que es la única verdad.
 */

import type { DomainEvent } from '$lib/bindings';
import { onDomainEvent } from './ipc';

/**
 * De qué depende lo que se pinta.
 *
 * `obras` es lo que se sabe de cada edición -título, autoría, portada, si está
 * en la biblioteca-; `progreso`, dónde va la lectura de cada una; `colecciones`,
 * qué colecciones hay y qué tienen; `extensiones`, lo que aporta lo instalado.
 */
export type Tema = 'obras' | 'progreso' | 'colecciones' | 'extensiones';

/** Lo que cambió desde el último aviso, agrupado. */
export interface Cambios {
  readonly temas: ReadonlySet<Tema>;
  /** Las ediciones que cambiaron, cuando el anuncio las dice. */
  readonly ediciones: ReadonlySet<string>;
  /** La última progresión anunciada de cada edición. */
  readonly progresos: ReadonlyMap<string, number | null>;
}

/** Los anuncios que dicen que algo que se pinta cambió. */
export const ANUNCIOS = [
  'DocumentImported',
  'DocumentDeleted',
  'AvailabilityChanged',
  'BulkImportFinished',
  'MetadataUpdated',
  'CoversRecovered',
  'ProgressSaved',
  'CollectionsChanged',
  'ExtensionsChanged'
] as const satisfies readonly DomainEvent['type'][];

export type Anuncio = Extract<DomainEvent, { type: (typeof ANUNCIOS)[number] }>;

/** Una acumulación vacía, para empezar. */
export function sinCambios(): {
  temas: Set<Tema>;
  ediciones: Set<string>;
  progresos: Map<string, number | null>;
} {
  return { temas: new Set(), ediciones: new Set(), progresos: new Map() };
}

/**
 * Suma un anuncio a lo acumulado.
 *
 * Pura a propósito: qué anuncio toca qué es la regla que decide si una pantalla
 * se entera, y una regla así se prueba sin navegador.
 */
export function acumular(cambios: ReturnType<typeof sinCambios>, anuncio: Anuncio): void {
  switch (anuncio.type) {
    case 'DocumentImported':
    case 'DocumentDeleted':
    case 'AvailabilityChanged':
    case 'MetadataUpdated':
      cambios.temas.add('obras');
      cambios.ediciones.add(anuncio.editionId);
      return;
    case 'BulkImportFinished':
    case 'CoversRecovered':
      cambios.temas.add('obras');
      return;
    case 'ProgressSaved':
      cambios.temas.add('progreso');
      cambios.ediciones.add(anuncio.editionId);
      cambios.progresos.set(anuncio.editionId, anuncio.progression);
      return;
    case 'CollectionsChanged':
      cambios.temas.add('colecciones');
      return;
    case 'ExtensionsChanged':
      cambios.temas.add('extensiones');
      return;
    default:
      // Un anuncio de `ANUNCIOS` sin su caso aquí no compila.
      anuncio satisfies never;
  }
}

/**
 * Cuánto se espera a que dejen de llegar anuncios antes de avisar.
 *
 * Una importación grande publica un documento tras otro, y sin agrupar cada uno
 * releería Inicio entero. Ciento cincuenta milisegundos no se perciben como
 * espera y bastan para que una ráfaga sea un solo aviso.
 */
export const AGRUPAR_MS = 150;

/**
 * Cada cuánto se avisa como mínimo aunque la ráfaga no termine.
 *
 * Sin esto, una importación de horas no avisaría hasta el final, porque los
 * anuncios nunca dejarían de llegar: agrupar no puede convertirse en callar.
 */
export const COMO_MUCHO_MS = 2000;

/**
 * Escucha los anuncios y avisa de lo que cambió, agrupado.
 *
 * Devuelve la función de baja. Si el bus no responde, la promesa se rechaza y
 * quien llama decide: la interfaz sigue respondiendo a lo que el usuario hace, y
 * lo que pierde es enterarse sola.
 */
export async function escucharCambios(
  alCambiar: (cambios: Cambios) => void,
  opciones: { agrupar?: number; comoMucho?: number } = {}
): Promise<() => void> {
  const agrupar = opciones.agrupar ?? AGRUPAR_MS;
  const comoMucho = opciones.comoMucho ?? COMO_MUCHO_MS;
  let pendientes = sinCambios();
  let espera: ReturnType<typeof setTimeout> | null = null;
  let primero: number | null = null;

  const avisar = () => {
    espera = null;
    primero = null;
    const hechos = pendientes;
    pendientes = sinCambios();
    if (hechos.temas.size > 0) alCambiar(hechos);
  };

  const oir = (anuncio: Anuncio) => {
    acumular(pendientes, anuncio);
    const ahora = Date.now();
    primero ??= ahora;
    if (espera !== null) clearTimeout(espera);
    // Se espera a que la ráfaga pare, pero nunca más de `comoMucho` desde el
    // primer anuncio sin avisar.
    const queda = Math.max(0, Math.min(agrupar, primero + comoMucho - ahora));
    espera = setTimeout(avisar, queda);
  };

  const bajas = await Promise.all(
    ANUNCIOS.map((tipo) => onDomainEvent(tipo, (e) => oir(e as Anuncio)))
  );
  return () => {
    if (espera !== null) clearTimeout(espera);
    for (const baja of bajas) baja();
  };
}
