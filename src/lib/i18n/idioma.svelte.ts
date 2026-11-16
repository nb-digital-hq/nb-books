/**
 * El idioma de la interfaz, y los textos que le corresponden.
 *
 * # Por qué el español viaja siempre y los demás se cargan
 *
 * Antes se importaban los tres diccionarios, con un argumento que
 * entonces era cierto: costaban unos kilobytes, y cargarlos después abría el
 * instante de interfaz sin traducir que se quería evitar. Medido al partir el
 * paquete, los dos que no se usan pesaban 53 KB, **más de la quinta
 * parte del presupuesto de arranque**, para un idioma que la persona no eligió.
 * `i18n.md` dejaba escrito que se rediscutiría cuando el peso importara, y ese es
 * el dato.
 *
 * El destello no vuelve, porque **la espera ocurre antes de montar nada**:
 * `+layout.ts` carga en su `load` el diccionario que dice la caché de arranque,
 * y SvelteKit no pinta la interfaz hasta que termina. Al cambiar de idioma con la
 * aplicación abierta, la interfaz sigue en el anterior hasta que llega el nuevo:
 * nunca pasa por un texto a medias. El español va siempre porque es el base, al
 * que se vuelve cuando algo no se reconoce, y ese camino no puede esperar.
 *
 * # De dónde sale el idioma en el primer fotograma
 *
 * De la caché de arranque de ADR-010, igual que el tema: `language` es una
 * preferencia que afecta al primer pintado, así que se lee sin esperar al
 * puente. Cuando el runtime responde, `+page.svelte` confirma el valor.
 *
 * Contrato: `docs/architecture/i18n.md` y
 * `docs/adr/010-settings-persistence.md`.
 */

import { type Diccionario, esIdioma, IDIOMA_BASE, type Idioma } from './diccionario';
import { es } from './locales/es';

type Cargable = Exclude<Idioma, 'es'>;

const CARGAS: Record<Cargable, () => Promise<Diccionario>> = {
  en: () => import('./locales/en').then((modulo) => modulo.en),
  pt: () => import('./locales/pt').then((modulo) => modulo.pt)
};

/** Los que ya están en memoria. El base, desde el principio. */
const DICCIONARIOS: Partial<Record<Idioma, Diccionario>> = { es: es as Diccionario };

let actual = $state<Idioma>(IDIOMA_BASE);

/** El último pedido: un diccionario que llega tarde no pisa a otro elegido después. */
let pedido: Idioma = IDIOMA_BASE;

/** El idioma en uso. */
export function idioma(): Idioma {
  return actual;
}

/**
 * Deja en memoria el diccionario de un idioma, sin cambiar a él.
 *
 * Un valor que no se reconoce vuelve al base en lugar de fallar: el idioma
 * puede venir de una preferencia guardada por una versión anterior, y dejar la
 * interfaz sin texto sería peor que enseñarla en español.
 */
export async function cargarIdioma(valor: unknown): Promise<Idioma> {
  const elegido = esIdioma(valor) ? valor : IDIOMA_BASE;
  if (!DICCIONARIOS[elegido]) {
    DICCIONARIOS[elegido] = await CARGAS[elegido as Cargable]();
  }
  return elegido;
}

/**
 * Cambia el idioma.
 *
 * Si su diccionario ya está en memoria el cambio es inmediato, que es el caso
 * del arranque. Si no, se carga primero y se cambia después.
 */
export function usarIdioma(valor: unknown): Promise<void> {
  const elegido = esIdioma(valor) ? valor : IDIOMA_BASE;
  pedido = elegido;
  if (DICCIONARIOS[elegido]) {
    actual = elegido;
    return Promise.resolve();
  }
  return cargarIdioma(elegido).then(() => {
    if (pedido === elegido) actual = elegido;
  });
}

/**
 * Los textos del idioma en uso.
 *
 * Se llama como función y no se exporta el objeto para que la lectura ocurra
 * **dentro** de quien la usa: exportar el diccionario ya resuelto capturaría el
 * valor del momento en que se importó el módulo, y cambiar de idioma no
 * repintaría nada.
 */
export function textos(): Diccionario {
  return DICCIONARIOS[actual] ?? (es as Diccionario);
}
