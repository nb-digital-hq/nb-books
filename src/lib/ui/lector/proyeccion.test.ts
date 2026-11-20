/**
 * Lo que se comprueba aquí es **que la reconstrucción coincide con la del
 * runtime**, que es de lo que depende que una marca caiga en su sitio.
 *
 * Los casos no son inventados: cada uno es una regla de `texto_plano` en
 * `src-tauri/lector/src/reader/epub.rs`. Si alguien cambia una de las dos partes sin la
 * otra, aquí se ve; y como el desplazamiento de una anotación es un índice sobre
 * este texto, un carácter de diferencia corre **todas** las marcas siguientes.
 */

import { describe, expect, it } from 'vitest';

import type { Locator } from '$lib/bindings';

import {
  colocar,
  desplazamientoDe,
  type Evento,
  indiceDe,
  marcaEn,
  normalizar,
  textoEntre
} from './proyeccion';
import { fundirRectangulos } from './resaltados';

/** Un texto suelto, que es el caso corriente. */
function t(texto: string, ref = 0): Evento {
  return { tipo: 'texto', texto, ref };
}

const FIN: Evento = { tipo: 'finDeBloque' };

describe('el texto se reconstruye con las reglas del runtime', () => {
  it('colapsa cualquier racha de espacio en blanco a uno solo', () => {
    // El archivo lo indentó su generador, y esa indentación no es contenido.
    const proyectado = normalizar([t('Uno   \n\t  dos')]);
    expect(proyectado.texto).toBe('Uno dos');
  });

  it('no deja espacio al principio ni al final', () => {
    const proyectado = normalizar([t('  \n  Hola  \n ')]);
    expect(proyectado.texto).toBe('Hola');
  });

  it('cerrar un bloque separa dos textos que en el marcado estaban pegados', () => {
    // `<p>uno</p><p>dos</p>` sin esto sería "unodos", y a partir de ahí todas
    // las marcas del capítulo caerían un carácter antes de donde deben.
    const proyectado = normalizar([t('uno', 0), FIN, t('dos', 1), FIN]);
    expect(proyectado.texto).toBe('uno dos');
  });

  it('cerrar el último bloque no añade un espacio final', () => {
    const proyectado = normalizar([t('uno'), FIN]);
    expect(proyectado.texto).toBe('uno');
  });

  it('varios cierres seguidos separan una sola vez', () => {
    const proyectado = normalizar([t('uno', 0), FIN, FIN, FIN, t('dos', 1)]);
    expect(proyectado.texto).toBe('uno dos');
  });

  it('cuenta puntos de código y no unidades de UTF-16', () => {
    // El runtime cuenta `char` de Rust. Un emoji ocupa dos unidades de UTF-16 y
    // un solo punto de código: contarlo como dos correría todo lo que viniera
    // detrás, y en un libro con un emoji en el prólogo eso son todas las marcas.
    const proyectado = normalizar([t('a🙂b')]);
    expect(proyectado.largo).toBe(3);
    expect(proyectado.ancho).toEqual([1, 2, 1]);
    expect(textoEntre(proyectado, 1, 1)).toBe('🙂');
  });

  it('recortar por índices no parte un par sustituto por la mitad', () => {
    const proyectado = normalizar([t('a🙂b')]);
    expect(textoEntre(proyectado, 0, 2)).toBe('a🙂');
    expect(textoEntre(proyectado, 2, 1)).toBe('b');
  });

  it('un tramo que se sale del texto no devuelve un recorte a medias', () => {
    const proyectado = normalizar([t('corto')]);
    expect(textoEntre(proyectado, 3, 10)).toBeNull();
    expect(textoEntre(proyectado, -1, 2)).toBeNull();
  });
});

describe('cada carácter sabe de dónde salió', () => {
  it('apunta al nodo y al desplazamiento donde estaba de verdad', () => {
    // Con la indentación de por medio: la 'd' de "dos" está en el nodo 1, y en
    // su desplazamiento 3, porque delante lleva tres espacios que no se emiten.
    const proyectado = normalizar([t('uno', 0), t('   dos', 1)]);
    expect(proyectado.texto).toBe('uno dos');
    const d = 4;
    expect(proyectado.refs[d]).toBe(1);
    expect(proyectado.desde[d]).toBe(3);
  });

  it('el espacio de relleno no presta sitio a nadie', () => {
    // Tiene ancho cero a propósito: no salió de ningún carácter del documento,
    // así que un rango que empiece ahí no puede comerse texto que no es suyo.
    const proyectado = normalizar([t('uno', 0), FIN, t('dos', 1)]);
    const relleno = 3;
    expect(proyectado.texto[relleno]).toBe(' ');
    expect(proyectado.ancho[relleno]).toBe(0);
  });
});

describe('una marca no se pinta sin comprobar que el sitio es el suyo', () => {
  const proyectado = normalizar([t('El ingenioso hidalgo don Quijote')]);

  it('coloca la que dice lo que hay donde dice', () => {
    const { marcas, sinColocar } = colocar(proyectado, [
      { id: 'a', clase: 'highlight', inicio: 13, cita: 'hidalgo' }
    ]);
    expect(sinColocar).toEqual([]);
    expect(marcas).toEqual([{ id: 'a', clase: 'highlight', inicio: 13, fin: 20 }]);
  });

  it('no coloca la que caería sobre otro texto, y lo dice', () => {
    // Es el caso que hace falta atrapar: un capítulo con contenido que el
    // saneado descarta y el runtime no deja todas las marcas corridas. Pintarla
    // sobre el párrafo equivocado es peor que no pintarla.
    const { marcas, sinColocar } = colocar(proyectado, [
      { id: 'a', clase: 'highlight', inicio: 12, cita: 'hidalgo' }
    ]);
    expect(marcas).toEqual([]);
    expect(sinColocar).toEqual(['a']);
  });

  it('tampoco coloca la que se saldría del capítulo', () => {
    const { marcas, sinColocar } = colocar(proyectado, [
      { id: 'a', clase: 'highlight', inicio: 900, cita: 'hidalgo' }
    ]);
    expect(marcas).toEqual([]);
    expect(sinColocar).toEqual(['a']);
  });

  it('una anotación sin cita no se pinta, porque no habría con qué comprobarla', () => {
    const { marcas, sinColocar } = colocar(proyectado, [
      { id: 'a', clase: 'bookmark', inicio: 0, cita: '' }
    ]);
    expect(marcas).toEqual([]);
    expect(sinColocar).toEqual(['a']);
  });

  it('lo que no se coloca no se descarta: sale en su propia lista', () => {
    const { marcas, sinColocar } = colocar(proyectado, [
      { id: 'buena', clase: 'highlight', inicio: 0, cita: 'El' },
      { id: 'mala', clase: 'highlight', inicio: 5, cita: 'no está' }
    ]);
    expect(marcas.map((m) => m.id)).toEqual(['buena']);
    expect(sinColocar).toEqual(['mala']);
  });
});

describe('señalar un punto encuentra la marca que lo cubre', () => {
  const marcas = [
    { id: 'a', clase: 'highlight', inicio: 0, fin: 5 },
    { id: 'b', clase: 'note', inicio: 10, fin: 20 }
  ];

  it('devuelve la que lo contiene', () => {
    expect(marcaEn(marcas, 3)?.id).toBe('a');
    expect(marcaEn(marcas, 15)?.id).toBe('b');
  });

  it('el final es abierto, así que dos marcas seguidas no se solapan', () => {
    expect(marcaEn(marcas, 5)).toBeNull();
  });

  it('con dos superpuestas gana la de encima, que es la última', () => {
    // Se puede anotar dentro de un resaltado, y al señalar el trozo común se
    // abre la de arriba: es lo que se está viendo.
    const encimadas = [...marcas, { id: 'c', clase: 'note', inicio: 12, fin: 14 }];
    expect(marcaEn(encimadas, 13)?.id).toBe('c');
  });
});

/*
 * `indiceDe` es la única parte del recorrido del árbol que se puede comprobar
 * sin navegador: solo usa los nodos para identificarlos, nunca para leerlos, así
 * que sirven objetos vacíos. Lo demás —el recorrido y los rangos— lo cubren los
 * recorridos de `e2e/`.
 */
describe('del punto del documento al índice del texto', () => {
  const nodos = [{}, {}, {}] as unknown as Text[];
  const proyectado = normalizar([
    t('Muchos años después.', 0),
    FIN,
    t('Macondo era una aldea de veinte casas.', 1),
    FIN
  ]);

  it('devuelve el carácter que hay justo en ese desplazamiento', () => {
    const indice = indiceDe(proyectado, nodos, nodos[1] as Text, 0);
    expect(indice).not.toBeNull();
    expect(proyectado.texto[indice as number]).toBe('M');
  });

  it('un desplazamiento sobre un espacio devuelve el carácter anterior', () => {
    // Es el caso normal y no el raro: el centro de un resaltado de varias
    // palabras cae en un hueco. Exigir coincidencia exacta hacía que pulsar en
    // medio de una marca no señalara nada.
    const espacio = 'Macondo era una aldea de'.length;
    expect('Macondo era una aldea de veinte casas.'[espacio]).toBe(' ');
    const indice = indiceDe(proyectado, nodos, nodos[1] as Text, espacio);
    expect(proyectado.texto[indice as number]).toBe('e');
  });

  it('un nodo que no es del texto proyectado no devuelve nada', () => {
    const ajeno = {} as unknown as Text;
    expect(indiceDe(proyectado, nodos, ajeno, 0)).toBeNull();
  });

  it('el camino de ida y el de vuelta llevan al mismo sitio', () => {
    // La propiedad que hace utilizable el gesto: lo que se pinta en un índice se
    // vuelve a encontrar desde el documento. Si las dos direcciones no
    // coincidieran, la marca se vería en un sitio y se pulsaría en otro.
    for (let i = 0; i < proyectado.largo; i += 1) {
      if (proyectado.ancho[i] === 0) continue;
      const nodo = nodos[proyectado.refs[i] as number] as Text;
      expect(indiceDe(proyectado, nodos, nodo, proyectado.desde[i] as number)).toBe(i);
    }
  });
});

describe('los puntos: lo que señala un sitio y no un tramo', () => {
  const locator = (href: string, char: number | null): Locator => ({
    version: 1,
    href,
    mediaType: 'application/xhtml+xml',
    title: null,
    locations: { fragments: char === null ? [] : [`#char=${char}`], progression: 0 },
    text: null
  });

  it('lee el desplazamiento del fragmento, que es donde ADR-004 lo pone', () => {
    expect(desplazamientoDe(locator('c1', 120), 'c1')).toBe(120);
  });

  it('un locator de otro capítulo no señala nada aquí', () => {
    // Sin esto, la cinta del capítulo tres saldría en el uno.
    expect(desplazamientoDe(locator('c3', 120), 'c1')).toBeNull();
  });

  it('sin selector de carácter no se inventa uno desde la progresión', () => {
    // Una fracción del documento entero no dice en qué carácter de este
    // capítulo cae: redondearla pondría la cinta en otro párrafo.
    expect(desplazamientoDe(locator('c1', null), 'c1')).toBeNull();
  });
});

describe('la capa de marcas: fundir rectángulos del mismo renglón', () => {
  it('une los trozos contiguos en los que la lectura biónica parte una línea', () => {
    const brutos = [
      { left: 10, top: 40, width: 18, height: 22 },
      { left: 28, top: 40, width: 32, height: 22 },
      { left: 60, top: 40, width: 15, height: 22 },
      { left: 75, top: 40, width: 25, height: 22 }
    ];
    expect(fundirRectangulos(brutos)).toEqual([{ left: 10, top: 40, width: 90, height: 22 }]);
  });

  it('conserva separados los renglones distintos y las columnas paginadas', () => {
    const brutos = [
      // Renglón 1 en columna 1
      { left: 10, top: 40, width: 120, height: 22 },
      // Renglón 2 en columna 1
      { left: 10, top: 68, width: 80, height: 22 },
      // Renglón a la misma altura pero en la columna 2 (separada por column-gap)
      { left: 600, top: 68, width: 95, height: 22 }
    ];
    expect(fundirRectangulos(brutos)).toEqual([
      { left: 10, top: 40, width: 120, height: 22 },
      { left: 10, top: 68, width: 80, height: 22 },
      { left: 600, top: 68, width: 95, height: 22 }
    ]);
  });

  it('descarta rectángulos vacíos de ancho o alto cero', () => {
    expect(
      fundirRectangulos([
        { left: 10, top: 40, width: 0, height: 22 },
        { left: 10, top: 40, width: 50, height: 0 },
        { left: 10, top: 40, width: 50, height: 22 }
      ])
    ).toEqual([{ left: 10, top: 40, width: 50, height: 22 }]);
  });
});
