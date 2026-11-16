/**
 * **El criterio de aceptación:** una prueba recorre el panel, salta a
 * una anotación, la edita y la borra sin usar el ratón ni una sola vez.
 *
 * La demostración es una lista de pulsaciones y nada más. El ayudante [`pulsar`]
 * solo acepta teclas, así que no hay forma de escribir aquí un paso que use el
 * ratón: si una acción no tiene tecla, la prueba no la puede ejecutar y el
 * criterio no se cumple. Es la misma forma que usó `teclado.test.ts` para el
 * criterio.
 *
 * Lo que esto **no** demuestra: que el navegador entregue esas teclas al panel.
 * Eso son eventos reales del DOM dentro del webview, y exige conducir el binario, que sigue
 * bloqueada porque WebKitWebDriver no está empaquetado en Arch.
 */

import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';

import type { Annotation, AnnotationKind, Locator, OutlineEntry } from '$lib/bindings';
import AnnotationsPanel from './AnnotationsPanel.svelte';
import {
  aplicarEnPanel,
  conCursorEn,
  conEntradas,
  type EfectoDelPanel,
  type EstadoDelPanel,
  entradaActiva,
  escribir,
  panelInicial,
  sePuedeEditar,
  sePuedeSaltar,
  teclaDelPanel
} from './anotaciones';
import { COMANDOS, comandosDisponibles, type Pulsacion } from './comandos';

function locator(href: string, cita: string, progresion: number): Locator {
  return {
    version: 1,
    href,
    mediaType: 'application/xhtml+xml',
    title: href,
    locations: { fragments: [`#char=0`], progression: 0, totalProgression: progresion },
    text: { highlight: cita }
  };
}

function anotacion(
  id: string,
  kind: AnnotationKind,
  body: string | null,
  target: Locator
): Annotation {
  return {
    id,
    editionId: 'e1',
    kind,
    body,
    target,
    payload: null,
    createdAt: '2026-08-28T10:00:00.000Z',
    deletedAt: null,
    supersedes: null
  };
}

function entrada(
  id: string,
  kind: AnnotationKind,
  body: string | null,
  cita: string,
  progresion: number,
  marcas: Partial<Pick<OutlineEntry, 'relocated' | 'warn' | 'lost'>> = {}
): OutlineEntry {
  const perdida = marcas.lost ?? false;
  return {
    annotation: anotacion(id, kind, body, locator(`OEBPS/c${progresion}.xhtml`, cita, progresion)),
    // Una perdida no tiene posición ni precisión: es lo que significa que
    // ninguna vía de la cascada supo colocarla.
    position: perdida ? null : { kind: 'page', value: 1 },
    precision: perdida ? null : 'exact',
    relocated: marcas.relocated ?? false,
    warn: marcas.warn ?? false,
    lost: perdida
  };
}

/* Las cuatro situaciones que existen, cada una con nombre: las pruebas hablan
 * de "la perdida" y de "el marcador", no del índice 3 y del índice 2. */
const NOTA = entrada('a1', 'note', 'Lo importante es el hielo', 'Muchos años después', 0.1);
const RESALTADO = entrada('a2', 'highlight', null, 'El coronel Aureliano Buendía', 0.4);
const MARCADOR = entrada('a3', 'bookmark', null, 'Macondo era entonces', 0.7);
const PERDIDA = entrada('a4', 'note', 'Esta ya no ancla', 'Una cita que desapareció', 0.9, {
  lost: true
});

/** El panel de un documento con las cuatro dentro, en orden de lectura. */
const ENTRADAS: OutlineEntry[] = [NOTA, RESALTADO, MARCADOR, PERDIDA];

/**
 * Ejecuta una lista de pulsaciones sobre el panel.
 *
 * **Solo acepta teclas.** No hay parámetro para "pulsar un botón", y esa
 * ausencia es la mitad de la demostración: el recorrido completo del criterio se
 * escribe aquí o no se escribe.
 */
function pulsar(
  inicial: EstadoDelPanel,
  ...teclas: Pulsacion[]
): { estado: EstadoDelPanel; efectos: EfectoDelPanel[] } {
  let estado = inicial;
  const efectos: EfectoDelPanel[] = [];

  for (const tecla of teclas) {
    const accion = teclaDelPanel(tecla, estado.modo);
    if (!accion) continue;
    const transicion = aplicarEnPanel(estado, accion);
    estado = transicion.estado;
    if (transicion.efecto) efectos.push(transicion.efecto);
  }

  return { estado, efectos };
}

const abajo: Pulsacion = { key: 'ArrowDown' };
const arriba: Pulsacion = { key: 'ArrowUp' };
const entrar: Pulsacion = { key: 'Enter' };
const editar: Pulsacion = { key: 'e' };
const suprimir: Pulsacion = { key: 'Delete' };
const guardar: Pulsacion = { key: 'Enter', ctrlKey: true };
const escapar: Pulsacion = { key: 'Escape' };

describe('el criterio de las anotaciones', () => {
  it('recorre el panel, salta, edita y borra sin una sola acción de ratón', () => {
    const panel = panelInicial(ENTRADAS);

    // Recorrer: de la primera a la segunda.
    const recorrido = pulsar(panel, abajo);
    expect(recorrido.estado.cursor).toBe(1);
    expect(entradaActiva(recorrido.estado)?.annotation.id).toBe('a2');

    // Saltar: la posición viaja como Locator, nunca como número.
    const salto = pulsar(recorrido.estado, entrar);
    expect(salto.efectos).toEqual([{ tipo: 'saltar', locator: RESALTADO.annotation.target }]);

    // Editar: se abre la redacción con lo que ya había, se escribe y se guarda.
    const abierta = pulsar(salto.estado, editar);
    expect(abierta.estado.modo).toBe('editando');
    expect(abierta.estado.borrador).toBe('');

    const escrito = escribir(abierta.estado, 'Aquí empieza la soledad');
    const guardado = pulsar(escrito, guardar);
    expect(guardado.estado.modo).toBe('lista');
    expect(guardado.efectos).toEqual([
      { tipo: 'editar', id: 'a2', cuerpo: 'Aquí empieza la soledad' }
    ]);

    // Borrar: pide confirmación, y la confirmación también es una tecla.
    const pidiendo = pulsar(guardado.estado, suprimir);
    expect(pidiendo.estado.modo).toBe('confirmando');
    expect(pidiendo.efectos).toEqual([]);

    const borrado = pulsar(pidiendo.estado, entrar);
    expect(borrado.estado.modo).toBe('lista');
    expect(borrado.efectos).toEqual([{ tipo: 'borrar', id: 'a2' }]);
  });

  it('el recorrido entero cabe en una sola secuencia de teclas', () => {
    // La misma historia sin cortes, que es como la vive quien no usa ratón.
    // `escribir` es lo que hace el campo de texto al teclear, no una acción.
    const inicio = panelInicial(ENTRADAS);
    const hastaEditar = pulsar(inicio, abajo, entrar, editar);
    const conTexto = escribir(hastaEditar.estado, 'Nota nueva');
    const final = pulsar(conTexto, guardar, suprimir, entrar);

    expect(final.efectos.map((e) => e.tipo)).toEqual(['editar', 'borrar']);
    expect(hastaEditar.efectos.map((e) => e.tipo)).toEqual(['saltar']);
  });
});

describe('recorrer la lista', () => {
  it('empieza en la primera anotación, no en ninguna', () => {
    // Un panel que abre sin fila activa obliga a pulsar una tecla antes de
    // poder hacer nada, y esa tecla no la anuncia nadie.
    expect(panelInicial(ENTRADAS).cursor).toBe(0);
  });

  it('un documento sin anotaciones no tiene fila activa', () => {
    const vacio = panelInicial([]);
    expect(vacio.cursor).toBe(-1);
    expect(entradaActiva(vacio)).toBeNull();
  });

  it('el cursor no da la vuelta por ninguno de los dos extremos', () => {
    const panel = panelInicial(ENTRADAS);
    expect(pulsar(panel, arriba, arriba).estado.cursor).toBe(0);
    expect(pulsar(panel, abajo, abajo, abajo, abajo, abajo).estado.cursor).toBe(
      ENTRADAS.length - 1
    );
  });

  it('Inicio y Fin llevan a los extremos de un salto', () => {
    const panel = panelInicial(ENTRADAS);
    expect(pulsar(panel, { key: 'End' }).estado.cursor).toBe(ENTRADAS.length - 1);
    expect(pulsar(panel, { key: 'End' }, { key: 'Home' }).estado.cursor).toBe(0);
  });

  it('responde igual con Bloq Mayús puesto', () => {
    // Con Bloq Mayús, `e` llega como `E`. Sin esto el panel dejaría de editar
    // sin ningún motivo visible.
    expect(teclaDelPanel({ key: 'E' }, 'lista')).toBe('editar');
  });

  it('acepta las dos teclas de borrado, porque no todo teclado trae Supr', () => {
    expect(teclaDelPanel({ key: 'Delete' }, 'lista')).toBe('borrar');
    expect(teclaDelPanel({ key: 'Backspace' }, 'lista')).toBe('borrar');
  });
});

describe('lo que no se puede hacer no se ofrece', () => {
  it('una anotación perdida no salta, porque el salto fallaría', () => {
    expect(sePuedeSaltar(PERDIDA)).toBe(false);

    const panel = { ...panelInicial(ENTRADAS), cursor: 3 };
    expect(pulsar(panel, entrar).efectos).toEqual([]);
  });

  it('una anotación perdida sí se puede borrar: sigue siendo del usuario', () => {
    const panel = { ...panelInicial(ENTRADAS), cursor: 3 };
    expect(pulsar(panel, suprimir, entrar).efectos).toEqual([{ tipo: 'borrar', id: 'a4' }]);
  });

  it('un marcador no se edita: darle texto lo convertiría en otra cosa', () => {
    expect(sePuedeEditar(MARCADOR)).toBe(false);

    const panel = { ...panelInicial(ENTRADAS), cursor: 2 };
    expect(pulsar(panel, editar).estado.modo).toBe('lista');
  });

  it('sobre una lista vacía ninguna tecla hace nada en lugar de fallar', () => {
    const vacio = panelInicial([]);
    const resultado = pulsar(vacio, abajo, entrar, editar, suprimir, entrar);
    expect(resultado.efectos).toEqual([]);
    expect(resultado.estado.modo).toBe('lista');
  });
});

describe('borrar pide confirmación', () => {
  it('una sola tecla no borra nada', () => {
    // Quien recorre la lista con las flechas no puede perder una anotación por
    // una pulsación de más.
    const panel = panelInicial(ENTRADAS);
    expect(pulsar(panel, suprimir).efectos).toEqual([]);
  });

  it('Escape cancela la confirmación sin borrar', () => {
    const panel = panelInicial(ENTRADAS);
    const resultado = pulsar(panel, suprimir, escapar);
    expect(resultado.estado.modo).toBe('lista');
    expect(resultado.efectos).toEqual([]);
  });

  it('confirmar exige Enter sin Control: el de guardar no vale aquí', () => {
    const panel = panelInicial(ENTRADAS);
    expect(pulsar(panel, suprimir, guardar).efectos).toEqual([]);
  });
});

describe('editar respeta que una anotación es inmutable', () => {
  it('la edición se abre con el cuerpo que ya tenía', () => {
    const panel = panelInicial(ENTRADAS);
    const abierta = pulsar(panel, editar);
    expect(abierta.estado.borrador).toBe('Lo importante es el hielo');
  });

  it('guardar lo mismo que había no crea una sucesora idéntica', () => {
    // Editar da de baja y crea otra. Guardar sin cambios dejaría la anterior en
    // el historial a cambio de nada.
    const panel = panelInicial(ENTRADAS);
    const abierta = pulsar(panel, editar);
    expect(pulsar(abierta.estado, guardar).efectos).toEqual([]);
  });

  it('vaciar una nota no la guarda vacía: una nota sin texto no es una nota', () => {
    const panel = panelInicial(ENTRADAS);
    const abierta = pulsar(panel, editar);
    const vaciada = escribir(abierta.estado, '   ');
    const resultado = pulsar(vaciada, guardar);
    expect(resultado.efectos).toEqual([]);
    expect(resultado.estado.modo).toBe('lista');
  });

  it('vaciar el comentario de un resaltado sí se guarda: sigue siendo un resaltado', () => {
    const conComentario = [entrada('a9', 'highlight', 'Sobra este comentario', 'Cita', 0.2)];
    const panel = panelInicial(conComentario);
    const abierta = pulsar(panel, editar);
    const vaciada = escribir(abierta.estado, '');
    expect(pulsar(vaciada, guardar).efectos).toEqual([{ tipo: 'editar', id: 'a9', cuerpo: null }]);
  });

  it('mientras se escribe, las letras son letras', () => {
    // Sin esto, escribir "eres" abriría cuatro ediciones y borraría algo por el
    // camino. Es la misma regla que `comandoParaTecla` aplica en el lector.
    expect(teclaDelPanel({ key: 'e' }, 'editando')).toBeNull();
    expect(teclaDelPanel({ key: 'Delete' }, 'editando')).toBeNull();
    expect(teclaDelPanel({ key: 'ArrowDown' }, 'editando')).toBeNull();
    expect(teclaDelPanel({ key: 'Enter' }, 'editando')).toBeNull();
  });

  it('de un campo se sale siempre, con guardar o con Escape', () => {
    expect(teclaDelPanel(guardar, 'editando')).toBe('guardar');
    expect(teclaDelPanel(escapar, 'editando')).toBe('cancelar');
  });

  it('Escape descarta lo escrito sin guardarlo', () => {
    const panel = panelInicial(ENTRADAS);
    const abierta = pulsar(panel, editar);
    const escrito = escribir(abierta.estado, 'Esto no se guarda');
    const resultado = pulsar(escrito, escapar);
    expect(resultado.efectos).toEqual([]);
    expect(resultado.estado.borrador).toBe('');
  });

  it('acepta Command donde Ctrl, porque en macOS los atajos son con Command', () => {
    expect(teclaDelPanel({ key: 'Enter', metaKey: true }, 'editando')).toBe('guardar');
  });
});

describe('recargar la lista no pierde el sitio', () => {
  it('conserva el cursor donde estaba', () => {
    const panel = { ...panelInicial(ENTRADAS), cursor: 2 };
    expect(conEntradas(panel, ENTRADAS).cursor).toBe(2);
  });

  it('acota el cursor cuando la lista encoge', () => {
    const panel = { ...panelInicial(ENTRADAS), cursor: 3 };
    expect(conEntradas(panel, ENTRADAS.slice(0, 2)).cursor).toBe(1);
  });

  it('deja la lista vacía sin fila activa', () => {
    const panel = { ...panelInicial(ENTRADAS), cursor: 2 };
    expect(conEntradas(panel, []).cursor).toBe(-1);
  });

  it('sale del modo de edición: la fila puede ya no ser la misma', () => {
    // Seguir confirmando un borrado sobre una lista que cambió bajo los pies es
    // cómo se borra lo que no se quería borrar.
    const panel = panelInicial(ENTRADAS);
    const confirmando = pulsar(panel, suprimir).estado;
    expect(conEntradas(confirmando, ENTRADAS).modo).toBe('lista');
  });
});

describe('el panel convive con el lector sin robarle teclas', () => {
  it('deja pasar Escape en la lista, para que el lector cierre el panel', () => {
    // La escalera de Escape es del lector: sale de un escalón cada vez. Si el
    // panel se lo quedara, el panel no se podría cerrar con el teclado.
    expect(teclaDelPanel(escapar, 'lista')).toBeNull();
  });

  it('deja pasar los atajos con Control, que son del lector', () => {
    expect(teclaDelPanel({ key: 'f', ctrlKey: true }, 'lista')).toBeNull();
    expect(teclaDelPanel({ key: 'Tab', ctrlKey: true }, 'lista')).toBeNull();
  });

  it('deja pasar las teclas de los otros paneles', () => {
    for (const tecla of ['i', 't', 'm', 'z', 'b', 'r', 'd']) {
      expect(teclaDelPanel({ key: tecla }, 'lista')).toBeNull();
    }
  });

  it('se queda las que significan otra cosa en el texto', () => {
    // Estas son las que el componente detiene. Sin detenerlas, la flecha abajo
    // movería la fila del panel y además el párrafo del texto.
    expect(teclaDelPanel(abajo, 'lista')).toBe('siguiente');
    expect(teclaDelPanel(arriba, 'lista')).toBe('anterior');
    expect(teclaDelPanel(entrar, 'lista')).toBe('saltar');
  });
});

describe('el panel está en el registro de comandos', () => {
  const comando = COMANDOS.find((c) => c.id === 'anotaciones');

  it('existe, con la tecla que tiene reservada', () => {
    expect(comando?.atajo).toEqual({ tecla: 'a' });
    expect(comando?.panel).toBe('anotaciones');
  });

  it('no depende de la selección de texto: un cómic también tiene marcadores', () => {
    const comic = comandosDisponibles({
      textSelection: false,
      internalSearch: false,
      reflow: false,
      typography: false,
      synchronizedAudio: false,
      textExtraction: false
    });
    expect(comic.map((c) => c.id)).toContain('anotaciones');
  });
});

/**
 * El otro medio entregable: **orden de tabulación predecible**.
 *
 * No se puede comprobar sobre la máquina pura, porque la tabulación la decide el
 * marcado. Se renderiza con `svelte/server`, que da el HTML sin navegador. Lo
 * que no cubre son los efectos y los eventos, que no corren en ese modo.
 */
describe('el orden de tabulación del panel', () => {
  function pintar(entradas: OutlineEntry[]): string {
    return render(AnnotationsPanel, {
      props: {
        entradas,
        alSaltar: () => {},
        alEditar: () => {},
        alBorrar: () => {}
      }
    }).body;
  }

  it('la lista entera es una sola parada de tabulación', () => {
    // Con tres acciones por fila y cuatro filas serían doce paradas. Con
    // tabulación móvil son las de una sola fila, y las flechas hacen el resto.
    const html = pintar(ENTRADAS);
    const alcanzables = html.match(/tabindex="0"/g) ?? [];
    const apartados = html.match(/tabindex="-1"/g) ?? [];

    expect(alcanzables).toHaveLength(3);
    expect(apartados.length).toBeGreaterThan(0);
  });

  it('no hay ningún tabindex positivo, que reordenaría el recorrido', () => {
    expect(pintar(ENTRADAS)).not.toMatch(/tabindex="[1-9]/);
  });

  it('una anotación perdida no ofrece el botón de ir al texto', () => {
    const soloPerdida = pintar([PERDIDA]);
    expect(soloPerdida).not.toContain('Ir al Texto');
    expect(soloPerdida).toContain('Borrar');
    expect(soloPerdida).toContain('No se pudo colocar');
  });

  it('un marcador no ofrece el botón de editar', () => {
    const soloMarcador = pintar([MARCADOR]);
    expect(soloMarcador).not.toContain('aria-label="Editar"');
    expect(soloMarcador).toContain('aria-label="Borrar"');
  });

  it('todo botón tiene nombre accesible, porque van sin texto visible', () => {
    const html = pintar(ENTRADAS);
    const botones = html.match(/<button[^>]*>/g) ?? [];
    expect(botones.length).toBeGreaterThan(0);
    for (const boton of botones) {
      expect(boton).toMatch(/aria-label="[^"]+"/);
    }
  });

  it('un documento sin anotaciones lo dice, en lugar de enseñar una lista vacía', () => {
    const html = pintar([]);
    expect(html).toContain('Todavía no has anotado nada');
    expect(html).not.toContain('<ul');
  });
});

describe('señalar una marca en la página lleva a su anotación', () => {
  it('pone el cursor sobre la que se señaló, y no sobre la primera', () => {
    // Sin esto, pulsar un resaltado contestaba "está entre estas cuarenta".
    const estado = panelInicial([hito('a'), hito('b'), hito('c')]);
    expect(conCursorEn(estado, 'c').cursor).toBe(2);
  });

  it('una anotación que ya no está en la lista no mueve el cursor', () => {
    // Es la que se acaba de borrar: saltar a otra fila sería peor que quedarse.
    const estado = { ...panelInicial([hito('a'), hito('b')]), cursor: 1 };
    expect(conCursorEn(estado, 'z').cursor).toBe(1);
  });

  it('señalar saca del modo de edición: se está mirando otra cosa', () => {
    const estado = { ...panelInicial([hito('a'), hito('b')]), modo: 'editando' as const };
    expect(conCursorEn(estado, 'b').modo).toBe('lista');
  });
});

/** Una entrada mínima, que aquí solo importa por su identificador. */
function hito(id: string): OutlineEntry {
  return entrada(id, 'highlight', null, `cita de ${id}`, 0.1);
}
