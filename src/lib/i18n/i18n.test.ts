/**
 * Los criterios de la internacionalización que se pueden comprobar sin lint.
 *
 * * Los textos están externalizados. La mitad que vigila que no vuelvan
 *   al marcado es `scripts/check-i18n.mjs`, que corre en `pnpm verify`; aquí se
 *   comprueba lo otro, que los tres diccionarios estén completos y que ninguna
 *   clave se quede sin traducir de verdad.
 * * **Todo** código de error tiene mensaje en todos los idiomas. La
 *   lista de códigos la genera Rust, así que añadir una variante allí y no
 *   traducirla aquí hace fallar esta prueba sin que nadie tenga que acordarse.
 *
 * Por qué no basta con el tipo: `Diccionario` obliga a que la clave exista, no
 * a que diga algo. Una cadena vacía compila y deja un botón sin nombre.
 */

import { describe, expect, it } from 'vitest';

import { ERROR_CODES } from '$lib/bindings/errorCodes';

import { COMANDOS } from '../ui/lector/comandos';
import { DESTINOS } from '../ui/lector/destinos';
import { MODOS } from '../ui/lector/modos';
import { type Diccionario, IDIOMAS, type Idioma } from './diccionario';
import { en } from './locales/en';
import { es } from './locales/es';
import { TEXTOS_DE_ESTUDIO } from './locales/estudio';
import { TEXTOS_DE_IA } from './locales/ia';
import { pt } from './locales/pt';
import { TEXTOS_DE_SIGNIFICADO } from './locales/significado';

/**
 * Cada diccionario con los textos que viajan aparte para no entrar en el arranque
 * —los de la IA en Ajustes, los de la búsqueda por significado y los de estudiar con
 * IA—, que tienen que
 * cumplir lo mismo.
 */
const DICCIONARIOS: Record<Idioma, Diccionario> = {
  es: {
    ...(es as Diccionario),
    ia: TEXTOS_DE_IA.es,
    significado: TEXTOS_DE_SIGNIFICADO.es,
    estudioConIa: TEXTOS_DE_ESTUDIO.es
  } as Diccionario,
  en: {
    ...en,
    ia: TEXTOS_DE_IA.en,
    significado: TEXTOS_DE_SIGNIFICADO.en,
    estudioConIa: TEXTOS_DE_ESTUDIO.en
  } as Diccionario,
  pt: {
    ...pt,
    ia: TEXTOS_DE_IA.pt,
    significado: TEXTOS_DE_SIGNIFICADO.pt,
    estudioConIa: TEXTOS_DE_ESTUDIO.pt
  } as Diccionario
};

/** Todas las claves de un diccionario, en la forma `grupo.clave`. */
function claves(diccionario: Diccionario): string[] {
  return Object.entries(diccionario)
    .flatMap(([grupo, textos]) => Object.keys(textos).map((clave) => `${grupo}.${clave}`))
    .sort();
}

/** Todos los valores de un diccionario, con su ruta, para poder señalar cuál. */
function entradas(diccionario: Diccionario): Array<[string, string]> {
  return Object.entries(diccionario).flatMap(([grupo, textos]) =>
    Object.entries(textos).map(([clave, valor]) => [`${grupo}.${clave}`, valor] as [string, string])
  );
}

describe('los tres diccionarios dicen lo mismo', () => {
  it('tienen exactamente las mismas claves', () => {
    // El tipo ya lo exige, pero el tipo se puede silenciar con un `as` y esto
    // no: si alguien fuerza una traducción incompleta, aquí se ve.
    const base = claves(DICCIONARIOS.es);
    for (const idioma of IDIOMAS) {
      expect(claves(DICCIONARIOS[idioma]), `${idioma} no coincide con el español`).toEqual(base);
    }
  });

  it('ninguna clave está vacía ni es un espacio en blanco', () => {
    for (const idioma of IDIOMAS) {
      for (const [ruta, valor] of entradas(DICCIONARIOS[idioma])) {
        expect(valor.trim().length, `${idioma}: ${ruta} está vacía`).toBeGreaterThan(0);
      }
    }
  });

  it('el español no se cuela sin traducir en los otros idiomas', () => {
    // Una clave copiada tal cual del español es la forma más común de creerse
    // que algo está traducido. No vale para todas —"Sepia" y "min" se escriben
    // igual en los tres—, así que se mide sobre las frases: a partir de tres
    // palabras, coincidir exactamente ya no es una coincidencia.
    const base = new Map(entradas(DICCIONARIOS.es));
    for (const idioma of ['en', 'pt'] as const) {
      for (const [ruta, valor] of entradas(DICCIONARIOS[idioma])) {
        const español = base.get(ruta) ?? '';
        if (español.split(/\s+/).length < 3) continue;
        if (IGUALES_A_PROPOSITO[idioma]?.includes(ruta)) continue;
        expect(valor, `${idioma}: ${ruta} sigue en español`).not.toBe(español);
      }
    }
  });
});

/**
 * Frases que **de verdad** se escriben igual en dos idiomas.
 *
 * La regla de las tres palabras acierta casi siempre y falla donde dos lenguas
 * cercanas coinciden de verdad. Cambiar el texto para que la prueba pase sería
 * traducir peor a propósito, así que se apunta aquí **con su motivo**: que
 * añadir una cueste escribir por qué es exactamente el punto, porque así se
 * decide en lugar de acumularse.
 */
const IGUALES_A_PROPOSITO: Partial<Record<'en' | 'pt', string[]>> = {
  // "Servidor de Voz" se escribe igual en español y en portugués: las tres
  // palabras son las mismas en las dos lenguas.
  //
  // "Modelo de IA", también: "IA" es la sigla de "Inteligência Artificial" en
  // portugués, y "modelo" y "de" coinciden. Escribir "Modelo de Inteligência
  // Artificial" para esquivar la regla sería traducir peor a propósito.
  //
  // "Preparar Este Capítulo" es la tercera: las tres palabras coinciden en las dos
  // lenguas, con la misma tilde. Cambiarla en portugués sería inventar una
  // diferencia que no existe.
  pt: ['ajustes.vozServidor', 'ajustes.proposito-model', 'voz.prepararTitulo']
};

describe('todo código de error tiene mensaje en todos los idiomas', () => {
  it('los trece códigos que genera Rust están traducidos', () => {
    // `ERROR_CODES` lo escribe `src-tauri/src/error.rs`, así que esta prueba no
    // puede quedarse atrás de la lista real: añadir una variante en Rust sin
    // traducirla la rompe.
    expect(ERROR_CODES.length).toBeGreaterThan(0);
    for (const idioma of IDIOMAS) {
      const mensajes = DICCIONARIOS[idioma].errores as Record<string, string>;
      for (const codigo of ERROR_CODES) {
        expect(mensajes[codigo]?.trim(), `${idioma}: ${codigo} sin mensaje`).toBeTruthy();
      }
    }
  });

  it('hay mensaje para lo que llegue sin catalogar', () => {
    // Sin esto, un código nuevo de una versión futura del runtime se pintaría en
    // crudo, y `STORAGE_FAILURE` en mitad de la pantalla no le dice nada a nadie.
    for (const idioma of IDIOMAS) {
      expect(DICCIONARIOS[idioma].errores.desconocido.trim().length).toBeGreaterThan(0);
    }
  });

  it('ningún mensaje enseña el código en crudo', () => {
    for (const idioma of IDIOMAS) {
      const mensajes = DICCIONARIOS[idioma].errores as Record<string, string>;
      for (const codigo of ERROR_CODES) {
        expect(mensajes[codigo], `${idioma}: ${codigo} se enseña a sí mismo`).not.toContain(codigo);
      }
    }
  });
});

describe('lo que la interfaz enumera tiene nombre en todos los idiomas', () => {
  it('todo comando del lector', () => {
    for (const idioma of IDIOMAS) {
      const nombres = DICCIONARIOS[idioma].comandos as Record<string, string>;
      for (const comando of COMANDOS) {
        expect(nombres[comando.id]?.trim(), `${idioma}: ${comando.id}`).toBeTruthy();
      }
    }
  });

  it('todo modo de lectura', () => {
    for (const idioma of IDIOMAS) {
      const nombres = DICCIONARIOS[idioma].modosDeLectura as Record<string, string>;
      for (const modo of MODOS) {
        expect(nombres[modo.nombre]?.trim(), `${idioma}: ${modo.nombre}`).toBeTruthy();
      }
    }
  });

  it('todo destino de la carcasa', () => {
    for (const idioma of IDIOMAS) {
      const nombres = DICCIONARIOS[idioma].destinos as Record<string, string>;
      for (const entrada of DESTINOS) {
        expect(nombres[entrada.destino]?.trim(), `${idioma}: ${entrada.destino}`).toBeTruthy();
      }
    }
  });

  it('y nada sobra: una clave de más es texto que ya no se usa', () => {
    // Un diccionario que crece y no encoge acaba lleno de textos de pantallas
    // que se retiraron, y entonces nadie sabe cuáles se pueden tocar.
    const comandos = new Set(COMANDOS.map((c) => c.id as string));
    const modos = new Set(MODOS.map((m) => m.nombre as string));
    const destinos = new Set(DESTINOS.map((d) => d.destino as string));

    expect(Object.keys(es.comandos).sort()).toEqual([...comandos].sort());
    expect(Object.keys(es.modosDeLectura).sort()).toEqual([...modos].sort());
    expect(Object.keys(es.destinos).sort()).toEqual([...destinos].sort());
  });
});
