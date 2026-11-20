/**
 * **El criterio de aceptación, primera mitad:** el flujo completo de
 * lectura y anotación es operable solo con teclado.
 *
 * La demostración es una secuencia de pulsaciones y nada más. No se llama a
 * ninguna transición a mano: todo lo que ocurre aquí ocurre porque una tecla lo
 * provocó, que es lo mismo que le pasa a quien no puede usar el ratón.
 *
 * Lo que esto **no** demuestra: que el navegador entregue esas teclas al
 * componente. Eso son eventos reales del DOM dentro del webview, y exige conducir el binario
 * (Playwright sobre el binario), como fija `accessibility.md`, sección 6.
 */

import { describe, expect, it } from 'vitest';

import type { Locator, ReaderCapabilities } from '$lib/bindings';

import { ICONOS } from '../iconos';
import {
  atajoLegible,
  barraDelLector,
  COMANDOS,
  comandoParaTecla,
  comandosDisponibles,
  type Pulsacion
} from './comandos';
import {
  aplicar,
  citaSeleccionada,
  type Efecto,
  type EstadoDelLector,
  estadoInicial,
  redactar,
  seleccion,
  zonasVisibles
} from './estado';

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
  locations: { fragments: ['#char=0'], progression: 0, totalProgression: 0 },
  text: { highlight: 'Muchos años después' }
};

const BLOQUES = [
  { texto: 'Muchos años después, frente al pelotón de fusilamiento.' },
  { texto: 'El coronel Aureliano Buendía había de recordar aquella tarde remota.' },
  { texto: 'Macondo era entonces una aldea de veinte casas de barro.' }
];

/**
 * Teclea sobre el estado y devuelve dónde queda, con lo que se produjo.
 *
 * Es el único camino que usan estas pruebas para cambiar nada: si una tecla no
 * sirve, aquí no pasa nada, exactamente como en la pantalla.
 */
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

describe('el criterio de teclado', () => {
  it('lleva de abrir el documento a tener una nota guardada sin tocar el ratón', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);

    // Entrar al texto y colocarse en el segundo párrafo.
    ({ estado } = teclear(estado, [tecla('ArrowDown'), tecla('ArrowDown')]));
    expect(estado.cursor).toBe(1);

    // Extender la selección al tercero.
    ({ estado } = teclear(estado, [tecla('ArrowDown', { shiftKey: true })]));
    expect(seleccion(estado)).toEqual({ desde: 1, hasta: 2 });

    // Abrir la nota: una sola tecla, y la de aceptar.
    ({ estado } = teclear(estado, [tecla('Enter')]));
    expect(estado.redaccion).toEqual({ cuerpo: '' });

    // Escribir. Mientras se escribe, las letras son letras.
    estado = redactar(estado, 'Aquí empieza el tiempo circular de la novela.');
    const alEscribir = teclear(estado, [tecla('r'), tecla('b'), tecla('z')]);
    expect(alEscribir.efectos).toEqual([]);
    expect(alEscribir.estado.redaccion?.cuerpo).toBe(
      'Aquí empieza el tiempo circular de la novela.'
    );

    // Guardar.
    const guardado = teclear(estado, [tecla('Enter', { ctrlKey: true })]);
    expect(guardado.efectos).toEqual([
      {
        tipo: 'anotar',
        clase: 'note',
        cita: 'El coronel Aureliano Buendía había de recordar aquella tarde remota. Macondo era entonces una aldea de veinte casas de barro.',
        cuerpo: 'Aquí empieza el tiempo circular de la novela.'
      }
    ]);
    expect(guardado.estado.redaccion).toBeNull();
  });

  it('deja poner un marcador y pasar página con una sola tecla cada uno', () => {
    const { efectos } = teclear(estadoInicial(LOCATOR, BLOQUES), [
      tecla('b'),
      tecla('ArrowRight'),
      tecla('ArrowLeft')
    ]);

    expect(efectos).toEqual([
      { tipo: 'marcador' },
      { tipo: 'pagina', direccion: 1 },
      { tipo: 'pagina', direccion: -1 }
    ]);
  });

  it('dispara la orden de paradero "¿Dónde estoy?" con la tecla P', () => {
    const { efectos } = teclear(estadoInicial(LOCATOR, BLOQUES), [tecla('p')]);
    expect(efectos).toEqual([{ tipo: 'donde-estoy' }]);
  });

  it('abre y cierra cada panel con su tecla, sin atrapar el foco', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);

    for (const [key, panel] of [
      ['i', 'indice'],
      ['t', 'ajustes-del-libro']
    ] as const) {
      ({ estado } = teclear(estado, [tecla(key)]));
      expect(estado.panel).toBe(panel);
      // La misma tecla lo cierra, y Escape también: dos vías de salida, ninguna
      // que dependa de alcanzar un botón.
      ({ estado } = teclear(estado, [tecla(key)]));
      expect(estado.panel).toBeNull();

      ({ estado } = teclear(estado, [tecla(key), tecla('Escape')]));
      expect(estado.panel).toBeNull();
    }

    ({ estado } = teclear(estado, [tecla('f', { ctrlKey: true })]));
    expect(estado.panel).toBe('buscar');
  });

  it('no deja al usuario encerrado: Escape sale de un escalón cada vez', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);
    ({ estado } = teclear(estado, [
      tecla('z'),
      tecla('ArrowDown'),
      tecla('ArrowDown', { shiftKey: true }),
      tecla('Enter')
    ]));
    expect(estado.inmersivo).toBe(true);
    expect(estado.redaccion).not.toBeNull();

    // Uno: cancela la nota. Dos: suelta la selección. Tres: sale de la
    // inmersión. Ninguna pulsación se salta un escalón.
    ({ estado } = teclear(estado, [tecla('Escape')]));
    expect(estado.redaccion).toBeNull();
    expect(estado.inmersivo).toBe(true);

    ({ estado } = teclear(estado, [tecla('Escape')]));
    expect(estado.ancla).toBeNull();
    expect(estado.inmersivo).toBe(true);

    ({ estado } = teclear(estado, [tecla('Escape')]));
    expect(estado.inmersivo).toBe(false);
  });

  it('responde igual con Bloq Mayús puesto', () => {
    const { efectos } = teclear(estadoInicial(LOCATOR, BLOQUES), [tecla('B')]);
    expect(efectos).toEqual([{ tipo: 'marcador' }]);
  });

  it('acepta Command donde Ctrl, porque en macOS los atajos son con Command', () => {
    const { estado } = teclear(estadoInicial(LOCATOR, BLOQUES), [tecla('f', { metaKey: true })]);
    expect(estado.panel).toBe('buscar');
  });
});

describe('anotar cuesta menos que resaltar', () => {
  /**
   * Mandato vinculante de la investigación de la experiencia de lectura, 1.1:
   * resaltar por sí solo no mejora el aprendizaje, y quien lee en digital casi
   * no subraya. La interfaz debe empujar hacia la nota.
   *
   * Aquí eso se comprueba de la única forma que no es una opinión: **escribir
   * una nota no exige resaltar antes**. En casi todos los lectores hay que
   * resaltar y después elegir "añadir nota", y eso es lo que hace que la
   * conducta que sirve cueste más que la que no.
   */
  it('la nota no obliga a pasar por un resaltado', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);
    ({ estado } = teclear(estado, [tecla('ArrowDown'), tecla('Enter')]));
    estado = redactar(estado, 'una nota');
    const { efectos } = teclear(estado, [tecla('Enter', { ctrlKey: true })]);

    expect(efectos.map((e) => e.tipo === 'anotar' && e.clase)).toEqual(['note']);
  });

  it('la nota es la acción por defecto de una selección', () => {
    const anotar = COMANDOS.find((c) => c.id === 'anotar');
    const resaltar = COMANDOS.find((c) => c.id === 'resaltar');

    expect(anotar?.atajo).toEqual({ tecla: 'Enter' });
    expect(resaltar?.atajo.tecla).not.toBe('Enter');
  });

  it('subrayar y resaltar son dos marcas distintas sobre la misma selección', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);
    ({ estado } = teclear(estado, [tecla('ArrowDown')]));

    const resaltado = teclear(estado, [tecla('r')]);
    const subrayado = teclear(estado, [tecla('s')]);

    expect(resaltado.efectos.map((e) => e.tipo === 'anotar' && e.clase)).toEqual(['highlight']);
    expect(subrayado.efectos.map((e) => e.tipo === 'anotar' && e.clase)).toEqual(['underline']);
  });

  /*
   * Sin selección no marcan nada, ninguna de las dos.
   *
   * Es la misma regla que ya cumplía anotar: marcar sin decir qué se marca
   * produciría una anotación anclada a lo que hubiera debajo del cursor, que no
   * es lo que nadie pidió.
   */
  it('subrayar sin selección no crea nada', () => {
    const estado = estadoInicial(LOCATOR, BLOQUES);

    const { efectos } = teclear(estado, [tecla('s')]);

    expect(efectos).toEqual([]);
  });

  /*
   * El lápiz es un modo, no un gesto suelto.
   *
   * Dibujar y seleccionar texto compiten por el mismo movimiento —apoyar y
   * arrastrar—, así que no pueden convivir. Con un modo explícito quien lee sabe
   * en cuál está; sin él, arrastrar haría una cosa u otra según un estado
   * invisible.
   */
  it('el lápiz se enciende y se apaga con la misma tecla', () => {
    const inicial = estadoInicial(LOCATOR, BLOQUES);

    const encendido = teclear(inicial, [tecla('l')]).estado;
    const apagado = teclear(encendido, [tecla('l')]).estado;

    expect(inicial.dibujando).toBe(false);
    expect(encendido.dibujando).toBe(true);
    expect(apagado.dibujando).toBe(false);
  });

  it('encender el lápiz suelta la selección extendida', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);
    ({ estado } = teclear(estado, [tecla('ArrowDown'), tecla('ArrowDown', { shiftKey: true })]));
    expect(estado.ancla).not.toBeNull();

    const conLapiz = teclear(estado, [tecla('l')]).estado;

    expect(conLapiz.ancla).toBeNull();
  });

  /* Dejarlo encendido haría que el siguiente arrastre dibujara sin querer. */
  it('Escape apaga el lápiz antes que nada más', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);
    ({ estado } = teclear(estado, [tecla('ArrowDown'), tecla('l')]));

    const salido = teclear(estado, [tecla('Escape')]).estado;

    expect(salido.dibujando).toBe(false);
  });

  it('una nota vacía no se guarda', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);
    ({ estado } = teclear(estado, [tecla('ArrowDown'), tecla('Enter')]));
    estado = redactar(estado, '   ');
    const { efectos, estado: despues } = teclear(estado, [tecla('Enter', { ctrlKey: true })]);

    expect(efectos).toEqual([]);
    expect(despues.redaccion).toBeNull();
  });
});

describe('los controles salen de las capacidades y no del tipo de medio', () => {
  it('un cómic no ofrece tipografía, ni búsqueda, ni anotar texto', () => {
    const ids = comandosDisponibles(COMIC).map((c) => c.id);

    expect(ids).not.toContain('ajustes-del-libro');
    expect(ids).not.toContain('buscar');
    expect(ids).not.toContain('anotar');
    expect(ids).not.toContain('resaltar');
    // Pero sí lo que no depende del texto: pasar página, marcar y leer inmerso.
    expect(ids).toEqual(
      expect.arrayContaining([
        'marcador',
        'pagina-siguiente',
        'pagina-anterior',
        'inmersion',
        'salir'
      ])
    );
  });

  it('sobre un cómic, la tecla de resaltar no hace nada en lugar de fallar', () => {
    const { efectos } = teclear(
      { ...estadoInicial(LOCATOR, BLOQUES), cursor: 0 },
      [tecla('r'), tecla('t')],
      COMIC
    );
    expect(efectos).toEqual([]);
  });

  it('un EPUB ofrece todos los que hay, salvo ampliar una página que no tiene', () => {
    const deImagen = COMANDOS.filter((c) => c.requiere === 'ajuste-de-pagina').length;
    expect(deImagen).toBe(2);
    expect(comandosDisponibles(EPUB)).toHaveLength(COMANDOS.length - deImagen);
  });
});

describe('el registro es la única fuente de las acciones', () => {
  it('todo comando tiene atajo', () => {
    for (const comando of COMANDOS) {
      expect(comando.atajo.tecla, `${comando.id} sin tecla`).toBeTruthy();
    }
  });

  it('ningún atajo está repetido', () => {
    const vistos = new Map<string, string>();
    for (const comando of COMANDOS) {
      const firma = `${comando.atajo.control ? 'C-' : ''}${comando.atajo.mayus ? 'S-' : ''}${comando.atajo.tecla}`;
      expect(vistos.get(firma), `${comando.id} repite el atajo de ${vistos.get(firma)}`).toBe(
        undefined
      );
      vistos.set(firma, comando.id);
    }
  });

  it('todo comando tiene icono del catálogo, porque no hay letras haciendo de icono', () => {
    for (const comando of COMANDOS) {
      expect(comando.icono in ICONOS, `${comando.id} sin icono`).toBe(true);
    }
  });

  // La etiqueta visible ya no vive en el registro: es del diccionario desde
  // cada comando tiene etiqueta, y `i18n.test.ts` comprueba que ningún comando se queda sin ella en
  // ninguno de los tres idiomas.

  it('el atajo se puede mostrar junto a su acción', () => {
    expect(atajoLegible({ tecla: 'r' })).toBe('R');
    expect(atajoLegible({ tecla: 'f', control: true })).toBe('Ctrl+F');
    expect(atajoLegible({ tecla: 'ArrowDown', mayus: true })).toBe('Mayús+ArrowDown');
    // Con los nombres del idioma, ninguna tecla sale en inglés.
    expect(
      atajoLegible(
        { tecla: 'ArrowDown', mayus: true },
        { ArrowDown: 'Flecha Abajo', Shift: 'Mayús' }
      )
    ).toBe('Mayús+Flecha Abajo');
  });
});

describe('la carcasa se retira entera y vuelve sin perder la posición', () => {
  it('en inmersión no queda ninguna zona de carcasa', () => {
    const { estado } = teclear(estadoInicial(LOCATOR, BLOQUES), [tecla('i'), tecla('z')]);

    expect(estado.inmersivo).toBe(true);
    expect(zonasVisibles(estado)).toEqual({
      banner: false,
      navegacion: false,
      paneles: false
    });
    // Un panel abierto es carcasa: entrar en inmersión lo cierra.
    expect(estado.panel).toBeNull();
  });

  it('entrar y salir de la inmersión no toca la posición ni la selección', () => {
    let estado = estadoInicial(LOCATOR, BLOQUES);
    ({ estado } = teclear(estado, [
      tecla('ArrowDown'),
      tecla('ArrowDown'),
      tecla('ArrowDown', { shiftKey: true })
    ]));
    const posicion = estado.locator;
    const cita = citaSeleccionada(estado);

    ({ estado } = teclear(estado, [tecla('z')]));
    expect(estado.locator).toEqual(posicion);

    ({ estado } = teclear(estado, [tecla('z')]));
    expect(estado.inmersivo).toBe(false);
    expect(estado.locator).toEqual(posicion);
    expect(citaSeleccionada(estado)).toBe(cita);
    expect(zonasVisibles(estado).banner).toBe(true);
  });

  it('la pantalla completa del sistema se pide al runtime y arrastra la inmersión', () => {
    const { estado, efectos } = teclear(estadoInicial(LOCATOR, BLOQUES), [tecla('F11')]);

    expect(estado.pantallaCompleta).toBe(true);
    expect(estado.inmersivo).toBe(true);
    expect(efectos).toEqual([{ tipo: 'pantalla-completa', activa: true }]);

    const salida = teclear(estado, [tecla('Escape')]);
    expect(salida.estado.pantallaCompleta).toBe(false);
    expect(salida.efectos).toEqual([{ tipo: 'pantalla-completa', activa: false }]);
  });
});

describe('la barra ofrece lo que aplica y nada más', () => {
  const contexto = (extra: Partial<Parameters<typeof barraDelLector>[0]> = {}) =>
    barraDelLector({
      capacidades: EPUB,
      haySeleccion: false,
      dividido: false,
      cabeDividir: true,
      ...extra
    });

  const ids = (familias: ReturnType<typeof barraDelLector>) =>
    familias.flatMap((f) => f.comandos.map((c) => c.id));

  it('va agrupada, y ninguna familia sale vacía', () => {
    const familias = contexto();

    expect(familias.length).toBeGreaterThan(1);
    for (const familia of familias) {
      expect(familia.comandos.length, `${familia.grupo} vacía`).toBeGreaterThan(0);
    }
  });

  it('sin selección no ofrece lo que exige una selección', () => {
    // Se pintaban siempre y eran inertes: pulsarlos no hacía nada.
    expect(ids(contexto())).not.toContain('anotar');
    expect(ids(contexto())).not.toContain('resaltar');
    expect(ids(contexto())).not.toContain('consultar');
  });

  it('con selección aparecen los cuatro de golpe, no de uno en uno', () => {
    const conSeleccion = ids(contexto({ haySeleccion: true }));

    expect(conSeleccion).toContain('anotar');
    expect(conSeleccion).toContain('resaltar');
    expect(conSeleccion).toContain('extender-siguiente');
  });

  it('guardar la nota no ocupa un botón: su sitio es el campo de la nota', () => {
    expect(ids(contexto({ haySeleccion: true }))).not.toContain('confirmar');
  });

  it('sin división no ofrece cambiar de mitad ni mover un separador que no hay', () => {
    const sinDividir = ids(contexto());

    expect(sinDividir).toContain('dividir');
    expect(sinDividir).not.toContain('cambiar-mitad');
    expect(sinDividir).not.toContain('estrechar-mitad');
    expect(sinDividir).not.toContain('ensanchar-mitad');
  });

  it('con división aparecen los tres', () => {
    const dividido = ids(contexto({ dividido: true }));

    expect(dividido).toContain('cambiar-mitad');
    expect(dividido).toContain('estrechar-mitad');
    expect(dividido).toContain('ensanchar-mitad');
  });

  it('en una ventana donde no caben dos columnas no se ofrece dividir', () => {
    expect(ids(contexto({ cabeDividir: false }))).not.toContain('dividir');
  });

  it('el teclado no depende de la barra: la tecla sigue haciendo lo suyo', () => {
    // La barra oculta `extender-siguiente` sin selección; la tecla no.
    const comando = comandoParaTecla(
      { key: 'ArrowDown', shiftKey: true },
      { capacidades: EPUB, haySeleccion: false, escribiendo: false }
    );

    expect(comando?.id).toBe('extender-siguiente');
  });
});

describe('la selección libre con el ratón', () => {
  it('la cita es el texto exacto, y moverse con el teclado la suelta', async () => {
    const { seleccionarLibre, citaSeleccionada, aplicar, estadoInicial } = await import('./estado');
    const { COMANDOS } = await import('./comandos');
    const base = estadoInicial(LOCATOR, BLOQUES);
    const libre = seleccionarLibre(base, 1, 0, '  una   frase ');
    expect(citaSeleccionada(libre)).toBe('una frase');
    expect(libre.ancla).toBe(0);
    expect(libre.cursor).toBe(1);
    const bajar = COMANDOS.find((c) => c.id === 'bloque-siguiente');
    if (!bajar) throw new Error('sin comando');
    expect(aplicar(libre, bajar).estado.libre).toBeNull();
  });
});

describe('las órdenes que nacieron con Leer en el PC tienen tecla', () => {
  it('Mayús con las flechas laterales salta de capítulo y sin Mayús pasa página', () => {
    const { efectos } = teclear(estadoInicial(LOCATOR, BLOQUES), [
      tecla('ArrowRight', { shiftKey: true }),
      tecla('ArrowLeft', { shiftKey: true }),
      tecla('ArrowRight')
    ]);
    expect(efectos).toEqual([
      { tipo: 'capitulo', direccion: 1 },
      { tipo: 'capitulo', direccion: -1 },
      { tipo: 'pagina', direccion: 1 }
    ]);
  });

  it('la G enciende y apaga el avance automático, y escribiendo es una letra', () => {
    const { efectos } = teclear(estadoInicial(LOCATOR, BLOQUES), [tecla('g')]);
    expect(efectos).toEqual([{ tipo: 'avance' }]);

    const escribiendo = { ...estadoInicial(LOCATOR, BLOQUES), redaccion: { cuerpo: '' } };
    expect(teclear(escribiendo, [tecla('g')]).efectos).toEqual([]);
  });

  it('todas las órdenes de comando del servidor existen en el registro', () => {
    const remotas = [
      'pagina-siguiente',
      'pagina-anterior',
      'bloque-siguiente',
      'bloque-anterior',
      'marcador',
      'voz',
      'modo-de-lectura',
      'fondo-sonoro',
      'inmersion',
      'pantalla-completa',
      'acercar',
      'alejar',
      'capitulo-siguiente',
      'capitulo-anterior',
      'avance-automatico',
      'salir'
    ];
    const ids = COMANDOS.map((c) => c.id as string);
    for (const id of remotas) expect(ids).toContain(id);
  });
});
