/**
 * **El criterio de aceptación:** la lectura en voz alta funciona sobre
 * EPUB y sobre PDF con capa de texto, y su ausencia en una plataforma **se
 * declara, no se disimula**.
 *
 * Las dos mitades se comprueban aquí:
 *
 * 1. Que funciona sobre los dos formatos no necesita dos pruebas, y eso es el
 *    resultado de una decisión de diseño, no un atajo: la narración recorre
 *    **bloques**, y la superficie de lectura ya entrega bloques para los dos
 *    —párrafos del marcado en un EPUB, fragmentos de la capa de texto en un
 *    PDF—. Lo que se comprueba es que no hay ninguna rama por formato.
 * 2. Que la ausencia se declara se comprueba sobre el proveedor del sistema, en
 *    las tres situaciones que hay: sin síntesis, con síntesis y sin voces, y con
 *    las dos cosas.
 */

import { afterEach, describe, expect, it } from 'vitest';

import type { Locator } from '$lib/bindings';

import type { MarcaDeVoz } from './contrato';
import {
  ajustar,
  alternar,
  avanzar,
  type BloqueNarrable,
  type EfectoDeVoz,
  type EstadoDeNarracion,
  empezar,
  narracionInicial,
  parar,
  seNarra,
  terminarTramo,
  VELOCIDADES
} from './narracion';
import { VozDelSistema } from './sistema';

const LOCATOR: Locator = {
  version: 1,
  href: 'OEBPS/c1.xhtml',
  mediaType: 'application/xhtml+xml',
  title: 'Capítulo 1',
  locations: { fragments: ['#char=0'], progression: 0, totalProgression: 0 },
  text: { highlight: 'Muchos años después' }
};

/** Bloques de un EPUB: párrafos del marcado. */
const PARRAFOS: BloqueNarrable[] = [
  { texto: 'Muchos años después, frente al pelotón de fusilamiento.' },
  { texto: '   ' },
  { texto: 'Macondo era entonces una aldea de veinte casas de barro.' }
];

/**
 * Bloques de un PDF con capa de texto: fragmentos con sus coordenadas, que la
 * superficie ya ha convertido en la misma forma. Es literalmente el mismo tipo.
 */
const FRAGMENTOS: BloqueNarrable[] = [
  { texto: 'El coronel Aureliano Buendía había de recordar' },
  { texto: 'aquella tarde remota en que su padre lo llevó' }
];

function recoger(
  transicion: { estado: EstadoDeNarracion; efecto?: EfectoDeVoz },
  efectos: EfectoDeVoz[]
): EstadoDeNarracion {
  if (transicion.efecto) efectos.push(transicion.efecto);
  return transicion.estado;
}

describe('narrar es recorrer bloques, y por eso vale para los dos formatos', () => {
  it('sobre los párrafos de un EPUB', () => {
    const efectos: EfectoDeVoz[] = [];
    const estado = recoger(empezar(narracionInicial(), PARRAFOS, 0, LOCATOR), efectos);

    expect(estado.estado).toBe('hablando');
    expect(estado.bloque).toBe(0);
    expect(efectos).toEqual([
      {
        tipo: 'hablar',
        tramo: { bloque: 0, texto: PARRAFOS[0]?.texto, locator: LOCATOR },
        // El bloque 1 son tres espacios, así que lo que se va preparando es el 2.
        siguiente: { bloque: 2, texto: PARRAFOS[2]?.texto, locator: LOCATOR }
      }
    ]);
  });

  it('sobre los fragmentos de la capa de texto de un PDF, con el mismo código', () => {
    const efectos: EfectoDeVoz[] = [];
    const estado = recoger(empezar(narracionInicial(), FRAGMENTOS, 0, LOCATOR), efectos);

    expect(estado.bloque).toBe(0);
    expect(efectos).toHaveLength(1);
  });

  it('un documento sin bloques no se narra, y se queda parada en lugar de fallar', () => {
    // Es el PDF escaneado sin capa de texto. Que no se pueda narrar lo dice
    // `motivoSinTexto`; aquí lo que importa es que no se rompa nada.
    const efectos: EfectoDeVoz[] = [];
    const estado = recoger(empezar(narracionInicial(), [], 0, LOCATOR), efectos);

    expect(estado.estado).toBe('parada');
    expect(efectos).toEqual([]);
  });

  it('salta los bloques en blanco en lugar de quedarse mudo en ellos', () => {
    const efectos: EfectoDeVoz[] = [];
    let estado = recoger(empezar(narracionInicial(), PARRAFOS, 0, LOCATOR), efectos);
    estado = recoger(terminarTramo(estado, 0, PARRAFOS, LOCATOR), efectos);

    // El bloque 1 son tres espacios: se salta hasta el 2.
    expect(estado.bloque).toBe(2);
    expect(efectos.at(-1)).toEqual({
      tipo: 'hablar',
      tramo: { bloque: 2, texto: PARRAFOS[2]?.texto, locator: LOCATOR },
      // Es el último con texto: no hay nada que adelantar.
      siguiente: null
    });
  });

  it('al acabar el último bloque se para, sin dar la vuelta al documento', () => {
    const efectos: EfectoDeVoz[] = [];
    let estado = recoger(empezar(narracionInicial(), FRAGMENTOS, 0, LOCATOR), efectos);
    estado = recoger(terminarTramo(estado, 0, FRAGMENTOS, LOCATOR), efectos);
    estado = recoger(terminarTramo(estado, 1, FRAGMENTOS, LOCATOR), efectos);

    expect(estado.estado).toBe('parada');
    expect(estado.bloque).toBe(-1);
  });
});

describe('el seguimiento visual del texto', () => {
  it('la marca lleva el Locator, no solo un número de bloque', () => {
    // Es lo que la ficha subraya del contrato: sin posición, el resaltado
    // sincronizado no se puede implementar después. Un número de bloque no
    // significa lo mismo al pasar de página; un Locator sí.
    const marca: MarcaDeVoz = { bloque: 0, locator: LOCATOR, desde: 0, hasta: 6 };
    const estado = avanzar({ ...narracionInicial(), estado: 'hablando', bloque: 0 }, marca);

    expect(estado.marca?.locator).toBe(LOCATOR);
    expect(estado.marca?.desde).toBe(0);
  });

  it('el bloque que se dice se puede señalar mientras dure', () => {
    const estado = { ...narracionInicial(), estado: 'hablando' as const, bloque: 2 };
    expect(seNarra(estado, 2)).toBe(true);
    expect(seNarra(estado, 1)).toBe(false);
    expect(seNarra(narracionInicial(), 2)).toBe(false);
  });

  it('una marca que llega tarde no mueve el resaltado hacia atrás', () => {
    // El proveedor puede emitir la marca de un bloque justo después de que se
    // cancele. Aplicarla haría saltar el resaltado al párrafo anterior.
    const estado = { ...narracionInicial(), estado: 'hablando' as const, bloque: 3 };
    const tardia: MarcaDeVoz = { bloque: 1, locator: LOCATOR, desde: 0, hasta: 4 };
    expect(avanzar(estado, tardia)).toBe(estado);
  });

  it('con la narración parada no se marca nada', () => {
    const marca: MarcaDeVoz = { bloque: 0, locator: LOCATOR, desde: 0, hasta: 4 };
    expect(avanzar(narracionInicial(), marca).marca).toBeNull();
  });
});

describe('pausar conserva el sitio y parar lo suelta', () => {
  it('la misma acción alterna entre hablar y pausar', () => {
    const efectos: EfectoDeVoz[] = [];
    let estado = recoger(alternar(narracionInicial(), PARRAFOS, 0, LOCATOR), efectos);
    expect(estado.estado).toBe('hablando');

    estado = recoger(alternar(estado, PARRAFOS, 0, LOCATOR), efectos);
    expect(estado.estado).toBe('pausada');
    expect(estado.bloque).toBe(0);

    estado = recoger(alternar(estado, PARRAFOS, 0, LOCATOR), efectos);
    expect(estado.estado).toBe('hablando');
    expect(efectos.map((e) => e.tipo)).toEqual(['hablar', 'pausar', 'reanudar']);
  });

  it('parar suelta el bloque, y volver a empezar no continúa donde estaba', () => {
    const efectos: EfectoDeVoz[] = [];
    let estado = recoger(empezar(narracionInicial(), PARRAFOS, 0, LOCATOR), efectos);
    estado = recoger(terminarTramo(estado, 0, PARRAFOS, LOCATOR), efectos);
    expect(estado.bloque).toBe(2);

    estado = recoger(parar(estado), efectos);
    expect(estado.bloque).toBe(-1);
    expect(estado.marca).toBeNull();
  });
});

describe('cambiar la velocidad o la voz se nota en el momento', () => {
  it('hablando, reinicia el bloque en curso con el ajuste nuevo', () => {
    // Ninguna plataforma deja cambiar la velocidad de un enunciado ya encolado.
    // Sin reiniciar, el ajuste no se notaría hasta el párrafo siguiente y
    // parecería roto.
    const efectos: EfectoDeVoz[] = [];
    let estado = recoger(empezar(narracionInicial(), PARRAFOS, 0, LOCATOR), efectos);
    estado = recoger(ajustar(estado, { velocidad: 1.5 }, PARRAFOS, LOCATOR), efectos);

    expect(estado.velocidad).toBe(1.5);
    expect(estado.bloque).toBe(0);
    expect(efectos.at(-1)).toEqual({
      tipo: 'hablar',
      tramo: { bloque: 0, texto: PARRAFOS[0]?.texto, locator: LOCATOR },
      siguiente: { bloque: 2, texto: PARRAFOS[2]?.texto, locator: LOCATOR }
    });
  });

  it('parada, solo guarda el ajuste y no empieza a hablar sola', () => {
    const efectos: EfectoDeVoz[] = [];
    const estado = recoger(
      ajustar(narracionInicial(), { voz: 'una-voz' }, PARRAFOS, LOCATOR),
      efectos
    );

    expect(estado.voz).toBe('una-voz');
    expect(estado.estado).toBe('parada');
    expect(efectos).toEqual([]);
  });

  it('las velocidades son discretas y cubren de la mitad al doble', () => {
    expect(VELOCIDADES[0]).toBeLessThan(1);
    expect(VELOCIDADES.at(-1)).toBeGreaterThan(1);
    expect(VELOCIDADES).toContain(1);
  });
});

describe('la ausencia de voz se declara, no se disimula', () => {
  const original = Object.getOwnPropertyDescriptor(globalThis, 'speechSynthesis');
  const originalCtor = Object.getOwnPropertyDescriptor(globalThis, 'SpeechSynthesisUtterance');

  afterEach(() => {
    // Se restaura lo que hubiera, incluida la ausencia: dejar un doble puesto
    // haría pasar en falso la prueba siguiente.
    delete (globalThis as Record<string, unknown>).speechSynthesis;
    delete (globalThis as Record<string, unknown>).SpeechSynthesisUtterance;
    if (original) Object.defineProperty(globalThis, 'speechSynthesis', original);
    if (originalCtor) Object.defineProperty(globalThis, 'SpeechSynthesisUtterance', originalCtor);
  });

  function fingir(voces: unknown[] | null) {
    if (voces === null) return;
    (globalThis as Record<string, unknown>).speechSynthesis = { getVoices: () => voces };
    (globalThis as Record<string, unknown>).SpeechSynthesisUtterance = class {};
  }

  it('sin síntesis en la plataforma, lo dice con su motivo', () => {
    // Es el caso de WebKitGTK, que es el webview de Linux y no siempre la trae.
    const declaracion = new VozDelSistema().declarar();
    expect(declaracion.disponible).toBe(false);
    expect(declaracion.motivo).toBe('sin-sintesis');
  });

  it('con síntesis pero sin ninguna voz instalada, lo dice distinto', () => {
    // No es lo mismo: esto se arregla instalando una voz, y lo otro no.
    fingir([]);
    const declaracion = new VozDelSistema().declarar();
    expect(declaracion.disponible).toBe(false);
    expect(declaracion.motivo).toBe('sin-voces');
  });

  it('con voces instaladas, está disponible y las enumera', () => {
    fingir([{ voiceURI: 'v1', name: 'Mónica', lang: 'es-ES', default: true }]);
    const proveedor = new VozDelSistema();

    expect(proveedor.declarar().disponible).toBe(true);
    expect(proveedor.voces()).toEqual([
      { id: 'v1', nombre: 'Mónica', idioma: 'es-ES', porDefecto: true }
    ]);
  });

  it('la voz del sistema no necesita red, y eso lo declara ella', () => {
    // Es la razón por la que es el suelo de accesibilidad: no depende de nada
    // que pueda faltar el día que haga falta.
    fingir([{ voiceURI: 'v1', name: 'Mónica', lang: 'es-ES', default: true }]);
    expect(new VozDelSistema().declarar().necesitaRed).toBe(false);
  });

  it('sin síntesis, pedir que hable falla en lugar de no hacer nada', () => {
    // Callar sin decir nada deja al usuario esperando una voz que no va a
    // llegar. Con el motivo, la interfaz puede explicarlo.
    const fallos: string[] = [];
    new VozDelSistema().hablar(
      { bloque: 0, texto: 'hola', locator: LOCATOR },
      { voz: null, velocidad: 1 },
      {
        alAvanzar: () => {},
        alTerminarTramo: () => {},
        alFallar: (motivo) => fallos.push(motivo)
      }
    );
    expect(fallos).toEqual(['sin-voz']);
  });
});

describe('se empieza a leer donde se señala, no por el principio', () => {
  it('arranca en el bloque que se le pide', () => {
    // La máquina siempre supo hacerlo —`desde` es un parámetro— y
    // nadie le pasaba otra cosa que cero: pedir que te leyeran en la página doce
    // te devolvía a la primera. Esto fija el comportamiento por si alguien
    // vuelve a cablear el arranque.
    const efectos: EfectoDeVoz[] = [];
    const estado = recoger(empezar(narracionInicial(), PARRAFOS, 2, LOCATOR), efectos);

    expect(estado.bloque).toBe(2);
    expect(efectos[0]).toMatchObject({ tipo: 'hablar', tramo: { bloque: 2 } });
  });

  it('y alternar desde parada también arranca ahí', () => {
    // Es el camino que recorre el botón: alternar sobre una narración parada
    // llama a empezar, y tiene que llevarse el bloque con él.
    const efectos: EfectoDeVoz[] = [];
    const estado = recoger(alternar(narracionInicial(), PARRAFOS, 2, LOCATOR), efectos);

    expect(estado.estado).toBe('hablando');
    expect(estado.bloque).toBe(2);
  });
});
