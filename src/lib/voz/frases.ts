/**
 * Partir un párrafo en frases, que es lo que se manda a decir.
 *
 * # Por qué no se manda el párrafo entero
 *
 * Porque sonaba mal, y no por culpa del sintetizador. Un párrafo de treinta
 * segundos llega al motor como **un solo enunciado**: lo dice de corrido, con
 * una única curva de entonación y sin las pausas que separan una idea de la
 * siguiente. Desde fuera eso se oye exactamente como se describió —"no
 * vocaliza, lee rápido"— y no se arregla bajando la velocidad, porque lo que
 * falta no es tiempo, son pausas.
 *
 * Y hay una segunda razón, que es la que se nota al usarlo: **ninguna
 * plataforma deja cambiar la velocidad de un enunciado ya encolado**, así que
 * ajustarla obliga a volver a decir el que suena. Con párrafos, subir la
 * velocidad devolvía la lectura una o dos líneas atrás; con frases, retrocede
 * una frase de dos segundos y no se percibe como retroceso.
 *
 * Es lo que hacen los lectores que suenan bien. Readest, sobre `foliate-js`,
 * segmenta con `Intl.Segmenter` y crea un enunciado por segmento; la velocidad
 * se aplica al construir cada uno, así que un cambio afecta al siguiente y no
 * interrumpe nada.
 *
 * # Por qué `Intl.Segmenter` y no cortar por puntos
 *
 * Porque cortar por puntos parte "D. Quijote", "3.14" y "etc." por la mitad, y
 * no parte nada en japonés ni en chino, que no usan el punto latino.
 * `Intl.Segmenter` conoce las reglas de segmentación de Unicode por idioma y ya
 * está en la plataforma: no es una dependencia, es una API estándar.
 *
 * Contrato: `docs/architecture/voice-services.md`.
 */

/**
 * Longitud por debajo de la cual una frase se junta con la siguiente.
 *
 * Un "Sí." suelto es un enunciado de medio segundo con su propia pausa antes y
 * después: dicho aparte suena entrecortado, y además multiplica los viajes al
 * sintetizador. Cuarenta caracteres es aproximadamente una línea corta.
 */
const MINIMO = 40;

/**
 * Longitud por encima de la cual una frase se parte por dentro.
 *
 * Una frase de mil caracteres —que las hay, y no solo en Proust— tiene el mismo
 * problema que el párrafo entero. Se parte por sus pausas internas: punto y
 * coma, dos puntos, raya y coma, en ese orden, que es el orden en que separan.
 */
const MAXIMO = 300;

/** Signos por los que se parte una frase larga, del que más separa al que menos. */
const PAUSAS = [';', ':', '—', ','];

/**
 * Parte un texto en los trozos que se van a decir.
 *
 * Devuelve una lista sin huecos: un párrafo en blanco devuelve una lista vacía,
 * que es lo que hace que la narración lo salte en lugar de quedarse muda unos
 * segundos delante de él.
 */
export function enFrases(texto: string, idioma = 'es'): string[] {
  const limpio = texto.trim();
  if (limpio.length === 0) return [];

  const crudas = segmentar(limpio, idioma);

  // Primero se parte lo demasiado largo y después se junta lo demasiado corto:
  // al revés, un trozo recién partido podría quedar por debajo del mínimo y
  // volver a juntarse con el que se acaba de separar de él.
  const partidas = crudas.flatMap((frase) => partirLarga(frase));
  return juntarCortas(partidas)
    .map((f) => f.trim())
    .filter((f) => f.length > 0);
}

/**
 * Segmenta con la plataforma, o por puntuación si no la hay.
 *
 * La reserva existe porque `Intl.Segmenter` puede faltar en un webview viejo, y
 * ahí la elección no es entre segmentar bien o mal: es entre segmentar mal o
 * volver a mandar el párrafo entero, que es el defecto que esto viene a
 * arreglar.
 */
function segmentar(texto: string, idioma: string): string[] {
  // forzado: `Intl.Segmenter` falta en los tipos de la lib que usa el proyecto,
  // y aquí se comprueba en tiempo de ejecución si existe.
  const Segmentador = (
    Intl as unknown as {
      Segmenter?: new (
        idioma: string,
        opciones: { granularity: string }
      ) => { segment: (t: string) => Iterable<{ segment: string }> };
    }
  ).Segmenter;

  if (Segmentador) {
    try {
      const segmentador = new Segmentador(idioma, { granularity: 'sentence' });
      /*
       * Los segmentos llegan **sin recortar**, con el separador que traían.
       *
       * Recortarlos aquí obligaba a inventar uno al volver a juntar los cortos,
       * y el espacio que se inventaba no existe en japonés ni en chino, que no
       * separan frases con espacios: el texto que se decía dejaba de ser el del
       * libro. Se recorta al final, cuando ya no hay que juntar nada.
       */
      const trozos = [...segmentador.segment(texto)].map((s) => s.segment);
      if (trozos.some((t) => t.trim().length > 0)) {
        return trozos.filter((t) => t.trim().length > 0);
      }
    } catch {
      // Una etiqueta de idioma que `Intl` no reconoce hace saltar al
      // constructor. Se cae a la reserva en lugar de dejar de narrar.
    }
  }

  return texto.split(/(?<=[.!?…])(?=\s)/u).filter((s) => s.trim().length > 0);
}

/** Parte una frase que se pasa de larga por sus pausas internas. */
function partirLarga(frase: string): string[] {
  if (frase.length <= MAXIMO) return [frase];

  for (const signo of PAUSAS) {
    if (!frase.includes(signo)) continue;
    const trozos = frase
      .split(signo)
      .map((t) => t.trim())
      .filter((t) => t.length > 0);
    // Se reconstruye con el signo al final de cada trozo menos el último: sin
    // él, el sintetizador pierde la pausa que ese signo representa.
    const rehechos = trozos.map((t, i) => (i < trozos.length - 1 ? `${t}${signo}` : t));
    if (rehechos.length > 1) return rehechos.flatMap((t) => partirLarga(t));
  }

  // Sin ninguna pausa dentro, se parte por palabras: es peor que por una pausa
  // y mejor que mandar mil caracteres de un tirón.
  return partirPorPalabras(frase);
}

function partirPorPalabras(frase: string): string[] {
  const palabras = frase.split(/\s+/u);
  const salida: string[] = [];
  let actual = '';
  for (const palabra of palabras) {
    if (actual.length > 0 && actual.length + palabra.length + 1 > MAXIMO) {
      salida.push(actual);
      actual = palabra;
    } else {
      actual = actual.length > 0 ? `${actual} ${palabra}` : palabra;
    }
  }
  if (actual.length > 0) salida.push(actual);
  return salida;
}

/**
 * Junta las frases cortas con la siguiente, hasta llegar al mínimo.
 *
 * Se concatenan **tal cual llegaron**, con el separador que ya traían. Poner uno
 * aquí escribiría un espacio donde el idioma no lo pone.
 */
function juntarCortas(frases: string[]): string[] {
  const salida: string[] = [];
  for (const frase of frases) {
    const anterior = salida.at(-1);
    if (anterior !== undefined && anterior.trim().length < MINIMO) {
      salida[salida.length - 1] = anterior + frase;
    } else {
      salida.push(frase);
    }
  }
  return salida;
}
