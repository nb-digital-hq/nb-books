/**
 * La parte comprobable del trazo libre: números, sin navegador.
 *
 * Lo que se vigila aquí es la propiedad que hace que un trazo siga sirviendo
 * después de cambiar el cuerpo de letra: las coordenadas van de 0 a 1 sobre la
 * caja del bloque, no en píxeles.
 */
import { describe, expect, it } from 'vitest';

import { aPayload, aRuta, desdePayload, normalizar, simplificar, type Trazo } from './trazos';

const CAJA = { left: 100, top: 50, width: 400, height: 200 };

describe('las coordenadas de un trazo', () => {
  it('van de cero a uno sobre la caja del bloque', () => {
    expect(normalizar(100, 50, CAJA)).toEqual([0, 0]);
    expect(normalizar(500, 250, CAJA)).toEqual([1, 1]);
    expect(normalizar(300, 150, CAJA)).toEqual([0.5, 0.5]);
  });

  /*
   * El mismo trazo sobre el mismo texto con otro cuerpo de letra.
   *
   * Es la propiedad que justifica normalizar: si se guardara en píxeles, el
   * bloque se encogería y la tinta se quedaría donde estaba.
   */
  it('no cambian porque el bloque cambie de tamaño', () => {
    const estrecha = { left: 100, top: 50, width: 200, height: 400 };
    const enGrande = normalizar(300, 150, CAJA);
    const enPequeno = normalizar(200, 250, estrecha);

    expect(enGrande).toEqual(enPequeno);
  });

  /*
   * Salirse por el margen se guarda pegado al borde y no en negativo: al
   * repintarlo, un valor negativo caería fuera del bloque y encima del texto de
   * al lado.
   */
  it('un trazo que se sale por el margen se pega al borde', () => {
    expect(normalizar(20, 10, CAJA)).toEqual([0, 0]);
    expect(normalizar(900, 900, CAJA)).toEqual([1, 1]);
  });

  it('una caja sin tamaño no produce coordenadas imposibles', () => {
    const vacia = { left: 0, top: 0, width: 0, height: 0 };

    const punto = normalizar(10, 10, vacia);

    expect(Number.isFinite(punto[0])).toBe(true);
    expect(Number.isFinite(punto[1])).toBe(true);
  });
});

describe('simplificar', () => {
  it('quita los puntos que no cambian la forma y conserva los extremos', () => {
    const puntos = [
      [0, 0],
      [0.0005, 0],
      [0.001, 0],
      [0.5, 0.5],
      [1, 1]
    ] as const;

    const salida = simplificar(puntos);

    expect(salida[0]).toEqual([0, 0]);
    expect(salida[salida.length - 1]).toEqual([1, 1]);
    expect(salida.length).toBeLessThan(puntos.length);
  });

  it('no toca un trazo de dos puntos', () => {
    const puntos = [
      [0, 0],
      [1, 1]
    ] as const;

    expect(simplificar(puntos)).toEqual(puntos);
  });
});

describe('la ruta que se pinta', () => {
  it('empieza donde empieza el trazo y sigue por sus puntos', () => {
    const trazo: Trazo = {
      puntos: [
        [0, 0],
        [0.5, 0.25]
      ]
    };

    expect(aRuta(trazo)).toBe('M0 0L0.5 0.25');
  });

  /* Un punto suelto tiene que verse: es tinta que alguien puso. */
  it('un punto suelto se pinta como punto', () => {
    expect(aRuta({ puntos: [[0.5, 0.5]] })).toBe('M0.5 0.5l0 0');
  });

  it('un trazo sin puntos no pinta nada', () => {
    expect(aRuta({ puntos: [] })).toBe('');
  });
});

describe('lo que venía guardado', () => {
  it('se reconoce si es tinta y se rechaza entero si no', () => {
    const tinta = {
      version: 1,
      trazos: [
        {
          puntos: [
            [0, 0],
            [1, 1]
          ]
        }
      ]
    };

    expect(desdePayload(tinta)?.trazos).toHaveLength(1);
    expect(desdePayload(null)).toBeNull();
    expect(desdePayload({ version: 2, trazos: [] })).toBeNull();
    expect(desdePayload({ version: 1, trazos: [{ puntos: [['a', 0]] }] })).toBeNull();
    expect(desdePayload({ version: 1, trazos: [{ puntos: [[Number.NaN, 0]] }] })).toBeNull();
  });

  /*
   * Una geometría corrupta no se aproxima: se rechaza.
   *
   * Es lo mismo que el reanclaje hace con una marca cuyo tramo no cuadra con su cita.
   * Pintar una aproximación pondría tinta donde nadie la puso, y eso es peor que
   * decir que no se pudo pintar.
   */
  it('no inventa una forma parecida cuando la guardada no vale', () => {
    expect(desdePayload({ version: 1, trazos: [{ puntos: 'x' }] })).toBeNull();
  });

  it('lo que se guarda vuelve igual', () => {
    const trazos: Trazo[] = [
      {
        puntos: [
          [0, 0],
          [0.5, 0.5],
          [1, 1]
        ]
      }
    ];

    const guardado = aPayload(trazos);
    const leido = desdePayload(JSON.parse(JSON.stringify(guardado)));

    expect(leido).toEqual(guardado);
  });

  it('un trazo vacío no se guarda', () => {
    expect(aPayload([{ puntos: [] }])).toBeNull();
  });
});
