/**
 * Prueba de humo del marcado de todas las vistas.
 *
 * No comprueba que estén bien diseñadas: comprueba que **existen y se pintan**,
 * incluidas sus dos situaciones límite —sin datos y con datos—, que es donde una
 * vista se rompe sin que nadie lo note hasta abrirla.
 *
 * Es barata y encuentra la clase de defecto que más cuesta encontrar de otro
 * modo: un dato que llega nulo y revienta el render entero. La auditoría con
 * motor de accesibilidad sobre estas mismas vistas lo hace `e2e/accesibilidad.spec.ts`.
 */

import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';

import type { BrowseItem, BrowsePage, HomeRow, Lectura, Note, Setting } from '$lib/bindings';
import HomeView from './HomeView.svelte';
import ImportPanel from './ImportPanel.svelte';
import SettingsView from './SettingsView.svelte';
import StudyView from './StudyView.svelte';

const VACIA: BrowsePage = { total: 0, items: [] };

const PAGINA: BrowsePage = {
  total: 2,
  items: [
    {
      editionId: 'e1',
      title: 'Cien Años de Soledad',
      authors: 'Gabriel García Márquez',
      seriesTitle: null,
      seriesIndex: null,
      availability: 'available',
      progression: 0.42,
      addedAt: '2026-08-01',
      copies: 2,
      fileId: 'f1',
      coverPath: null,
      mediaType: null,
      byteSize: null
    },
    {
      editionId: 'e2',
      title: 'El Otoño del Patriarca',
      authors: null,
      seriesTitle: null,
      seriesIndex: null,
      availability: 'missing',
      progression: null,
      addedAt: '2026-08-02',
      copies: 1,
      fileId: null,
      coverPath: null,
      mediaType: null,
      byteSize: null
    }
  ]
};

const LECTURA: Lectura = {
  minutesToday: 42,
  minutesWeek: 310,
  streakDays: 5,
  inProgress: 2,
  documents: 120
};

/** Nueve obras del mismo autor; la segunda, sin archivo disponible. */
const DEL_AUTOR: BrowseItem[] = Array.from({ length: 9 }, (_, i) => ({
  editionId: `r${i}`,
  title: i === 0 ? 'La Hojarasca' : `Obra ${i}`,
  authors: 'Gabriel García Márquez',
  seriesTitle: null,
  seriesIndex: null,
  availability: i === 1 ? 'missing' : 'available',
  progression: null,
  addedAt: '2026-08-01',
  copies: 1,
  fileId: i === 1 ? null : `fr${i}`,
  coverPath: null,
  mediaType: null,
  byteSize: null
}));

const FILAS: HomeRow[] = [
  {
    id: 'porque-e1',
    kind: {
      kind: 'porqueLees',
      value: { title: 'Cien Años de Soledad', author: 'Gabriel García Márquez' }
    },
    layout: 'portadas',
    items: DEL_AUTOR
  }
];

const NOTA: Note = {
  id: 'n1',
  title: 'El Tiempo Circular',
  body: 'Aquí empieza.',
  createdAt: '2026-08-01',
  updatedAt: '2026-08-02'
};

const PREFERENCIAS: Setting[] = [
  { key: 'theme', value: 'dark', affectsFirstPaint: true, plane: 'application' },
  { key: 'high_contrast', value: false, affectsFirstPaint: true, plane: 'application' },
  { key: 'font_scale', value: 1.2, affectsFirstPaint: true, plane: 'application' },
  { key: 'language', value: 'es', affectsFirstPaint: true, plane: 'application' },
  { key: 'offline_mode', value: false, affectsFirstPaint: false, plane: 'application' },
  { key: 'book_font_size', value: 20, affectsFirstPaint: false, plane: 'book' }
];

describe('Inicio', () => {
  it('con la biblioteca vacía produce una pantalla útil, no una rota', () => {
    const html = render(HomeView, {
      props: { enCurso: VACIA, novedades: VACIA, lectura: null }
    }).body;

    expect(html).toContain('Importar Documentos');
    // Y no enseña secciones vacías: una lista de cero elementos no es contenido.
    expect(html).not.toContain('Sigue Leyendo');
  });

  it('con datos muestra el progreso, las cifras y el porqué de cada fila', () => {
    const html = render(HomeView, {
      props: {
        enCurso: PAGINA,
        novedades: PAGINA,
        filas: FILAS,
        lectura: LECTURA
      }
    }).body;

    expect(html).toContain('Sigue Leyendo');
    expect(html).toContain('Cien Años de Soledad');
    expect(html).toContain('42');
    // La portada se dibuja aunque la edición no traiga imagen: un hueco gris no
    // es una portada, es una ausencia, y con 152.080 documentos serían casi
    // todos.
    expect(html).toContain('data-generada="true"');
    // El porqué va siempre: una fila sin porqué no se pinta.
    expect(html).toContain('Más de Gabriel García Márquez');
    expect(html).toContain('Porque estás leyendo Cien Años de Soledad');
  });

  it('una obra de una fila sin archivo disponible no ofrece abrirse', () => {
    // Se pinta igual, porque lo que dice sigue siendo cierto. Lo que no hace es
    // ofrecer una acción que no puede cumplir: antes todas eran
    // botones y ninguna abría nada.
    const html = render(HomeView, {
      props: {
        enCurso: PAGINA,
        novedades: PAGINA,
        filas: FILAS,
        lectura: LECTURA
      }
    }).body;

    /*
     * Se cuentan por `nb-obra-de-fila`, que es una clase de esta vista, y no por la
     * que le ponga el sistema de diseño: la marquesina usa otros botones, y contar
     * por la forma dejaría de poder exigir cuántas obras tiene la fila.
     */
    const obras = html.match(/<button[^>]*class="[^"]*nb-obra-de-fila[^"]*"[^>]*>/g) ?? [];
    expect(obras.length).toBeGreaterThan(0);
    expect(obras.filter((t) => t.includes('disabled'))).toHaveLength(1);
  });

  it('con las sugerencias apagadas no pinta ninguna fila que sugiera', () => {
    // ADR-013 promete que se pueden desactivar enteras. Antes la propiedad
    // existía y nadie la pasaba, así que la promesa no se cumplía.
    const html = render(HomeView, {
      props: { enCurso: PAGINA, novedades: PAGINA, filas: FILAS, sugerir: false, lectura: LECTURA }
    }).body;

    expect(html).not.toContain('Más de Gabriel García Márquez');
    expect(html).toContain('Novedades');
  });

  it('sin haber empezado nada lo dice en lugar de dejar un hueco', () => {
    const html = render(HomeView, {
      props: { enCurso: VACIA, novedades: PAGINA, lectura: LECTURA }
    }).body;

    // Sin nada empezado no hay pieza de retomar, y la pantalla sigue teniendo
    // sentido. Y sin filas —una biblioteca con menos obras de las que llenan una—
    // lo que hay se enseña entero como novedades en lugar de dejar la pantalla
    // vacía.
    expect(html).not.toContain('Sigue Leyendo');
    expect(html).toContain('Novedades');
  });
});

describe('Estudio', () => {
  it('sin notas invita a escribir la primera', () => {
    const html = render(StudyView, {
      props: {
        notas: [],
        retroenlaces: [],
        abierta: null,
        alAbrir: () => {},
        alCrear: () => {},
        alGuardar: () => {},
        alBorrar: () => {}
      }
    }).body;

    expect(html).toContain('Nota Nueva');
  });

  it('con una nota abierta muestra su contenido y sus retroenlaces', () => {
    const html = render(StudyView, {
      props: {
        notas: [NOTA],
        retroenlaces: [{ ...NOTA, id: 'n2', title: 'Macondo' }],
        abierta: NOTA,
        alAbrir: () => {},
        alCrear: () => {},
        alGuardar: () => {},
        alBorrar: () => {}
      }
    }).body;

    expect(html).toContain('El Tiempo Circular');
    expect(html).toContain('Apuntan Aquí');
    expect(html).toContain('Macondo');
  });
});

describe('Ajustes', () => {
  it('muestra el plano de la aplicación y ninguna preferencia del libro', () => {
    const html = render(SettingsView, {
      props: { preferencias: PREFERENCIAS, alCambiar: () => {} }
    }).body;

    expect(html).toContain('Apariencia');
    expect(html).toContain('Accesibilidad');
    // El plano del libro se ajusta leyendo, que es donde se ve el efecto.
    expect(html).not.toContain('book_font_size');
    expect(html).not.toContain('Cuerpo');
  });

  it('agrupa los permisos de red por propósito con los anfitriones dentro', () => {
    const html = render(SettingsView, {
      props: {
        seccion: 'red',
        preferencias: PREFERENCIAS,
        alCambiar: () => {},
        red: {
          offline: false,
          consents: [
            {
              origin: '',
              host: 'es.wikipedia.org',
              purpose: 'lookup',
              grantedAt: '2026-10-16T12:00:00Z'
            },
            {
              origin: '',
              host: 'en.wiktionary.org',
              purpose: 'lookup',
              grantedAt: '2026-10-16T12:00:00Z'
            },
            {
              origin: '',
              host: 'standardebooks.org',
              purpose: 'catalog',
              grantedAt: '2026-10-16T12:00:00Z'
            },
            {
              origin: '',
              host: '192.168.1.50',
              purpose: 'speech',
              grantedAt: '2026-10-16T12:00:00Z'
            }
          ],
          pending: [],
          log: []
        }
      }
    }).body;

    // Secciones por propósito presentes
    expect(html).toContain('Catálogo');
    expect(html).toContain('Consulta');
    expect(html).toContain('Voz Remota');

    // Anfitriones dentro de sus listas
    expect(html).toContain('standardebooks.org');
    expect(html).toContain('es.wikipedia.org');
    expect(html).toContain('en.wiktionary.org');
    expect(html).toContain('192.168.1.50');

    // Botones individuales de revocación presentes
    expect(html).toContain('Revocar');

    // Estructura agrupada
    expect(html).toContain('nb-grupo-permisos');
    expect(html).toContain('nb-permiso-proposito');
  });

  it('enseña mensaje claro cuando no hay permisos concedidos', () => {
    const html = render(SettingsView, {
      props: {
        seccion: 'red',
        preferencias: PREFERENCIAS,
        alCambiar: () => {},
        red: {
          offline: false,
          consents: [],
          pending: [],
          log: []
        }
      }
    }).body;

    expect(html).toContain('No has autorizado ningún destino.');
    expect(html).not.toContain('nb-grupo-permisos');
  });

  it('ofrece borrar el historial, las anotaciones o la biblioteca diciendo qué conserva y que el disco no se toca', () => {
    const html = render(SettingsView, {
      props: {
        seccion: 'biblioteca',
        preferencias: PREFERENCIAS,
        alCambiar: () => {}
      }
    }).body;

    expect(html).toContain('Borrar lo Tuyo de la Aplicación');
    expect(html).toContain('Tus archivos en el disco no se borran nunca.');
    expect(html).toContain('Borrar el Historial de Lectura');
    expect(html).toContain('Borrar las Anotaciones y las Notas');
    expect(html).toContain('Vaciar la Biblioteca en la Aplicación');
  });

  it('enseña LICENSE, NOTICE y THIRD-PARTY-NOTICES.md en Acerca de', () => {
    const html = render(SettingsView, {
      props: {
        seccion: 'acerca',
        preferencias: PREFERENCIAS,
        alCambiar: () => {},
        avisosLegales: {
          license: 'Apache License\nVersion 2.0, January 2004',
          notice: 'NB Books\nCopyright 2026 The NB Books Authors',
          thirdParty: '# Avisos de Terceros\n\n| `rusqlite` | 0.37.0 | MIT |'
        }
      }
    }).body;

    expect(html).toContain('Licencia y Avisos de Terceros');
    expect(html).toContain('Aviso de atribución (NOTICE)');
    expect(html).toContain('Copyright 2026 The NB Books Authors');
    expect(html).toContain('Licencia de NB Books (LICENSE)');
    expect(html).toContain('Apache License');
    expect(html).toContain('Componentes de terceros (THIRD-PARTY-NOTICES.md)');
    expect(html).toContain('rusqlite');
  });
});

describe('Importar', () => {
  it('sin diálogo del sistema, la ruta se escribe y se puede soltar', () => {
    const html = render(ImportPanel, {
      props: {
        alImportar: async () => ({
          processed: 0,
          imported: 0,
          alreadyInLibrary: 0,
          duplicates: 0,
          rejected: [],
          cancelled: false,
          running: false
        })
      }
    }).body;

    // La vía de teclado no es un apaño: una acción exclusiva del ratón está
    // prohibida, así que escribir la ruta es tan de primera clase como soltarla.
    expect(html).toContain('Ruta');
    expect(html).toContain('soltar archivos');
    // Sin quien lo abra, no se ofrece un botón que no haría nada.
    expect(html).not.toContain('Elegir Carpeta');
  });

  it('ofrece el diálogo del sistema cuando hay quien lo abra', () => {
    const html = render(ImportPanel, {
      props: {
        alImportar: async () => ({
          processed: 0,
          imported: 0,
          alreadyInLibrary: 0,
          duplicates: 0,
          rejected: [],
          cancelled: false,
          running: false
        }),
        alElegir: async () => null
      }
    }).body;

    expect(html).toContain('Elegir Carpeta');
    // Y la vía de teclado sigue estando: no es un apaño, es la que hace que la
    // importación no dependa del ratón.
    expect(html).toContain('Ruta');
  });

  /*
   * Las copias exactas se dicen aparte de lo que ya estaba, con de qué es
   * copia cada una; y una importación sin copias no pinta un cero de más.
   */
  it('dice las copias exactas de la última importación y de qué es copia cada una', () => {
    const ejecucion = {
      id: 'e1',
      root: '/libros',
      storageMode: 'referenced',
      startedAt: '2026-09-24T00:00:00Z',
      finishedAt: '2026-09-24T00:01:00Z',
      processed: 6,
      imported: 3,
      alreadyInLibrary: 0,
      rejected: 0,
      duplicates: 3,
      cancelled: false
    };
    const html = render(ImportPanel, {
      props: {
        alImportar: async () => {
          throw new Error('no se importa');
        },
        guardado: {
          ejecucion,
          rechazos: [],
          copias: [
            {
              path: 'descargas/dos.pdf',
              originalTitle: 'Dos',
              originalPath: 'autor-b/dos.pdf',
              createdAt: '2026-09-24T00:00:30Z'
            }
          ]
        }
      }
    }).body;
    expect(html).toMatch(/: 3\s+copias exactas/);
    expect(html).toContain('se listan los primeros 1');
    expect(html).toContain('descargas/dos.pdf');
    expect(html).toContain('copia de «Dos»');

    const sin = render(ImportPanel, {
      props: {
        alImportar: async () => {
          throw new Error('no se importa');
        },
        avance: { processed: 5, imported: 5, skipped: 0, rejected: 0, duplicates: 0, total: 9 },
        guardado: { ejecucion: { ...ejecucion, duplicates: 0 }, rechazos: [], copias: [] }
      }
    }).body;
    expect(sin).not.toContain('copias exactas');
  });

  it('mientras importa muestra cifras y deja seguir usando la aplicación', () => {
    const html = render(ImportPanel, {
      props: {
        alImportar: async () => ({
          processed: 0,
          imported: 0,
          alreadyInLibrary: 0,
          duplicates: 0,
          rejected: [],
          cancelled: false,
          running: true
        }),
        avance: { processed: 1200, imported: 1150, skipped: 40, rejected: 10, total: 4000 }
      }
    }).body;

    // Barra **y** cifras. La barra se puede pintar porque el runtime recorre la
    // carpeta antes de importar y manda el total desde el primer aviso: un
    // contador que sube sin decir hasta dónde no responde a la única pregunta
    // que importa cuando entran ciento cincuenta mil documentos.
    expect(html).toContain('1200');
    expect(html).toContain('4000');
    expect(html).toContain('role="progressbar"');
    expect(html).toContain('aria-valuemax="4000"');
    expect(html).toContain('aria-valuenow="1200"');
    // 30 %, y no una barra al azar: la anchura sale de la proporción real.
    expect(html).toContain('30%');
    expect(html).toContain('Puedes seguir usando la aplicación');
    expect(html).toContain('role="status"');
  });
});
