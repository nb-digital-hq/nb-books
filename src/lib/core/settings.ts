/**
 * Preferencias del usuario y su caché de arranque.
 *
 * SQLite es la fuente de verdad. Esta capa mantiene además una **caché de solo
 * lectura** en el almacenamiento del webview con los cinco valores que afectan
 * al primer pintado, por un motivo que no es cosmético: el webview se pinta
 * antes de que el runtime responda, y pintar con los valores por defecto para
 * corregirlos después produce un destello de tema claro para quien usa tema
 * oscuro, o de tipografía sin escalar para quien depende de la escala. Para
 * quien necesita esas preferencias, ese destello es una barrera real.
 *
 * Las cuatro reglas de coherencia son las de
 * `docs/adr/010-settings-persistence.md`:
 *
 * 1. Escribir una preferencia escribe **primero** en SQLite; la caché después.
 * 2. Al arrancar, la caché pinta; cuando llega el valor de SQLite, si difiere,
 *    gana SQLite y la caché se reescribe.
 * 3. Una caché ausente o corrupta no es un error: se pinta con los valores por
 *    defecto y se reconstruye.
 * 4. Los secretos quedan fuera de ambos sitios.
 */

import { FIRST_PAINT_DEFAULTS, type Setting, type SettingValue } from '$lib/bindings';

import { settingsSnapshot, updateSetting } from './ipc';

/** Clave bajo la que vive la caché en el almacenamiento del webview. */
const CLAVE_CACHE = 'nb-books.first-paint.v1';

/**
 * Valores por defecto de las cinco preferencias del primer pintado.
 *
 * **Se generan desde Rust** (`src-tauri/src/settings.rs`), no se escriben aquí:
 * son lo que se pinta cuando todavía no ha respondido nadie, así que no pueden
 * depender de una llamada al puente, y escribirlos a mano los condenaría a
 * divergir en silencio. La consecuencia de esa divergencia sería un destello de
 * tema claro para quien usa tema oscuro.
 */
export const PRIMER_PINTADO_POR_DEFECTO: Record<string, SettingValue> = FIRST_PAINT_DEFAULTS;

/** Las cinco claves que entran en la caché. Ninguna más. */
export const CLAVES_PRIMER_PINTADO = Object.keys(PRIMER_PINTADO_POR_DEFECTO);

function almacen(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    // Un webview con el almacenamiento desactivado no es un error: es un modo
    // de funcionamiento con una preferencia menos, no una aplicación rota.
    return null;
  }
}

/**
 * Lee la caché de arranque.
 *
 * Nunca falla: una caché ausente, ilegible o con claves que no reconoce se
 * trata como una caché vacía y se completa con los valores por defecto.
 */
export function leerCacheDeArranque(): Record<string, SettingValue> {
  const valores = { ...PRIMER_PINTADO_POR_DEFECTO };
  const store = almacen();
  if (!store) return valores;

  try {
    const crudo = store.getItem(CLAVE_CACHE);
    if (!crudo) return valores;

    const leido: unknown = JSON.parse(crudo);
    if (typeof leido !== 'object' || leido === null || Array.isArray(leido)) return valores;

    for (const [clave, valor] of Object.entries(leido as Record<string, unknown>)) {
      // Una clave que no es del primer pintado no se acepta aunque venga en la
      // caché: es la forma de que la caché no se convierta en un espejo de toda
      // la configuración.
      if (!CLAVES_PRIMER_PINTADO.includes(clave)) continue;
      if (typeof valor !== typeof PRIMER_PINTADO_POR_DEFECTO[clave]) continue;
      valores[clave] = valor as SettingValue;
    }
  } catch {
    // JSON corrupto: se ignora y se reconstruye en la primera escritura.
  }

  return valores;
}

/** Reescribe la caché a partir de lo que dice el runtime, que es quien manda. */
export function escribirCacheDeArranque(preferencias: readonly Setting[]): void {
  const store = almacen();
  if (!store) return;

  const soloArranque: Record<string, SettingValue> = {};
  for (const preferencia of preferencias) {
    if (preferencia.affectsFirstPaint) soloArranque[preferencia.key] = preferencia.value;
  }

  try {
    store.setItem(CLAVE_CACHE, JSON.stringify(soloArranque));
  } catch {
    // Cuota agotada o almacenamiento en solo lectura. Perder la caché cuesta un
    // destello al arrancar; impedir que la aplicación siga costaría mucho más.
  }
}

/** Borra la caché. Existe para poder demostrar que no se pierde nada al hacerlo. */
export function olvidarCacheDeArranque(): void {
  try {
    almacen()?.removeItem(CLAVE_CACHE);
  } catch {
    // Ver arriba.
  }
}

/**
 * Carga las preferencias del runtime y deja la caché al día.
 *
 * Es la regla 2: la caché pinta primero, y cuando llega el valor de SQLite, si
 * difiere, gana SQLite.
 */
export async function cargarPreferencias(): Promise<Setting[]> {
  const preferencias = await settingsSnapshot();
  escribirCacheDeArranque(preferencias);
  return preferencias;
}

/**
 * Escribe una preferencia.
 *
 * Es la regla 1: primero SQLite, y solo después la caché, y con el valor que
 * responde el runtime. Al revés, un fallo de escritura dejaría la caché
 * afirmando algo que la fuente de verdad no dice.
 */
export async function guardarPreferencia(
  key: string,
  value: SettingValue,
  actuales: readonly Setting[] = []
): Promise<Setting> {
  const guardada = await updateSetting(key, value);

  if (guardada.affectsFirstPaint) {
    const siguientes = actuales.filter((s) => s.key !== guardada.key).concat(guardada);
    escribirCacheDeArranque(siguientes);
  }

  return guardada;
}
