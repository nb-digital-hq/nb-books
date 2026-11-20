/**
 * **El criterio**, en la parte que se puede demostrar
 * sin navegador:
 *
 * "Consultar y cerrar devuelve el foco exactamente donde estaba, **y la posición
 * de lectura no se mueve ni un locator**; el recorrido entero se hace sin
 * ratón."
 *
 * Las dos afirmaciones sobre la **posición** y sobre el **teclado** son
 * propiedades de la máquina pura, y aquí se comprueban como tales: una secuencia
 * de pulsaciones y una comparación del estado antes y después. La del **foco**
 * es del navegador y se demuestra en `e2e/accesibilidad.spec.ts`, porque el foco
 * no existe fuera de un documento real.
 *
 * Y la parte de partir la selección en palabras, que es lo que hace que esto
 * sirva de algo: se selecciona por bloques, así que lo seleccionado es
 * un párrafo, y un párrafo no está en ningún diccionario.
 */

import { describe, expect, it } from 'vitest';

import type { Locator, ReaderCapabilities } from '$lib/bindings';

import { comandoParaTecla, type Pulsacion } from './comandos';
import { MAXIMO_PALABRAS, palabrasDe, terminoInicial } from './consulta';
import { aplicar, type Efecto, type EstadoDelLector, estadoInicial, seleccion } from './estado';

const EPUB: ReaderCapabilities = {
  textSelection: true,
  internalSearch: true,
  reflow: true,
  typography: true,
  synchronizedAudio: false,
  textExtraction: true
};

const COMIC: ReaderCapabilities = {
  textSelection: false,
  internalSearch: false,
  reflow: false,
  typography: false,
  synchronizedAudio: false,
  textExtraction: false
};

const LOCATOR: Locator = {
  version: 1,
  href: 'OEBPS/c1.xhtml',
  mediaType: 'application/xhtml+xml',
  title: 'Capítulo 1',
  locations: { fragments: ['#char=0'], progression: 0.31, totalProgression: 0.12 },
  text: { highlight: 'Muchos años después' }
};

const BLOQUES = [
  { texto: 'El coronel Aureliano Buendía recordó aquella tarde remota.' },
  { texto: 'Macondo era entonces una aldea de veinte casas de barro y cañabrava.' }
];

function teclear(
  estado: EstadoDelLector,
  pulsaciones: Pulsacion[],
  capacidades: ReaderCapabilities = EPUB
): { estado: EstadoDelLector; efectos: Efecto[] } {
  let actual = estado;
  const efectos: Efecto[] = [];

  for (const pulsacion of pulsaciones) {
    const comando = comandoParaTecla(pulsacion, {
      capacidades,
      haySeleccion: seleccion(actual) !== null,
      escribiendo: actual.redaccion !== null
    });
    if (!comando) continue;

    const transicion = aplicar(actual, comando);
    actual = transicion.estado;
    if (transicion.efecto) efectos.push(transicion.efecto);
  }

  return { estado: actual, efectos };
}

const tecla = (key: string, modificadores: Partial<Pulsacion> = {}): Pulsacion => ({
  key,
  ...modificadores
});

describe('consultar sin perder el sitio', () => {
  it('abrir y cerrar la consulta deja la posición de lectura idéntica', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);
    ({ estado } = teclear(estado, [tecla('ArrowDown'), tecla('ArrowDown')]));

    const antes = structuredClone(estado);
    const abierto = teclear(estado, [tecla('c')]);
    expect(abierto.estado.panel).toBe('consulta');

    const cerrado = teclear(abierto.estado, [tecla('Escape')]);
    expect(cerrado.estado.panel).toBeNull();

    // No "el mismo href": el estado entero. Un salto que conserva el capítulo y
    // pierde el párrafo cumpliría una comprobación floja y no el criterio, que
    // dice **ni un locator**.
    expect(cerrado.estado).toEqual(antes);
  });

  it('consultar no toca el cursor ni la selección extendida', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);
    ({ estado } = teclear(estado, [
      tecla('ArrowDown'),
      tecla('ArrowDown'),
      tecla('ArrowDown', { shiftKey: true })
    ]));
    const rango = seleccion(estado);

    const { estado: despues } = teclear(estado, [tecla('c')]);

    expect(despues.cursor).toBe(estado.cursor);
    expect(despues.ancla).toBe(estado.ancla);
    expect(seleccion(despues)).toEqual(rango);
    expect(despues.locator).toEqual(estado.locator);
  });

  it('el recorrido entero es de teclado: una tecla abre y otra cierra', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);
    ({ estado } = teclear(estado, [tecla('ArrowDown')]));

    const abierto = teclear(estado, [tecla('c')]);
    expect(abierto.efectos).toEqual([{ tipo: 'consultar', seleccion: BLOQUES[0]?.texto ?? '' }]);

    // La misma tecla cierra, y cerrar no vuelve a consultar: sería preguntar
    // otra vez lo mismo al salir.
    const cerrado = teclear(abierto.estado, [tecla('c')]);
    expect(cerrado.estado.panel).toBeNull();
    expect(cerrado.efectos).toEqual([]);
  });

  it('sin nada seleccionado no se consulta ni se abre el panel', () => {
    const estado = estadoInicial(LOCATOR, BLOQUES);
    expect(seleccion(estado)).toBeNull();

    // Dos defensas, y se comprueban las dos: el registro no reconoce la tecla
    // —igual que con anotar— y la máquina tampoco haría nada si la reconociera.
    // Con una sola, quitar la otra pasaría desapercibido.
    expect(
      comandoParaTecla(tecla('c'), {
        capacidades: EPUB,
        haySeleccion: false,
        escribiendo: false
      })
    ).toBeNull();

    const { estado: despues, efectos } = teclear(estado, [tecla('c')]);

    expect(despues.panel).toBeNull();
    expect(efectos).toEqual([]);
  });

  it('un documento sin capa de texto no ofrece la consulta', () => {
    // No es que el control se atenúe: es que no existe. Un cómic sin texto no
    // tiene nada que consultar, y un botón que no hace nada al pulsarlo es peor
    // que ninguno.
    let estado = estadoInicial(LOCATOR, BLOQUES);
    estado = { ...estado, cursor: 0 };

    const { estado: despues, efectos } = teclear(estado, [tecla('c')], COMIC);

    expect(despues.panel).toBeNull();
    expect(efectos).toEqual([]);
  });

  it('escribiendo una nota, la c es una letra', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);
    ({ estado } = teclear(estado, [tecla('ArrowDown'), tecla('Enter')]));
    expect(estado.redaccion).not.toBeNull();

    const { estado: despues, efectos } = teclear(estado, [tecla('c')]);

    expect(despues.panel).toBeNull();
    expect(efectos).toEqual([]);
  });
});

describe('qué palabra se consulta de un párrafo', () => {
  it('parte la selección en palabras, en orden y sin repetir', () => {
    // Sin repetir **ignorando mayúsculas**: "El" y "el" son la misma palabra
    // para consultar, y ofrecer las dos sería ofrecer dos veces lo mismo.
    expect(palabrasDe('El coronel y el coronel Aureliano')).toEqual([
      'El',
      'coronel',
      'y',
      'Aureliano'
    ]);
  });

  it('conserva guiones y apóstrofos dentro de la palabra', () => {
    // En varios idiomas forman parte de ella: partirlos daría dos palabras que
    // no están en ningún diccionario.
    expect(palabrasDe("l'aigua i el sud-oest")).toEqual(["l'aigua", 'i', 'el', 'sud-oest']);
  });

  it('descarta lo que no tiene ninguna letra', () => {
    // Un número o una llamada a nota no se consultan en un diccionario.
    expect(palabrasDe('capítulo 12, página 480')).toEqual(['capítulo', 'página']);
  });

  it('no ofrece más palabras de las que alguien recorrería', () => {
    const parrafo = Array.from({ length: 200 }, (_, i) => `palabra${i}`).join(' ');
    expect(palabrasDe(parrafo)).toHaveLength(MAXIMO_PALABRAS);
  });

  it('la palabra de partida es la primera del párrafo', () => {
    expect(terminoInicial('  —Macondo era entonces una aldea.')).toBe('Macondo');
  });

  it('un párrafo sin palabras no consulta nada', () => {
    // Cadena vacía y no la puntuación: preguntar por «—» devolvería un panel
    // vacío que nadie pidió.
    expect(terminoInicial('— … «» ')).toBe('');
  });
});
