/**
 * Qué voz se usa para leer en alto.
 *
 * # Por qué esto es una función y no un valor por defecto
 *
 * Porque el valor por defecto que había era **la primera voz del catálogo**, y
 * el catálogo de `speech-dispatcher` viene ordenado alfabéticamente por idioma:
 * la primera es afrikáans. Una novela en español se leía con una voz afrikáans,
 * y eso no se oye como "la voz está mal elegida", se oye como que la función no
 * sirve: no vocaliza, no se entiende y no hay forma de saber por qué.
 *
 * Elegir bien es una regla con cuatro pasos y ningún caso especial, y por eso
 * está aquí, aparte y probada, en lugar de repartida entre el panel y la ruta.
 *
 * Contrato: `docs/architecture/voice-services.md`.
 */

import type { VozDisponible } from './contrato';

/** Idioma base de una etiqueta BCP-47: `es-MX` es `es`, y `es` es `es`. */
export function idiomaBase(etiqueta: string): string {
  return etiqueta.trim().toLowerCase().split(/[-_]/)[0] ?? '';
}

/**
 * La voz con la que se lee, en orden de preferencia.
 *
 * 1. **La que el usuario eligió**, si sigue existiendo. Una elección explícita
 *    gana siempre: es la única que sabemos que es deliberada.
 * 2. La del idioma exacto del documento (`es-MX` con `es-MX`).
 * 3. La del idioma base del documento (`es-MX` con `es`).
 * 4. Lo mismo con el idioma de la interfaz, que es el que la persona lee.
 * 5. La primera del catálogo. Es el último recurso y sigue siendo mala, pero a
 *    estas alturas ya no hay ningún dato con el que hacerlo mejor.
 *
 * Devuelve `null` si no hay ninguna voz: sin catálogo no hay nada que elegir, y
 * decir que sí lo hay obligaría a quien llama a comprobarlo igualmente.
 */
export function elegirVoz(
  voces: readonly VozDisponible[],
  opciones: {
    /** Idioma del documento abierto, si se conoce. */
    documento?: string | null;
    /** Idioma de la interfaz. Siempre hay uno. */
    interfaz: string;
    /** La que el usuario eligió y se guardó. */
    guardada?: string | null;
  }
): string | null {
  if (voces.length === 0) return null;

  const guardada = opciones.guardada?.trim();
  if (guardada && voces.some((v) => v.id === guardada)) return guardada;

  for (const etiqueta of [opciones.documento, opciones.interfaz]) {
    const buscada = etiqueta?.trim().toLowerCase();
    if (!buscada) continue;

    const exacta = voces.find((v) => v.idioma.trim().toLowerCase() === buscada);
    if (exacta) return exacta.id;

    /*
     * Sin variante exacta, la voz **genérica** del idioma antes que cualquier
     * variante suya: para `es-MX`, `es` antes que `es-419`.
     *
     * Es lo predecible. Al revés —quedarse con la primera etiqueta que empiece
     * por `es`— la elección dependía del orden en que el sintetizador
     * devolviera su catálogo, que no es ningún orden y cambia entre versiones.
     */
    const base = idiomaBase(buscada);
    const generica = voces.find((v) => v.idioma.trim().toLowerCase() === base);
    if (generica) return generica.id;

    const porBase = voces.find((v) => idiomaBase(v.idioma) === base);
    if (porBase) return porBase.id;
  }

  return voces[0]?.id ?? null;
}

/**
 * El catálogo listo para enseñarlo: con el nombre del idioma en la lengua de
 * quien mira, y ordenado por él.
 *
 * Son ciento treinta idiomas. Sin nombre legible, el desplegable es una lista de
 * códigos —`af`, `bpy`, `hyw`— que no se puede recorrer buscando el propio; sin
 * ordenar, el orden es el que devuelva el sintetizador, que no es ninguno.
 *
 * `Intl.DisplayNames` puede no conocer una etiqueta: entonces se queda el código,
 * que es peor que un nombre y mejor que esconder la voz.
 */
export function catalogoLegible(
  voces: readonly VozDisponible[],
  idiomaDeLaInterfaz: string
): { id: string; etiqueta: string }[] {
  let nombreDe: (etiqueta: string) => string;
  try {
    const nombres = new Intl.DisplayNames([idiomaDeLaInterfaz], { type: 'language' });
    nombreDe = (etiqueta) => nombres.of(etiqueta) ?? etiqueta;
  } catch {
    nombreDe = (etiqueta) => etiqueta;
  }

  const comparador = new Intl.Collator(idiomaDeLaInterfaz);
  return voces
    .map((voz) => {
      let nombre = voz.idioma;
      try {
        nombre = nombreDe(voz.idioma);
      } catch {
        // Una etiqueta que no es BCP-47 hace saltar a `Intl`. Se queda el código.
      }
      return { id: voz.id, etiqueta: nombre };
    })
    .sort((a, b) => comparador.compare(a.etiqueta, b.etiqueta));
}
