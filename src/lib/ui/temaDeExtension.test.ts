import { describe, expect, it } from 'vitest';

import { UMBRAL_TEXTO } from './contraste';

import { paletaDe } from './paleta';
import { type Apariencia, atributosDeApariencia, esquemaDelTemaDeExtension } from './tema';
import { fallosDeContraste } from './temaDeExtension';

/**
 * Un tema legible, del que parten las variantes.
 *
 * Es el mismo de `extensiones/tema-de-prueba/`, para que si alguna vez deja de
 * cumplir el umbral se entere esta prueba y no quien lo instale.
 */
const LEGIBLE: Record<string, string> = {
  '--nb-surface': '#161a22',
  '--nb-text': '#e8ecf2',
  '--nb-text-muted': '#aab3c0',
  '--nb-accent': '#7fb2e5',
  '--nb-on-accent': '#0d1420',
  '--nb-focus': '#7fb2e5'
};

describe('el contraste de un tema aportado por una extensión', () => {
  it('acepta el tema de ejemplo del repositorio', () => {
    expect(fallosDeContraste(LEGIBLE)).toEqual([]);
  });

  it('rechaza texto que no se lee sobre su fondo', () => {
    const fallos = fallosDeContraste({ ...LEGIBLE, '--nb-text': '#1a1e26' });
    expect(fallos.map((f) => f.encima)).toContain('--nb-text');
  });

  it('rechaza un foco que no se ve, que es la forma de apagarlo sin apagarlo', () => {
    // No hay ningún token para quitar el contorno del foco, así que la única
    // manera de esconderlo sería darle el color del fondo. El umbral de
    // interfaz lo impide.
    const fallos = fallosDeContraste({ ...LEGIBLE, '--nb-focus': '#171b23' });
    expect(fallos.map((f) => f.encima)).toContain('--nb-focus');
  });

  it('no mide un par al que le falta uno de sus dos colores', () => {
    // El que falte lo pone `app.css`, y mezclar uno de cada sitio mediría algo
    // que no es lo que se va a ver.
    expect(fallosDeContraste({ '--nb-text': '#000000' })).toEqual([]);
  });

  it('dice cuánto midió y cuánto hacía falta', () => {
    const fallos = fallosDeContraste({ ...LEGIBLE, '--nb-text': '#1a1e26' });
    // Sin las dos cifras, quien escribió el tema sabe que algo falla y no
    // cuánto le falta para arreglarlo.
    expect(fallos).toHaveLength(1);
    expect(fallos[0]?.medido).toBeLessThan(UMBRAL_TEXTO);
    expect(fallos[0]?.umbral).toBe(UMBRAL_TEXTO);
  });
});

/**
 * La apariencia de la que parten las pruebas del camino nuevo: tema **claro** del
 * núcleo, sin acento propio y sin alto contraste.
 */
const EN_CLARO: Apariencia = {
  tema: 'papel',
  altoContraste: false,
  movimientoReducido: false,
  escalaTipografica: 1,
  acento: null,
  temaDeExtension: null
};

/**
 * **Regresión del 2026-09-12.** Un tema de extensión se aplicaba por su cuenta,
 * escribiendo los doce tokens `--nb-*` que declara, y con el rediseño
 * media interfaz pasó a leer las variables de Material, que un tema no declara.
 * Resultado: un tema oscuro sobre una paleta clara dejaba la pantalla mitad y
 * mitad. Medido en WebKitGTK, un texto quedaba a **1,13** de contraste.
 *
 * Lo que se fija aquí es que el tema gobierna **toda** la apariencia: el esquema,
 * el tema base del núcleo que rellena lo que no declara, y la paleta de Material.
 */
describe('un tema de extensión gobierna la apariencia entera', () => {
  const conTema: Apariencia = { ...EN_CLARO, temaDeExtension: LEGIBLE };

  it('su superficie decide el esquema, aunque el tema del núcleo sea el contrario', () => {
    expect(esquemaDelTemaDeExtension(LEGIBLE)).toBe('dark');
    const atributos = atributosDeApariencia(conTema, false);
    expect(atributos['data-resolved-scheme']).toBe('dark');
    // La base es el neutro oscuro: de ahí salen los doscientos y pico tokens que
    // el tema no declara, y por eso no puede ser un tema claro.
    expect(atributos['data-theme']).toBe('grafito');
  });

  it('sus colores viajan con los atributos, para que nadie los escriba aparte', () => {
    const atributos = atributosDeApariencia(conTema, false);
    for (const [token, valor] of Object.entries(LEGIBLE)) {
      expect(atributos[token]).toBe(valor);
    }
  });

  it('un tema claro sobre un núcleo oscuro también manda', () => {
    const claro = {
      '--nb-surface': '#f7f3ec',
      '--nb-text': '#1d1b16',
      '--nb-accent': '#8c2a20',
      '--nb-on-accent': '#ffffff',
      '--nb-focus': '#8c2a20'
    };
    expect(fallosDeContraste(claro)).toEqual([]);
    const atributos = atributosDeApariencia(
      { ...EN_CLARO, tema: 'grafito', temaDeExtension: claro },
      true
    );
    expect(atributos['data-theme']).toBe('papel');
    expect(atributos['data-resolved-scheme']).toBe('light');
  });

  it('sin superficie declarada no declara esquema, y manda el del núcleo', () => {
    // Es lo único que se puede afirmar sin inventarle un color al autor.
    expect(esquemaDelTemaDeExtension({ '--nb-accent': '#7fb2e5' })).toBeNull();
    const atributos = atributosDeApariencia(
      { ...EN_CLARO, temaDeExtension: { '--nb-accent': '#7fb2e5' } },
      false
    );
    expect(atributos['data-theme']).toBe('papel');
  });

  it('la paleta de Material sale de sus colores, no de los del tema anterior', () => {
    const variables = paletaDe(conTema, false);
    // Las superficies y los textos son los que el autor eligió, no los que
    // Material derivaría: un tema que se ve "parecido" no es el tema.
    expect(variables['--m3c-surface']).toBe(LEGIBLE['--nb-surface']);
    expect(variables['--m3c-on-surface']).toBe(LEGIBLE['--nb-text']);
    expect(variables['--m3c-on-surface-variant']).toBe(LEGIBLE['--nb-text-muted']);
    expect(variables['--m3c-primary']).toBe(LEGIBLE['--nb-accent']);
    expect(variables['--m3c-on-primary']).toBe(LEGIBLE['--nb-on-accent']);
  });

  it('y lo que el tema no declara lo pone Material en su esquema', () => {
    const variables = paletaDe(conTema, false);
    // Un contenedor que el tema no declara tiene que salir oscuro, porque el tema
    // es oscuro: es justo lo que antes se quedaba claro.
    const contenedor = variables['--m3c-secondary-container'] ?? '';
    expect(contenedor).toMatch(/^#[0-9a-f]{6}$/i);
    const claridad = Number.parseInt(contenedor.slice(1, 3), 16);
    expect(claridad).toBeLessThan(0x80);
  });
});
