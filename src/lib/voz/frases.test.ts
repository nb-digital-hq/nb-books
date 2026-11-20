/**
 * Partir en frases es lo que hace que la voz suene a voz.
 *
 * Lo que se comprueba no es que corte por puntos —eso lo hace la plataforma—
 * sino las tres decisiones propias: que ningún trozo se pierda, que los muy
 * cortos no vayan solos y que los muy largos no vayan enteros.
 */

import { describe, expect, it } from 'vitest';

import { enFrases } from './frases';

/** Un párrafo real, del corpus con el que se prueba todo lo demás. */
const PARRAFO =
  'Muchos años después, frente al pelotón de fusilamiento, el coronel Aureliano ' +
  'Buendía había de recordar aquella tarde remota en que su padre lo llevó a ' +
  'conocer el hielo. Macondo era entonces una aldea de veinte casas de barro y ' +
  'cañabrava construidas a la orilla de un río de aguas diáfanas. El mundo era ' +
  'tan reciente que muchas cosas carecían de nombre.';

describe('partir un párrafo en frases', () => {
  it('devuelve más de una, que es todo el propósito', () => {
    // Con una sola, el sintetizador lo dice de corrido y sin pausas, que es
    // exactamente lo que se oía como "no vocaliza".
    expect(enFrases(PARRAFO).length).toBeGreaterThan(1);
  });

  it('no pierde ni inventa una palabra', () => {
    const palabras = (t: string) => t.split(/\s+/u).filter((p) => p.length > 0);
    expect(palabras(enFrases(PARRAFO).join(' '))).toEqual(palabras(PARRAFO));
  });

  it('un párrafo en blanco no se dice', () => {
    // Un trozo vacío dejaría la voz muda unos segundos, y desde fuera eso se ve
    // como que se ha colgado.
    expect(enFrases('')).toEqual([]);
    expect(enFrases('   \n  ')).toEqual([]);
  });

  it('un texto sin puntuación se dice entero', () => {
    expect(enFrases('sin puntuacion ninguna')).toEqual(['sin puntuacion ninguna']);
  });
});

describe('las frases muy cortas', () => {
  it('no van solas', () => {
    // "Sí." dicho aparte es un enunciado de medio segundo con su pausa antes y
    // después: suena entrecortado y multiplica los viajes al sintetizador.
    const frases = enFrases('Sí. No. Quizá. Y entonces empezó a llover sobre el pueblo entero.');
    expect(frases.every((f) => f.length >= 10)).toBe(true);
    expect(frases.length).toBeLessThan(4);
  });

  it('juntarlas no cambia lo que se dice', () => {
    const texto = 'Sí. No. Quizá.';
    expect(enFrases(texto).join(' ')).toBe(texto);
  });
});

describe('las frases muy largas', () => {
  it('se parten por sus pausas internas', () => {
    // Una frase de mil caracteres tiene el mismo problema que el párrafo
    // entero: una sola curva de entonación y ninguna pausa.
    const larga = `${Array.from({ length: 12 }, (_, i) => `parte número ${i} de la enumeración`).join('; ')}.`;
    expect(larga.length).toBeGreaterThan(300);

    const frases = enFrases(larga);
    expect(frases.length).toBeGreaterThan(1);
    expect(frases.every((f) => f.length <= 300)).toBe(true);
  });

  it('conserva el signo que marcaba la pausa', () => {
    // Sin él, el sintetizador pierde justo la pausa por la que se partió.
    const larga = `${'palabra '.repeat(20)}; ${'otra '.repeat(20)}; ${'mas '.repeat(20)}.`;
    const frases = enFrases(larga);
    expect(frases[0]?.endsWith(';')).toBe(true);
  });

  it('una frase larga sin ninguna pausa se parte por palabras', () => {
    const larga = 'palabra '.repeat(80).trim();
    const frases = enFrases(larga);
    expect(frases.length).toBeGreaterThan(1);
    expect(frases.every((f) => f.length <= 300)).toBe(true);
    expect(frases.join(' ')).toBe(larga);
  });
});

describe('otros idiomas', () => {
  it('parte donde el idioma parte, y no donde lo haría el español', () => {
    // El japonés no usa el punto latino. Cortar por `.` no habría partido nada.
    const japones = 'これは最初の文です。これは二番目の文です。これは三番目の文です。';
    const frases = enFrases(japones, 'ja');
    expect(frases.join('')).toBe(japones);
  });

  it('una etiqueta de idioma que no existe no deja de narrar', () => {
    // `Intl` salta con una etiqueta que no reconoce, y quedarse sin voz por eso
    // sería peor que segmentar con la reserva.
    expect(enFrases(PARRAFO, 'no-es-un-idioma').length).toBeGreaterThan(0);
  });
});
