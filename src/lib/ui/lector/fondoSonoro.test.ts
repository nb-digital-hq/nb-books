import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { agudeza, buferDe, CLASES, type ClaseDeRuido, picos, RECETAS, ruido } from './fondoSonoro';

/** Una secuencia repetible, para que la prueba mire la forma y no el azar. */
function azarFijo(semillaInicial = 12345): () => number {
  let semilla = semillaInicial;
  return () => {
    semilla = (semilla * 1103515245 + 12345) % 2147483648;
    return semilla / 2147483648;
  };
}

/** Tasa de muestreo de trabajo. La real la pone el navegador. */
const TASA = 44100;

/**
 * El pico de un tramo.
 *
 * Con un bucle y no con `Math.max(...)`: el segundo desborda la pila a partir de
 * unas cien mil muestras, y un búfer de tres segundos tiene ciento treinta mil.
 * Lo destapó esta misma prueba al alargar los tramos.
 */
function maximoDe(muestras: Float32Array): number {
  let maximo = 0;
  for (const valor of muestras) maximo = Math.max(maximo, Math.abs(valor));
  return maximo;
}

/** El nivel eficaz de un tramo. */
function eficazDe(muestras: Float32Array): number {
  let suma = 0;
  for (const valor of muestras) suma += valor * valor;
  return Math.sqrt(suma / Math.max(1, muestras.length));
}

/** Cuánto varía la energía a lo largo del tramo, en ventanas de un cuarto de segundo. */
function respiracion(muestras: Float32Array, tasa: number): number {
  const ventana = Math.round(tasa / 4);
  const energias: number[] = [];
  for (let inicio = 0; inicio + ventana <= muestras.length; inicio += ventana) {
    let suma = 0;
    for (let i = inicio; i < inicio + ventana; i += 1) suma += (muestras[i] ?? 0) ** 2;
    energias.push(Math.sqrt(suma / ventana));
  }
  if (energias.length === 0) return 0;
  const media = energias.reduce((a, b) => a + b, 0) / energias.length;
  if (media === 0) return 0;
  const varianza = energias.reduce((a, b) => a + (b - media) ** 2, 0) / energias.length;
  return Math.sqrt(varianza) / media;
}

describe('el ruido de base', () => {
  it('se queda dentro del rango que admite el audio', () => {
    // Fuera de [-1, 1] el audio satura, y saturar suena a chasquido: es el
    // defecto que convierte un fondo para concentrarse en algo que molesta.
    for (const color of ['blanco', 'rosa', 'marron'] as const) {
      const muestras = ruido(color, 40000, azarFijo());
      expect(maximoDe(muestras), color).toBeLessThanOrEqual(1);
      expect(maximoDe(muestras), color).toBeGreaterThan(0.05);
    }
  });

  it('los tres colores son de verdad distintos, y en el orden que dicen ser', () => {
    const blanco = agudeza(ruido('blanco', 40000, azarFijo()));
    const rosa = agudeza(ruido('rosa', 40000, azarFijo()));
    const marron = agudeza(ruido('marron', 40000, azarFijo()));

    expect(rosa).toBeLessThan(blanco);
    expect(marron).toBeLessThan(rosa);
  });

  it('con la misma secuencia sale el mismo ruido', () => {
    const uno = ruido('rosa', 1000, azarFijo());
    const otro = ruido('rosa', 1000, azarFijo());
    expect(Array.from(uno)).toEqual(Array.from(otro));
  });
});

describe('el catálogo de ambientes', () => {
  it('todo ambiente que el runtime admite sabe sonar aquí', () => {
    // La lista cerrada vive en Rust y las recetas aquí. Un ambiente admitido
    // allí y sin receta aquí sería una opción que falla al pulsarla, y una
    // receta sin ambiente sería código que nadie puede alcanzar.
    const fuente = readFileSync('src-tauri/src/extensions/manifest.rs', 'utf8');
    const bloque = fuente.slice(fuente.indexOf('pub const CLASES_DE_RUIDO'));
    const declarados = [...bloque.slice(0, bloque.indexOf('];')).matchAll(/"([a-z]+)"/g)].map(
      (m) => m[1]
    );

    expect(declarados.length).toBeGreaterThan(3);
    expect([...declarados].sort()).toEqual([...CLASES].sort());
  });

  it('ninguna receta satura ni se queda muda', () => {
    for (const clase of CLASES) {
      const muestras = buferDe(clase, TASA, 2, azarFijo());
      expect(maximoDe(muestras), clase).toBeLessThanOrEqual(1);
      // Y suena: una receta cuyo ruido de base se cancelara daría silencio, que
      // pasaría todas las demás comprobaciones.
      expect(maximoDe(muestras), clase).toBeGreaterThan(0.05);
    }
  });

  it('cada capa declara una frecuencia y una mezcla que se pueden usar', () => {
    for (const clase of CLASES) {
      const receta = RECETAS[clase];
      expect(receta.capas.length, clase).toBeGreaterThan(0);
      for (const capa of receta.capas) {
        // Fuera del rango audible el filtro no hace nada, y una mezcla mayor
        // que uno satura al sumarse con las demás capas.
        expect(capa.frecuencia, clase).toBeGreaterThan(20);
        expect(capa.frecuencia, clase).toBeLessThan(20000);
        expect(capa.ganancia, clase).toBeGreaterThan(0);
        expect(capa.ganancia, clase).toBeLessThanOrEqual(1);
      }
      // Las capas juntas no pueden pasarse: el maestro no es un limitador.
      const suma = receta.capas.reduce((total, capa) => total + capa.ganancia, 0);
      expect(suma, clase).toBeLessThanOrEqual(2);
    }
  });
});

describe('lo que hace que un ambiente suene a lo que dice', () => {
  it('la lluvia tiene gotas y el oleaje no', () => {
    // Es la diferencia que la primera versión no tenía: los dos eran ruido, y
    // el nombre prometía algo que el oído no encontraba. Medido: la lluvia da
    // treinta y tres sucesos por segundo y el oleaje ninguno, porque su carácter
    // no está en el búfer sino en el vaivén de los filtros.
    const lluvia = picos(buferDe('lluvia', TASA, 3, azarFijo()), TASA);
    const oleaje = picos(buferDe('oleaje', TASA, 3, azarFijo()), TASA);

    expect(lluvia).toBeGreaterThan(20);
    expect(oleaje).toBe(0);
  });

  it('el fuego chasquea: pocos golpes y fuertes, no muchos y flojos', () => {
    const fuego = buferDe('fuego', TASA, 4, azarFijo());
    const lluvia = buferDe('lluvia', TASA, 4, azarFijo());

    // Menos sucesos que la lluvia -seis por segundo contra treinta y tres-...
    expect(picos(fuego, TASA)).toBeLessThan(picos(lluvia, TASA) / 3);
    // ...y más altos: un chasquido destaca sobre la brasa mucho más de lo que
    // una gota destaca sobre el agua.
    expect(maximoDe(fuego) / eficazDe(fuego)).toBeGreaterThan(maximoDe(lluvia) / eficazDe(lluvia));
  });

  it('el traqueteo del tren es regular y el de la lluvia no', () => {
    // Un tren a intervalos al azar suena a lluvia con prisa. La regularidad es
    // lo que se reconoce, así que se mide.
    const irregularidad = (muestras: Float32Array) => {
      const umbral = eficazDe(muestras) * 4;
      const posiciones: number[] = [];
      let ultimo = -TASA;
      muestras.forEach((valor, i) => {
        if (Math.abs(valor) > umbral && i - ultimo >= TASA * 0.01) {
          posiciones.push(i);
          ultimo = i;
        }
      });
      const huecos = posiciones.slice(1).map((p, i) => p - (posiciones[i] ?? 0));
      const media = huecos.reduce((a, b) => a + b, 0) / Math.max(1, huecos.length);
      const desviacion = Math.sqrt(
        huecos.reduce((a, b) => a + (b - media) ** 2, 0) / Math.max(1, huecos.length)
      );
      return media === 0 ? 1 : desviacion / media;
    };

    expect(irregularidad(buferDe('tren', TASA, 12, azarFijo()))).toBeLessThan(
      irregularidad(buferDe('lluvia', TASA, 12, azarFijo()))
    );
  });

  it('todos los ambientes suenan al mismo volumen', () => {
    // Medido antes de nivelarlos: el ruido blanco salía a 0,58 y la chimenea a
    // 0,09, seis veces más flojo. Cambiar de uno a otro con el volumen puesto
    // habría dado un susto, que es lo contrario de un fondo para concentrarse.
    for (const clase of CLASES) {
      const nivel = eficazDe(buferDe(clase, TASA, 3, azarFijo()));
      expect(nivel, clase).toBeGreaterThan(0.08);
      expect(nivel, clase).toBeLessThanOrEqual(0.16);
    }
  });

  it('ninguno recorta, que es lo que suena a altavoz roto', () => {
    // Recortar es distorsionar. Por eso al nivelar se respeta el pico además
    // del nivel: con solo el nivel, la chimenea perdía ciento treinta y seis
    // muestras por búfer.
    for (const clase of CLASES) {
      const muestras = buferDe(clase, TASA, 3, azarFijo());
      const recortadas = Array.from(muestras).filter((v) => Math.abs(v) >= 0.999).length;
      expect(recortadas, clase).toBe(0);
    }
  });

  it('los ambientes con vaivén respiran y los ruidos puros no', () => {
    // La respiración de las olas y las ráfagas del viento las ponen los nodos
    // de audio, así que aquí solo se comprueba que estén declaradas: medir el
    // resultado exige un navegador, y eso lo hace `e2e/rendimiento.spec.ts`.
    const conVaiven = (clase: ClaseDeRuido) =>
      RECETAS[clase].capas.some((capa) => capa.vaiven !== undefined);

    expect(conVaiven('oleaje')).toBe(true);
    expect(conVaiven('viento')).toBe(true);
    expect(conVaiven('tormenta')).toBe(true);
    expect(conVaiven('blanco')).toBe(false);
    expect(conVaiven('rosa')).toBe(false);
  });

  it('el ruido puro no respira, que es justo lo que se le pide', () => {
    // Sirve de control de la medida: si `respiracion` diera alto para el ruido
    // blanco, no estaría midiendo nada.
    expect(respiracion(buferDe('blanco', TASA, 4, azarFijo()), TASA)).toBeLessThan(0.1);
  });
});
