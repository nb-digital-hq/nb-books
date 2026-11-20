import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { afterEach, describe, expect, it, vi } from 'vitest';

import { contrasteHex, interpretarHex, UMBRAL_INTERFAZ, UMBRAL_TEXTO } from './contraste';
import { paletaDe } from './paleta';
import {
  APARIENCIA_POR_DEFECTO,
  type Apariencia,
  aplicarApariencia,
  atributosDeApariencia,
  ESCALA_MAXIMA,
  ESCALA_MINIMA,
  esquemaDe,
  TEMAS as LOS_SIETE,
  leerApariencia,
  resolverTema
} from './tema';

/* ------------------------------------------------------------------ *
 * Resolución de la apariencia
 * ------------------------------------------------------------------ */

describe('resolución de la apariencia', () => {
  it('la elección explícita gana sobre la preferencia del sistema', () => {
    expect(resolverTema('papel', true)).toBe('papel');
    expect(resolverTema('grafito', false)).toBe('grafito');
  });

  it('sin elección explícita se sigue al sistema', () => {
    expect(resolverTema('system', true)).toBe('grafito');
    expect(resolverTema('system', false)).toBe('papel');
  });

  it('un tema guardado con los nombres antiguos se traduce en lugar de perderse', () => {
    // Una base anterior trae `light` o `dark`. Sin traducirlos, quien actualice
    // se encontraría con el tema por defecto y creería haber perdido su
    // elección.
    expect(leerApariencia({ theme: 'dark' }).tema).toBe('grafito');
    expect(leerApariencia({ theme: 'light' }).tema).toBe('papel');
    // Brasa se retiró al llegar Grafito, que conserva su acento: quien lo tenía
    // elegido no puede despertarse con el tema por defecto.
    expect(leerApariencia({ theme: 'brasa' }).tema).toBe('grafito');
  });

  it('el acento del usuario se acepta solo si es un color', () => {
    expect(leerApariencia({ accent_color: '#c8674a' }).acento).toBe('#c8674a');
    // Cualquier otra cosa dejaría la variable con basura y el acento sin pintar.
    expect(leerApariencia({ accent_color: 'rojo' }).acento).toBeNull();
    expect(leerApariencia({ accent_color: 'javascript:alert(1)' }).acento).toBeNull();
    expect(leerApariencia({}).acento).toBeNull();
  });

  it('una preferencia corrupta no deja la interfaz sin pintar', () => {
    // Una preferencia ilegible no puede impedir el arranque, y menos una de
    // accesibilidad: se sustituye por su defecto en silencio.
    const leida = leerApariencia({
      theme: 'escritorio-virtual' as never,
      high_contrast: 'sí' as never,
      font_scale: Number.NaN as never
    });
    expect(leida).toEqual(APARIENCIA_POR_DEFECTO);
  });

  it('la escala tipográfica se acota en lugar de deformar la interfaz', () => {
    expect(leerApariencia({ font_scale: 40 }).escalaTipografica).toBe(ESCALA_MAXIMA);
    expect(leerApariencia({ font_scale: 0.01 }).escalaTipografica).toBe(ESCALA_MINIMA);
    expect(leerApariencia({ font_scale: 1.5 }).escalaTipografica).toBe(1.5);
  });

  it('lo no pedido no se escribe: el alto contraste y el movimiento reducido se omiten', () => {
    // Escribirlos vacíos también seleccionaría en la hoja de estilos, con lo que
    // todo el mundo tendría alto contraste sin haberlo pedido.
    const atributos = atributosDeApariencia(APARIENCIA_POR_DEFECTO, false);
    expect(atributos['data-contrast']).toBeNull();
    expect(atributos['data-motion']).toBeNull();
  });

  it('la hoja recibe el tema resuelto, nunca `system`', () => {
    // La hoja no sabe seguir al sistema: si le llegara `system`, no habría
    // ningún bloque que aplicar y la interfaz se quedaría con Grafito pintado
    // sobre un escritorio claro.
    const atributos = atributosDeApariencia({ ...APARIENCIA_POR_DEFECTO, tema: 'system' }, true);
    expect(atributos['data-theme']).toBe('grafito');
    expect(atributos['data-resolved-scheme']).toBe('dark');
  });

  it('los temas claros se declaran claros, y eso es lo que ve el alto contraste', () => {
    // Sin esto, el alto contraste sobre Pergamino aplicaría la variante oscura:
    // texto blanco sobre un fondo de vitela.
    for (const [tema, esquema] of [
      ['papel', 'light'],
      ['pergamino', 'light'],
      ['gabinete', 'dark'],
      ['tinta', 'dark'],
      ['vigilia', 'dark'],
      ['grafito', 'dark'],
      ['musgo', 'dark']
    ] as const) {
      expect(esquemaDe(tema), `${tema} mal clasificado`).toBe(esquema);
    }
  });

  it('desactivar una preferencia retira su atributo en lugar de dejarlo puesto', () => {
    const raiz = new ElementoSimulado();
    // Sin alto contraste a propósito: con él, el acento del usuario no se
    // aplica —ver la prueba siguiente— y esta comprueba otra cosa.
    const conTodo: Apariencia = {
      tema: 'grafito',
      altoContraste: false,
      movimientoReducido: true,
      escalaTipografica: 1.25,
      temaDeExtension: null,
      acento: '#c8674a'
    };

    aplicarApariencia(raiz as unknown as HTMLElement, conTodo, false);
    expect(raiz.atributos.get('data-motion')).toBe('reduced');
    expect(raiz.propiedades.get('--nb-font-scale')).toBe('1.25');

    expect(raiz.propiedades.get('--nb-accent')).toBe('#c8674a');

    aplicarApariencia(raiz as unknown as HTMLElement, { ...conTodo, altoContraste: true }, false);
    expect(raiz.atributos.get('data-contrast')).toBe('high');

    aplicarApariencia(raiz as unknown as HTMLElement, APARIENCIA_POR_DEFECTO, false);

    // Y el acento vuelve al del tema en lugar de quedarse el anterior: si se
    // quedara, cambiar de tema no cambiaría el acento y parecería roto.
    expect(raiz.propiedades.has('--nb-accent')).toBe(false);
    expect(raiz.atributos.has('data-contrast')).toBe(false);
    expect(raiz.atributos.has('data-motion')).toBe(false);
    expect(raiz.propiedades.get('--nb-font-scale')).toBe('1');
  });

  it('el acento elegido no le gana al alto contraste', () => {
    /*
     * El acento del usuario se escribe en el estilo del propio elemento, y eso
     * gana a cualquier hoja de estilos. Si se aplicara con alto contraste
     * puesto, sustituiría al acento medido de esa paleta por uno que nadie ha
     * medido contra ese fondo: elegir un color le quitaría el contraste a quien
     * lo necesita. La regla es "gana a cualquier tema, nunca al revés".
     */
    const raiz = new ElementoSimulado();

    aplicarApariencia(
      raiz as unknown as HTMLElement,
      {
        tema: 'grafito',
        altoContraste: true,
        movimientoReducido: false,
        escalaTipografica: 1,
        temaDeExtension: null,
        acento: '#c8674a'
      },
      false
    );

    expect(raiz.atributos.get('data-contrast')).toBe('high');
    expect(raiz.propiedades.has('--nb-accent')).toBe(false);
  });
});

describe('la paleta de un acento propio se calcula a demanda', () => {
  /*
   * Ahora la utilería de color no viaja en el paquete de arranque:
   * las catorce paletas de fábrica están en `paletas.css`, y solo un acento propio
   * obliga a calcular. Estas pruebas fijan las tres formas de hacerlo mal: pintar
   * en línea lo que ya pone la hoja, dejar puesto un acento que ya no se quiere, y
   * que una paleta que llega tarde pise a la que se eligió después.
   */
  const conAcento: Apariencia = {
    ...APARIENCIA_POR_DEFECTO,
    tema: 'grafito',
    temaDeExtension: null,
    acento: '#4a90c8'
  };
  const enLinea = (raiz: ElementoSimulado) =>
    [...raiz.propiedades.keys()].filter((clave) => clave.startsWith('--m3c-'));

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('sin acento no se escribe ningún color de Material en línea: lo pone la hoja', async () => {
    const raiz = new ElementoSimulado();
    await aplicarApariencia(
      raiz as unknown as HTMLElement,
      { ...APARIENCIA_POR_DEFECTO, tema: 'gabinete', altoContraste: true },
      false
    );
    expect(enLinea(raiz)).toEqual([]);
  });

  it('con acento llega la paleta calculada, y quitarlo la retira entera', async () => {
    const raiz = new ElementoSimulado();
    const esperada = paletaDe(conAcento, false);

    await aplicarApariencia(raiz as unknown as HTMLElement, conAcento, false);
    expect(raiz.propiedades.get('--m3c-primary')).toBe(esperada['--m3c-primary']);
    expect(enLinea(raiz)).toHaveLength(Object.keys(esperada).length);

    await aplicarApariencia(raiz as unknown as HTMLElement, { ...conAcento, acento: null }, false);
    expect(enLinea(raiz), 'se quedó el acento anterior: el del tema no se vería').toEqual([]);
  });

  it('una paleta que llega tarde no pisa a la apariencia elegida después', async () => {
    const raiz = new ElementoSimulado();
    const tardia = aplicarApariencia(raiz as unknown as HTMLElement, conAcento, false);
    await aplicarApariencia(raiz as unknown as HTMLElement, APARIENCIA_POR_DEFECTO, false);
    await tardia;
    expect(enLinea(raiz)).toEqual([]);
  });

  it('la última paleta calculada se pinta sin esperar a la carga', async () => {
    // La regla 2 de ADR-010, para el acento: sin esto, quien tiene uno propio
    // vería arrancar la aplicación con el del tema y cambiar al suyo.
    const guardado = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (clave: string) => guardado.get(clave) ?? null,
      setItem: (clave: string, valor: string) => {
        guardado.set(clave, valor);
      }
    });
    await aplicarApariencia(new ElementoSimulado() as unknown as HTMLElement, conAcento, false);

    const raiz = new ElementoSimulado();
    const pendiente = aplicarApariencia(raiz as unknown as HTMLElement, conAcento, false);
    expect(raiz.propiedades.get('--m3c-primary'), 'esperó a la carga para pintar el acento').toBe(
      paletaDe(conAcento, false)['--m3c-primary']
    );
    await pendiente;

    // La guardada de otro acento no sirve: pintaría uno que no es el elegido.
    const otra = new ElementoSimulado();
    const pendienteOtra = aplicarApariencia(
      otra as unknown as HTMLElement,
      { ...conAcento, temaDeExtension: null, acento: '#c8674a' },
      false
    );
    expect(enLinea(otra)).toEqual([]);
    await pendienteOtra;
  });
});

/** Elemento mínimo con el mismo contrato que usa `aplicarApariencia`. */
class ElementoSimulado {
  atributos = new Map<string, string>();
  propiedades = new Map<string, string>();
  style = {
    setProperty: (clave: string, valor: string) => {
      this.propiedades.set(clave, valor);
    },
    removeProperty: (clave: string) => {
      this.propiedades.delete(clave);
    }
  };
  setAttribute(clave: string, valor: string) {
    this.atributos.set(clave, valor);
  }
  removeAttribute(clave: string) {
    this.atributos.delete(clave);
  }
}

/* ------------------------------------------------------------------ *
 * La paleta, medida sobre el archivo real
 * ------------------------------------------------------------------ */

const CSS = readFileSync(resolve(import.meta.dirname, '../../app.css'), 'utf8');

/**
 * Extrae el bloque de declaraciones que sigue a un selector literal.
 *
 * El selector puede traer su llave de apertura, que es lo que desambigua
 * `:root {` de `:root[data-contrast='high']`. Al buscar la llave se descuenta,
 * porque si no se buscaría la **siguiente** y se leería el bloque equivocado:
 * al escribir esto ocurrió exactamente eso, y los contrastes del tema claro se
 * estuvieron midiendo sobre la paleta oscura sin que nada fallara.
 */
function bloque(selector: string): string {
  const inicio = CSS.indexOf(selector);
  if (inicio === -1) throw new Error(`El selector no está en app.css: ${selector}`);

  const sinLlave = selector.replace(/\s*\{$/, '');
  let i = CSS.indexOf('{', inicio + sinLlave.length);
  let profundidad = 0;
  const desde = i + 1;

  for (; i < CSS.length; i += 1) {
    if (CSS[i] === '{') profundidad += 1;
    else if (CSS[i] === '}') {
      profundidad -= 1;
      if (profundidad === 0) return CSS.slice(desde, i);
    }
  }

  throw new Error(`Bloque sin cerrar: ${selector}`);
}

/** Tokens de color declarados en un bloque, en hexadecimal. */
function coloresDe(selector: string): Record<string, string> {
  const colores: Record<string, string> = {};
  const declaraciones = bloque(selector).matchAll(/(--nb-[a-z-]+)\s*:\s*([^;]+);/g);

  for (const [, token, valor] of declaraciones) {
    if (token && valor && interpretarHex(valor.trim())) colores[token] = valor.trim();
  }

  return colores;
}

/**
 * Los siete temas más las dos variantes de alto contraste.
 *
 * Los selectores no llevan `:root` a propósito: con el selector abierto, poner
 * `data-theme` en cualquier elemento lo viste con ese tema, y eso es lo que
 * permite que la miniatura de Ajustes se pinte con los tokens de verdad en
 * lugar de con una copia de sus colores que acabaría divergiendo.
 *
 * Un tema con carácter que no se puede leer no es un tema, es un defecto: aquí
 * se mide, no se afirma. Los que solo cambian la paleta heredan de Gabinete lo
 * demás, así que se comparan contra sus propios colores y no contra los
 * heredados.
 */
const TEMAS = {
  gabinete: coloresDe("[data-theme='gabinete'] {"),
  papel: coloresDe("[data-theme='papel']"),
  tinta: coloresDe("[data-theme='tinta']"),
  pergamino: coloresDe("[data-theme='pergamino']"),
  vigilia: coloresDe("[data-theme='vigilia']"),
  grafito: coloresDe("[data-theme='grafito']"),
  musgo: coloresDe("[data-theme='musgo']"),
  'alto contraste claro': coloresDe("[data-contrast='high'] {"),
  'alto contraste oscuro': coloresDe("[data-contrast='high'][data-resolved-scheme='dark']")
};

/**
 * Pares que se pintan uno encima del otro, con el umbral que les corresponde.
 * Texto normal: criterio 1.4.3. Trazos de control: criterio 1.4.11.
 */
const PARES: ReadonlyArray<readonly [string, string, number]> = [
  ['--nb-text', '--nb-surface', UMBRAL_TEXTO],
  ['--nb-text', '--nb-surface-raised', UMBRAL_TEXTO],
  ['--nb-text', '--nb-surface-sunken', UMBRAL_TEXTO],
  ['--nb-text', '--nb-accent-soft', UMBRAL_TEXTO],
  ['--nb-text-muted', '--nb-surface', UMBRAL_TEXTO],
  ['--nb-text-muted', '--nb-surface-raised', UMBRAL_TEXTO],
  ['--nb-text-subtle', '--nb-surface', UMBRAL_TEXTO],
  ['--nb-text-subtle', '--nb-surface-raised', UMBRAL_TEXTO],
  ['--nb-accent', '--nb-surface', UMBRAL_TEXTO],
  ['--nb-accent', '--nb-surface-raised', UMBRAL_TEXTO],
  ['--nb-danger', '--nb-surface', UMBRAL_TEXTO],
  ['--nb-on-accent', '--nb-accent', UMBRAL_TEXTO],
  ['--nb-on-danger', '--nb-danger', UMBRAL_TEXTO],
  ['--nb-border-strong', '--nb-surface', UMBRAL_INTERFAZ],
  ['--nb-border-strong', '--nb-surface-raised', UMBRAL_INTERFAZ],
  ['--nb-focus', '--nb-surface', UMBRAL_INTERFAZ],
  ['--nb-focus', '--nb-surface-raised', UMBRAL_INTERFAZ]
];

describe('la paleta cumple WCAG 2.1 AA, medido sobre app.css', () => {
  for (const [nombre, tokens] of Object.entries(TEMAS)) {
    for (const [frente, fondo, umbral] of PARES) {
      it(`${nombre}: ${frente} sobre ${fondo} alcanza ${umbral}:1`, () => {
        const a = tokens[frente];
        const b = tokens[fondo];
        expect(a, `${nombre} no declara ${frente}`).toBeDefined();
        expect(b, `${nombre} no declara ${fondo}`).toBeDefined();
        expect(contrasteHex(a as string, b as string)).toBeGreaterThanOrEqual(umbral);
      });
    }
  }

  it('ningún tema deja un token de color sin declarar', () => {
    // Un token que falta en un tema hereda el de Gabinete. No es un error de
    // compilación ni se ve en el tema por defecto: solo lo sufre quien usa el
    // tema que lo perdió, que es la peor forma posible de romper esto.
    const esperados = Object.keys(TEMAS.gabinete).sort();
    for (const [nombre, tokens] of Object.entries(TEMAS)) {
      expect(Object.keys(tokens).sort(), `faltan tokens en ${nombre}`).toEqual(esperados);
    }
  });

  it('el alto contraste supera con holgura a cualquiera de los siete', () => {
    // Un tema con carácter no puede costarle el contraste a quien lo necesita:
    // el alto contraste se aplica encima y gana a todos.
    const alto = contrasteHex(
      TEMAS['alto contraste claro']['--nb-text'] as string,
      TEMAS['alto contraste claro']['--nb-surface'] as string
    );
    expect(alto).toBeGreaterThanOrEqual(7);

    for (const nombre of [
      'grafito',
      'gabinete',
      'papel',
      'tinta',
      'pergamino',
      'vigilia',
      'musgo'
    ]) {
      const tema = TEMAS[nombre as keyof typeof TEMAS];
      const normal = contrasteHex(tema['--nb-text'] as string, tema['--nb-surface'] as string);
      expect(alto, `${nombre} no debería superar al alto contraste`).toBeGreaterThan(normal);
    }
  });

  it('los siete tienen carácter propio y no son el mismo tema repintado', () => {
    // Dos temas con el mismo fondo son un tema con dos nombres. La comprobación
    // parece obvia y no lo es: al escribir estos, dos acabaron con el mismo
    // gris de superficie por copiar el bloque anterior.
    const fondos = ['grafito', 'gabinete', 'papel', 'tinta', 'pergamino', 'vigilia', 'musgo'].map(
      (nombre) => TEMAS[nombre as keyof typeof TEMAS]['--nb-surface']
    );
    expect(new Set(fondos).size).toBe(fondos.length);
  });
});

/* ------------------------------------------------------------------ *
 * La paleta que de verdad se pinta
 * ------------------------------------------------------------------ */

/**
 * **Por qué hacía falta una segunda suite y no bastaba la de arriba.**
 *
 * La de arriba mide los tokens `--nb-*` de `app.css`, que es donde vivían los
 * colores hasta que ADR-014 trajo Material 3. Desde entonces los componentes
 * pintan con `--m3c-*`, derivados de la semilla en tiempo de ejecución: más de
 * cuatrocientos usos contra unas decenas de los otros. Medir una paleta que ya
 * casi no se pinta deja la garantía de contraste en verde y la pantalla sin
 * comprobar, que es la peor combinación posible.
 *
 * Los pares son los que Material considera legibles por contrato -un color
 * `on-X` sobre su `X`- más los que este producto usa como texto secundario. No
 * está `outline-variant`, y es a propósito: en Material es un separador
 * decorativo, no un borde con significado, y exigirle 3:1 obligaría a subirlo
 * hasta convertirlo en una línea que grita. Los bordes que sí significan algo
 * usan `outline`, que sí está.
 */
const PARES_M3: [string, string, number][] = [
  ['--m3c-on-surface', '--m3c-surface', UMBRAL_TEXTO],
  ['--m3c-on-surface-variant', '--m3c-surface', UMBRAL_TEXTO],
  ['--m3c-on-surface', '--m3c-surface-container-highest', UMBRAL_TEXTO],
  ['--m3c-on-surface-variant', '--m3c-surface-container-highest', UMBRAL_TEXTO],
  ['--m3c-on-primary', '--m3c-primary', UMBRAL_TEXTO],
  ['--m3c-on-secondary-container', '--m3c-secondary-container', UMBRAL_TEXTO],
  ['--m3c-primary', '--m3c-surface', UMBRAL_TEXTO],
  ['--m3c-error', '--m3c-surface', UMBRAL_TEXTO],
  ['--m3c-outline', '--m3c-surface', UMBRAL_INTERFAZ]
];

describe('la paleta de Material cumple WCAG 2.1 AA, medida donde se pinta', () => {
  for (const tema of LOS_SIETE) {
    const paleta = paletaDe({ ...APARIENCIA_POR_DEFECTO, tema }, false);
    for (const [frente, fondo, umbral] of PARES_M3) {
      it(`${tema}: ${frente} sobre ${fondo} alcanza ${umbral}:1`, () => {
        expect(paleta[frente], `${tema} no deriva ${frente}`).toBeDefined();
        expect(paleta[fondo], `${tema} no deriva ${fondo}`).toBeDefined();
        expect(
          contrasteHex(paleta[frente] as string, paleta[fondo] as string)
        ).toBeGreaterThanOrEqual(umbral);
      });
    }
  }

  it('los siete se derivan distintos: siguen siendo siete temas y no uno repintado', () => {
    const fondos = LOS_SIETE.map(
      (tema) => paletaDe({ ...APARIENCIA_POR_DEFECTO, tema }, false)['--m3c-surface']
    );
    expect(new Set(fondos).size).toBe(fondos.length);
  });
});

describe('el alto contraste contrasta más, y esto lo mide', () => {
  /*
   * **Regresión.** Antes el alto contraste sustituía la semilla
   * por blanco o negro puro en lugar de subir el nivel de contraste de Material.
   * Medido, no cambiaba nada: 14,36 de texto sobre fondo con alto contraste y
   * 14,36 sin él, sobre Gabinete. Y de paso le cambiaba el tono al tema, porque
   * una semilla acromática no tiene matiz del que partir.
   *
   * Pasó desapercibido porque la única prueba que hablaba de alto contraste
   * medía la paleta de `app.css`, que el alto contraste sí cambiaba y que la
   * pantalla ya casi no usa.
   */
  for (const tema of LOS_SIETE) {
    it(`${tema}: pedirlo sube el contraste en lugar de dejarlo igual`, () => {
      const normal = paletaDe({ ...APARIENCIA_POR_DEFECTO, tema }, false);
      const alto = paletaDe({ ...APARIENCIA_POR_DEFECTO, tema, altoContraste: true }, false);

      const de = (p: Record<string, string | null>, x: string, y: string) =>
        contrasteHex(p[x] as string, p[y] as string);

      expect(
        de(alto, '--m3c-on-surface', '--m3c-surface'),
        'el texto sobre el fondo tiene que separarse más'
      ).toBeGreaterThan(de(normal, '--m3c-on-surface', '--m3c-surface') + 1);

      expect(
        de(alto, '--m3c-outline', '--m3c-surface'),
        'y los bordes también: son lo que delimita las cosas'
      ).toBeGreaterThan(de(normal, '--m3c-outline', '--m3c-surface') + 1);

      expect(
        de(alto, '--m3c-on-surface', '--m3c-surface'),
        'y llegar al nivel AAA, que es de lo que se trata'
      ).toBeGreaterThanOrEqual(7);
    });
  }

  it('y no le cambia el tono al tema, que es lo que pasaba antes', () => {
    // Con la semilla acromática, Gabinete -ámbar- salía cian al pedir alto
    // contraste: quien lo necesita perdía además el tema que había elegido.
    const normal = paletaDe({ ...APARIENCIA_POR_DEFECTO, tema: 'gabinete' }, false);
    const alto = paletaDe(
      { ...APARIENCIA_POR_DEFECTO, tema: 'gabinete', altoContraste: true },
      false
    );
    const matiz = (hex: string | null | undefined) => {
      const { r, g, b } = interpretarHex(hex as string) as { r: number; g: number; b: number };
      return r >= g && g >= b;
    };
    expect(matiz(alto['--m3c-primary'])).toBe(matiz(normal['--m3c-primary']));
  });
});

/* ------------------------------------------------------------------ *
 * Que ningún token pintado se quede sin derivar
 * ------------------------------------------------------------------ */

/**
 * El equivalente para Material de la regla que ya vigila `check-ui.mjs` sobre
 * las variables `--nb-*`: **una variable de CSS que no existe no falla en
 * ninguna parte**. El navegador se encuentra `var(--m3c-lo-que-sea)` sin valor,
 * aplica el valor inicial de la propiedad y sigue, así que un token mal escrito
 * o retirado se ve como un color que no acaba de ser el que era. Es la clase de
 * defecto que se descubre mirando la pantalla y dudando.
 *
 * Vive aquí y no en `check-ui.mjs` porque los tokens de Material no están
 * escritos en ningún archivo: los deriva el generador de la semilla, y para
 * saber cuáles hay hay que ejecutarlo.
 */
describe('ningún componente pinta con un token de Material que no se derive', () => {
  it('todos los que se usan salen del generador', async () => {
    const { generarVariablesM3 } = await import('./generadorM3');
    const { readdirSync, statSync } = await import('node:fs');

    const derivados = new Set(Object.keys(generarVariablesM3('#d9a441', true)));
    const recorrer = (dir: string): string[] =>
      readdirSync(dir).flatMap((nombre) => {
        const ruta = resolve(dir, nombre);
        return statSync(ruta).isDirectory()
          ? recorrer(ruta)
          : // Las pruebas no pintan: esta misma nombra un token de ejemplo.
            /\.(svelte|css|ts)$/.test(nombre) && !nombre.endsWith('.test.ts')
            ? [ruta]
            : [];
      });

    const sinDerivar = new Map<string, string>();
    for (const archivo of recorrer(resolve('src'))) {
      for (const uso of readFileSync(archivo, 'utf8').matchAll(/var\(\s*(--m3c-[a-z0-9-]+)/g)) {
        const token = uso[1] as string;
        if (!derivados.has(token)) sinDerivar.set(token, archivo);
      }
    }

    expect(
      Object.fromEntries(sinDerivar),
      'estos tokens se pintan y no los deriva nadie: se ven como el color que no acaba de ser'
    ).toEqual({});
  });
});

/* ------------------------------------------------------------------ *
 * Grafito: grises de verdad y un solo color
 * ------------------------------------------------------------------ */

/**
 * Se pidió el 2026-09-11 al fijar el tema de partida: "que no tenga el
 * fondo anaranjado ni nada, solo en escala de grises excepto el color de acento".
 * Material tiñe los neutros de la semilla por defecto, así que sin esto la
 * superficie de Grafito sale café y nadie lo notaría leyendo el código.
 */
describe('Grafito deja los grises en gris y el color en el acento', () => {
  const croma = (hex: string | null | undefined) => {
    const { r, g, b } = interpretarHex(hex as string) as { r: number; g: number; b: number };
    return Math.max(r, g, b) - Math.min(r, g, b);
  };
  const NEUTROS = [
    '--m3c-surface',
    '--m3c-surface-container-low',
    '--m3c-surface-container',
    '--m3c-surface-container-highest',
    '--m3c-on-surface',
    '--m3c-on-surface-variant',
    '--m3c-outline',
    '--m3c-outline-variant',
    '--m3c-secondary-container'
  ];

  it('sus superficies, textos y trazos no tienen matiz, y su acento sí', () => {
    for (const altoContraste of [false, true]) {
      const paleta = paletaDe({ ...APARIENCIA_POR_DEFECTO, tema: 'grafito', altoContraste }, false);
      for (const token of NEUTROS) {
        expect(croma(paleta[token]), `${token} tiene matiz`).toBe(0);
      }
      // Con alto contraste Material aclara la primaria hacia el blanco, así que
      // conserva menos croma; lo que no puede es perderlo entero.
      expect(croma(paleta['--m3c-primary']), 'el acento se quedó sin color').toBeGreaterThan(
        altoContraste ? 10 : 40
      );
    }
  });

  it('otro acento sobre Grafito cambia el acento y deja los grises', () => {
    const paleta = paletaDe(
      { ...APARIENCIA_POR_DEFECTO, tema: 'grafito', temaDeExtension: null, acento: '#4a90c8' },
      false
    );
    expect(croma(paleta['--m3c-surface'])).toBe(0);
    expect(croma(paleta['--m3c-primary'])).toBeGreaterThan(40);
  });

  it('los demás temas siguen tiñendo sus neutros, que es parte de su carácter', () => {
    const gabinete = paletaDe({ ...APARIENCIA_POR_DEFECTO, tema: 'gabinete' }, false);
    expect(croma(gabinete['--m3c-surface'])).toBeGreaterThan(0);
  });
});
