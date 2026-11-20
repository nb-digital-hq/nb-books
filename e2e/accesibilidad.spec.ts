/**
 * **El criterio de aceptación:** cero violaciones serias o críticas en
 * las vistas principales, y su aparición bloquea la fusión.
 *
 * # Por qué axe y no reglas propias
 *
 * Porque "serias o críticas" no es una forma de hablar: son dos de las cuatro
 * severidades que axe asigna, y la ficha se escribió con ese vocabulario. Unas
 * comprobaciones caseras cubrirían un puñado de reglas y no podrían clasificar
 * nada, así que el criterio quedaría cumplido de nombre.
 *
 * `@axe-core/playwright` es dependencia **de desarrollo**: no entra en el
 * binario. `dependency-policy.md`, sección 4, exime a las herramientas que no se
 * distribuyen, y MPL-2.0 está además en la lista de permitidas.
 *
 * # Qué no hace esto
 *
 * Una fracción de los problemas reales. Encuentra un botón sin nombre; no
 * encuentra que ese nombre sea inútil ni que el orden en que se anuncian tres
 * cosas sea incomprensible. Por eso existe además
 * `docs/developer/testing/screen-readers.md`, que es manual y no se puede
 * saltar. Declarar conformidad apoyándose solo en un analizador sería
 * exactamente lo que `accessibility.md`, sección 6, prohíbe.
 *
 * # Por qué se listan las violaciones y no solo se cuentan
 *
 * Un fallo que dice "hay 3" obliga a reproducirlo a mano para saber qué son.
 * Aquí el mensaje trae la regla, la severidad y el selector del nodo, que es lo
 * que hace falta para arreglarlo sin volver a ejecutar nada.
 */

import AxeBuilder from '@axe-core/playwright';
import { expect, type Page, test } from '@playwright/test';

import { montarPuente, vistaDeEstudio } from './puente';

/**
 * Las severidades que bloquean.
 *
 * `minor` y `moderate` no bloquean **todavía**, y no es una rebaja del criterio:
 * es el criterio, que dice "serias o críticas". Subir el listón es una decisión
 * de plan, no algo que se cuele en un commit de otra cosa.
 */
const BLOQUEAN = new Set(['serious', 'critical']);

/**
 * Espera a que el lector atienda al teclado, no solo a que se vea su título.
 *
 * El título "Capítulo 1" lo pinta la carcasa en cuanto hay sesión, y el lector se
 * monta un instante después: una tecla pulsada entre medias no llega a nadie. Las
 * pruebas de paneles fallaban así una de cada tres veces en algunas máquinas. El
 * texto del capítulo lo pinta la superficie del lector, así que con él a la vista
 * el lector ya escucha.
 */
async function lectorListo(page: Page) {
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();
  await expect(page.getByText('Muchos años después')).toBeVisible();
}

async function auditar(page: Page, dentroDe?: string) {
  const analizador = new AxeBuilder({ page });
  const analisis = await (dentroDe ? analizador.include(dentroDe) : analizador).analyze();

  const graves = analisis.violations.filter((v) => BLOQUEAN.has(v.impact ?? ''));

  // El mensaje lleva regla, severidad y nodo: sin eso, un fallo obliga a
  // reproducirlo a mano para saber qué hay que arreglar.
  const detalle = graves
    .map(
      (v) =>
        `[${v.impact}] ${v.id}: ${v.help}\n` +
        v.nodes.map((n) => `      ${n.target.join(' ')}`).join('\n')
    )
    .join('\n');

  expect(graves, `violaciones serias o críticas:\n${detalle}`).toEqual([]);
}

test.beforeEach(async ({ page }) => {
  await montarPuente(page);
});

test('Inicio no tiene violaciones serias ni críticas', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Inicio' })).toBeVisible();
  await auditar(page);
});

test('Biblioteca no tiene violaciones serias ni críticas', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Biblioteca' })).toBeVisible();
  await auditar(page);
});

test('Estudio no tiene violaciones serias ni críticas, en sus vistas y en un tablero', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Estudio', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Estudio' })).toBeVisible();
  await auditar(page);

  await vistaDeEstudio(page, 'Anotaciones').click();
  await auditar(page);

  // Y el tablero, que son columnas con tarjetas que se arrastran.
  await vistaDeEstudio(page, /Tableros/).click();
  await page
    .getByRole('group', { name: 'Tableros' })
    .getByRole('button', { name: /Capítulo tres/ })
    .click();
  await auditar(page);
});

test('Ajustes no tiene violaciones serias ni críticas', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Ajustes' })).toBeVisible();
  await auditar(page);
});

test('el Lector no tiene violaciones serias ni críticas', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await lectorListo(page);
  await auditar(page);
});

/*
 * Los paneles se auditan uno a uno y no todos a la vez: son zonas que aparecen y
 * desaparecen, y una violación dentro de un panel cerrado no la ve nadie. Se
 * abren con su tecla, que además comprueba de paso que la tecla sigue ahí.
 */
for (const [tecla, panel] of [
  ['i', 'Índice'],
  ['a', 'Anotaciones'],
  ['v', 'Lectura en Voz Alta'],
  ['t', 'Ajustes del Libro'],
  ['m', 'Modo de Lectura']
] as const) {
  test(`el panel ${panel} del Lector no tiene violaciones serias ni críticas`, async ({ page }) => {
    await page.goto('/');
    await page.getByRole('button', { name: 'Continuar' }).click();
    await lectorListo(page);

    await page.keyboard.press(tecla);
    await expect(page.getByRole('complementary', { name: panel })).toBeVisible();
    await auditar(page);
  });
}

test('el panel de Consulta del Lector no tiene violaciones serias ni críticas', async ({
  page
}) => {
  // Fuera del bucle de arriba porque su tecla necesita una selección: consultar
  // sin nada seleccionado no abre nada, por la misma regla que anotar.
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await lectorListo(page);

  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('c');
  await expect(page.getByRole('complementary', { name: 'Consulta' })).toBeVisible();
  await auditar(page);
});

test('Descubrir no tiene violaciones serias ni críticas', async ({ page }) => {
  // Con su propio puente: sin catálogos el destino no existe, así que auditarlo
  // sin uno sería auditar una pantalla que nadie ve.
  await montarPuente(page, { conCatalogo: true });
  await page.goto('/');
  await page.getByRole('button', { name: 'Descubrir', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Catálogos' })).toBeVisible();
  await auditar(page);
});

test('los paneles de consulta de la carcasa no tienen violaciones serias ni críticas', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('searchbox', { name: 'Buscar' }).click();
  await expect(page.getByRole('complementary', { name: 'Búsqueda' })).toBeVisible();
  await auditar(page);

  await page.getByRole('button', { name: 'Importar Documentos' }).click();
  await expect(page.getByRole('complementary', { name: 'Importar' })).toBeVisible();
  await auditar(page);
});

/*
 * El alto contraste y la escala tipográfica no se auditan por completismo: son
 * los dos ajustes que **cambian el contraste y el tamaño**, que es justo lo que
 * axe mide. Un tema que pase en claro y falle en alto contraste solo lo sufriría
 * quien depende del alto contraste, y por eso no se puede auditar uno solo.
 */
test('con alto contraste tampoco hay violaciones serias ni críticas', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'Accesibilidad', exact: true }).click();
  await page.getByRole('switch', { name: 'Alto contraste' }).click();
  await auditar(page);

  // Y con el alto contraste puesto, el Lector: es donde el contraste importa
  // más rato seguido, y donde un tema mal medido se nota en cada párrafo.
  await page.getByRole('button', { name: 'Inicio', exact: true }).click();
  await page.getByRole('button', { name: 'Continuar' }).click();
  await lectorListo(page);
  await auditar(page);
});

test('en lectura inmersiva, lo poco que queda tampoco tiene violaciones', async ({ page }) => {
  // La inmersión retira la carcasa entera. Es el estado con menos puntos de
  // referencia de toda la aplicación, y el que más fácil se queda sin ninguno.
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await lectorListo(page);
  await page.keyboard.press('z');
  await expect(page.getByText('Lectura inmersiva')).toBeVisible();
  await auditar(page);
});
