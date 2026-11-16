/**
 * **El criterio de aceptación:** el flujo de abrir, leer y anotar se
 * recorre entero sobre la interfaz real, con eventos de teclado de verdad.
 *
 * Es la mitad que las pruebas declaraban como no cubierta: una máquina
 * de estado pura demuestra que la lógica es correcta, no que el navegador
 * entregue las teclas al componente ni que el foco vaya donde se cree.
 *
 * Ninguna prueba de este archivo usa el ratón salvo donde lo dice su nombre.
 */

import { readFileSync } from 'node:fs';

import { expect, type Page, test } from '@playwright/test';

import { argumentosDe, comandosPedidos, montarPuente, vistaDeEstudio } from './puente';

test.beforeEach(async ({ page }) => {
  await montarPuente(page);

  // Un error de consola es un fallo, no un aviso: la carcasa tiene que montar
  // limpia. Es el punto 3 del contrato de regresión.
  const errores: string[] = [];
  page.on('console', async (mensaje) => {
    if (mensaje.type() !== 'error') return;
    // El texto de un error con objetos dentro es "Object": se piden los valores
    // para que el fallo diga qué pasó y no solo que pasó algo.
    const partes = await Promise.all(
      mensaje.args().map((a) => a.jsonValue().catch(() => mensaje.text()))
    );
    errores.push(partes.map((p) => JSON.stringify(p)).join(' '));
  });
  page.on('pageerror', (error) => errores.push(error.message));
  (page as unknown as { __errores: string[] }).__errores = errores;
});

test.afterEach(async ({ page }) => {
  const errores = (page as unknown as { __errores: string[] }).__errores ?? [];
  expect(errores, 'la consola no debe traer errores').toEqual([]);
});

test('la aplicación arranca en Inicio y pinta lo que se está leyendo', async ({ page }) => {
  await page.goto('/');

  await expect(page.getByRole('heading', { name: 'Inicio' })).toBeVisible();
  await expect(page.getByText('Sigue Leyendo')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Cien Años de Soledad' })).toBeVisible();

  // Las cifras salen de datos que ya existen, sin red.
  await expect(page.getByText('47')).toBeVisible();
  const comandos = await comandosPedidos(page);
  expect(comandos).toContain('reading_stats');
  expect(comandos).toContain('home_rows');
});

test('sin bus de eventos, la aplicación monta igual y sin errores', async ({ page }) => {
  // No poder escuchar el bus es un modo degradado: la interfaz deja de
  // enterarse sola de lo que pasa por detrás, pero sigue respondiendo a lo que
  // el usuario hace. Lo que no puede es dejar una promesa rechazada colgando.
  await montarPuente(page, { rechaza: ['plugin:event|listen'] });
  await page.goto('/');

  await expect(page.getByRole('heading', { name: 'Inicio' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Cien Años de Soledad' })).toBeVisible();
});

test('cada fila de Inicio dice por qué aparece', async ({ page }) => {
  await page.goto('/');

  // Sin el porqué no se pinta: es lo que impide que Inicio sea un escaparate
  // (ADR-013 y su enmienda del 2026-09-12). Va junto al título de su fila.
  await expect(page.getByRole('heading', { name: 'Más de Gabriel García Márquez' })).toBeVisible();
  await expect(page.getByText('Porque estás leyendo Cien Años de Soledad')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Para Hoy, al Azar' })).toBeVisible();
  // La lista numerada es otra forma, no otra hilera: sus diez se ven enteras.
  await expect(page.locator('.nb-lista-obra')).toHaveCount(10);
  // Una obra sin archivo disponible no ofrece abrirse.
  await expect(page.locator('.nb-lista-obra').nth(1)).toBeDisabled();
});

test('volver a Inicio pide otra pantalla, y quedarse no la baraja', async ({ page }) => {
  // Observación de uso del 2026-09-12: "cada vez que entre debe cambiar". Las filas de la
  // visita siguiente se piden al salir, con otra semilla, para que al volver ya
  // estén y la pantalla no se rehaga delante de quien mira.
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Más de Gabriel García Márquez' })).toBeVisible();
  const semillas = async () =>
    ((await argumentosDe(page, 'home_rows')) as Array<{ seed: number }>).map((a) => a.seed);
  expect(await semillas()).toHaveLength(1);

  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Biblioteca' })).toBeVisible();
  await expect.poll(async () => (await semillas()).length).toBe(2);
  const [primera, segunda] = await semillas();
  expect(segunda).not.toBe(primera);

  await page.getByRole('button', { name: 'Inicio', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Más de Gabriel García Márquez' })).toBeVisible();
  // Entrar no vuelve a pedir: las de esta visita ya estaban.
  expect(await semillas()).toHaveLength(2);
});

test('las sugerencias de Inicio se apagan desde Ajustes', async ({ page }) => {
  // La segunda mitigación de ADR-013 contra un Inicio que empuja. Figuraba como
  // sin un interruptor detrás.
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Más de Gabriel García Márquez' })).toBeVisible();

  // En Apariencia, que es la sección con la que abre Ajustes: es lo que Inicio enseña.
  await page.getByRole('button', { name: 'Ajustes' }).click();
  const interruptor = page.getByRole('switch', { name: 'Sugerencias en Inicio' });
  await expect(interruptor).toHaveAttribute('aria-checked', 'true');
  await interruptor.click();
  expect(await argumentosDe(page, 'update_setting')).toContainEqual({
    key: 'home_suggestions',
    value: false
  });

  await page.getByRole('button', { name: 'Inicio', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Inicio' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Más de Gabriel García Márquez' })).toHaveCount(0);
});

test('se llega a la biblioteca solo con el teclado', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Inicio' })).toBeVisible();

  // Tabular hasta el destino y activarlo con Enter, como quien no usa ratón.
  await page.keyboard.press('Tab');
  for (let i = 0; i < 12; i += 1) {
    const nombre = await page.evaluate(
      () =>
        document.activeElement?.getAttribute('aria-label') ??
        document.activeElement?.textContent?.trim()
    );
    if (nombre?.includes('Biblioteca')) break;
    await page.keyboard.press('Tab');
  }

  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Biblioteca' })).toBeVisible();
});

test('una frase seleccionada con el ratón se resalta sola, no el párrafo entero', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // Una selección del navegador sobre una parte del párrafo, y soltar el ratón.
  await page.evaluate(() => {
    const p = [...document.querySelectorAll('p')].find((e) =>
      e.textContent?.startsWith('Macondo era entonces')
    );
    const texto = p?.firstChild;
    if (!p || !texto) throw new Error('sin párrafo');
    const rango = document.createRange();
    rango.setStart(texto, 0);
    rango.setEnd(texto, 'Macondo era entonces'.length);
    const sel = window.getSelection();
    sel?.removeAllRanges();
    sel?.addRange(rango);
    p.dispatchEvent(new PointerEvent('pointerup', { bubbles: true }));
  });

  await expect(page.getByRole('button', { name: /Resaltar/ })).toBeVisible();
  await page.getByRole('button', { name: /Resaltar/ }).click();
  const creadas = (await llamadasA(page, 'locator_for_selection')) as Array<{ quote: string }>;
  expect(creadas.at(-1)?.quote).toBe('Macondo era entonces');
});

test('el flujo de abrir, mover el cursor y anotar se hace sin ratón', async ({ page }) => {
  await page.goto('/');

  // Abrir desde Inicio con el teclado.
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // La cabecera del lector dice el capítulo, no la obra. La obra estuvo hasta el
  // 2026-08-31 en una tira al pie de la barra lateral, que se quitó al rediseñar
  // la barra; lo que sigue siendo vinculante de ADR-013 es que **el destino
  // Lector permanece** al navegar a otro sitio, y eso lo comprueba la prueba de
  // "volver del lector a la biblioteca y regresar".
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // Bajar el cursor de bloque: es la selección que no depende del ratón.
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('[data-nb-bloque="1"]')).toBeFocused();

  // Extender la selección al bloque siguiente.
  await page.keyboard.press('Shift+ArrowDown');
  await expect(page.locator('[data-nb-seleccionado="true"]')).toHaveCount(2);

  // Una tecla abre la nota: no hay que resaltar antes.
  await page.keyboard.press('Enter');
  const campo = page.getByRole('textbox', { name: 'Nota' });
  await expect(campo).toBeFocused();

  await page.keyboard.type('Aquí empieza el tiempo circular.');
  await page.keyboard.press('Control+Enter');

  const comandos = await comandosPedidos(page);
  expect(comandos).toContain('locator_for_selection');
  expect(comandos).toContain('create_annotation');
});

test('mientras se escribe una nota, las letras son letras', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await page.keyboard.type('resaltar y marcar');

  // Si `r` resaltara y `b` marcara mientras se escribe, aquí habría anotaciones.
  const comandos = await comandosPedidos(page);
  expect(comandos).not.toContain('create_bookmark');
  await expect(page.getByRole('textbox', { name: 'Nota' })).toHaveValue('resaltar y marcar');
});

test('la carcasa se retira entera en inmersión y vuelve donde estaba', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  await page.keyboard.press('z');

  // Ni barra superior, ni navegación, ni paneles.
  await expect(page.locator('header')).toHaveCount(0);
  await expect(page.locator('nav')).toHaveCount(0);
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // Y el recordatorio dice por dónde se vuelve. Se comprueba el anuncio entero
  // y no la palabra suelta: "Esc" aparece también dentro de los atajos, y una
  // prueba que pase por coincidir con otra cosa no prueba nada.
  await expect(page.getByText('Lectura inmersiva. Pulsa Escape para volver.')).toBeVisible();

  await page.keyboard.press('Escape');
  await expect(page.locator('nav')).toHaveCount(1);
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
});

test('el Lector no se cierra al ir a la biblioteca', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // Regla 2 de ADR-013: consultar algo en la biblioteca no termina la lectura.
  await page.getByRole('button', { name: 'Biblioteca' }).click();
  await expect(page.getByRole('heading', { name: 'Biblioteca' })).toBeVisible();

  await page.getByRole('button', { name: 'Lector' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // Y no se ha vuelto a abrir el documento: la sesión seguía viva.
  const comandos = await comandosPedidos(page);
  expect(comandos.filter((c) => c === 'open_document')).toHaveLength(1);
});

test('los controles del lector salen de las capacidades del formato', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // Un EPUB los tiene todos; lo que importa es que estén con su atajo escrito,
  // porque es así como se aprenden sin manual.
  await expect(page.getByRole('button', { name: /Ajustes del Libro \(T\)/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /Poner un Marcador \(B\)/ })).toBeVisible();
});

test('el tema se cambia y la interfaz entera cambia con él', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();

  await expect(page.getByRole('heading', { name: 'Ajustes' })).toBeVisible();

  // Se elige un tema **distinto del que ya está puesto**. Comprobar contra el
  // inicial haría que la prueba pasara sin que nada cambiara, que es como
  // estaba escrita hasta que una mutación lo destapó.
  const inicial = await page.locator('html').getAttribute('data-theme');
  expect(inicial).not.toBe('grafito');

  await page.getByRole('radio', { name: 'Grafito' }).click();

  // Lo que se guarda es el nombre del tema, no un color suelto.
  const llamadas = await page.evaluate(
    () =>
      (
        globalThis as unknown as {
          __nbLlamadas: Array<{ comando: string; args: Record<string, unknown> }>;
        }
      ).__nbLlamadas
  );
  const guardado = llamadas.find((l) => l.comando === 'update_setting');
  expect(guardado?.args.key).toBe('theme');
  expect(guardado?.args.value).toBe('grafito');

  // Y se ve **sin recargar nada**. Un ajuste que exige reiniciar es un ajuste
  // roto: parece que funciona y no funciona.
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'grafito');

  // El tema no es una paleta: cambia también el fondo pintado, que es lo que
  // hace que se sienta otro sitio y no el mismo con otros botones.
  const fondo = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  expect(fondo).not.toBe('rgba(0, 0, 0, 0)');

  // La caché del primer pintado queda al día, que es lo que hace que el tema
  // siga puesto en el siguiente arranque y sin destello.
  const cache = await page.evaluate(() => localStorage.getItem('nb-books.first-paint.v1'));
  expect(cache).toContain('grafito');
});

test('los ajustes se aplican al momento, sin reiniciar ni recargar', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();
  await page.getByRole('button', { name: 'Accesibilidad', exact: true }).click();

  // La escala tipográfica es la que más se nota y la que peor envejece si hay
  // que reiniciar: quien la sube es porque no está leyendo bien ahora.
  const antes = await page.evaluate(() =>
    getComputedStyle(document.documentElement).getPropertyValue('--nb-font-scale').trim()
  );

  const escala = page.getByRole('slider', { name: 'Escala de la interfaz' });
  await escala.focus();
  await page.keyboard.press('ArrowRight');

  await expect
    .poll(async () =>
      page.evaluate(() =>
        getComputedStyle(document.documentElement).getPropertyValue('--nb-font-scale').trim()
      )
    )
    .not.toBe(antes);

  // Y el alto contraste, que cambia el esquema entero.
  await page.getByRole('switch', { name: 'Alto contraste' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-contrast', 'high');
});

/*
 * # Las dos reglas de composición que no dependen del gusto
 *
 * Un desbordamiento horizontal no es una cuestión de diseño: es un fallo. La
 * decisión de producto del 2026-08-27 lo dice sin matices —Inicio crece en
 * vertical y no hay deslizadores horizontales, por pequeña que sea la
 * pantalla—, y lo mismo vale para el resto de destinos. Lo que sí puede
 * desplazarse en horizontal es un bloque que lo declare por dentro; lo que no
 * puede es la página.
 *
 * Y un token de color sin definir deja texto invisible sin que nada avise.
 * `check-ui.mjs` comprueba que todo `var(--nb-…)` esté declarado en app.css;
 * aquí se comprueba lo que aquel no puede ver, que es el resultado: que el
 * color de fondo esté realmente pintado en los siete temas.
 */
const DESTINOS = ['Inicio', 'Biblioteca', 'Estudio', 'Ajustes'] as const;

// Los tres escalones de `anchos.ts`, y el más estrecho que la carcasa admite.
const ANCHOS = [
  { nombre: 'compacto', ancho: 640, alto: 900 },
  { nombre: 'medio', ancho: 900, alto: 900 },
  { nombre: 'amplio', ancho: 1440, alto: 900 }
];

for (const { nombre, ancho, alto } of ANCHOS) {
  test(`ningún destino desborda en horizontal a ancho ${nombre}`, async ({ page }) => {
    await page.setViewportSize({ width: ancho, height: alto });
    await page.goto('/');

    for (const destino of DESTINOS) {
      await page.getByRole('button', { name: destino, exact: true }).click();
      await expect(page.getByRole('heading', { name: destino, level: 1 })).toBeVisible();

      const desborde = await page.evaluate(() => ({
        documento: document.documentElement.scrollWidth - document.documentElement.clientWidth,
        cuerpo: document.body.scrollWidth - document.body.clientWidth,
        // No basta con que la ventana no se desplace: un contenedor con
        // `overflow: auto` se lleva el desbordamiento dentro y produce la misma
        // barra horizontal, solo que escondida en mitad de la vista.
        dentro: Array.from(document.querySelectorAll<HTMLElement>('*'))
          .filter((el) => {
            const cs = getComputedStyle(el);
            if (cs.overflowX !== 'auto' && cs.overflowX !== 'scroll') return false;
            return el.scrollWidth - el.clientWidth > 1;
          })
          .map((el) => `${el.tagName.toLowerCase()}.${String(el.className).slice(0, 40)}`)
      }));

      expect(desborde.documento, `${destino} desborda el documento`).toBeLessThanOrEqual(0);
      expect(desborde.cuerpo, `${destino} desborda el cuerpo`).toBeLessThanOrEqual(0);
      expect(desborde.dentro, `${destino} esconde una barra horizontal dentro`).toEqual([]);
    }
  });
}

test('los siete temas pintan un fondo, y ninguno deja el texto sin color', async ({ page }) => {
  await page.goto('/');

  const temas = ['grafito', 'gabinete', 'papel', 'tinta', 'pergamino', 'vigilia', 'musgo'];

  for (const tema of temas) {
    await page.evaluate((t) => document.documentElement.setAttribute('data-theme', t), tema);

    const pintado = await page.evaluate(() => {
      const cuerpo = getComputedStyle(document.body);
      return { fondo: cuerpo.backgroundColor, texto: cuerpo.color };
    });

    // Un fondo transparente deja que se vea el del anfitrión, que es justo lo
    // que hace que un tema se sienta a medias.
    expect(pintado.fondo, `${tema} sin fondo`).not.toBe('rgba(0, 0, 0, 0)');
    expect(pintado.fondo, `${tema} sin fondo`).not.toBe('transparent');
    expect(pintado.texto, `${tema} sin color de texto`).not.toBe('rgba(0, 0, 0, 0)');
  }
});

test('el lector con sus paneles tampoco desborda, y menos en estrecho', async ({ page }) => {
  // El lector es donde más fácil es desbordar: dos paneles acoplados, una barra
  // de veinte acciones y una columna de texto con su propio ancho.
  await page.setViewportSize({ width: 640, height: 900 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // Con el índice abierto, con los ajustes del libro abiertos, y en dividida.
  for (const [tecla, panel] of [
    ['i', 'Índice'],
    ['t', 'Ajustes del Libro']
  ]) {
    await page.keyboard.press(tecla);
    // Que el panel esté de verdad ahí: si no, la prueba mediría una vista sin
    // paneles y pasaría por no haber abierto nada.
    await expect(page.getByRole('complementary', { name: panel })).toBeVisible();

    const desborde = await page.evaluate(() => ({
      documento: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      dentro: Array.from(document.querySelectorAll<HTMLElement>('*'))
        .filter((el) => {
          const cs = getComputedStyle(el);
          if (cs.overflowX !== 'auto' && cs.overflowX !== 'scroll') return false;
          return el.scrollWidth - el.clientWidth > 1;
        })
        .map((el) => `${el.tagName.toLowerCase()}.${String(el.className).slice(0, 40)}`)
    }));

    expect(desborde.documento, `desborda con ${panel} abierto`).toBeLessThanOrEqual(0);
    expect(desborde.dentro, `barra horizontal escondida con ${panel} abierto`).toEqual([]);
  }
});

test('los paneles de la derecha caben en la ventana', async ({ page }) => {
  // Un panel que se sale por la derecha no está "un poco cortado": está fuera
  // del alcance del ratón y del tabulador, y lo que dice no se puede leer.
  await page.goto('/');

  // Ahora se abren debajo de su control: Importar desde su botón y los
  // resultados de buscar al entrar en la caja de la barra.
  const paneles: Array<[() => Promise<void>, string]> = [
    [() => page.getByRole('button', { name: 'Importar Documentos' }).click(), 'Importar'],
    [() => page.getByRole('searchbox', { name: 'Buscar' }).click(), 'Búsqueda']
  ];

  // Los tres escalones de `anchos.ts`, uno por debajo del mínimo —una ventana se
  // puede estrechar más de lo que a nadie le conviene— y una pantalla entera,
  // que es donde se usa de verdad.
  for (const ancho of [560, 720, 900, 1440, 1920]) {
    await page.setViewportSize({ width: ancho, height: 900 });

    for (const [abrir, panel] of paneles) {
      await abrir();
      const region = page.getByRole('complementary', { name: panel });
      await expect(region).toBeVisible();

      const caja = await region.boundingBox();
      if (!caja) throw new Error(`${panel} no tiene caja a ${ancho}`);

      expect(caja.x, `${panel} empieza fuera por la izquierda a ${ancho}`).toBeGreaterThanOrEqual(
        0
      );
      expect(
        Math.round(caja.x + caja.width),
        `${panel} se sale por la derecha a ${ancho}`
      ).toBeLessThanOrEqual(ancho);

      // Y su contenido tampoco: un panel que cabe pero cuyo texto se corta no
      // sirve de nada.
      const dentro = await page.evaluate(() =>
        Array.from(document.querySelectorAll<HTMLElement>('*'))
          .filter((el) => {
            const cs = getComputedStyle(el);
            if (cs.overflowX !== 'auto' && cs.overflowX !== 'scroll') return false;
            return el.scrollWidth - el.clientWidth > 1;
          })
          .map((el) => `${el.tagName.toLowerCase()}.${String(el.className).slice(0, 40)}`)
      );
      expect(dentro, `${panel} corta su contenido a ${ancho}`).toEqual([]);

      await page.keyboard.press('Escape');
    }
  }
});

test('la vista previa de los temas dice la verdad sobre lo elegido', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();

  const muestra = (tema: string) =>
    page.locator(`.nb-tema[data-theme="${tema}"] .nb-tema-boton`).first();
  const acentoDe = (loc: ReturnType<typeof muestra>) =>
    loc.evaluate((el) => getComputedStyle(el).backgroundColor);

  // Con el acento del tema, cada miniatura enseña **el suyo**: si todas
  // enseñaran el mismo, la vista previa no estaría previendo nada.
  const grafito = await acentoDe(muestra('grafito'));
  const musgo = await acentoDe(muestra('musgo'));
  expect(grafito).not.toBe(musgo);

  // Al elegir un acento, las miniaturas lo reflejan: es lo que se va a ver.
  await page.getByRole('radio', { name: 'Malva' }).click();
  await expect.poll(() => acentoDe(muestra('grafito'))).toBe('rgb(176, 127, 176)');
  await expect.poll(() => acentoDe(muestra('musgo'))).toBe('rgb(176, 127, 176)');

  // Y no hay una casilla "el del tema": se retiró el 2026-08-28. Enseñaba el
  // acento del tema, que a veces coincide con uno de los seis de al lado, y
  // entonces se leía como una casilla repetida. Con seis colores delante, la
  // séptima no aportaba una elección: aportaba una duda.
  const casillas = await page
    .locator('[role="radiogroup"][aria-label="Color de Acento"] [role="radio"]')
    .count();
  expect(casillas, 'sobra una casilla de acento').toBe(6);
});

test('el alto contraste se ve en la vista previa, y gana al acento elegido', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();

  await page.getByRole('radio', { name: 'Malva' }).click();
  // El alto contraste vive en Accesibilidad y las miniaturas en Apariencia: se
  // enciende allí y se mira aquí, que es el camino de quien lo usa.
  await page.getByRole('button', { name: 'Accesibilidad', exact: true }).click();
  await page.getByRole('switch', { name: 'Alto contraste' }).click();
  await page.getByRole('button', { name: 'Apariencia', exact: true }).click();

  // La miniatura de cada tema enseña el alto contraste, no solo la del sistema:
  // es lo que hay que ver antes de encenderlo. Y enseña **su** variante: el
  // alto contraste tiene una clara y una oscura, y a un tema oscuro le toca la
  // oscura, así que las dos miniaturas no salen iguales.
  const fondoDe = (tema: string) =>
    page
      .locator(`.nb-tema[data-theme="${tema}"] .nb-tema-muestra`)
      .first()
      .evaluate((el) => getComputedStyle(el).backgroundColor);

  await expect.poll(() => fondoDe('grafito')).toBe('rgb(0, 0, 0)');
  await expect.poll(() => fondoDe('papel')).toBe('rgb(255, 255, 255)');

  // Y el acento elegido deja de aplicarse: la paleta de contraste trae el suyo,
  // medido. Lo contrario es que elegir un color le quite el contraste a quien
  // lo necesita.
  const acento = await page.evaluate(() =>
    document.documentElement.style.getPropertyValue('--nb-accent')
  );
  expect(acento).toBe('');
});

test('los nombres de los temas están en español, también el comodín', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();

  // "system" es el valor que se guarda, no lo que se lee. La interfaz está en
  // un idioma o en otro; nunca a medias.
  await expect(page.getByRole('radio', { name: 'Sistema', exact: true })).toBeVisible();
  await expect(page.getByText('system', { exact: true })).toHaveCount(0);
});

test('con la escala tipográfica subida, el panel tampoco se sale', async ({ page }) => {
  // La escala agranda el panel y el texto de dentro, así que es la forma más
  // rápida de quedarse sin sitio. Quien la sube depende de ella: que la
  // interfaz se descoloque justo entonces sería exactamente al revés de lo que
  // hace falta.
  await page.setViewportSize({ width: 1920, height: 1030 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();
  await page.getByRole('button', { name: 'Accesibilidad', exact: true }).click();

  const escala = page.getByRole('slider', { name: 'Escala de la interfaz' });

  // Buscar e Importar se abren debajo de su control, no pegados al
  // borde de la ventana: lo que se comprueba es que sigan cabiendo, y su
  // contenido también, con la letra al doble.
  const menus: Array<[() => Promise<void>, string]> = [
    [() => page.getByRole('searchbox', { name: 'Buscar' }).click(), 'Búsqueda'],
    [
      () => page.getByRole('button', { name: 'Importar Documentos', exact: true }).click(),
      'Importar'
    ]
  ];

  for (const valor of [1.5, 2]) {
    await escala.evaluate((el, v) => {
      const input = el as HTMLInputElement;
      input.value = String(v);
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }, valor);

    for (const [abrir, nombre] of menus) {
      await abrir();
      const panel = page.getByRole('complementary', { name: nombre });
      await expect(panel).toBeVisible();

      const caja = await panel.boundingBox();
      if (!caja) throw new Error(`sin caja a escala ${valor}`);

      expect(caja.x, `${nombre} se sale por la izquierda a escala ${valor}`).toBeGreaterThanOrEqual(
        0
      );
      expect(
        Math.round(caja.x + caja.width),
        `${nombre} se sale por la derecha a escala ${valor}`
      ).toBeLessThanOrEqual(1920);

      const dentro = await page.evaluate(() =>
        Array.from(document.querySelectorAll<HTMLElement>('*'))
          .filter((el) => {
            const cs = getComputedStyle(el);
            if (cs.overflowX !== 'auto' && cs.overflowX !== 'scroll') return false;
            return el.scrollWidth - el.clientWidth > 1;
          })
          .map((el) => el.tagName.toLowerCase())
      );
      expect(dentro, `algo se corta a escala ${valor}`).toEqual([]);

      await page.keyboard.press('Escape');
      await expect(panel).toHaveCount(0);
    }
  }
});

/**
 * Contraste de lo que se pulsa, **también con el ratón encima**.
 *
 * El caso que lo motiva: el botón primario tenía su fondo en una regla de dos
 * selectores y el realce del ratón en una de tres, así que al pasar por encima
 * ganaba el realce, el fondo se volvía casi negro y el texto seguía siendo
 * oscuro. Invisible, en el sitio donde más se pulsa. Ninguna prueba de las que
 * había podía verlo: el contraste se calculaba sobre `app.css`, y esto no es un
 * color de la paleta sino el resultado de dos reglas peleándose.
 */
async function contrasteDe(elemento: import('@playwright/test').Locator): Promise<number> {
  return elemento.evaluate((el) => {
    const relativa = (c: string) => {
      // `color-mix()` se computa como `color(srgb r g b)`, con canales de 0 a 1,
      // y no como `rgb()`, con canales de 0 a 255. Leídos como si fueran lo
      // mismo, un color claro se medía casi negro y cualquier contraste salía
      // 1,1:1. La escala la decide la sintaxis, no el número.
      const porUno = c.startsWith('color(');
      const [r, g, b] = (c.match(/[\d.]+/g) ?? ['0', '0', '0']).slice(0, 3).map(Number);
      const canal = (v: number) => {
        const s = porUno ? v : v / 255;
        return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
      };
      return 0.2126 * canal(r) + 0.7152 * canal(g) + 0.0722 * canal(b);
    };

    /*
     * El fondo se compone, no se coge el primero que aparezca.
     *
     * Los estados de la interfaz se pintan con veladuras —un 5 % del color de
     * texto sobre transparente, por ejemplo—, y una veladura leída como si fuera
     * opaca da el color de la veladura y no el que se ve: un realce clarísimo
     * sobre un fondo oscuro se medía como fondo claro, y el contraste salía
     * 1,9:1 donde de verdad hay 6:1. Se apilan las capas de abajo arriba y se
     * componen sobre la primera que sea opaca.
     */
    const canales = (c: string): [number, number, number, number] => {
      const porUno = c.startsWith('color(');
      const n = (c.match(/[\d.]+/g) ?? ['0', '0', '0']).map(Number);
      const escala = porUno ? 255 : 1;
      return [
        (n[0] ?? 0) * escala,
        (n[1] ?? 0) * escala,
        (n[2] ?? 0) * escala,
        n.length > 3 ? (n[3] ?? 1) : 1
      ];
    };

    const capas: Array<[number, number, number, number]> = [];
    for (let n: HTMLElement | null = el; n; n = n.parentElement) {
      const c = canales(getComputedStyle(n).backgroundColor);
      if (c[3] === 0) continue;
      capas.push(c);
      if (c[3] === 1) break;
    }
    // Si ninguna llegó a ser opaca, debajo está el blanco del navegador.
    capas.push([255, 255, 255, 1]);

    let [fr, fg, fb] = capas[capas.length - 1] as [number, number, number, number];
    for (let i = capas.length - 2; i >= 0; i -= 1) {
      const [r, g, b, a] = capas[i] as [number, number, number, number];
      fr = r * a + fr * (1 - a);
      fg = g * a + fg * (1 - a);
      fb = b * a + fb * (1 - a);
    }

    const a = relativa(getComputedStyle(el).color);
    const b = relativa(`rgb(${fr}, ${fg}, ${fb})`);
    return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
  });
}

test('lo que se pulsa se lee, y también con el ratón encima', async ({ page }) => {
  await page.goto('/');

  const casos: Array<[string, string]> = [
    ['Continuar', 'primario en Inicio'],
    ['Detalles', 'secundario en Inicio'],
    ['Biblioteca', 'destino en reposo de la barra']
  ];

  for (const [nombre, donde] of casos) {
    const boton = page.getByRole('button', { name: nombre, exact: true });
    await expect(boton).toBeVisible();

    // 4,5:1 es el umbral de texto de la WCAG, el mismo que exige `tema.test.ts`
    // sobre la paleta. Aquí se mide el resultado, no la paleta.
    expect(await contrasteDe(boton), `${donde} en reposo`).toBeGreaterThanOrEqual(4.5);

    await boton.hover();
    await page.waitForTimeout(200);
    expect(await contrasteDe(boton), `${donde} con el ratón encima`).toBeGreaterThanOrEqual(4.5);
  }
});

test('cambiar de tema no deja un instante ilegible', async ({ page }) => {
  /*
   * Se mide **sin esperar nada**, porque el fallo vivía justo ahí. Al arrancar,
   * la caché de primer pintado y lo que dice el runtime pueden no coincidir, así
   * que el tema cambia una vez. Con la transición puesta, el fondo del destino
   * activo se quedaba seiscientos milisegundos en el color del tema anterior
   * mientras el texto ya era el del nuevo: contraste de 1,02, es decir, el
   * nombre del destino no se veía. Ver `data-tema-cambiando` en app.css.
   */
  await page.goto('/');

  const activo = page.locator('[aria-current="page"]').first();
  await expect(activo).toBeVisible();

  for (let intento = 0; intento < 5; intento += 1) {
    expect(
      await contrasteDe(activo),
      `el destino activo no se lee al arrancar (medida ${intento})`
    ).toBeGreaterThanOrEqual(4.5);
    await page.waitForTimeout(120);
  }
});

test('los paneles de consulta son momentáneos y se van al pulsar fuera', async ({ page }) => {
  /*
   * Hay dos clases de panel y se comportan distinto a propósito. La ficha de la
   * biblioteca es parte de la vista: ocupa su hueco y está mientras estés ahí.
   * Buscar, importar y el estado se consultan y se cierran, así que se ponen
   * encima y se van solos: quitarle sitio al contenido para algo que dura veinte
   * segundos es cobrar caro por poco.
   */
  await page.setViewportSize({ width: 1600, height: 950 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();

  const ficha = page.getByRole('complementary', { name: 'Ficha' });

  // Sin ninguna obra elegida no hay ficha: un panel de 288 píxeles cuyo único
  // contenido es "elige una obra" no es un panel, es un hueco con título. Y en
  // una columna de contenido estrecha se superpone, así que llegó a tapar la
  // biblioteca entera.
  await expect(ficha).toHaveCount(0);

  await page
    .getByRole('button', { name: /Cien Años de Soledad/ })
    .last()
    .click();
  await expect(ficha).toBeVisible();

  // Buscar es una caja en la barra: los resultados se abren al
  // entrar en ella.
  await page.getByRole('searchbox', { name: 'Buscar' }).click();
  const busqueda = page.getByRole('complementary', { name: 'Búsqueda' });
  await expect(busqueda).toBeVisible();

  // Se superpone: la ficha sigue ahí debajo en lugar de haber sido sustituida.
  await expect(ficha).toBeAttached();

  // Y no le quita sitio al contenido: la rejilla mide lo mismo con el panel
  // abierto que sin él.
  const anchoConPanel = await page.locator('main').boundingBox();
  await page.mouse.click(700, 600);
  await expect(busqueda).toHaveCount(0);
  const anchoSinPanel = await page.locator('main').boundingBox();
  expect(anchoConPanel?.width).toBe(anchoSinPanel?.width);

  // La ficha, en cambio, no se va al pulsar fuera: no es momentánea.
  await page.mouse.click(700, 600);
  await expect(ficha).toBeVisible();

  // Y pulsar el botón que lo abrió sigue cerrándolo, sin que el cierre por
  // pulsar fuera se lo coma y lo vuelva a abrir.
  const boton = page.getByRole('button', { name: 'Importar Documentos', exact: true }).first();
  await boton.click();
  await expect(page.getByRole('complementary', { name: 'Importar' })).toBeVisible();
  await boton.click();
  await expect(page.getByRole('complementary', { name: 'Importar' })).toHaveCount(0);
});

/**
 * **El criterio de aceptación:** lo que se marcó se ve donde se marcó.
 *
 * Antes la aplicación guardaba las anotaciones y no las enseñaba
 * en la página: solo salían en un panel. Descrito por quien la usaba, *"haces un
 * marcador y no sabes qué marcaste"*.
 *
 * Lo que se comprueba no es que haya algo pintado, que sería fácil de aprobar
 * por accidente: es que **el tramo pintado dice exactamente lo que la anotación
 * dice citar**. Esa es la única forma de distinguir una marca en su sitio de una
 * marca corrida, que es peor que ninguna.
 */
test('lo que se anotó se ve sobre el texto, sin CSS.highlights y en su sitio exacto', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  const pintado = async (clase: string) =>
    page.evaluate((tipo) => {
      const elementos = document.querySelectorAll<HTMLElement>(
        `.nb-capa-marcas .nb-marca-item[data-nb-marca="${tipo}"][data-nb-marca-cita]`
      );
      if (elementos.length === 0) return null;
      return [...elementos].map((el) => el.dataset.nbMarcaCita ?? '');
    }, clase);

  // El resaltado del segundo párrafo y la nota del primero, cada uno con su
  // cita. Si la proyección se desviara un solo carácter, esto no coincidiría.
  await expect.poll(() => pintado('highlight')).toEqual(['una aldea de veinte casas']);
  await expect.poll(() => pintado('note')).toEqual(['Muchos años después']);

  // Y nada queda registrado en CSS.highlights.
  expect(await page.evaluate(() => CSS.highlights.size)).toBe(0);

  // Y lo de otro capítulo **no** se pinta aquí. Sin esta comprobación, pintar
  // todas las anotaciones del documento en todas las páginas también pasaría.
  const todo = [...((await pintado('highlight')) ?? []), ...((await pintado('note')) ?? [])];
  expect(todo).not.toContain('El coronel Aureliano Buendía');

  // La caja de la marca coincide con el tramo real del texto y se recoloca al
  // cambiar el cuerpo de letra.
  const cajaAntes = await page
    .locator('.nb-capa-marcas .nb-marca-item[data-nb-marca="highlight"]')
    .first()
    .boundingBox();
  expect(cajaAntes).not.toBeNull();
  expect(cajaAntes?.width).toBeGreaterThan(20);

  await page.keyboard.press('t');
  const cuerpo = page.getByRole('slider', { name: 'Cuerpo' });
  await cuerpo.fill('24');
  await cuerpo.dispatchEvent('input');
  await page.keyboard.press('Escape');

  await expect
    .poll(async () => {
      const cajaDespues = await page
        .locator('.nb-capa-marcas .nb-marca-item[data-nb-marca="highlight"]')
        .first()
        .boundingBox();
      return cajaDespues && cajaAntes ? cajaDespues.width > cajaAntes.width : false;
    })
    .toBe(true);
});

/**
 * **El criterio de aceptación, primera mitad:** por dónde vas y qué
 * marcaste se ven en el documento, no solo en un panel.
 *
 * Un marcador es lo que más claramente delataba el problema: lo pones, la
 * aplicación lo guarda, y en la página no cambia absolutamente nada. En papel es
 * una cinta que asoma.
 */
test('el marcador y el separador se ven en el texto, cada uno en su párrafo', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // El marcador está en el tercer párrafo, que es donde cae su cita.
  const marcado = page.locator('[data-nb-marca-punto="marcador"]');
  await expect(marcado).toHaveCount(1);
  await expect(marcado).toContainText('El mundo era tan reciente');

  // Y el separador, en el párrafo por donde iba la lectura al abrir el libro.
  const separador = page.locator('[data-nb-marca-punto="separador"]');
  await expect(separador).toHaveCount(1);
  await expect(separador).toContainText('Muchos años después');

  // La cinta se pinta fuera de la caja del párrafo: marcar no puede mover el
  // texto de sitio, porque eso hace perder el renglón a quien está leyendo.
  const desplazamiento = await marcado.evaluate((elemento) => {
    const estilo = getComputedStyle(elemento);
    return { borde: estilo.borderLeftWidth, relleno: estilo.paddingLeft };
  });
  expect(desplazamiento).toEqual({ borde: '0px', relleno: '0px' });
});

/**
 * Señalar una marca contesta **cuál es**, y no "está entre estas cuarenta".
 *
 * Las marcas se pintan en `.nb-capa-marcas` con `pointer-events: none` para no
 * estorbar la selección de texto, y el gesto pregunta qué carácter o caja hay
 * bajo el punto.
 */
test('pulsar una marca abre el panel en esa anotación', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  const marca = page.locator('.nb-capa-marcas .nb-marca-item[data-nb-marca="highlight"]').first();
  await expect(marca).toBeAttached();
  const caja = await marca.boundingBox();
  if (!caja) throw new Error('no hay ninguna marca pintada que señalar');

  await page.mouse.click(caja.x + caja.width / 2, caja.y + caja.height / 2);

  // Se abre el panel, y la fila activa es la del resaltado que se pulsó: la
  // segunda de la lista no, la suya.
  await expect(page.getByRole('heading', { name: 'Anotaciones' })).toBeVisible();
  const activa = page.locator('[data-activa="true"]');
  await expect(activa).toHaveCount(1);
  await expect(activa).toContainText('una aldea de veinte casas');
});

/**
 * **El criterio de aceptación, sobre el navegador de verdad:** el
 * panel se recorre, se salta a una anotación, se edita y se borra sin usar el
 * ratón ni una sola vez.
 *
 * La máquina pura de `anotaciones.test.ts` demuestra que la lógica es correcta.
 * Esto demuestra lo otro: que el navegador entrega las teclas al panel, que el
 * foco va donde se cree, y que **una tecla no hace dos cosas**.
 */
test('el panel de anotaciones se recorre, salta, edita y borra sin ratón', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();
  // Y se espera al **texto**, no solo al título: el lector y sus teclas se cargan
  // al abrir el primer documento, y el título aparece antes que ellos. Sin esta
  // línea, la tecla de abajo se pierde cuando el navegador llega frío, que es lo
  // que pasaba al ejecutar esta prueba sola.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // La tecla reservada abre el panel, y el foco entra en él: sin eso,
  // llegar a lo que acabas de abrir costaría un número desconocido de
  // tabuladores.
  await page.keyboard.press('a');
  await expect(page.getByRole('heading', { name: 'Anotaciones' })).toBeVisible();
  await expect(page.locator('[data-fila="0"] .nb-accion').first()).toBeFocused();

  // Recorrer: la flecha mueve de fila y el foco la sigue.
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('[data-fila="1"] .nb-accion').first()).toBeFocused();
  await expect(page.locator('[data-fila="1"]')).toHaveAttribute('data-activa', 'true');

  // Saltar: mueve la lectura de verdad, y se ve en el Locator que viaja. La
  // cabecera no sirve para comprobarlo: dice el título del documento, no el del
  // punto donde se está.
  await page.keyboard.press('Enter');
  await expect
    .poll(async () => {
      const destinos = await argumentosDe(page, 'save_progress');
      return destinos.map((a) => (a as { locator: { href: string } }).locator.href);
    })
    .toContain('OEBPS/c2.xhtml');

  // Saltar no cierra el panel ni suelta el foco: quien va repasando lo que
  // anotó no tiene que volver a abrirlo en cada salto.
  await expect(page.locator('[data-fila="1"] .nb-accion').first()).toBeFocused();

  // Editar: se abre el campo con lo que había y el foco cae dentro.
  await page.keyboard.press('e');
  const campo = page.getByRole('textbox', { name: 'Texto de la anotación' });
  await expect(campo).toBeFocused();

  await page.keyboard.type('Aquí empieza la soledad');
  await page.keyboard.press('Control+Enter');

  // Guardar destruye el campo que tenía el foco. Sin devolverlo a la fila, la
  // siguiente tecla no haría nada y quien navega por teclado se quedaría fuera
  // del panel que estaba usando.
  await expect(page.locator('[data-fila="1"] .nb-accion').first()).toBeFocused();

  // Borrar: pide confirmación, y la confirmación también es una tecla.
  await page.keyboard.press('Delete');
  await expect(page.getByText('¿Borrar esta anotación?')).toBeVisible();
  await page.keyboard.press('Enter');

  const comandos = await comandosPedidos(page);
  expect(comandos).toContain('annotation_outline');
  expect(comandos).toContain('edit_annotation');
  expect(comandos).toContain('delete_annotation');
});

test('con el panel de anotaciones abierto, una tecla no hace dos cosas', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // Se deja una selección viva en el texto: es la situación en la que las dos
  // teclas del panel significan además otra cosa en el lector.
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('[data-nb-bloque="0"]')).toBeFocused();

  await page.keyboard.press('a');
  await expect(page.locator('[data-fila="0"] .nb-accion').first()).toBeFocused();

  // La flecha mueve la fila del panel y **no** el párrafo del texto. Sin
  // detener la propagación, el lector escucha en `window` y movería los dos, y
  // además el foco saltaría al bloque.
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('[data-fila="1"] .nb-accion').first()).toBeFocused();
  await expect(page.locator('[data-nb-bloque="1"]')).not.toBeFocused();

  // Enter salta a la anotación y **no** abre una nota sobre lo seleccionado.
  await page.keyboard.press('Enter');
  await expect(page.getByRole('textbox', { name: 'Nota' })).toHaveCount(0);
});

test('consultar la selección y cerrar deja el foco y la posición donde estaban', async ({
  page
}) => {
  /*
   * **El criterio**, la mitad que no se puede demostrar
   * sin navegador: "consultar y cerrar devuelve el foco exactamente donde
   * estaba". La otra mitad —que la posición de lectura no se mueve ni un
   * locator— es una propiedad de la máquina pura y vive en `consulta.test.ts`.
   *
   * El foco no existe fuera de un documento real, así que esto son teclas de
   * verdad sobre Chromium.
   */
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('[data-nb-bloque="1"]')).toBeFocused();

  await page.keyboard.press('c');
  const panel = page.getByRole('complementary', { name: 'Consulta' });
  await expect(panel).toBeVisible();

  // El panel no atrapa el foco: sigue en el párrafo desde el que se consultó,
  // que es lo que hace que volver sea no hacer nada.
  await expect(page.locator('[data-nb-bloque="1"]')).toBeFocused();

  // Lo que se consulta es una palabra y no el párrafo: se selecciona por
  // bloques, y un párrafo entero no está en ningún diccionario.
  const consultada = await page.evaluate(() => {
    const llamadas = (globalThis as Record<string, unknown>).__nbLlamadas as Array<{
      comando: string;
      args: Record<string, unknown>;
    }>;
    const ultima = llamadas.filter((l) => l.comando === 'lookup_selection').at(-1);
    return ultima?.args.term as string;
  });
  expect(consultada).toBe('Macondo');

  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  await expect(page.locator('[data-nb-bloque="1"]')).toBeFocused();
});

test('el panel de consulta dice quién no contestó y ofrece las otras palabras', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('c');

  const panel = page.getByRole('complementary', { name: 'Consulta' });

  // Un proveedor omitido en silencio se lee como "esta palabra no existe", que
  // no es lo mismo que "falta autorizar el destino".
  await expect(panel.getByText('Wikipedia', { exact: true })).toBeVisible();
  await expect(panel.getByText('Falta autorizar este destino.')).toBeVisible();

  // **El criterio**: antes del botón, qué sale y a dónde. Un permiso
  // concedido sin saber qué se envía no es un consentimiento informado.
  await expect(panel.getByText('el texto que selecciones')).toBeVisible();
  await expect(panel.getByText('es.wikipedia.org')).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Permitir Esta Consulta' })).toBeVisible();

  // Y se puede consultar otra palabra del mismo párrafo sin cerrar nada.
  await panel.getByRole('button', { name: 'pelotón' }).click();
  await expect(panel.getByText('pelotón', { exact: true }).first()).toBeVisible();
});

test('sin catálogos añadidos, Descubrir no se anuncia', async ({ page }) => {
  // Un destino cuya única pantalla sería un formulario de configuración es la
  // regla 5 de ADR-013 vista del otro lado.
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Descubrir', exact: true })).toHaveCount(0);
});

test('con un catálogo añadido, Descubrir aparece y dice qué falta autorizar', async ({ page }) => {
  await montarPuente(page, { conCatalogo: true });
  await page.goto('/');

  await page.getByRole('button', { name: 'Descubrir', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Catálogos' })).toBeVisible();

  // Abrirlo no basta con haberlo añadido: añadir una dirección no es autorizar
  // una salida, y eso se dice con la acción que lo arregla.
  await page.getByRole('button', { name: 'Biblioteca de ejemplo' }).click();
  await expect(page.getByText(/Falta autorizar la salida hacia/)).toBeVisible();

  /*
   * Se nombra el anfitrión que falta, que no es el del catálogo: el
   * catálogo redirige a un espejo. Antes se autorizaba el del
   * catálogo, que no era el que faltaba, y el catálogo no abría nunca.
   */
  await expect(page.getByText('espejo.example')).toBeVisible();

  // Y la acción está donde se lee el aviso, no en otra pantalla: sin esto, el
  // permiso existía en el runtime y no había forma de darlo desde aquí.
  await page.getByRole('button', { name: 'Autorizar', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Biblioteca de ejemplo' })).toBeVisible();
  expect(await argumentosDe(page, 'allow_catalog_host')).toContainEqual({
    host: 'espejo.example',
    cleartext: false
  });
});

test('la voz de un servicio se enciende, se configura y hay que autorizarla aparte', async ({
  page
}) => {
  /*
   * El criterio, y la regla que lo gobierna: **escribir el servidor no
   * autoriza nada**. Es la misma que los catálogos, y aquí pesa más porque lo
   * que sale es el texto del libro, párrafo a párrafo.
   */
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'Voz', exact: true }).click();

  // De fábrica, ni servidor ni campos: la voz local no necesita ninguno.
  // El nombre accesible llega con la mayúscula que le pone el estilo, así que se
  // compara sin distinguirla en lugar de repetir aquí una decisión de la hoja.
  await expect(page.getByRole('heading', { name: /^voz$/i })).toBeVisible();
  await expect(page.getByLabel('Servidor de Voz')).toHaveCount(0);

  // Ahora son tres opciones y no un interruptor: la de un servicio es una
  // de ellas, y es la única que sale del equipo.
  await page.getByRole('radio', { name: 'De un servicio' }).click();
  const servidor = page.getByLabel('Servidor de Voz');
  await expect(servidor).toBeVisible();

  // Acotado a la sección de Voz: Red y Privacidad tiene su propio "Autorizar"
  // para los destinos pendientes, y buscarlo en toda la página encontraría aquel.
  const seccionDeVoz = page
    .locator('.nb-seccion')
    .filter({ has: page.getByLabel('Servidor de Voz') });

  // Sin servidor no hay nada que autorizar: el botón no aparece.
  await expect(seccionDeVoz.getByRole('button', { name: 'Autorizar' })).toHaveCount(0);

  await servidor.fill('voz.example');
  await servidor.blur();

  // Y con servidor sí, porque escribirlo no lo autoriza.
  await expect(seccionDeVoz.getByRole('button', { name: 'Autorizar' })).toBeVisible();
});

test('una estantería se encuentra escribiendo, y acota la biblioteca', async ({ page }) => {
  /*
   * La pregunta I8, respondida y conectada.
   *
   * El runtime calculaba las estanterías, la ruta las cargaba y se
   * las pasaba a la vista, y la vista **no las pintaba**. Era el defecto de
   * siempre —capacidad construida y sin conectar— en un sitio donde
   * `check-puente.mjs` no mira, porque no cruza el puente: era una prop muerta.
   *
   * Se busca en lugar de desplegar, y es la misma decisión que tomó H6 por el
   * mismo motivo: con 152.080 obras hay decenas de miles de autores, y un
   * desplegable de miles de elementos no es un gesto.
   */
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  // El botón vive en la barra superior, con buscar e importar: abre un panel
  // momentáneo, que es lo que son los tres.
  await page.locator('.nb-banner').getByRole('button', { name: 'Estanterías' }).click();

  const panel = page.getByRole('complementary', { name: 'Estanterías' });
  await expect(panel).toBeVisible();
  await expect(panel.getByRole('button', { name: /Jorge Luis Borges/ })).toBeVisible();
  // Idioma y formato se nombran en palabras, no con su código.
  await expect(panel.getByRole('button', { name: /^Español/ })).toBeVisible();
  await expect(panel.getByRole('button', { name: /^PDF/ })).toBeVisible();
  await expect(panel.getByRole('button', { name: /^1967/ })).toBeVisible();

  // Escribir acota: es lo que hace que decenas de miles de autores quepan.
  await panel.getByRole('searchbox', { name: 'Buscar una Estantería' }).fill('borges');
  await expect(panel.getByRole('button', { name: /Gabriel García Márquez/ })).toHaveCount(0);
  await expect(panel.getByRole('button', { name: /Jorge Luis Borges/ })).toBeVisible();

  // Y elegirla acota la biblioteca y se dice cuál está puesta.
  await panel.getByRole('button', { name: /Jorge Luis Borges/ }).click();
  await expect(page.getByRole('complementary', { name: 'Estanterías' })).toHaveCount(0);
  const marca = page.getByRole('button', { name: /Jorge Luis Borges/ });
  await expect(marca).toBeVisible();

  // Quitarla devuelve la biblioteca entera.
  await marca.click();
  await expect(page.getByRole('button', { name: /Jorge Luis Borges/ })).toHaveCount(0);
});

test('al entrar en una sección del catálogo se sigue sabiendo en cuál estás', async ({ page }) => {
  /*
   * La regresión del 2026-09-02. El catálogo activo se marcaba comparando su
   * dirección con la del feed en curso, así que entrar en cualquier sección lo
   * desmarcaba: dejabas de saber dónde estabas en cuanto navegabas. Lo que se
   * marca es la **raíz**, igual que en la barra lateral, donde lo marcado es
   * dónde estás y no qué acabas de pulsar.
   *
   * Y hay vuelta. Sin ella, entrar en una sección era un viaje de ida: la única
   * salida era pulsar el catálogo otra vez y perder el camino recorrido.
   */
  await montarPuente(page, { conCatalogo: true, catalogoAbierto: true });
  await page.goto('/');

  await page.getByRole('button', { name: 'Descubrir', exact: true }).click();
  const catalogo = page.getByRole('button', { name: 'Biblioteca de ejemplo' });
  await catalogo.click();
  await expect(catalogo).toHaveAttribute('aria-current', 'page');

  await page.getByRole('button', { name: 'Ver Detalles' }).click();
  await expect(page.getByRole('heading', { name: 'Novedades' })).toBeVisible();
  await expect(catalogo, 'entrar en una sección desmarcó el catálogo').toHaveAttribute(
    'aria-current',
    'page'
  );

  await page.getByRole('button', { name: 'Volver' }).click();
  await expect(page.getByRole('heading', { name: 'Biblioteca de ejemplo' })).toBeVisible();
  await expect(catalogo).toHaveAttribute('aria-current', 'page');
});

test('la ficha se corrige a mano y quitar una obra pide confirmación y no borra el archivo', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page.locator('.nb-tarjeta').first().click();
  const ficha = page.getByRole('complementary', { name: 'Ficha' });
  await expect(ficha).toBeVisible();

  // Corregir: solo viaja lo que cambió, con el nombre del campo que admite el runtime.
  await ficha.getByRole('button', { name: 'Editar la Ficha' }).click();
  await ficha.getByLabel('Editorial').fill('Editorial Corregida');
  await ficha.getByRole('button', { name: 'Guardar' }).click();
  const llamadas = await argumentosDe(page, 'edit_edition_field');
  expect(llamadas).toHaveLength(1);
  expect(llamadas[0]).toMatchObject({ field: 'publisher', value: 'Editorial Corregida' });

  // Quitar: el primer botón no quita nada, solo pregunta.
  await ficha.getByRole('button', { name: 'Quitar de la Biblioteca' }).click();
  expect(await comandosPedidos(page)).not.toContain('delete_document');
  await expect(ficha.getByText('El archivo se queda donde está', { exact: false })).toBeVisible();
  // Una copia referenciada no ofrece borrar nada del disco.
  await expect(ficha.getByRole('switch')).toHaveCount(0);
  await ficha.getByRole('button', { name: 'Quitar', exact: true }).click();
  const bajas = await argumentosDe(page, 'delete_document');
  expect(bajas).toHaveLength(1);
  expect(bajas[0]).toMatchObject({ deleteManagedCopy: false });
  await expect(ficha).toBeHidden();
});

test('completar una ficha desde un catálogo es explícito y dice qué falta', async ({ page }) => {
  /*
   * **El criterio**, en su parte visible: la consulta es explícita y
   * nunca automática, y lo que impide salir se dice con la acción que lo
   * arregla en lugar de con un error genérico.
   */
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page.locator('.nb-tarjeta').first().click();

  const ficha = page.getByRole('complementary', { name: 'Ficha' });
  await expect(ficha).toBeVisible();

  // Al abrir la ficha no se ha consultado nada: la petición es del usuario.
  expect(await comandosPedidos(page)).not.toContain('search_online_metadata');

  await ficha.getByRole('button', { name: 'Completar Desde un Catálogo' }).click();
  expect(await comandosPedidos(page)).toContain('search_online_metadata');

  await expect(
    ficha.getByText('Falta autorizar la salida a Open Library', { exact: false })
  ).toBeVisible();

  // Y se autoriza ahí mismo, como en Descubrir: autorizar vuelve a buscar.
  await ficha.getByRole('button', { name: 'Autorizar' }).click();
  await expect(ficha.getByText('Cien años de soledad').first()).toBeVisible();
});

test('una anotación que no se pudo colocar no ofrece un salto que fallaría', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('a');

  // La perdida va al final de la misma lista, marcada, no escondida en otra
  // pestaña: esa era la mitad del objetivo.
  const perdida = page.locator('[data-perdida="true"]');
  await expect(perdida).toHaveCount(1);
  await expect(perdida.getByText('No se pudo colocar en este documento')).toBeVisible();

  // Sin botón de ir, pero con el de borrar: sigue siendo suya.
  await expect(perdida.locator('[aria-label="Ir al Texto"]')).toHaveCount(0);
  await expect(perdida.locator('[aria-label="Borrar"]')).toHaveCount(1);
});

test('un marcador no ofrece editar, porque no tiene texto que editar', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('a');

  const marcador = page.locator('[data-fila="2"]');
  await expect(marcador.getByText('Marcador')).toBeVisible();
  await expect(marcador.locator('[aria-label="Editar"]')).toHaveCount(0);
  await expect(marcador.locator('[aria-label="Borrar"]')).toHaveCount(1);
});

test('escribir en el panel de anotaciones no dispara comandos del lector', async ({ page }) => {
  /*
   * Regresión de un defecto real, encontrado al escribir esta prueba. El lector
   * escucha en `window` y daba por sentado que el único campo de texto era el
   * suyo. Escribir "empieza" dentro del panel disparaba la lectura inmersiva
   * por la `z`, y "resaltar" habría creado resaltados. Ahora `escribiendo`
   * significa lo que el registro decía que significaba: el foco está en un
   * campo, sea de quien sea.
   */
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  await page.keyboard.press('a');
  /*
   * Se espera a que el foco esté **dentro** del panel antes de mandarle la
   * siguiente tecla, y no solo a que el panel se vea.
   *
   * El panel no escucha en `window`: sus manejadores están en sus propios
   * elementos, y al abrirse coloca el foco en una fila. Hasta que eso ocurre, la
   * `e` no la recibe nadie y se pierde. Esperar a que sea visible no basta -se
   * ve antes de colocar el foco-, y sin esta espera la prueba falla con el
   * código correcto en cuanto la máquina va cargada: es la clase de fragilidad
   * que la sección 4.12.2 de presupuestos manda corregir en lugar de tolerar.
   * Encontrada el 2026-09-12, fallando tres de tres con la máquina ocupada.
   */
  await expect(
    page
      .getByRole('complementary', { name: 'Anotaciones' })
      .getByRole('button', { name: 'Ir al Texto' })
      .first()
  ).toBeFocused();

  await page.keyboard.press('e');
  const campo = page.getByRole('textbox', { name: 'Texto de la anotación' });
  await expect(campo).toBeFocused();

  // Cada letra de esto es el atajo de algo: z inmersión, b marcador,
  // r resaltar, d dividir, i índice, m modo, t tipografía, a anotaciones.
  await page.keyboard.type('zbrdimta');

  // El campo se abre con lo que la anotación ya tenía, así que lo tecleado se
  // añade a eso: es la edición de una nota, no una nota nueva.
  await expect(campo).toHaveValue('Lo importante es el hielozbrdimta');
  await expect(page.getByRole('toolbar', { name: 'Acciones del documento' })).toBeVisible();
  const comandos = await comandosPedidos(page);
  expect(comandos).not.toContain('create_bookmark');
  expect(comandos).not.toContain('create_annotation');
});

/**
 * **Lo que faltaba:** las anotaciones de **todos** los documentos
 * en un solo sitio, filtrables, y el salto desde cualquiera a su posición.
 *
 * Es la frontera con el Lector, no una duplicación: allí se ven las del
 * documento abierto y en orden de lectura; aquí las de todos y por fecha,
 * porque la progresión de un libro no es comparable con la de otro.
 */
test('las anotaciones de todos los libros se ven en Estudio y llevan a su sitio', async ({
  page
}) => {
  await page.goto('/');

  await page.getByRole('button', { name: 'Estudio', exact: true }).focus();
  await page.keyboard.press('Enter');

  await vistaDeEstudio(page, 'Anotaciones').focus();
  await page.keyboard.press('Enter');

  // De qué libro es cada una, sin abrirlo: una lista de citas sueltas no dice
  // de dónde salió ninguna.
  // `first`, y no un identificador: dos anotaciones distintas sobre la misma
  // frase es una situación normal, no un defecto del escenario.
  await expect(page.getByText('El coronel Aureliano Buendía').first()).toBeVisible();
  await expect(page.getByText('Cien Años de Soledad').first()).toBeVisible();

  // Filtrar por clase.
  await page.getByRole('button', { name: 'Marcadores' }).focus();
  await page.keyboard.press('Enter');
  await expect
    .poll(
      async () => (await comandosPedidos(page)).filter((c) => c === 'library_annotations').length
    )
    .toBeGreaterThan(1);

  await page.getByRole('button', { name: 'Todas' }).focus();
  await page.keyboard.press('Enter');

  // Abrir una y saltar a su posición: el locator viaja entero, y el reanclaje
  // se resuelve al abrir, que es cuando hay documento contra el que resolverlo.
  await page
    .getByRole('button', { name: /El coronel Aureliano Buendía/ })
    .first()
    .focus();
  await page.keyboard.press('Enter');

  await page.getByRole('button', { name: 'Abrir en el Lector' }).focus();
  await page.keyboard.press('Enter');

  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();
  await expect
    .poll(async () => {
      const destinos = await argumentosDe(page, 'save_progress');
      return destinos.map((a) => (a as { locator: { href: string } }).locator.href);
    })
    .toContain('OEBPS/c2.xhtml');
});

test('una anotación sin copia disponible no ofrece abrir el documento', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Estudio', exact: true }).focus();
  await page.keyboard.press('Enter');
  await vistaDeEstudio(page, 'Anotaciones').focus();
  await page.keyboard.press('Enter');

  await page.getByRole('button', { name: /Una cita que desapareció/ }).focus();
  await page.keyboard.press('Enter');

  // Se dice por qué, en lugar de ofrecer un botón que fallaría al pulsarlo.
  await expect(page.getByText('No hay ninguna copia disponible')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Abrir en el Lector' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Borrar' })).toBeVisible();
});

/**
 * **El criterio de aceptación, la mitad que solo se ve en el
 * navegador:** su ausencia en una plataforma se declara, no se disimula.
 *
 * Chromium sí trae síntesis, pero puede no tener ninguna voz instalada, que es
 * exactamente la situación de una instalación mínima de Linux. Sea cual sea de
 * las dos, el panel tiene que decir algo: lo que no puede es enseñar un botón
 * que no habla.
 */
test('el panel de voz dice lo que hay, y nunca ofrece un botón que no habla', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  await page.keyboard.press('v');
  const panel = page.getByRole('complementary', { name: 'Lectura en Voz Alta' });
  await expect(panel).toBeVisible();

  // Se busca **dentro del panel**: el botón de la barra del lector se llama
  // parecido, y contarlo haría pasar la prueba sin mirar lo que se quería mirar.
  const leer = panel.getByRole('button', { name: 'Leer en Voz Alta' });
  const aviso = panel.getByText(/no trae síntesis de voz|No hay ninguna voz instalada/);

  // Una de las dos, nunca ninguna y nunca las dos: o hay controles o hay motivo.
  if ((await leer.count()) > 0) {
    await expect(leer).toBeVisible();
    await expect(panel.getByText('La voz es la del sistema')).toBeVisible();
    await expect(aviso).toHaveCount(0);
  } else {
    await expect(aviso).toBeVisible();
    await expect(panel.getByRole('button', { name: 'Parar' })).toHaveCount(0);
    await expect(panel.getByRole('combobox')).toHaveCount(0);
  }
});

test('un documento sin capa de texto no ofrece leer en voz alta', async ({ page }) => {
  // El control sale de las capacidades que declara el proveedor, no del tipo de
  // medio: es la misma regla que la tipografía en un cómic. Aquí se comprueba
  // sobre el documento real que el puente simulado declara.
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // El EPUB del escenario sí extrae texto, así que el control está.
  await expect(page.getByRole('button', { name: /Leer en Voz Alta \(V\)/ })).toBeVisible();
});

/**
 * **Escenario 2 de `chaos.md`: latencia de disco.**
 *
 * Lo que se comprueba no es que la interfaz sea rápida, sino que el puente es de
 * verdad asíncrono. Con un puente síncrono la ventana se congela mientras el
 * runtime responde: no hay estado de carga que enseñar porque no hay fotograma
 * que pintar, y el sistema operativo acaba ofreciendo cerrar la aplicación.
 *
 * Con doscientos milisegundos por respuesta —que es una eternidad para SQLite y
 * lo que se nota de verdad en un disco mecánico con la biblioteca real de ciento
 * cincuenta mil documentos—, la carcasa tiene que estar entera y responder al
 * teclado antes de que llegue ningún dato.
 */
test('con el runtime lento, la interfaz responde antes de tener los datos', async ({ page }) => {
  await montarPuente(page, { lento: 200 });
  await page.goto('/');

  // La carcasa está antes que los datos: es lo que distingue un puente asíncrono
  // de uno que bloquea.
  await expect(page.getByRole('navigation', { name: 'Navegación Principal' })).toBeVisible();

  // Y responde al teclado mientras espera. Si la ventana estuviera congelada,
  // esta pulsación no llegaría a ninguna parte.
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Biblioteca' })).toBeVisible();

  // Cuando los datos llegan, llegan: el retardo no los pierde.
  await expect(page.getByText('Cien Años de Soledad').first()).toBeVisible({ timeout: 10_000 });
});

test('con el runtime lento, abrir un documento no congela la ventana', async ({ page }) => {
  await montarPuente(page, { lento: 200 });
  await page.goto('/');

  await page.getByRole('button', { name: 'Continuar' }).click();

  // El lector monta antes de tener el contenido, y lo dice en lugar de enseñar
  // una pantalla en blanco mientras espera.
  await expect(page.getByRole('region', { name: 'Lector' })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Documento' })).toBeVisible();

  await expect(page.getByText('Muchos años después')).toBeVisible({ timeout: 10_000 });
});

/**
 * **El entregable que solo se ve en la interfaz:** vista previa antes
 * de guardar.
 *
 * Que el paquete no lleve rutas ni títulos ni secretos lo comprueban las pruebas
 * de Rust, que es donde se monta. Aquí se comprueba lo otro, que es igual de
 * parte del criterio: que **se puede leer antes de enviarlo**. Un paquete que se
 * genera y se sube sin enseñarse es lo que la Constitución no permite, porque
 * consentir sin ver no es consentir.
 */
test('el diagnóstico se enseña antes de poder guardarlo', async ({ page }) => {
  await page.goto('/');

  // En Ajustes y no en el panel de estado: ese panel solo aparece cuando algo va
  // mal, y un diagnóstico se exporta precisamente para contar un problema que el
  // monitor todavía no conoce.
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Ajustes', level: 1 })).toBeVisible();
  // En Acerca de, junto al estado de cada componente: es donde se mira cuando se
  // sospecha que algo va mal.
  await page.getByRole('button', { name: 'Acerca de', exact: true }).click();
  const panel = page.getByRole('main');

  // Antes de pedirlo no hay nada que guardar: preparar el paquete lee el
  // registro del disco, y quien mira el estado de los componentes no pidió eso.
  await expect(panel.getByRole('button', { name: 'Guardar el Diagnóstico' })).toHaveCount(0);

  await panel.getByRole('button', { name: 'Preparar un Diagnóstico' }).click();

  // Y lo que se enseña es lo que se va a enviar, línea por línea.
  await expect(panel.getByText('code=DOCUMENT_CORRUPTED')).toBeVisible();
  await expect(panel.getByText('línea retirada: ruta-de-usuario')).toBeVisible();

  // Incluido cuánto se dejó fuera: un recorte silencioso hace sacar conclusiones
  // de un principio que no es el principio.
  await expect(panel.getByText(/1200\s+líneas anteriores omitidas/)).toBeVisible();

  // Ahora sí se puede guardar.
  await expect(panel.getByRole('button', { name: 'Guardar el Diagnóstico' })).toBeVisible();
});

/**
 * **El criterio de aceptación, la mitad que se ve:** una base corrupta
 * produce un arranque en modo recuperación, nunca una pérdida silenciosa.
 *
 * Que la base se aparte y no se pierda lo comprueban las pruebas de Rust. Aquí
 * se comprueba lo otro, que es tan parte del criterio como aquello: que el
 * usuario **se entera**. Una biblioteca vacía sin explicación es indistinguible
 * de haberlo perdido todo.
 */
test('tras recuperar, la aplicación dice que esa biblioteca no es la tuya', async ({ page }) => {
  await montarPuente(page, { recuperada: true });
  await page.goto('/');

  const aviso = page.getByRole('alert');
  await expect(aviso).toBeVisible();
  await expect(aviso.getByText('No se pudo abrir tu biblioteca')).toBeVisible();

  // Y dice dónde está lo suyo, que es lo primero que se pregunta.
  await expect(aviso.getByText('nb-books.sqlite.corrupted-1787970000')).toBeVisible();
  await expect(aviso.getByText(/La anterior no se ha borrado/)).toBeVisible();

  // El motivo va traducido, no en crudo: STORAGE_FAILURE no le dice nada a nadie.
  await expect(aviso.getByText('No se pudo escribir en la biblioteca.')).toBeVisible();
  await expect(aviso.getByText('STORAGE_FAILURE')).toHaveCount(0);

  // Sigue en todas las vistas: mientras esté, lo que se ve no es su biblioteca.
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
});

test('en un arranque normal no hay ningún aviso de recuperación', async ({ page }) => {
  // Un aviso donde no pasó nada enseña a ignorar los avisos, que es cómo se
  // pierde el que importa.
  await page.goto('/');
  await expect(page.getByText('No se pudo abrir tu biblioteca')).toHaveCount(0);
});

/**
 * **Regresión: en inmersión la carcasa ocupaba 550 píxeles de 1920.**
 *
 * La retícula seguía declarando dos columnas —`auto 1fr`— cuando la barra
 * lateral ya no existía, así que la columna del contenido caía en la celda
 * `auto`, que se dimensiona por su contenido. Se veía como "pantalla completa
 * que no ocupa la pantalla", con el texto pegado al borde izquierdo: `margin: 0
 * auto` no centra dentro de un contenedor que mide lo mismo que él.
 */
test('en inmersión el lector ocupa la ventana entera', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Muchos años después')).toBeVisible();

  await page.keyboard.press('z');
  await expect(page.getByText('Lectura inmersiva')).toBeVisible();

  const medidas = await page.evaluate(() => {
    const caja = (sel: string) => {
      const el = document.querySelector(sel);
      if (!el) return null;
      const r = el.getBoundingClientRect();
      return { x: Math.round(r.x), w: Math.round(r.width) };
    };
    return { ventana: window.innerWidth, main: caja('main'), texto: caja('.nb-texto') };
  });

  expect(medidas.main?.w).toBe(medidas.ventana);
  expect(medidas.main?.x).toBe(0);

  // Y el texto queda centrado, no pegado al borde: su margen izquierdo tiene que
  // ser aproximadamente el mismo que el derecho.
  const izquierda = medidas.texto?.x ?? 0;
  const derecha = medidas.ventana - izquierda - (medidas.texto?.w ?? 0);
  expect(Math.abs(izquierda - derecha)).toBeLessThanOrEqual(2);
  expect(izquierda).toBeGreaterThan(100);
});

/**
 * **La cuadrícula nunca se mete debajo de la ficha.**
 *
 * Las columnas las cuenta el navegador con `auto-fill` y no una división hecha
 * en JavaScript. La cuenta propia acierta casi siempre y falla justo donde no se
 * puede comprobar desde el código: con la barra de desplazamiento apareciendo en
 * el mismo fotograma, con una escala de pantalla distinta del cien por cien, o
 * con un tema que cambie el hueco entre tarjetas. Cuando falla, sobra una
 * columna y la última se sale por la derecha, por debajo de la ficha.
 */
for (const ancho of [1920, 1600, 1366, 1280, 1100]) {
  test(`a ${ancho} la cuadrícula cabe junto a la ficha, en sus dos modos`, async ({ page }) => {
    await page.setViewportSize({ width: ancho, height: 900 });
    await page.goto('/');
    await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Biblioteca' })).toBeVisible();

    for (const modo of ['Ver en Cuadrícula', 'Ver en Lista']) {
      await page.getByRole('button', { name: modo }).click();
      await page.waitForTimeout(80);

      const medidas = await page.evaluate(() => {
        const principal = document.querySelector('main') as HTMLElement | null;
        const rejilla = document.querySelector('[aria-label="Biblioteca"] ul');
        return {
          derechaDelContenido: Math.round(principal?.getBoundingClientRect().right ?? 0),
          derechaDeLaRejilla: Math.round(rejilla?.getBoundingClientRect().right ?? 0),
          desbordaEnHorizontal: (principal?.scrollWidth ?? 0) > (principal?.clientWidth ?? 0),
          columnas: rejilla
            ? getComputedStyle(rejilla)
                .gridTemplateColumns.split(' ')
                .filter((c) => c.trim() !== '' && c !== 'none').length
            : 0
        };
      });

      expect(medidas.desbordaEnHorizontal, `${modo} a ${ancho}`).toBe(false);
      // Y ni un píxel por debajo de la ficha, que empieza justo donde el
      // contenido acaba.
      expect(medidas.derechaDeLaRejilla).toBeLessThanOrEqual(medidas.derechaDelContenido);

      // En lista hay **una** columna. Se comprueba el número y no solo que
      // quepa: heredar las columnas de la cuadrícula ponía cinco filas de lista
      // una al lado de otra, y con pocos documentos eso todavía cabía.
      if (modo === 'Ver en Lista') {
        expect(medidas.columnas, `columnas en lista a ${ancho}`).toBe(1);
      } else {
        expect(medidas.columnas).toBeGreaterThan(1);
      }
    }
  });
}

/**
 * **Un clic elige, dos abren.**
 *
 * Es lo que hace cualquier lista de archivos del sistema, y aquí importa más:
 * elegir cambia la ficha y abrir cambia de pantalla. Con un solo clic para
 * abrir, mirar de qué va un libro obligaba a abrirlo y volver.
 */
test('en la biblioteca un clic cambia la ficha y dos abren el libro', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Biblioteca' })).toBeVisible();

  const libro = page.getByRole('button', { name: /Pedro Páramo/ }).first();

  await libro.click();
  // La ficha cambia y **no** se ha abierto nada: seguimos en la biblioteca.
  const ficha = page.getByRole('complementary', { name: 'Ficha' });
  await expect(ficha.getByRole('heading', { name: 'Pedro Páramo' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Biblioteca' })).toBeVisible();

  await libro.dblclick();
  await expect(page.getByRole('region', { name: 'Lector' })).toBeVisible();
});

test('con el teclado, Enter abre: no hay doble pulsación que hacer', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Biblioteca' })).toBeVisible();

  await page
    .getByRole('button', { name: /Pedro Páramo/ })
    .first()
    .focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('region', { name: 'Lector' })).toBeVisible();
});

/*
 * # La aplicación dice lo que ha pasado
 *
 * Antes no lo decía nunca. Los fallos se guardaban en una
 * variable que solo se pintaba dentro del panel de estado —y ese panel solo
 * aparece cuando algún componente está en mal estado—, así que abrir un
 * documento que falla dejaba la pantalla idéntica a si hubiera ido bien. Lo
 * confirmado sufría lo mismo por el otro lado: anotar, resaltar o marcar no
 * dejaban ninguna señal.
 *
 * Estos dos recorridos son el criterio: una acción que sale bien se confirma, y
 * una que sale mal se ve **sin abrir nada**.
 */

test('marcar la posición se confirma en pantalla', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();
  // El título lo pinta la carcasa en cuanto hay sesión, y el lector, que es quien
  // escucha el teclado, llega en su propio trozo un momento después. Pulsar al ver
  // el título era pulsar antes de que hubiera nadie escuchando: la prueba fallaba
  // según lo cargada que fuera la máquina, y subir el plazo no lo arreglaba, porque
  // la tecla ya se había perdido. Se espera a la barra del lector.
  await expect(page.getByRole('button', { name: 'Poner un Marcador (B)' })).toBeVisible();

  await page.keyboard.press('b');

  // El marcador se guarda en un panel que puede estar cerrado, así que sin el
  // aviso no hay forma de saber si la tecla llegó.
  //
  // El 2026-09-12 esto falló dos veces y se subió el plazo sin encontrar la causa.
  // La causa apareció el 2026-09-16, cuando empezó a fallar siempre: la tecla se
  // pulsaba antes de que el lector escuchara, y ningún plazo recupera una tecla
  // perdida. Se arregló arriba, esperando a la barra del lector; el plazo se queda.
  await expect(page.getByText('Marcador puesto')).toBeVisible({ timeout: 15_000 });
});

test('un fallo del runtime se ve sin tener que abrir ningún panel', async ({ page }) => {
  await montarPuente(page, { rechaza: ['create_bookmark'] });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();
  // El título lo pinta la carcasa en cuanto hay sesión, y el lector, que es quien
  // escucha el teclado, llega en su propio trozo un momento después. Pulsar al ver
  // el título era pulsar antes de que hubiera nadie escuchando: la prueba fallaba
  // según lo cargada que fuera la máquina, y subir el plazo no lo arreglaba, porque
  // la tecla ya se había perdido. Se espera a la barra del lector.
  await expect(page.getByRole('button', { name: 'Poner un Marcador (B)' })).toBeVisible();

  await page.keyboard.press('b');

  // El mensaje en palabras, y el código canónico debajo. Nunca `error.message`.
  await expect(page.getByRole('log')).toContainText('Algo falló por dentro');
  await expect(page.getByRole('log')).toContainText('INTERNAL');
});

/**
 * **Un paquete incompleto se dice como lo que es** (2026-09-18).
 *
 * Defecto observado en uso: instalar una extensión desde su carpeta de fuentes
 * contestaba "el manifiesto de la extensión no es válido", así que mandaba a
 * revisar un archivo que estaba bien. Lo que falta es el `.wasm` compilado, y
 * ahora se dice con su nombre y con dónde está el paquete que sí lo trae.
 */
test('instalar un paquete sin sus módulos dice cuál falta, no que el manifiesto esté mal', async ({
  page
}) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbExtensionIncompleta = true;
  });
  await montarPuente(page, {});
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'Extensiones', exact: true }).click();
  await page.getByRole('button', { name: 'Instalar una Extensión' }).click();

  const aviso = page.getByRole('log');
  await expect(aviso).toContainText('le faltan los módulos que su manifiesto declara');
  await expect(aviso).toContainText('citas.wasm');
  await expect(aviso).not.toContainText('manifiesto de la extensión no es válido');
});

test('un aviso se puede cerrar, y el fallo dura mientras se lee', async ({ page }) => {
  await montarPuente(page, { rechaza: ['create_bookmark'] });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('b');

  const aviso = page.getByRole('log');
  await expect(aviso).toContainText('Algo falló por dentro');

  // Un acierto se lee de reojo y se va a los cuatro segundos; un fallo hay que
  // poder volver a leerlo y copiar su código, así que dura quince. Cinco
  // segundos después sigue ahí.
  await page.waitForTimeout(5000);
  await expect(aviso).toContainText('Algo falló por dentro');

  await page.getByRole('button', { name: 'Cerrar el aviso' }).click();
  await expect(aviso).not.toContainText('Algo falló por dentro');
});

/**
 * **La barra dice lo que está puesto** (2026-09-18).
 *
 * Tres cosas que antes no se veían: un panel abierto se marca en su botón, la
 * voz sonando se marca **de otro color** porque no es lo mismo que un panel
 * abierto, y las acciones sobre lo elegido salen en **otra barra al lado** en
 * lugar de alargar la de siempre y mover todos los botones de sitio.
 */
test('la barra del lector marca lo que está abierto y lo que está funcionando', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  const indice = page.getByRole('button', { name: /^Índice/ });
  await expect(indice).toHaveAttribute('aria-expanded', 'false');
  await indice.click();
  await expect(indice).toHaveAttribute('aria-expanded', 'true');
  await page.keyboard.press('Escape');
  await expect(indice).toHaveAttribute('aria-expanded', 'false');

  // Inmersión no despliega nada: **es** un modo puesto, y por eso se anuncia
  // como elección y no como divulgación.
  const inmersion = page.getByRole('button', { name: /^Lectura Inmersiva/ });
  await expect(inmersion).toHaveAttribute('aria-pressed', 'false');
});

/**
 * Y lo de la selección no alarga la barra: sale en la suya, al lado.
 */
test('lo que se puede hacer con lo elegido sale en su propia barra', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  const barra = page.getByRole('toolbar', { name: 'Acciones del documento' });
  const laDeLoElegido = page.getByRole('toolbar', { name: 'Acciones sobre lo elegido' });
  await expect(barra).toBeVisible();
  await expect(laDeLoElegido).toHaveCount(0);

  const anchoAntes = (await barra.boundingBox())?.width ?? 0;
  const botonesAntes = await barra.getByRole('button').count();

  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Shift+ArrowDown');
  await expect(page.locator('[data-nb-seleccionado="true"]')).toHaveCount(2);
  await expect(laDeLoElegido).toBeVisible();

  // La de siempre no cambia: ni de tamaño ni de contenido. Es lo que permite
  // saberse dónde está cada botón.
  expect(await barra.getByRole('button').count()).toBe(botonesAntes);
  const anchoDespues = (await barra.boundingBox())?.width ?? 0;
  expect(Math.abs(anchoDespues - anchoAntes)).toBeLessThan(2);
});

/**
 * Y el progreso va **arriba**, no apoyado sobre la última línea.
 */
test('el progreso de la lectura no se apoya sobre el texto', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  const progreso = page.getByRole('progressbar', { name: 'Progreso de la lectura' });
  const texto = page.locator('[data-nb-bloque="0"]');
  const cajaDelProgreso = await progreso.boundingBox();
  const cajaDelTexto = await texto.boundingBox();

  expect(cajaDelProgreso).not.toBeNull();
  expect(cajaDelTexto).not.toBeNull();
  // Termina por encima de donde empieza el texto: ni lo tapa ni flota sobre él.
  expect((cajaDelProgreso?.y ?? 0) + (cajaDelProgreso?.height ?? 0)).toBeLessThanOrEqual(
    cajaDelTexto?.y ?? 0
  );
});

test('el lector dice por dónde va la lectura', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();

  // El capítulo, cuál de cuántos y el porcentaje de la obra. Ninguna de las tres
  // se pintaba en ninguna parte, y el dato ya existía.
  const posicion = page.getByRole('progressbar', { name: 'Progreso de la lectura' });
  await expect(posicion).toBeVisible();
  await expect(posicion).toHaveAttribute('aria-valuenow', '0');
});

test('sin nada elegido, la barra del lector no ofrece lo que no haría nada', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();

  // Cuatro controles se pintaban siempre y eran inertes sin selección.
  await expect(page.getByRole('button', { name: /Escribir una Nota/ })).toHaveCount(0);
  await expect(page.getByRole('button', { name: /Resaltar/ })).toHaveCount(0);

  // Al elegir un párrafo aparecen, y entonces sí hacen algo. Se espera al texto:
  // el encabezado sale antes que el capítulo, y una flecha sin párrafos no elige.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('button', { name: /Escribir una Nota/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /Resaltar/ })).toBeVisible();
});

test('abrir un panel del lector con el ratón lo deja abierto', async ({ page }) => {
  /*
   * Regresión. Al dar estado visible a los controles alternables se
   * cambió `aria-expanded` por `aria-pressed` en los botones de panel, y `Panel`
   * cierra el panel momentáneo al pulsar fuera dejando pasar solo lo que lleva
   * `aria-expanded`: el mismo clic abría el panel y lo cerraba, así que ninguno
   * de los cinco llegaba a verse con el ratón. Con el teclado sí, que es por lo
   * que las pruebas no lo veían.
   */
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();

  for (const [boton, panel] of [
    ['Índice (I)', 'Índice'],
    ['Anotaciones (A)', 'Anotaciones'],
    ['Ajustes del Libro (T)', 'Ajustes del Libro'],
    // El botón existía y no pintaba ningún panel; se iluminaba y nada más.
    ['Buscar en el Documento (Ctrl+F)', 'Búsqueda'],
    ['Modo de Lectura (M)', 'Modo de Lectura']
  ] as const) {
    await page.getByRole('button', { name: boton }).click();
    await expect(page.getByRole('complementary', { name: panel })).toBeVisible();
    await page.getByRole('button', { name: boton }).click();
    await expect(page.getByRole('complementary', { name: panel })).toHaveCount(0);
  }
});

test('la vista dividida se abre, cambia de mitad y reparte el ancho', async ({ page }) => {
  /*
   * la vista estaba marcada como hecha y **no se podía abrir por ningún camino**:
   * `alternarDivision` estaba escrita, probada y exportada, y no la llamaba
   * nadie, así que la tecla `D` y su botón llevaban sin hacer nada.
   * La única vía que la miraba era abrir un segundo documento con la división ya
   * activa, y nunca podía estarlo.
   */
  await page.setViewportSize({ width: 1600, height: 950 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();

  await expect(page.locator('[data-enfocada]')).toHaveCount(1);

  await page.keyboard.press('d');
  await expect(page.locator('[data-enfocada]')).toHaveCount(2);

  // La segunda mitad todavía no tiene documento, y lo dice en lugar de quedarse
  // en blanco.
  await expect(page.getByText('Elige la segunda obra')).toBeVisible();

  // Con la segunda vacía, cambiar de mitad no lleva a ninguna parte: allí no hay
  // lector que gobierne el teclado, y el foco quedaría en un sitio donde
  // ninguna tecla responde.
  await page.keyboard.press('Control+Tab');
  await expect(page.locator('.nb-mitad').first()).toHaveAttribute('data-enfocada', 'true');

  // El segundo documento entra por la derecha, que es lo que hace que comparar
  // dos textos sea abrir dos textos y no un procedimiento.
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page
    .getByRole('button', { name: /Pedro Páramo/ })
    .last()
    .dblclick();
  await expect(page.locator('.nb-mitad')).toHaveCount(2);
  await expect(page.getByText('Elige la segunda obra')).toHaveCount(0);

  // Ahora sí manda la derecha, y se ve cuál manda: con dos lectores iguales no
  // hay otra forma de saber a cuál va la tecla siguiente.
  await expect(page.locator('.nb-mitad').nth(1)).toHaveAttribute('data-enfocada', 'true');
  await page.keyboard.press('Control+Tab');
  await expect(page.locator('.nb-mitad').first()).toHaveAttribute('data-enfocada', 'true');

  // El separador se mueve con el teclado, que es lo que el contrato
  // prometía y no cumplía nadie: `repartir` estaba escrita, probada y sin llamar.
  const anchoAntes = (await page.locator('.nb-mitad').first().boundingBox())?.width ?? 0;
  await page.keyboard.press('Control+ArrowRight');
  const anchoDespues = (await page.locator('.nb-mitad').first().boundingBox())?.width ?? 0;
  expect(anchoDespues).toBeGreaterThan(anchoAntes);

  // Y se cierra por donde se abrió.
  await page.keyboard.press('d');
  await expect(page.locator('[data-enfocada]')).toHaveCount(1);
});

test('con la división abierta, una tecla mueve una sola mitad', async ({ page }) => {
  /*
   * Los dos lectores escuchan en `window`. Sin gobernar quién manda, una sola
   * pulsación llegaba a los dos y bajar el cursor movía las dos a la vez.
   */
  await page.setViewportSize({ width: 1600, height: 950 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  // El lector se carga al abrir el primer documento: las teclas, cuando está.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('d');

  // Con el foco en la principal, el cursor baja ahí y solo ahí.
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('.nb-mitad').first().locator('[data-nb-bloque="0"]')).toBeFocused();
});

test('en una ventana estrecha no se ofrece dividir', async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 800 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();

  await expect(page.getByRole('button', { name: /Vista Dividida/ })).toHaveCount(0);
});

test('la lectura biónica y la regla hacen algo al encenderlas', async ({ page }) => {
  /*
   * Los dos interruptores del kit accesible existían y **no hacían
   * nada**: `partirBionica` estaba escrita y probada y no la llamaba nadie, y
   * `regla` se guardaba en la composición sin que la leyera ningún estilo. Un
   * ajuste de accesibilidad que se guarda y no se aplica es la peor clase de
   * ajuste: quien lo necesita concluye que su problema no tiene arreglo.
   */
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  const texto = page.locator('.nb-texto');
  await expect(texto.locator('b[data-nb-bionica]')).toHaveCount(0);

  await page.getByRole('button', { name: /Modo de Lectura/ }).click();
  await page.getByRole('switch', { name: 'Lectura biónica' }).click();

  // El punto de fijación aparece, y el texto **no cambia**: solo se envuelve.
  await expect(texto.locator('b[data-nb-bionica]').first()).toBeVisible();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  // Y se apaga entera, sin dejar rastro.
  await page.getByRole('switch', { name: 'Lectura biónica' }).click();
  await expect(texto.locator('b[data-nb-bionica]')).toHaveCount(0);

  await page.getByRole('switch', { name: 'Regla de lectura' }).click();
  await expect(page.locator('section[aria-label="Documento"]')).toHaveAttribute(
    'data-regla',
    'true'
  );

  // La regla acompaña al cursor, no a la selección: extenderla a tres párrafos
  // no dibuja tres reglas.
  await page.keyboard.press('Escape');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Shift+ArrowDown');
  await expect(texto.locator('[data-nb-cursor="true"]')).toHaveCount(1);
  await expect(texto.locator('[data-nb-seleccionado="true"]')).toHaveCount(2);
});

/** Abre el tablero de ejemplo del simulador desde la navegación de Estudio. */
async function abrirTableroDeEjemplo(page: import('@playwright/test').Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Estudio', exact: true }).click();
  await vistaDeEstudio(page, /Tableros/).click();
  await page
    .getByRole('group', { name: 'Tableros' })
    .getByRole('button', { name: /Capítulo tres/ })
    .click();
  await expect(page.getByRole('region', { name: 'Columnas del Tablero' })).toBeVisible();
}

test('un tablero se opera entero con el teclado', async ({ page }) => {
  // **El criterio, que se conserva al cambiar de forma.** Si el
  // tablero solo se pudiera usar arrastrando, sería una barrera.
  await abrirTableroDeEjemplo(page);

  // La tarjeta dice qué es y dónde está: quien usa un lector de pantalla no ve
  // las columnas.
  const primera = page.getByRole('button', {
    name: /Sobre el tiempo circular, en Por Revisar, posición 1 de 2/
  });
  await primera.focus();

  // Las flechas recorren...
  await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('button', { name: /La segunda nota, en Por Revisar/ })).toBeFocused();

  // ...y con Alt mueven la tarjeta, que conserva el foco en su sitio nuevo.
  await page.keyboard.press('Alt+ArrowRight');
  expect(await argumentosDe(page, 'board_move_card')).toContainEqual({
    boardId: 'brd-1',
    cardId: 'crd-b',
    columnId: 'col-2',
    position: 1
  });
  await expect(page.getByRole('button', { name: /La segunda nota, en En Curso/ })).toBeFocused();

  // Supr la quita del tablero, no de las notas.
  await page.keyboard.press('Delete');
  expect(await argumentosDe(page, 'board_remove_card')).toContainEqual({
    boardId: 'brd-1',
    cardId: 'crd-b'
  });
  await expect(page.getByRole('button', { name: /La segunda nota/ })).toHaveCount(0);
});

test('una tarjeta se arrastra a otra columna', async ({ page }) => {
  await abrirTableroDeEjemplo(page);

  await page
    .getByRole('button', { name: /Sobre el tiempo circular, en Por Revisar/ })
    .dragTo(page.locator('[data-columna="col-3"]'));

  expect(await argumentosDe(page, 'board_move_card')).toContainEqual({
    boardId: 'brd-1',
    cardId: 'crd-a',
    columnId: 'col-3',
    position: 0
  });
  await expect(
    page.getByRole('button', { name: /Sobre el tiempo circular, en Hecho, posición 1 de 1/ })
  ).toBeVisible();
});

test('desde una columna se crea una nota, y la columna se renombra', async ({ page }) => {
  // Crear la nota donde se va a ordenar es el gesto que faltaba: antes había que
  // ir a Notas, escribirla, volver y colocarla.
  await abrirTableroDeEjemplo(page);

  const hecho = page.locator('[data-columna="col-3"]');
  await hecho.getByRole('button', { name: 'Añadir Nota' }).click();
  await page.getByRole('textbox', { name: 'Título de la Nota Nueva' }).fill('Idea nueva');
  await page.getByRole('button', { name: 'Crear Nota' }).click();
  await expect(hecho.getByRole('button', { name: /Idea nueva, en Hecho/ })).toBeVisible();
  expect(await argumentosDe(page, 'board_create_note')).toContainEqual({
    boardId: 'brd-1',
    columnId: 'col-3',
    title: 'Idea nueva',
    body: ''
  });

  await page.getByRole('button', { name: 'Renombrar Columna: Hecho' }).click();
  const nombre = page.getByRole('textbox', { name: 'Nombre de la Columna' }).first();
  await nombre.fill('Terminado');
  await nombre.press('Enter');
  await expect(page.getByRole('heading', { name: 'Terminado' })).toBeVisible();

  // Y la tarjeta abre su nota.
  await hecho.getByRole('button', { name: /Idea nueva/ }).click();
  await expect(page.getByRole('textbox', { name: 'Título' })).toHaveValue('Idea nueva');
});

test('un tablero nuevo nace con columnas con nombre', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Estudio', exact: true }).click();
  await vistaDeEstudio(page, /Tableros/).click();

  await page.getByRole('textbox', { name: 'Nombre del Tablero' }).fill('Tesis');
  await page.getByRole('button', { name: 'Crear Tablero' }).click();

  expect(await argumentosDe(page, 'boards_create')).toContainEqual({
    title: 'Tesis',
    columns: ['Por Revisar', 'En Curso', 'Hecho']
  });
  await expect(page.getByRole('heading', { name: 'Tesis' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'En Curso' })).toBeVisible();
});

test('todo tablero tiene equivalente en lista, sobre los mismos datos', async ({ page }) => {
  await abrirTableroDeEjemplo(page);

  await page.getByRole('button', { name: 'Ver Como Lista' }).click();

  // La lista no es un resumen: es la misma respuesta, columna a columna. Lo que
  // está en las columnas está aquí, bajo el nombre de la suya.
  const lista = page.getByRole('list', { name: /columna a columna/ });
  await expect(lista).toContainText('Por Revisar');
  await expect(lista).toContainText('Sobre el tiempo circular');
  await expect(lista).toContainText('En Curso');
  await expect(lista).toContainText('Personajes');
});

test('Estudio no ofrece un filtro de anotaciones que no lleva a nada', async ({ page }) => {
  // Observación de uso del 2026-09-12: "si no tiene ítems no se muestra". El simulador tiene
  // notas, resaltados y un marcador, y ningún subrayado ni trazo.
  await page.goto('/');
  await page.getByRole('button', { name: 'Estudio', exact: true }).click();
  await vistaDeEstudio(page, 'Anotaciones').click();

  const filtros = page.getByRole('group', { name: 'Filtrar por Clase' });
  await expect(filtros.getByRole('button', { name: /Marcadores/ })).toBeVisible();
  await expect(filtros.getByRole('button', { name: /Resaltados/ })).toBeVisible();
  await expect(filtros.getByRole('button', { name: /Subrayados/ })).toHaveCount(0);
  await expect(filtros.getByRole('button', { name: /Trazos/ })).toHaveCount(0);
});

test('revisar metadatos por lotes actúa sobre lo que se está viendo', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();

  // El botón sale del manifiesto de la extensión: aparece sin que se haya
  // cargado una sola instrucción suya.
  await page.getByRole('button', { name: 'Revisar Metadatos' }).click();

  const argumentos = await argumentosDe(page, 'review_metadata_in_bulk');
  const enviado = argumentos[0] as { extension: string; editionIds: string[] };
  expect(enviado.extension).toBe('org.ejemplo.metadatos');
  // Las ediciones que se mandan son las de la página, no la biblioteca entera:
  // revisar lo que no se está mirando produce propuestas que nadie pidió.
  expect(enviado.editionIds.length).toBeGreaterThan(0);
});

test('las citas se exportan al formato que aporta una extensión', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  await page.getByRole('button', { name: /Anotaciones/ }).click();

  // Un botón por formato: con dos, elegir y confirmar serían dos gestos donde
  // basta uno. El nombre lo pone la extensión y no se traduce.
  await page.getByText('Otros Formatos').click();
  await page.getByRole('button', { name: /Citas en BibTeX/ }).click();

  const argumentos = await argumentosDe(page, 'save_citations');
  expect(argumentos[0]).toMatchObject({
    format: 'lat.nbdigitallabs.books.citas.bibliografia/bibtex'
  });
});

test('un modo de lectura que aporta un paquete se elige donde los del núcleo', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  await page.getByRole('button', { name: /Modo de Lectura/ }).click();

  // En la misma lista que los seis del núcleo, y con el nombre que le puso su
  // autor: para quien lee son modos, y separarlos sería enseñar un detalle de
  // implementación como si fuera una categoría.
  const pergamino = page.getByRole('button', { name: 'Pergamino' });
  await expect(pergamino).toBeVisible();
  await expect(page.getByRole('button', { name: 'Sepia' })).toBeVisible();

  await pergamino.click();

  // Lo elegido se guarda con el identificador de quien lo aporta: sin el
  // prefijo, dos paquetes que llamaran igual a su modo se pisarían.
  const argumentos = await argumentosDe(page, 'update_setting');
  expect(argumentos.at(-1)).toMatchObject({
    key: 'book_reading_mode',
    value: 'lat.nbdigitallabs.books.personalizacion.papel/pergamino'
  });

  // Y se pinta. Guardarlo no bastaba: el texto seguía con los colores de "día"
  // porque el lector solo conocía los modos del núcleo (segunda auditoría).
  await expect
    .poll(() =>
      page
        .locator('section[aria-label="Documento"]')
        .evaluate((el) => getComputedStyle(el).backgroundColor)
    )
    .toBe('rgb(242, 232, 213)');
});

test('los ajustes del libro se aplican al texto en el momento', async ({ page }) => {
  /*
   * Este panel entero no se podía comprobar: el simulador del
   * puente devolvía `plane: 'application'` para toda preferencia, y la
   * composición del libro solo lee lo que viene marcado como `book`. Con eso,
   * ningún ajuste del libro se aplicaba nunca en estas pruebas y la suite daba
   * por bueno un panel que no podía ver.
   */
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  const superficie = page.locator('section[aria-label="Documento"]');
  const cuerpoDe = () =>
    superficie.evaluate((el) => getComputedStyle(el).getPropertyValue('--nb-libro-cuerpo').trim());

  const antes = await cuerpoDe();

  await page.getByRole('button', { name: /Ajustes del Libro/ }).click();
  const deslizador = page.getByRole('slider', { name: 'Cuerpo' });
  await deslizador.focus();
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');

  // Nada de reiniciar ni de reabrir el documento: se aplica cuando se hace.
  await expect.poll(cuerpoDe).not.toBe(antes);

  // Y la alineación cambia la composición, no solo el control. Es un solo
  // ajuste y no dos: justificar es una alineación, y tenerlo también como
  // interruptor aparte dejaba a los dos peleándose por la misma propiedad.
  await expect(superficie).toHaveAttribute('data-alineacion', 'izquierda');
  await page.getByRole('button', { name: 'Alineación del texto' }).click();
  await page.getByRole('option', { name: 'Justificado' }).click();
  await expect(superficie).toHaveAttribute('data-alineacion', 'justificado');

  /*
   * Y usar el panel no lo cierra.
   *
   * La regresión del 2026-09-02: cambiar la alineación cerraba el panel y dejaba
   * su botón de la barra marcado como abierto, así que hacía falta pulsarlo dos
   * veces —una para desmarcarlo y otra para volver a abrirlo—. Eran dos defectos
   * a la vez. Uno, que un control del panel que rehace su propio marcado deja al
   * nodo del clic fuera del documento antes de que el evento llegue a `window`,
   * y `Panel` lo tomaba por un clic **fuera**. Y dos, que la carcasa guardaba su
   * propia copia de qué panel estaba abierto y la cerraba por su cuenta, sin
   * decírselo al lector, que es de quien es ese estado.
   */
  await expect(page.getByRole('complementary', { name: 'Ajustes del Libro' })).toBeVisible();
  await expect(page.getByRole('button', { name: /Ajustes del Libro/ })).toHaveAttribute(
    'aria-expanded',
    'true'
  );

  // Y cerrarlo desde el panel desmarca el botón, que es la otra mitad.
  await page.getByRole('button', { name: 'Cerrar panel' }).click();
  await expect(page.getByRole('complementary', { name: 'Ajustes del Libro' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: /Ajustes del Libro/ })).toHaveAttribute(
    'aria-expanded',
    'false'
  );
});

test('las imágenes del libro se piden por el protocolo de assets, no al servidor', async ({
  page
}) => {
  /*
   * El runtime extrae cada recurso del contenedor a su caché y deja en el `src`
   * **la ruta del archivo extraído**, porque el webview no puede leer dentro del
   * EPUB. Una ruta no es una dirección: puesta tal cual, el webview la resuelve
   * contra el origen de la aplicación y pide `http://localhost:1420/home/...`.
   * El síntoma era un 404 por imagen en el registro del servidor de desarrollo y
   * ninguna ilustración de ningún libro a la vista.
   *
   * Ninguna prueba podía verlo porque el simulador del puente devolvía la ruta
   * tal cual desde `convertFileSrc`, así que convertir y no convertir daban el
   * mismo resultado.
   */
  // Lo que se vigila es que **la ruta del recurso del libro** no se le pida al
  // servidor. El logotipo de la aplicación sí sale de ahí y debe seguir
  // haciéndolo: es un archivo del paquete, no del libro.
  const pedidasAlServidor: string[] = [];
  page.on('request', (peticion) => {
    const url = peticion.url();
    if (url.startsWith('http://localhost:4173/') && url.includes('/recursos/')) {
      pedidasAlServidor.push(url);
    }
  });

  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  const imagen = page.locator('.nb-texto img').first();
  await expect(imagen).toHaveAttribute('src', /^http:\/\/asset\.localhost\//);

  // Y nada del libro se le ha pedido al servidor que sirve la aplicación.
  expect(pedidasAlServidor, 'una imagen del libro pedida al servidor').toEqual([]);
});

/**
 * Mudar los libros de disco y recuperarlos.
 *
 * Es el criterio visto desde donde hay que verlo: la interfaz. El
 * runtime sabía hacerlo desde entonces y `tests/mudanza.rs` lo demostraba, pero
 * ninguno de sus tres comandos estaba conectado, así que desde la aplicación la
 * mudanza no existía. Una biblioteca que no sabe seguir a sus archivos es
 * exactamente la pérdida que esta fase existe para no tener.
 */
test('una biblioteca que se mudó de disco se recupera desde Ajustes', async ({ page }) => {
  await montarPuente(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'Dónde Viven tus Libros', exact: true }).click();

  // El estado de partida se **dice**: una raíz sin localizar y sus archivos
  // ausentes. Sin decirlo, el usuario ve una biblioteca que no abre y no sabe
  // por qué.
  await expect(page.getByRole('heading', { name: 'Dónde Viven tus Libros' })).toBeVisible();
  await expect(page.getByText('Sin localizar: falta decir dónde está ahora')).toBeVisible();
  await expect(page.getByText('3 ausentes de 3')).toBeVisible();

  await page.getByRole('button', { name: 'Decir Dónde Está Ahora' }).click();

  // Y vuelven: la ruta nueva, ningún ausente, y el recuento como confirmación.
  await expect(page.getByText('/otro/disco/biblioteca')).toBeVisible();
  await expect(page.getByText('3 archivos, todos donde deben')).toBeVisible();
  await expect(page.getByText('3 disponibles de 3')).toBeVisible();
});

test('revisar dónde está todo se pide, y dice el recuento', async ({ page }) => {
  await montarPuente(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'Dónde Viven tus Libros', exact: true }).click();

  // Nada se revisa solo al abrir Ajustes: sobre ciento cincuenta mil archivos
  // es trabajo de disco, y quien entra a cambiar el tema no lo ha pedido.
  await expect(page.getByText(/disponibles de/)).toHaveCount(0);

  await page.getByRole('button', { name: 'Revisar Dónde Está Todo' }).click();
  await expect(page.getByText('0 disponibles de 3')).toBeVisible();
  await expect(page.getByText(/3 no están donde el catálogo dice/)).toBeVisible();
});

/**
 * Llevarse las anotaciones de un libro.
 *
 * El contrato de exportación dice "el archivo exportado es legible e interpretable sin NB Books" y
 * lo demuestra en Rust, leyendo el JSON sin usar ni uno de nuestros tipos. Lo
 * que no había era forma de producir ese archivo: `export_annotations` existía
 * en el puente y **no lo llamaba nadie**, así que el criterio se cumplía en una
 * prueba y no en el producto. La especificación ya decía que elegir
 * destino era del diálogo nativo "que llega con la interfaz"; llegó la interfaz
 * y no llegó el botón.
 */
test('las anotaciones de un libro se pueden llevar a otra herramienta', async ({ page }) => {
  await montarPuente(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  await page
    .getByRole('button', { name: /Anotaciones/ })
    .first()
    .click();
  await page.getByRole('button', { name: 'Llevarme Estas Anotaciones' }).click();

  // Se guarda por el runtime, no trayendo el JSON al webview para devolverlo:
  // el webview no escribe en disco, y mover la biblioteca de anotaciones por el
  // canal para devolverla no tendría motivo.
  await expect(page.getByText('Anotaciones guardadas')).toBeVisible();
  expect(await argumentosDe(page, 'save_annotations')).toHaveLength(1);
  expect(await comandosPedidos(page)).not.toContain('export_annotations');
});

/*
 * # Las anotaciones salen en texto
 *
 * El JSON de W3C lo entienden otras herramientas y no lo lee nadie. Markdown y texto
 * plano se abren y se leen: lo que se comprueba aquí es que salen desde donde viven
 * las anotaciones, por el comando que ya existía y con el formato y el idioma
 * pedidos; cómo queda el archivo lo comprueba `tests/exportacion_de_anotaciones.rs`.
 */
test('las anotaciones se exportan en Markdown y en texto plano desde su panel', async ({
  page
}) => {
  await montarPuente(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page
    .getByRole('button', { name: /Anotaciones/ })
    .first()
    .click();

  await page.getByText('Otros Formatos').click();
  await page.getByRole('button', { name: 'Como Markdown', exact: true }).click();
  await expect(page.getByText('Anotaciones guardadas')).toBeVisible();
  await page.getByRole('button', { name: 'Como Texto Plano', exact: true }).click();
  await expect(page.getByText('Anotaciones guardadas').first()).toBeVisible();

  await expect.poll(async () => (await argumentosDe(page, 'save_annotations')).length).toBe(2);
  const [markdown, plano] = await argumentosDe(page, 'save_annotations');
  expect(markdown).toMatchObject({ format: 'markdown', language: 'es' });
  expect(plano).toMatchObject({ format: 'text', language: 'es' });
  // El nombre que propone el diálogo lleva la extensión del formato: quien lo abre
  // después con otro programa lo elige por ella.
  const dialogos = (await argumentosDe(page, 'plugin:dialog|save')) as Array<{
    options: { defaultPath: string };
  }>;
  expect(dialogos.map((d) => d.options.defaultPath.split('.').pop())).toEqual(['md', 'txt']);
});

/**
 * Citar un pasaje en una nota.
 *
 * Es el último entregable que quedaba sin llegar a la interfaz:
 * "enlaces entre notas y **desde una nota a una anotación anclada**". El runtime
 * lo resolvía entero —`link_annotation`, `unlink_annotation`,
 * `notes_for_annotation`— sin un solo comando detrás.
 *
 * El gesto vive en la anotación y no en la nota, y es una decisión de tamaño:
 * elegir una nota entre las propias es elegir entre unas decenas, elegir una
 * anotación entre las de la biblioteca es elegir entre miles. Se enlaza donde la
 * elección es la corta, y se **ve** desde los dos lados.
 */
test('un pasaje se cita en una nota, y se ve desde los dos lados', async ({ page }) => {
  await montarPuente(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Estudio', exact: true }).click();

  // Desde la anotación: se elige entre las notas propias.
  await vistaDeEstudio(page, 'Anotaciones').click();
  await page
    .getByRole('button', { name: /Muchos años después/ })
    .first()
    .click();
  await page.getByRole('button', { name: 'Citar en una nota' }).click();
  await page.getByRole('option', { name: 'Sobre el tiempo circular' }).click();
  await expect(page.getByText('Pasaje citado en la nota')).toBeVisible();

  // Y desde la anotación se ve quién la cita: el retroenlace, que no se guarda.
  await expect(page.getByRole('heading', { name: 'Notas que Citan Este Pasaje' })).toBeVisible();

  // Desde la nota se ve el pasaje, con de qué libro es.
  await vistaDeEstudio(page, 'Notas').click();
  await page
    .getByRole('button', { name: /Sobre el tiempo circular/ })
    .first()
    .click();
  await expect(page.getByRole('heading', { name: 'Pasajes que Cita' })).toBeVisible();
  await expect(page.getByText('Muchos años después').first()).toBeVisible();

  // Y se quita desde donde se ve.
  await page.getByRole('button', { name: 'Quitar este pasaje de la nota' }).click();
  await expect(page.getByText('Esta nota no cita ningún pasaje todavía.')).toBeVisible();
});

/**
 * Reescribir una nota no deshace los pasajes que cita.
 *
 * Las dos clases de enlace de una nota tienen mecanismos distintos: la que va a
 * otra nota la declara el cuerpo con `[[Título]]` y se rehace en cada guardado;
 * la que va a una anotación la declara un gesto. Si el guardado se llevara por
 * delante la segunda, citar un pasaje duraría hasta la siguiente errata.
 */
test('guardar una nota no se lleva por delante los pasajes que cita', async ({ page }) => {
  await montarPuente(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Estudio', exact: true }).click();

  await vistaDeEstudio(page, 'Anotaciones').click();
  await page
    .getByRole('button', { name: /Muchos años después/ })
    .first()
    .click();
  await page.getByRole('button', { name: 'Citar en una nota' }).click();
  await page.getByRole('option', { name: 'Sobre el tiempo circular' }).click();

  await vistaDeEstudio(page, 'Notas').click();
  await page
    .getByRole('button', { name: /Sobre el tiempo circular/ })
    .first()
    .click();
  await expect(page.getByText('Muchos años después').first()).toBeVisible();

  await page.getByRole('textbox', { name: 'Contenido' }).fill('Otra redacción, sin corchetes.');
  await page.getByRole('button', { name: 'Guardar' }).click();

  await expect(page.getByText('Muchos años después').first()).toBeVisible();
});

test('una nota sin pulsar Guardar no se pierde al cambiar de destino', async ({ page }) => {
  // Antes una nota solo se guardaba con el botón: cambiar de
  // destino con lo escrito a medias lo tiraba sin avisar.
  await page.goto('/');
  await page.getByRole('button', { name: 'Estudio', exact: true }).click();
  await vistaDeEstudio(page, 'Notas').click();
  await page.getByRole('button', { name: 'Nota Nueva' }).first().click();
  await page.getByRole('textbox', { name: 'Título' }).fill('Sin pulsar guardar');
  await page.getByRole('textbox', { name: 'Contenido' }).fill('Lo escrito tiene que seguir aquí.');

  await page.getByRole('button', { name: 'Inicio', exact: true }).click();
  await page.getByRole('button', { name: 'Estudio', exact: true }).click();
  await vistaDeEstudio(page, 'Notas').click();
  await expect(page.getByRole('button', { name: /Sin pulsar guardar/ }).first()).toBeVisible();
});

/**
 * Cambiar de destino no puede devolverte al principio de la biblioteca.
 *
 * # Por qué esto se rompía y nada lo decía
 *
 * Porque los destinos se montan con un `{#if}`, así que irse a Estudio
 * **desmonta** la vista de biblioteca y con ella todo su estado interno. La
 * página en la que estabas era estado interno. Sobre tres obras de prueba eso no
 * se nota, porque solo hay una página; sobre ciento cincuenta mil, estar en la
 * 1.500, mirar una nota y volver al principio es perder el sitio.
 *
 * Es la mitad de sesión: lo que sobrevive a moverse por la
 * aplicación lo guarda la carcasa, y lo que muere con la vista muere porque debe.
 */
test('volver a la biblioteca te deja en la página en la que estabas', async ({ page }) => {
  await montarPuente(page, { obras: 5_000 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();

  // Se avanza con "siguiente" y no pulsando el número: en la primera página la
  // paginación pinta 1, 2, la elipsis y la última, así que el 3 no existe
  // todavía. Depender de qué números hay pintados sería atar la prueba a una
  // decisión de diseño que no es lo que se está comprobando.
  await expect(page.locator('.nb-pagina[aria-current="page"]')).toHaveText('1');
  await page.getByRole('button', { name: 'Página Siguiente' }).click();
  await page.getByRole('button', { name: 'Página Siguiente' }).click();
  await expect(page.locator('.nb-pagina[aria-current="page"]')).toHaveText('3');

  await page.getByRole('button', { name: 'Estudio', exact: true }).click();
  // Se comprueba que la biblioteca **se desmontó**, que es justo lo que hace
  // perder el estado interno. Estudio se ancla por su navegación, que
  // sustituye a las pestañas: su contenedor no es una región para un lector de
  // pantalla.
  await expect(page.getByRole('region', { name: 'Biblioteca' })).toBeHidden();
  await expect(page.getByRole('navigation', { name: 'Secciones de Estudio' })).toBeVisible();

  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await expect(
    page.locator('.nb-pagina[aria-current="page"]'),
    'volver a la biblioteca devolvió al principio'
  ).toHaveText('3');
});

/**
 * Una importación que quedó a medias se ofrece, y se puede decir que no.
 *
 * # Por qué esto faltaba
 *
 * Reanudar ya era gratis por la identidad de contenido —lo ya importado se
 * reconoce y se salta, y hay prueba con mil archivos—, pero **solo si el usuario
 * se acordaba** de volver a lanzarla sobre la misma carpeta. Una importación de
 * ciento cincuenta mil archivos dura horas y nadie recuerda tres días después
 * qué carpeta eligió. Es la mitad.
 *
 * Se comprueban las dos salidas y no solo la de continuar: una oferta que solo
 * se puede aceptar se convierte en el aviso que la gente aprende a ignorar.
 */
test('una importación a medias se ofrece continuar, y se puede descartar', async ({ page }) => {
  await montarPuente(page, { aMedias: true });
  await page.goto('/');

  await page.getByRole('button', { name: 'Importar Documentos', exact: true }).first().click();
  const panel = page.getByRole('complementary', { name: 'Importar' });
  await expect(panel).toBeVisible();

  // Se dice qué carpeta era: "quedó una importación a medias" sin decir cuál no
  // se puede contestar.
  await expect(panel.getByText('/una/carpeta/con/libros')).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Continuar' })).toBeVisible();

  await panel.getByRole('button', { name: 'Descartar' }).click();
  await expect(
    panel.getByRole('button', { name: 'Continuar' }),
    'descartar no retiró la oferta: volvería a aparecer en cada arranque'
  ).toBeHidden();
});

/**
 * Lo que una importación dejó fuera sigue estando sin haberlo apuntado a mano.
 *
 * Antes el informe vivía en el panel de esa ejecución y en ningún sitio más:
 * cerrarlo era perderlo, y saber qué no entró de una carpeta de miles exigía
 * comparar a mano. Es la otra mitad de J12.
 */
test('lo que la última importación rechazó se puede mirar después', async ({ page }) => {
  await montarPuente(page);
  await page.goto('/');

  await page.getByRole('button', { name: 'Importar Documentos', exact: true }).first().click();
  const panel = page.getByRole('complementary', { name: 'Importar' });

  await expect(panel.getByText('roto/documento.epub')).toBeVisible();
  // El motivo en palabras: el código crudo no le dice nada a nadie. Va en
  // el `title`, para quien lo necesite al contarlo en un informe.
  const motivo = panel.locator('.nb-rechazo-motivo').first();
  await expect(motivo).not.toHaveText('DOCUMENT_CORRUPTED');
  await expect(motivo).toHaveAttribute('title', 'DOCUMENT_CORRUPTED');
});

/*
 * # Lo duplicado se dice al importar
 *
 * El runtime ya reconocía las copias exactas y las contaba junto a lo que ya estaba.
 * Aquí se comprueba el camino entero en la interfaz: el avance las cuenta aparte
 * mientras importa, y al terminar el informe guardado dice cuántas y de qué es copia
 * cada una. Que no entran dos veces lo comprueba `tests/copias_exactas.rs`.
 */
test('importar una carpeta con copias exactas las cuenta aparte y dice de qué es copia cada una', async ({
  page
}) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbCopias = true;
  });
  await montarPuente(page, {});
  await page.goto('/');
  await page.getByRole('button', { name: 'Importar Documentos', exact: true }).first().click();
  const panel = page.getByRole('complementary', { name: 'Importar' });
  await panel.getByRole('button', { name: 'Elegir Carpeta' }).click();

  const cifras = panel.locator('.nb-informe').first();
  await expect(cifras.getByText('copias exactas', { exact: true })).toBeVisible();
  await expect(cifras.locator('div', { hasText: 'copias exactas' }).locator('dd')).toHaveText('2');

  await expect(panel.getByText(/En la última importación: 2\s+copias exactas/)).toBeVisible();
  const lista = panel.getByRole('list', { name: 'copias exactas' });
  await expect(lista.getByRole('listitem')).toHaveCount(2);
  await expect(lista.getByText('descargas/cien años.epub')).toBeVisible();
  await expect(lista.getByText('copia de «Cien años de soledad»')).toBeVisible();
  expect(await argumentosDe(page, 'import_duplicates')).toEqual([
    { runId: 'run-copias', limit: expect.any(Number), offset: 0 }
  ]);
});

/**
 * **El criterio:** las marcas también se ven sobre un PDF.
 *
 * # Por qué esto no existía, y por qué es más que una prueba
 *
 * El pintado de marcas se salía en cuanto no había marcado, así que sobre un PDF
 * no se pintaba nada. Y no lo veía nadie porque **el simulador no tenía PDF**:
 * `text_layer` y `page_size` lanzaban formato no soportado, de modo que ningún
 * recorrido tocaba la mitad del producto que lee documentos paginados. Es el mismo
 * hueco que se corrigió con las portadas.
 *
 * La cita **cruza dos fragmentos** a propósito. El texto de un PDF no es marcado
 * sino trozos colocados, y van en `<span>`, que no puede separar bloques porque en
 * un EPUB vive dentro de un párrafo. Sin separarlos, la proyección los concatena
 * —"anualresume"— y ninguna cita coincide. Con esta prueba, esa regresión se ve.
 */
test('una página de PDF se amplía y se vuelve al ancho', async ({ page }) => {
  await montarPuente(page, { pdf: true });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('resume los datos')).toBeVisible();
  const ancho = () => page.locator('.nb-pagina').evaluate((p) => p.getBoundingClientRect().width);
  const inicial = await ancho();
  await page.getByRole('button', { name: /Acercar/ }).click();
  await page.getByRole('button', { name: /Acercar/ }).click();
  await expect.poll(ancho).toBeGreaterThan(inicial * 1.4);
  await page.getByRole('button', { name: /Alejar/ }).click();
  await page.getByRole('button', { name: /Alejar/ }).click();
  await expect.poll(ancho).toBeLessThanOrEqual(inicial + 1);
});

test('sobre un PDF, una marca se pinta donde dice su cita', async ({ page }) => {
  await montarPuente(page, { pdf: true });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');

  // La capa de texto está montada: es lo que se marca, no la imagen.
  await expect(page.getByText('resume los datos')).toBeVisible();

  const pintado = async () =>
    page.evaluate(() => {
      const elementos = document.querySelectorAll<HTMLElement>(
        '.nb-pagina .nb-capa-marcas .nb-marca-item[data-nb-marca="highlight"][data-nb-marca-cita]'
      );
      if (elementos.length === 0) return null;
      return [...elementos].map((el) => el.dataset.nbMarcaCita ?? '');
    });

  const sinEspacios = (texto: string) => texto.replace(/\s+/g, '');

  await expect
    .poll(async () => (await pintado())?.map(sinEspacios), {
      message: 'la marca del PDF no se pintó, o no dice lo que su anotación cita'
    })
    .toEqual([sinEspacios('anual resume')]);
  expect(await page.evaluate(() => CSS.highlights.size)).toBe(0);
});

test('las extensiones se ven en Ajustes con lo que cada una puede hacer', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();
  await page.getByRole('button', { name: 'Extensiones', exact: true }).click();

  await expect(page.getByRole('heading', { name: 'Extensiones' })).toBeVisible();

  // Con la API 1.0.0 estable no queda aviso de desarrollo: sale del número
  // que da el anfitrión, no de una cadena escrita en la vista.
  await expect(page.getByText(/API de extensiones\s+\d+\.x/)).toHaveCount(0);

  // La fila dice qué es y cuánto aporta, sin que su código se haya cargado:
  // sale del manifiesto, que es de lo que vive la activación perezosa.
  await expect(page.getByText('Metadatos Académicos')).toBeVisible();

  // Los dos estados de un permiso se distinguen. Sin esto, "concedido" y "sin
  // conceder" se verían igual y decidir sería adivinar.
  await expect(page.getByText('datos.ejemplo.org')).toBeVisible();
  await expect(page.getByText('otros.ejemplo.org')).toBeVisible();

  // Y lo que se envía se dice **en la fila**, no escondido detrás de un aviso:
  // es lo que hace falta para poder revocar con criterio.
  await expect(page.getByText(/no podemos acotar/).first()).toBeVisible();

  // Revocar llega al runtime con la extensión y el anfitrión, no solo con uno
  // de los dos: el permiso es de esa extensión, no del servidor.
  //
  // Se pulsa el **segundo** botón, y eso importa: con el primero, una fila que
  // revocara siempre el primer permiso de la lista pasaría igual. Lo destapó
  // una mutación.
  await page.getByRole('button', { name: 'Revocar' }).nth(1).click();
  const argumentos = await argumentosDe(page, 'extensions_revoke');
  expect(argumentos[0]).toMatchObject({
    id: 'org.ejemplo.metadatos',
    host: 'terceros.ejemplo.org'
  });
});

test('la lista de extensiones dice cuál va firmada y por quién', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();
  await page.getByRole('button', { name: 'Extensiones', exact: true }).click();

  // Las dos filas se diferencian **solo** en la firma: la segunda no tiene
  // permisos ni órdenes. Si esta prueba pasara con una sola, "sin firma" y
  // "firmada" podrían estar pintándose igual sin que nadie se enterara.
  await expect(page.getByText('Firmada por a1b2c3d4e5f60718')).toBeVisible();
  await expect(page.getByText('Sin firma')).toBeVisible();
});

test('una extensión retirada dice en su fila por qué está apagada', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();
  await page.getByRole('button', { name: 'Extensiones', exact: true }).click();

  // Apagada y sin motivo delante sería una desaparición sin explicación: la
  // fila tiene que decir que fue el registro y por qué.
  await expect(page.getByText('Retirada por maliciosa y apagada')).toBeVisible();

  // Y la que no está retirada no lo dice, que es lo que hace que el aviso
  // signifique algo.
  const otra = page.locator('li', { hasText: 'Metadatos Académicos' }).first();
  await expect(otra).not.toContainText('Retirada');
});

test('comprobar retiradas le pregunta al registro y no manda lo instalado', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();
  await page.getByRole('button', { name: 'Extensiones', exact: true }).click();

  // Sin registro configurado no hay a quién preguntar: el botón se apaga.
  await expect(page.getByRole('button', { name: 'Comprobar Retiradas' })).toBeDisabled();
  await page
    .getByPlaceholder(/registro|registry/i)
    .first()
    .fill('registro.ejemplo.org');
  await page
    .getByPlaceholder(/registro|registry/i)
    .first()
    .press('Tab');
  await page.getByRole('button', { name: 'Comprobar Retiradas' }).click();

  // Sin argumentos: lo que hay instalado no sale de este equipo. Se descarga la
  // lista entera y se compara en el runtime.
  const argumentos = await argumentosDe(page, 'extensions_check_withdrawals');
  expect(argumentos[0] ?? {}).toEqual({});
});

test('una extensión se apaga desde Ajustes sin desinstalarla', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();
  await page.getByRole('button', { name: 'Extensiones', exact: true }).click();

  // El interruptor lleva el nombre de la extensión, que es lo que permite
  // señalar cuál se está apagando cuando hay varias.
  await page.getByRole('switch', { name: 'Metadatos Académicos' }).click();

  const argumentos = await argumentosDe(page, 'extensions_set_enabled');
  expect(argumentos[0]).toMatchObject({ id: 'org.ejemplo.metadatos', enabled: false });
});

test('un candidato que propone una extensión dice de dónde sale', async ({ page }) => {
  await montarPuente(page, { conFuenteDeExtension: true });
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page.locator('.nb-tarjeta').first().click();

  const ficha = page.getByRole('complementary', { name: 'Ficha' });
  await ficha.getByRole('button', { name: 'Completar Desde un Catálogo' }).click();

  // Las dos fichas se ven, y solo una lleva procedencia. Sin esa diferencia, el
  // candidato de una extensión instalada la semana pasada se vería exactamente
  // igual que el del catálogo de siempre, y elegir sería adivinar.
  await expect(ficha.getByText('Sudamericana', { exact: false })).toBeVisible();
  await expect(
    ficha.getByText('lat.nbdigitallabs.books.banco.fuente-de-metadatos', { exact: false })
  ).toBeVisible();
});

test('una orden que aporta una extensión está en la ficha y se invoca desde ahí', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page.locator('.nb-tarjeta').first().click();

  const ficha = page.getByRole('complementary', { name: 'Ficha' });
  await expect(ficha).toBeVisible();

  // Está antes de tocar nada, porque sale del manifiesto: hasta que alguien
  // pulse, no existe una sola instrucción compilada de la extensión.
  await expect(ficha.getByRole('button', { name: 'Consultar la Fuente de Prueba' })).toBeVisible();
  expect(await comandosPedidos(page)).not.toContain('extensions_run_command');

  // Y va en la ficha y no en un menú del botón derecho: ADR-012 prohíbe la
  // acción exclusiva de ratón, así que se alcanza con el teclado como todo lo
  // demás.
  await ficha.getByRole('button', { name: 'Consultar la Fuente de Prueba' }).click();

  const argumentos = await argumentosDe(page, 'extensions_run_command');
  expect(argumentos[0]).toMatchObject({
    extension: 'lat.nbdigitallabs.books.banco.usa-el-mundo',
    command: 'usa-el-mundo.consultar'
  });
});

test('una vista que aporta una extensión se pinta con los componentes del núcleo', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Estudio' }).click();

  // La pestaña está antes de componer nada: sale del manifiesto.
  const pestana = vistaDeEstudio(page, /Resumen de Prueba/);
  await expect(pestana).toBeVisible();
  expect(await comandosPedidos(page)).not.toContain('extensions_render_view');

  await pestana.click();

  // Y lo que se pinta son las cuatro clases de bloque, con el marcado del
  // núcleo: la extensión aportó contenido, no interfaz.
  await expect(page.getByRole('heading', { name: 'Lo que hay' })).toBeVisible();
  await expect(page.getByText('Esta vista la compone una extensión.')).toBeVisible();
  await expect(page.getByText('Primero')).toBeVisible();
  await expect(page.getByText('Origen', { exact: false })).toBeVisible();

  const argumentos = await argumentosDe(page, 'extensions_render_view');
  expect(argumentos[0]).toMatchObject({
    extension: 'lat.nbdigitallabs.books.banco.vista-de-estudio',
    view: 'vista-de-estudio.resumen'
  });
});

test('un tema que aporta una extensión se elige donde se eligen los temas', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();

  // Está en el mismo grupo que los del núcleo, porque un tema es un tema.
  const suyo = page.getByRole('radio', { name: /Crepúsculo/ });
  await expect(suyo).toBeVisible();
  await expect(suyo).toHaveAttribute('aria-checked', 'false');

  await suyo.click();

  // Y al elegirlo se aplican sus colores de verdad, no una etiqueta.
  await expect(suyo).toHaveAttribute('aria-checked', 'true');
  const superficie = await page.evaluate(() =>
    document.documentElement.style.getPropertyValue('--nb-surface').trim()
  );
  expect(superficie).toBe('#161a22');
});

test('las obras sin portada reciben una que se distingue y se lee', async ({ page }) => {
  /*
   * **La portada generada no se ejercitaba en la suite**, y es lo que ve la mayor
   * parte de una biblioteca real: el 2026-09-12, con el corpus entero dentro, 982
   * obras de 152.080 no traían imagen. El banco servía siempre una portada por
   * obra -lo que arregló J8, y con razón, porque sin eso no se medía el coste de
   * descodificarlas-, así que esta mitad quedó sin cubrir. `sinPortadas` la
   * descubre.
   *
   * Lo que se fija son las dos cosas que fallaban el 2026-09-12: que los colores
   * se **distingan** -el reparto de tonos apiñaba títulos parecidos en el mismo, y
   * una rejilla así es medio camino hacia el hueco gris que esto evita- y que el
   * texto encima se **lea**.
   */
  await montarPuente(page, { sinPortadas: true, obras: 24 });
  await page.goto('/');
  await page
    .getByRole('button', { name: /Biblioteca/ })
    .first()
    .click();

  const portadas = page.locator('.nb-portada[data-generada="true"]');
  await portadas.first().waitFor();
  expect(await portadas.count()).toBeGreaterThan(10);

  const medido = await page.evaluate(() => {
    /*
     * El color se resuelve **en un lienzo** y no leyendo `getComputedStyle`: el
     * motor conserva la representación OKLCH -devuelve `oklch(0.34 0.11 52)`- y
     * parsear eso como si fueran tres canales da un contraste de 1 para todo, que
     * es lo que hizo fallar la primera versión de esta prueba. El lienzo es sRGB y
     * devuelve bytes de verdad.
     */
    const lienzo = document.createElement('canvas');
    lienzo.width = 1;
    lienzo.height = 1;
    const pincel = lienzo.getContext('2d');
    if (!pincel) return { tonos: [], contrastes: [], resolvio: false };
    const resolver = (color: string) => {
      pincel.clearRect(0, 0, 1, 1);
      pincel.fillStyle = color;
      pincel.fillRect(0, 0, 1, 1);
      const datos = pincel.getImageData(0, 0, 1, 1).data;
      return [datos[0], datos[1], datos[2]];
    };
    const luminancia = ([r, g, b]: number[]) => {
      const canal = (v: number) => {
        const c = v / 255;
        return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
      };
      return 0.2126 * canal(r) + 0.7152 * canal(g) + 0.0722 * canal(b);
    };
    const contraste = (a: number[], b: number[]) => {
      const [alto, bajo] = [luminancia(a), luminancia(b)].sort((x, y) => y - x);
      return (alto + 0.05) / (bajo + 0.05);
    };

    const tonos: number[] = [];
    const contrastes: number[] = [];
    let resolvio = true;
    for (const portada of document.querySelectorAll('.nb-portada[data-generada="true"]')) {
      const estilo = getComputedStyle(portada);
      const leer = (nombre: string) => estilo.getPropertyValue(nombre).trim();
      const tono = Number(leer('--nb-portada-tono'));
      tonos.push(tono);
      const fondo = resolver(
        `oklch(${leer('--nb-portada-luz')} ${leer('--nb-portada-croma')} ${tono})`
      );
      const tinta = resolver(
        `oklch(${leer('--nb-portada-tinta-luz')} ${leer('--nb-portada-tinta-croma')} ${tono})`
      );
      // Si el motor no supiera parsear OKLCH en un lienzo, los dos saldrían
      // negros y el contraste sería 21 para todo: un falso verde.
      if (fondo.join() === tinta.join()) resolvio = false;
      contrastes.push(Math.round(contraste(tinta, fondo) * 100) / 100);
    }
    return { tonos, contrastes, resolvio };
  });

  expect(medido.resolvio, 'el motor no resolvió los colores de la portada').toBe(true);

  // Ni un título por debajo del umbral de texto, con el color que le toque.
  const flojos = medido.contrastes.filter((razon) => razon < 4.5);
  expect(flojos, `contrastes: ${JSON.stringify(medido.contrastes)}`).toEqual([]);

  /*
   * Y los tonos se reparten. Doce grados es lo que separa dos portadas que se
   * distinguen de dos que parecen la misma: con el reparto anterior, dos de las
   * tres obras del banco caían a menos de eso.
   */
  const distintos = new Set(medido.tonos.map((tono) => Math.round(tono / 12))).size;
  expect(distintos, `tonos: ${JSON.stringify(medido.tonos)}`).toBeGreaterThan(
    medido.tonos.length * 0.7
  );
});

test('un tema de extensión no deja dos temas puestos a la vez', async ({ page }) => {
  /*
   * **Regresión del 2026-09-12, y son dos defectos con la misma raíz.**
   *
   * El primero se veía: los temas del núcleo y los de extensión viven en el mismo
   * grupo de radio, y los dos se marcaban a la vez, así que Ajustes decía que
   * había dos temas puestos.
   *
   * El segundo se veía peor. Un tema declara catorce colores `--nb-*`, y con el
   * rediseño media interfaz pasó a leer las variables de Material,
   * que un tema no declara: un tema oscuro sobre una paleta clara dejaba la mitad
   * de la pantalla con los colores del tema anterior. Medido en WebKitGTK, un
   * texto quedaba a 1,13 de contraste, es decir, invisible.
   *
   * Se comprueba sobre un tema **claro** del núcleo a propósito: con el núcleo ya
   * oscuro la mezcla no se ve, y era así como pasaba desapercibida.
   */
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();

  const grupo = page.getByRole('radiogroup', { name: 'Tema' });
  const claro = grupo.getByRole('radio', { name: /Papel/ });
  await claro.click();
  await expect(page.locator('html')).toHaveAttribute('data-resolved-scheme', 'light');

  const suyo = grupo.getByRole('radio', { name: /Crepúsculo/ });
  await suyo.click();
  await expect(suyo).toHaveAttribute('aria-checked', 'true');

  // Una sola selección en el grupo, siempre.
  await expect(grupo.getByRole('radio', { checked: true })).toHaveCount(1);
  await expect(claro).toHaveAttribute('aria-checked', 'false');

  /*
   * Con el tema puesto, el acento es el suyo y no se puede cambiar.
   * Antes seguían los seis colores, se marcaba el elegido y
   * pulsarlos no cambiaba nada.
   */
  const acentos = page.getByRole('radiogroup', { name: 'Color de Acento' });
  await expect(acentos.getByRole('radio')).toHaveCount(1);
  await expect(acentos.getByRole('radio', { name: '#7fb2e5' })).toBeDisabled();
  await expect(acentos.getByRole('radio', { name: '#7fb2e5' })).toHaveAttribute(
    'aria-checked',
    'true'
  );
  await expect(page.getByText('Lo pone el tema que tienes elegido: Crepúsculo')).toBeVisible();

  // Y el tema gobierna la apariencia entera: esquema, base y paleta de Material.
  await expect(page.locator('html')).toHaveAttribute('data-resolved-scheme', 'dark');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'grafito');
  expect(
    await page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue('--m3c-surface').trim()
    )
  ).toBe('#161a22');

  /*
   * El mecanismo que faltaba: nada de lo que se lee puede bajar del umbral de
   * texto. Se mide el color de cada elemento con texto contra el primer fondo
   * opaco que tenga encima, que es lo que de verdad se ve.
   */
  const ilegibles = await page.evaluate(() => {
    const aRgb = (valor: string) => (valor.match(/[\d.]+/g) ?? []).map(Number);
    const luminancia = ([r, g, b]: number[]) => {
      const canal = (v: number) => {
        const c = v / 255;
        return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
      };
      return 0.2126 * canal(r) + 0.7152 * canal(g) + 0.0722 * canal(b);
    };
    const contraste = (a: number[], b: number[]) => {
      const [alto, bajo] = [luminancia(a), luminancia(b)].sort((x, y) => y - x);
      return (alto + 0.05) / (bajo + 0.05);
    };
    const fondoDe = (el: Element | null) => {
      while (el) {
        const rgba = aRgb(getComputedStyle(el).backgroundColor);
        if (rgba.length >= 3 && (rgba[3] ?? 1) > 0.9) return rgba.slice(0, 3);
        el = el.parentElement;
      }
      return [0, 0, 0];
    };
    const malos: { texto: string; razon: number }[] = [];
    for (const el of document.querySelectorAll('h1,h2,h3,h4,h5,p,span,li,label,button')) {
      const texto = [...el.childNodes]
        .filter((n) => n.nodeType === Node.TEXT_NODE)
        .map((n) => n.textContent?.trim() ?? '')
        .join(' ')
        .trim();
      if (!texto || el.getBoundingClientRect().width === 0) continue;
      const razon = contraste(aRgb(getComputedStyle(el).color).slice(0, 3), fondoDe(el));
      if (razon < 4.5)
        malos.push({ texto: texto.slice(0, 30), razon: Math.round(razon * 100) / 100 });
    }
    return malos;
  });
  expect(ilegibles, JSON.stringify(ilegibles)).toEqual([]);

  // Y volver a un tema del núcleo retira el de la extensión de verdad, sin restos.
  await claro.click();
  await expect(suyo).toHaveAttribute('aria-checked', 'false');
  // Y el acento vuelve a poderse elegir.
  await expect(acentos.getByRole('radio')).toHaveCount(6);
  await expect(grupo.getByRole('radio', { checked: true })).toHaveCount(1);
  await expect(page.locator('html')).toHaveAttribute('data-resolved-scheme', 'light');
  expect(
    await page.evaluate(() =>
      document.documentElement.style.getPropertyValue('--nb-surface').trim()
    )
  ).toBe('');
});

test('teclear en el filtro de estanterías solo pinta la última respuesta', async ({ page }) => {
  /*
   * **El criterio, regla 3 del contrato de carga.**
   *
   * No es un problema de velocidad: es de corrección, y se manifiesta como
   * lentitud. Se dispara una petición por pulsación, y el simulador contesta al
   * revés de como se le pide -cuanto más corta la búsqueda, más tarda-, que es
   * lo que pasa de verdad cuando una consulta tarda segundos y otra
   * milisegundos. Sin la guarda de respuesta obsoleta, lo que se queda en
   * pantalla es el resultado de algo que ya nadie escribió.
   */
  await montarPuente(page, { estanteriasDesordenadas: true });
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page.locator('.nb-banner').getByRole('button', { name: 'Estanterías' }).click();

  const panel = page.getByRole('complementary', { name: 'Estanterías' });
  const filtro = panel.getByRole('searchbox');

  // "bo" contesta tarde y trae a Borges; "garcía" contesta pronto y trae a
  // García Márquez. Quien escribe acaba en lo segundo.
  await filtro.fill('bo');
  await filtro.fill('garcía');

  await expect(panel.getByText('Gabriel García Márquez')).toBeVisible();

  // Y la respuesta de "bo", que llega después, no puede pintar encima.
  await expect(panel.getByText('Jorge Luis Borges')).toHaveCount(0);
  await page.waitForTimeout(500);
  await expect(panel.getByText('Jorge Luis Borges')).toHaveCount(0);
  await expect(panel.getByText('Gabriel García Márquez')).toBeVisible();
});

/**
 * Vigila desde antes de pulsar si el indicador de carga llega a verse.
 *
 * Mirar después no sirve: cuando la aserción corre, la carga ya terminó y el
 * indicador ya no está, así que la prueba pasaría igual con umbral y sin él. Lo
 * que hay que capturar es si **en algún momento** se pintó, y eso solo lo ve
 * algo que estuviera mirando mientras pasaba.
 */
async function vigilarElIndicador(page: import('@playwright/test').Page) {
  await page.evaluate(() => {
    const ventana = window as unknown as { __vioIndicador?: boolean };
    ventana.__vioIndicador = false;

    const mirar = () => {
      if (document.querySelector('[role="status"][data-visible="true"]')) {
        ventana.__vioIndicador = true;
      }
    };
    mirar();
    new MutationObserver(mirar).observe(document.body, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ['data-visible']
    });
  });
}

const seVioElIndicador = (page: import('@playwright/test').Page) =>
  page.evaluate(() => (window as unknown as { __vioIndicador?: boolean }).__vioIndicador === true);

test('el indicador de carga espera antes de aparecer, y aparece si la espera es larga', async ({
  page
}) => {
  /*
   * **El criterio**.
   *
   * Un indicador de carga se mueve, y el innegociable dice que nada se anima con
   * la aplicación en reposo. La salida no es prohibirlo ni permitirlo siempre:
   * es que **espere**. Desde que los comandos salieron del hilo de la ventana y
   * las consultas van acotadas, casi todo contesta antes de que nadie lo note
   * -el panel de estanterías, 6 ms sobre 55.740 autores-, y poner un indicador
   * ahí no informa: destella.
   */
  await montarPuente(page, { lento: 900 });
  await page.goto('/');
  await vigilarElIndicador(page);
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();

  // Por encima del umbral hay que decir que se está esperando: el silencio a
  // esas alturas parece que se ha roto algo.
  await expect(page.getByRole('status').filter({ hasText: /Cargando/i })).toBeVisible();
});

test('con una espera corta el indicador de carga no llega a destellar', async ({ page }) => {
  // Cien milisegundos: la carga existe -el indicador se monta- pero termina muy
  // por debajo del umbral. Si se pintara, sería un destello: llama la atención y
  // no dice nada.
  await montarPuente(page, { lento: 100 });
  await page.goto('/');
  await vigilarElIndicador(page);
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();

  await expect(page.getByRole('region', { name: 'Biblioteca' })).toBeVisible();
  expect(await seVioElIndicador(page), 'el indicador destelló en una espera de 100 ms').toBe(false);
});

/**
 * La biblioteca entera se exporta desde Ajustes, y se dice qué salió.
 *
 * Antes la exportación completa existía en el runtime y solo la
 * llamaban las pruebas. Aquí se comprueba la mitad del navegador: que el botón
 * existe, que pide el destino al diálogo del sistema y que esa ruta llega al
 * runtime. Que el archivo esté completo, se relea igual y no pise la exportación
 * anterior si falla lo comprueban las pruebas de Rust.
 */
test('la biblioteca entera se exporta desde Ajustes', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'Dónde Viven tus Libros', exact: true }).click();

  await expect(
    page.getByText('El audio preparado no viaja: es almacenamiento derivado', { exact: false })
  ).toBeVisible();
  await page.getByRole('button', { name: 'Exportar la Biblioteca' }).click();
  await expect(page.getByText('Biblioteca exportada. Documentos incluidos:')).toBeVisible();

  const ruta = await page.evaluate(() => {
    const llamadas = (globalThis as Record<string, unknown>).__nbLlamadas as Array<{
      comando: string;
      args: Record<string, unknown>;
    }>;
    return llamadas.find((l) => l.comando === 'save_library_export')?.args.path;
  });
  expect(ruta, 'la ruta que eligió el diálogo no llegó al runtime').toBe('/tmp/anotaciones.json');
});

/**
 * Pulsar un resultado de búsqueda abre el libro.
 *
 * Regresión del defecto encontrado usando la aplicación el 2026-09-12:
 * "el servicio de búsqueda al tocar un libro no me lo abre en el lector o ni
 * siquiera lo busca en la biblioteca".
 *
 * Eran dos huecos con la misma causa. El resultado no traía su archivo, así que
 * el manejador de la carcasa recibía una edición y no tenía con qué abrirla: se
 * limitaba a cambiar de pantalla y descartaba el identificador con un `void`. Y
 * el salto dentro del texto ponía el objetivo **sin abrir el documento**, lo que
 * solo funciona si ya está abierto y es el mismo; con el alcance de corpus los
 * resultados vienen de toda la biblioteca.
 *
 * No lo vio ningún recorrido porque el simulador del puente **no tenía el
 * comando `search`**: el panel de búsqueda no se abría en ninguna prueba.
 */
test('pulsar un resultado de búsqueda abre el documento en el lector', async ({ page }) => {
  await page.goto('/');

  // La búsqueda es una caja en la barra, no un botón que abre otra caja: se
  // escribe en ella y los resultados salen debajo.
  await page.getByRole('searchbox', { name: 'Buscar' }).fill('soledad');

  // Acotado al panel de resultados: el mismo título existe también en la
  // biblioteca de detrás, y un localizador que resolviera a dos elementos
  // fallaría sin decir que el problema es la ambigüedad.
  const panel = page.locator('.nb-resultados');
  const resultado = panel.getByRole('button', { name: /Cien Años de Soledad/ });
  await expect(resultado).toBeVisible();
  await resultado.click();

  // La señal de que se abrió de verdad es el lector con el documento dentro, no
  // que hayamos llegado a otra pantalla: ir a la biblioteca sin el libro es lo
  // que hacía antes, y para quien pulsa era lo mismo que no hacer nada.
  await expect(page.getByRole('region', { name: 'Lector' })).toBeVisible();

  const abiertos = await page.evaluate(() => {
    const llamadas = (globalThis as Record<string, unknown>).__nbLlamadas as Array<{
      comando: string;
      args: Record<string, unknown>;
    }>;
    return llamadas.filter((l) => l.comando === 'open_document').map((l) => l.args.fileId);
  });
  expect(abiertos, 'el resultado no llegó a pedirle el documento al runtime').toContain('f0');
});

/**
 * Y la otra mitad: un resultado cuyo archivo no está **se ve y dice por qué no
 * se abre**, en lugar de ofrecer un salto que va a fallar.
 *
 * Sin esta mitad, la prueba de arriba pasaría igual con una interfaz que
 * ofreciera abrirlo todo. El volumen desconectado es el caso cotidiano.
 */
test('un resultado sin copia disponible se ve y dice que no está', async ({ page }) => {
  await page.goto('/');

  await page.getByRole('searchbox', { name: 'Buscar' }).fill('soledad');

  const panel = page.locator('.nb-resultados');
  const sinCopia = panel.getByRole('button', { name: /Obra en un Volumen Desconectado/ });
  await expect(sinCopia).toBeVisible();
  await expect(sinCopia).toBeDisabled();
  await expect(sinCopia.getByText('No disponible')).toBeVisible();
});

/**
 * el coste en disco y en tiempo se ve **antes** de aceptar
 * preparar la búsqueda por significado del libro abierto, y nada se prepara hasta
 * pulsar. Después se ve el avance, y al terminar, que está preparado.
 */
test('el coste de preparar la búsqueda por significado se ve antes de aceptar', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.getByRole('button', { name: 'Biblioteca' }).click();

  await page.getByRole('searchbox', { name: 'Buscar' }).click();
  const panel = page.getByRole('complementary', { name: 'Búsqueda' });
  await panel.getByRole('tab', { name: 'En Este Libro' }).click();

  const significado = panel.getByRole('region', { name: 'Por Significado' });
  await expect(significado.getByText(/Prepararlo ocupa unos/)).toBeVisible();
  await expect(significado.getByText(/145[,.]3 KB/)).toBeVisible();
  await expect(significado.getByText('2 min')).toBeVisible();
  expect(await comandosPedidos(page), 'se preparó sin aceptar').not.toContain('start_vectors');

  await significado.getByRole('button', { name: 'Preparar' }).click();

  expect(await comandosPedidos(page)).toContain('start_vectors');
  await expect(significado.getByRole('progressbar')).toBeVisible();
  await expect(significado.getByText(/Preparado para buscar por significado/)).toBeVisible();
});

/**
 * los resultados léxicos y los de significado se funden en una lista, y
 * cada uno dice de dónde sale. Lo que sale en las dos se dice una vez.
 */
test('la búsqueda funde lo exacto y lo cercano por significado y dice de dónde sale', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('searchbox', { name: 'Buscar' }).fill('soledad');
  const panel = page.getByRole('complementary', { name: 'Búsqueda' });
  await panel.getByRole('tab', { name: 'Dentro de los Libros' }).click();

  // Y cada resultado dice de qué libro es.
  await expect(panel.getByText('Cien Años de Soledad', { exact: false }).first()).toBeVisible();
  const ambas = panel.getByRole('button', { name: /frente al pelotón.*Exacta y por significado/ });
  await expect(ambas).toBeVisible();
  await expect(ambas).toHaveCount(1);
  await expect(
    panel.getByRole('button', { name: /Macondo era entonces.*Por significado/ })
  ).toBeVisible();
  await expect(
    panel.getByRole('button', { name: /no está a mano.*Exacta No disponible/ })
  ).toBeVisible();
});

/**
 * con la IA caída, la búsqueda léxica sigue igual. Sin
 * errores en consola —lo comprueba el `afterEach`—, con los mismos resultados y sin
 * nada por significado.
 */
test('con la búsqueda por significado caída la léxica sigue igual', async ({ page }) => {
  await montarPuente(page, { rechaza: ['semantic_search'] });
  await page.goto('/');
  await page.getByRole('searchbox', { name: 'Buscar' }).fill('soledad');
  const panel = page.getByRole('complementary', { name: 'Búsqueda' });
  await panel.getByRole('tab', { name: 'Dentro de los Libros' }).click();

  // Todo exacto: ninguna fila dice "Exacta", que sin otra cosa al lado no informa.
  await expect(panel.getByRole('button', { name: /frente al pelotón/ })).toBeVisible();
  await expect(panel.getByText('Exacta', { exact: true })).toHaveCount(0);
  await expect(panel.getByRole('button', { name: /no está a mano.*No disponible/ })).toBeVisible();
  // Se deja pasar la pausa de la búsqueda por significado y su fallo.
  await page.waitForTimeout(1_200);
  expect(await comandosPedidos(page)).toContain('semantic_search');
  await expect(panel.getByText('Por significado')).toHaveCount(0);
  await expect(panel.getByRole('listitem')).toHaveCount(2);
});

/**
 * Y lo léxico **no espera** a lo de significado: con una búsqueda por significado
 * que tarda tres segundos, los resultados exactos están antes.
 */
test('lo léxico no espera a la búsqueda por significado', async ({ page }) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbSignificadoLento = true;
  });
  await page.goto('/');
  await page.getByRole('searchbox', { name: 'Buscar' }).fill('soledad');
  const panel = page.getByRole('complementary', { name: 'Búsqueda' });
  await panel.getByRole('tab', { name: 'Dentro de los Libros' }).click();

  await expect(panel.getByRole('button', { name: /frente al pelotón/ })).toBeVisible({
    timeout: 2_000
  });
  await expect(panel.getByText('Por significado', { exact: true })).toBeVisible({
    timeout: 6_000
  });
});

/**
 * Sin quien genere, el lector no ofrece estudiar: la IA es opcional y no
 * se anuncia con un botón que abre un panel para decir que no hay IA.
 */
test('sin quien genere el lector no ofrece estudiar', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByRole('button', { name: 'Poner un Marcador (B)' })).toBeVisible();
  await expect(page.getByRole('button', { name: /Estudiar con IA/ })).toHaveCount(0);
  await page.keyboard.press('e');
  await expect(page.getByRole('complementary', { name: 'Estudiar con IA' })).toHaveCount(0);
});

/**
 * El catálogo lleva varios modelos que generan y la aplicación **lee el
 * equipo y recomienda uno**; quien lo tiene elige, y se usa uno a la vez. Lo decidió
 * Criterio del 2026-09-17: la máquina es lo único que no se puede igualar entre usuarios.
 */
/**
 * **Recomendar no es obligar** (2026-09-22). Todos los modelos se pueden
 * elegir, cada uno dice lo que es probable que pase en este equipo, y el que
 * probablemente cierra la aplicación **se ofrece con confirmación fuerte**: nada se
 * cambia antes de confirmarlo.
 */
test('un modelo que el equipo no aguanta se ofrece igual, y pide confirmación fuerte', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();
  await page.getByRole('button', { name: 'IA y Modelos' }).click();

  const grande = page.getByRole('article').filter({ hasText: 'Qwen3 4B' });
  await expect(grande.getByText('más memoria de la que tiene el equipo en total')).toBeVisible();
  await expect(grande.getByText('unos 5 minutos', { exact: false })).toBeVisible();
  await expect(grande.getByText('estimado a partir de lo medido', { exact: false })).toBeVisible();

  await grande.getByRole('button', { name: 'Usar Este' }).click();
  await expect(grande.getByText('Lo más probable es que el sistema cierre NB Books')).toBeVisible();
  // Nada cambia hasta confirmar.
  expect(
    ((await llamadasA(page, 'update_setting')) as Array<{ key: string }>).some(
      (l) => l.key === 'ai_model_local'
    )
  ).toBe(false);
  await grande.getByRole('button', { name: 'Usarlo Igualmente' }).click();
  expect(await llamadasA(page, 'update_setting')).toContainEqual({
    key: 'ai_model_local',
    value: 'qwen3-4b-q4km'
  });
});

/**
 * Con el modelo local, las preguntas de autoevaluación **se ofrecen desaconsejadas**:
 * aparecen, dicen por qué no se recomiendan y se pueden generar igual.
 */
test('las preguntas en local se ofrecen, dicen por qué no se recomiendan y se generan igual', async ({
  page
}) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbEstudio = {
      listo: true,
      externo: false,
      autorizado: false
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await page.getByRole('button', { name: 'Estudiar con IA (E)' }).click();
  const panel = page.getByRole('complementary', { name: 'Estudiar con IA' });
  await expect(panel.getByText('En este equipo tardará unos 2 minutos')).toBeVisible();

  await panel.getByRole('button', { name: 'Qué Hacer' }).click();
  await panel.getByRole('option', { name: 'Preguntas (no recomendada)' }).click();
  await expect(
    panel.getByText('las preguntas de opción múltiple salen mal a menudo', { exact: false })
  ).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Generar' })).toBeEnabled();
});

test('el modelo que genera se recomienda por la máquina y lo elige quien la tiene', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes' }).click();
  await page.getByRole('button', { name: 'IA y Modelos' }).click();

  const tarjeta = page.getByRole('article').filter({ hasText: 'Qwen2.5 1.5B Instruct' });
  await expect(tarjeta.getByText('Recomendado para este equipo.')).toBeVisible();
  // Dice para qué sirve y desde qué equipo se recomienda, antes de nada.
  await expect(tarjeta.getByText('Resúmenes, tarjetas y conceptos')).toBeVisible();
  await expect(tarjeta.getByText('de memoria y', { exact: false })).toBeVisible();
  // `first`: ahora el porqué de la recomendación también habla de
  // núcleos, y lo que se comprueba aquí es que el dato está, no cuántas veces.
  await expect(tarjeta.getByText('núcleos', { exact: false }).first()).toBeVisible();

  await tarjeta.getByRole('button', { name: 'Usar Este' }).click();
  expect(await llamadasA(page, 'update_setting')).toContainEqual({
    key: 'ai_model_local',
    value: 'qwen25-15b-instruct-q4km'
  });
  await expect(tarjeta.getByText('Es el que genera ahora.')).toBeVisible();
  await expect(tarjeta.getByRole('button', { name: 'Usar Este' })).toHaveCount(0);
});

/**
 * Quién genera se ve antes de generar; lo generado se pinta
 * como texto aunque traiga marcado con código; cada cita abre su fragmento; y lo
 * que no cita nada se marca como no verificable.
 */
test('estudiar un capítulo dice quién genera, pinta texto y marca lo no verificable', async ({
  page
}) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbEstudio = {
      listo: true,
      externo: false,
      autorizado: false
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await page.getByRole('button', { name: 'Estudiar con IA (E)' }).click();

  const panel = page.getByRole('complementary', { name: 'Estudiar con IA' });
  await expect(panel.getByText('Qwen2.5 1.5B Instruct', { exact: true })).toBeVisible();
  await expect(panel.getByText(/en este equipo: el texto no sale de él/)).toBeVisible();

  await panel.getByRole('button', { name: 'Generar' }).click();
  await expect(panel.getByText('El coronel recuerda el hielo.')).toBeVisible();
  // El marcado llega como los caracteres que son: ni imagen ni código ejecutado.
  await expect(panel.getByText('<img src=x onerror="window.__nbInyectado=1">')).toBeVisible();
  await expect(panel.locator('img')).toHaveCount(0);
  expect(await page.evaluate(() => (globalThis as Record<string, unknown>).__nbInyectado)).toBe(
    undefined
  );
  await expect(panel.getByText('No verificable')).toHaveCount(1);
  await expect(
    panel.getByText('citas no apuntaban a ningún fragmento', { exact: false })
  ).toBeVisible();

  await panel.getByRole('button', { name: 'Abrir el fragmento 2' }).first().click();
  await expect
    .poll(async () =>
      ((await llamadasA(page, 'save_progress')) as Array<{ locator: { href: string } }>).some(
        (llamada) => llamada.locator.href === 'OEBPS/c2.xhtml'
      )
    )
    .toBe(true);
});

/**
 * Marcar una salida como incorrecta la deja anotada **en este equipo**,
 * con la plantilla y el modelo que la hicieron, y se ve y se borra en Ajustes.
 *
 * Lo que se comprueba aquí y no en Rust: que la acción existe donde hace falta -al
 * lado de quién generó, sin competir con "guardar"- y que lo marcado **aparece**.
 * Un botón que anota en una tabla que nadie enseña es lo mismo que ningún botón.
 */
test('cancelar la IA se ve al momento y lo que llegue después no se enseña', async ({ page }) => {
  // En un procesador sin AVX2 el runtime tardaba minutos en soltar el modelo y
  // el panel seguía en "Generando" todo ese tiempo: parecía que cancelar no hacía
  // nada. Con el simulador lento, la respuesta llega 1,2 s después de cancelar.
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbEstudio = {
      listo: true,
      externo: false,
      autorizado: false,
      lento: true
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await page.getByRole('button', { name: 'Estudiar con IA (E)' }).click();
  const panel = page.getByRole('complementary', { name: 'Estudiar con IA' });
  await panel.getByRole('button', { name: 'Generar' }).click();
  await panel.getByRole('button', { name: 'Cancelar' }).click();
  await expect(panel.getByRole('button', { name: 'Generar' })).toBeVisible({ timeout: 300 });
  await page.waitForTimeout(1500);
  await expect(panel.getByText('El coronel recuerda el hielo.')).toHaveCount(0);
});

test('marcar una salida como incorrecta la deja anotada y visible en Ajustes', async ({ page }) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbEstudio = {
      listo: true,
      externo: false,
      autorizado: false
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await page.getByRole('button', { name: 'Estudiar con IA (E)' }).click();
  const panel = page.getByRole('complementary', { name: 'Estudiar con IA' });
  await panel.getByRole('button', { name: 'Generar' }).click();
  await expect(panel.getByText('El coronel recuerda el hielo.')).toBeVisible();

  await panel.getByRole('button', { name: 'Marcar como Incorrecta' }).click();
  await panel.getByRole('button', { name: 'Qué está mal' }).click();
  await panel.getByRole('option', { name: 'Se deja fuera lo importante' }).click();
  await panel.getByRole('textbox', { name: 'Algo que añadir (opcional)' }).fill('falta el final');
  await panel.getByRole('button', { name: 'Guardar el Aviso' }).click();
  await expect(panel.getByText('Se queda en este equipo', { exact: false })).toBeVisible();

  // Y lo marcado no se queda en una tabla que nadie enseña.
  const marcado = (await llamadasA(page, 'report_ai_output')) as Array<{
    report: { reason: string; note: string; editionId: string };
  }>;
  expect(marcado).toHaveLength(1);
  expect(marcado[0]?.report.reason).toBe('incompleto');
  expect(marcado[0]?.report.note).toBe('falta el final');

  // Cerrando el documento antes: con el lector abierto la navegación no está, que
  // es lo que pide la lectura sin distracciones.
  await page.getByRole('button', { name: 'Cerrar el Documento' }).click();
  await page.getByRole('button', { name: 'Ajustes' }).click();
  await page.getByRole('button', { name: 'IA y Modelos' }).click();
  await expect(page.getByText('Lo Marcado como Incorrecto')).toBeVisible();
  await expect(page.getByText('Incompleto', { exact: false }).first()).toBeVisible();
});

/**
 * Las tarjetas son una propuesta: se descartan antes de guardar, y lo
 * guardado aparece en Estudio con quién lo generó.
 */
test('las tarjetas se descartan antes de guardar y lo guardado llega a Estudio', async ({
  page
}) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbEstudio = {
      listo: true,
      externo: false,
      autorizado: false
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await page.getByRole('button', { name: 'Estudiar con IA (E)' }).click();
  const panel = page.getByRole('complementary', { name: 'Estudiar con IA' });
  await panel.getByRole('button', { name: 'Qué Hacer' }).click();
  await panel.getByRole('option', { name: 'Tarjetas' }).click();
  await panel.getByRole('button', { name: 'Generar' }).click();

  await expect(panel.getByText('elemento no cumplía el formato', { exact: false })).toBeVisible();
  await expect(panel.getByRole('textbox', { name: 'Pregunta' })).toHaveCount(2);
  await panel.getByRole('button', { name: 'Descartar' }).last().click();
  await expect(panel.getByRole('textbox', { name: 'Pregunta' })).toHaveCount(1);
  await panel.getByRole('button', { name: 'Guardar en Estudio' }).click();
  await expect(panel.getByText('Guardado en Estudio.')).toBeVisible();
  const guardadas = (await llamadasA(page, 'save_study_cards')) as Array<{
    cards: Array<{ provider: string; templateId: string }>;
  }>;
  expect(guardadas[0]?.cards).toHaveLength(1);
  expect(guardadas[0]?.cards[0]?.provider).toBe('local-generativo');

  await page.getByRole('button', { name: 'Estudio', exact: true }).click();
  await page.getByRole('button', { name: /^Tarjetas \d/ }).click();
  await expect(page.getByText('¿Qué recordó el coronel?')).toBeVisible();
  await expect(page.getByText('Generado por el modelo local', { exact: false })).toBeVisible();
  await expect(page.getByRole('tab', { name: 'Repasar hoy', exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Mostrar Respuesta' }).click();
  await expect(page.getByText('El hielo', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Bien' })).toBeVisible();
  await page.getByRole('button', { name: 'Bien' }).click();
  await expect(page.getByText('No hay tarjetas pendientes de repasar hoy.')).toBeVisible();
  await page.getByRole('tab', { name: 'Todas', exact: false }).click();
  await expect(page.getByText('¿Qué recordó el coronel?')).toBeVisible();
});

/**
 * Con un proveedor externo elegido, el panel
 * dice a dónde sale el texto, y sin autorizarlo no se genera: se pide el permiso.
 */
test('con un proveedor externo sin autorizar se dice a dónde sale y se pide permiso', async ({
  page
}) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbEstudio = {
      listo: true,
      externo: true,
      autorizado: false
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await page.getByRole('button', { name: 'Estudiar con IA (E)' }).click();
  const panel = page.getByRole('complementary', { name: 'Estudiar con IA' });
  await expect(panel.getByText(/El texto del capítulo sale hacia/)).toBeVisible();
  await expect(panel.getByTestId('estudio-estimacion-coste')).toHaveText(
    /Saldrán unos \d+ caracteres \(~\d+ tokens\)\./
  );
  await panel.getByRole('button', { name: 'Generar' }).click();
  await expect(panel.getByRole('button', { name: 'Autorizar y Generar' })).toBeVisible();
  await expect(panel.getByText(/Todavía no has autorizado enviar texto a/)).toBeVisible();
});

/**
 * Cambiar de alcance no enseña lo que contestó el otro, y la espera se ve.
 *
 * Regresión de lo encontrado el 2026-09-12 con la biblioteca real: al
 * pasar de "Títulos" a "Dentro de los Libros" seguían en pantalla los títulos
 * hasta que llegaba lo otro, y nada decía que se estaba buscando. La lista no
 * guardaba de qué pregunta era, así que no tenía forma de saber que ya no tocaba.
 *
 * El simulador contesta por título enseguida y dentro de los libros en segundo y
 * medio, que es la proporción medida. Con las dos al mismo ritmo el hueco en el
 * que se veía lo que no tocaba no llegaría a existir y la prueba no diría nada.
 */
test('cambiar de alcance no enseña lo del anterior y dice que está buscando', async ({ page }) => {
  await montarPuente(page, { busquedaLenta: true });
  await page.goto('/');

  await page.getByRole('searchbox', { name: 'Buscar' }).fill('soledad');
  const panel = page.getByRole('complementary', { name: 'Búsqueda' });
  await expect(panel.getByRole('button', { name: /Cien Años de Soledad/ })).toBeVisible();

  await panel.getByRole('tab', { name: 'Dentro de los Libros' }).click();

  // Lo del alcance anterior se va en el acto, sin esperar a la respuesta nueva.
  await expect(panel.getByRole('button', { name: /Cien Años de Soledad/ })).toHaveCount(0);
  // Y mientras tanto se dice que se está buscando.
  await expect(panel.getByRole('status', { name: 'Buscando' })).toBeVisible();
  await expect(panel.getByRole('button', { name: /frente al pelotón/ })).toBeVisible();
  await expect(panel.getByRole('status', { name: 'Buscando' })).toHaveCount(0);
});

/**
 * Y lo que se escribe no se busca letra a letra: con menos letras de las que el
 * índice puede contestar se dice cuántas faltan y no se pregunta al runtime.
 */
test('con menos letras de las necesarias no se busca y se dice por qué', async ({ page }) => {
  await page.goto('/');

  await page.getByRole('searchbox', { name: 'Buscar' }).fill('s');
  const panel = page.getByRole('complementary', { name: 'Búsqueda' });
  await expect(panel.getByText('Escribe al menos 2 letras para buscar.')).toBeVisible();
  // Se deja pasar más que la pausa: si fuera a preguntar, ya lo habría hecho.
  await page.waitForTimeout(600);
  expect(await comandosPedidos(page)).not.toContain('search');
});

/**
 * Un filtro que no lleva a ninguna obra no se ofrece.
 *
 * Observación de uso del 2026-09-12: "si no tiene ítems no se muestra". Con la biblioteca
 * sana, "Duplicados" y "No disponibles" son cero, y una ficha que al pulsarla
 * vacía la vista miente sobre lo que hay. La prueba mira las dos mitades: los
 * vacíos no están y los que tienen obras sí.
 */
test('los filtros sin obras no se ofrecen en la biblioteca', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();

  const barra = page.getByRole('toolbar', { name: 'Controles de la Biblioteca' });
  await expect(barra.getByRole('button', { name: 'Todos' })).toBeVisible();
  await expect(barra.getByRole('button', { name: 'Empezados' })).toBeVisible();
  await expect(barra.getByRole('button', { name: 'Duplicados' })).toHaveCount(0);
  await expect(barra.getByRole('button', { name: 'No disponibles' })).toHaveCount(0);
});

/**
 * "Duplicados" dice qué considera duplicado: el nombre del filtro solo no
 * lo dice, y antes significaba el mismo archivo dos veces.
 */
test('el filtro de duplicados dice que junta el mismo título y autor', async ({ page }) => {
  await montarPuente(page, { duplicados: 2 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();

  const barra = page.getByRole('toolbar', { name: 'Controles de la Biblioteca' });
  await expect(barra).not.toContainText('con el mismo título y autor');
  await barra.getByRole('button', { name: 'Duplicados' }).click();
  await expect(barra).toContainText('2 documentos con el mismo título y autor');
});

/**
 * Se llega a una página escribiéndola, y el campo no admite nada que no
 * lleve a una.
 */
test('ir a una página escribiendo su número, sin poder escribir nada fuera de la biblioteca', async ({
  page
}) => {
  await montarPuente(page, { obras: 1000 });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();

  const pie = page.getByRole('navigation', { name: 'Páginas de la Biblioteca' });
  const campo = pie.getByRole('textbox', { name: 'Ir a la Página' });
  await expect(campo).toBeVisible();

  // Letras y un número mayor que la última: se queda en la última.
  await campo.pressSequentially('x9a99');
  await expect(campo).toHaveValue('20');

  await campo.fill('');
  await campo.pressSequentially('7');
  await campo.press('Enter');
  await expect(campo).toHaveValue('');
  await expect(pie.getByRole('button', { name: '7', exact: true })).toHaveAttribute(
    'aria-current',
    'page'
  );
});

/**
 * Una tarjeta de la barra lateral se pliega a una línea, y se queda
 * plegada.
 *
 * Plegada no desaparece: sigue diciendo qué es y su cifra. Lo que se va es el
 * detalle, y la decisión se guarda como preferencia.
 */
test('una tarjeta de la barra lateral se pliega sin desaparecer y se recuerda', async ({
  page
}) => {
  await page.goto('/');
  const barra = page.getByRole('navigation', { name: 'Navegación Principal' });
  const tarjeta = barra.getByRole('button', { name: /Cien Años de Soledad/ });
  await expect(tarjeta).toBeVisible();
  await expect(tarjeta.getByText('Leyendo')).toBeVisible();

  await barra.getByRole('button', { name: 'Plegar la Tarjeta' }).first().click();

  await expect(tarjeta).toBeVisible();
  await expect(tarjeta.getByText('42 %')).toBeVisible();
  await expect(tarjeta.getByText('Leyendo')).toHaveCount(0);
  await expect(barra.getByRole('button', { name: 'Desplegar la Tarjeta' })).toBeVisible();

  const guardadas = (await argumentosDe(page, 'update_setting')) as Array<{
    key: string;
    value: string;
  }>;
  expect(guardadas).toContainEqual({ key: 'sidebar_folded_cards', value: 'leyendo' });
});

/**
 * Plegar la barra no se anima con un ancho, y con movimiento reducido
 * no se anima nada.
 *
 * Es la regla y no el reloj lo que se comprueba, por lo mismo que en
 * `rendimiento.spec.ts`: un tiempo de fotograma en una suite en paralelo es
 * ruido. Lo que hacía temblar la biblioteca era animar `width`, que recompone la
 * columna en cada fotograma, y eso sí se puede leer.
 */
test('plegar la barra lateral no anima su ancho', async ({ page }) => {
  await page.goto('/');
  const transicion = await page
    .locator('.nb-lateral')
    .evaluate((barra) => getComputedStyle(barra).transitionProperty);
  expect(transicion.split(',').map((p) => p.trim())).not.toContain('width');

  await page.getByRole('button', { name: 'Plegar la Barra' }).click();
  await expect(page.getByRole('button', { name: 'Desplegar la Barra' })).toBeVisible();
  // Terminado el cambio, la columna queda en su sitio y sin transformación.
  await expect
    .poll(() => page.locator('.nb-columna').evaluate((c) => getComputedStyle(c).transform))
    .toBe('none');
  await expect(page.locator('.nb-carcasa')).toHaveAttribute('data-plegada', 'true');
});

/**
 * Acerca de dice qué le pasa a cada componente por su motivo, no solo
 * por su estado.
 *
 * Observación de uso del 2026-09-12: "dice Detenido lo que no existe". `STOPPED` es lo mismo
 * para lo que se paró que para lo que ninguna versión ha traído, y lo que los
 * distingue es el motivo, que ya viajaba con el estado.
 */
test('acerca de no dice detenido de lo que todavía no existe', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'Acerca de', exact: true }).click();

  const fila = (nombre: string) => page.locator('.nb-fila', { hasText: nombre });
  // Lo que no está construido no se enseña: la institución se retiró del
  // plan, y un estado que nunca cambia no dice nada del runtime.
  await expect(fila('Sincronización')).toHaveCount(0);
  await expect(fila('Institución')).toHaveCount(0);
  await expect(fila('Inteligencia artificial')).toContainText('Sin activar');
  await expect(page.getByText('Detenido')).toHaveCount(0);
});

/**
 * Cuando el índice se para por el tope de disco, se dice por qué, con
 * las cifras, y se puede subir el tope desde ahí.
 *
 * Observación de uso del 2026-09-12: "no entiendo por qué para y, si para, por qué sigue
 * diciendo que está detenido". La tarjeta decía "En pausa" y Ajustes no decía
 * nada del disco.
 */
test('el índice parado por falta de sitio lo explica y deja subir el tope', async ({ page }) => {
  await montarPuente(page, { indiceSinSitio: true });
  await page.goto('/');

  const barra = page.getByRole('navigation', { name: 'Navegación Principal' });
  await expect(barra.getByText('Sin sitio')).toBeVisible();
  await barra.getByRole('button', { name: /Índice de Búsqueda/ }).click();

  await expect(page.getByText(/28\.552\s+de\s+152\.080\s+obras se pueden buscar/)).toBeVisible();
  await expect(page.getByText(/Se detuvo para no ocupar más disco del que elegiste/)).toBeVisible();

  const topes = page.getByRole('radiogroup', { name: 'Espacio para la búsqueda' });
  await expect(topes.getByRole('radio', { name: /^20 %/ })).toHaveAttribute('aria-checked', 'true');
  await topes.getByRole('radio', { name: /^40 %/ }).click();
  expect(await argumentosDe(page, 'update_setting')).toContainEqual({
    key: 'derived_budget',
    value: 40
  });
});

/**
 * Un subsistema que nadie ha pedido todavía no pide atención.
 *
 * Un componente detenido a propósito no puede publicarse como `NOT_INSTALLED`:
 * `salud.ts` convierte ese estado en un aviso **accionable**, y la barra lateral
 * ofrecería instalar algo que nadie puede traer.
 *
 * Es lo que prohíbe la clase "Ajeno" de `component-model.md`: lo que no está se
 * oculta, y lo que no tiene arreglo ni se anuncia.
 *
 * La prueba mira las dos mitades, porque una sola no distingue "está bien" de
 * "dejó de mirarse": con los siete componentes sanos no hay tarjeta, y con uno
 * roto de verdad sí la hay.
 */
test('un componente que nadie ha pedido todavía no pone un aviso de atención', async ({ page }) => {
  await page.goto('/');

  // La biblioteca ya pintada es la señal de que la carcasa terminó de montarse:
  // afirmar la ausencia de algo antes de eso pasaría por el motivo equivocado.
  await expect(page.getByRole('button', { name: 'Ajustes', exact: true })).toBeVisible();
  await expect(page.getByText('Algo necesita atención')).toBeHidden();
});

/**
 * La otra mitad: la tarjeta sigue apareciendo cuando hay algo que decir de
 * verdad. Sin esto, la prueba de arriba pasaría igual si alguien borrara el
 * aviso entero.
 */
test('un componente realmente caído sí pone el aviso de atención', async ({ page }) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbEstadosDeComponente = [
      { component: 'database', state: 'READY', reasonCode: null },
      { component: 'reader', state: 'FAILED', reasonCode: 'ENGINE_CRASH' },
      { component: 'ai', state: 'STOPPED', reasonCode: 'AI_DISABLED' }
    ];
  });
  await page.goto('/');

  await expect(page.getByText('Algo necesita atención')).toBeVisible();
});

/** Los comandos que la interfaz ha pedido al runtime, con sus argumentos. */
const llamadasA = (page: import('@playwright/test').Page, comando: string) =>
  page.evaluate(
    (nombre) =>
      (
        (globalThis as Record<string, unknown>).__nbLlamadas as Array<{
          comando: string;
          args: Record<string, unknown>;
        }>
      )
        .filter((l) => l.comando === nombre)
        .map((l) => l.args),
    comando
  );

/**
 * el usuario ve la licencia antes de
 * descargar.
 *
 * La prueba mira las dos mitades del criterio, y la segunda es la que importa:
 * que la licencia **esté** a la vista y que **nada** se haya pedido al runtime —ni
 * el permiso de red ni la descarga— hasta confirmar con ella delante. Una
 * interfaz que enseñara la licencia después de empezar pasaría la primera mitad.
 *
 * Y recorre la redirección como ocurre de verdad con Hugging Face: autorizar el
 * anfitrión del catálogo no autoriza su red de distribución, que pide su propio
 * permiso con su nombre antes de traer nada.
 */
test('la licencia de un modelo se ve antes de descargarlo, y nada sale hasta confirmar', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'IA y Modelos', exact: true }).click();

  const ficha = page.getByRole('article', { name: 'Granite Embedding 107M Multilingual' });
  await expect(ficha).toBeVisible();
  await expect(ficha.getByText('Apache-2.0')).toBeVisible();

  await ficha.getByRole('button', { name: 'Descargar', exact: true }).click();

  // La confirmación: licencia, de quién es, a dónde se sale y qué se envía.
  await expect(ficha.getByText('Antes de Descargar')).toBeVisible();
  await expect(ficha.getByText('huggingface.co')).toBeVisible();
  await expect(ficha.getByText(/nada de tu biblioteca/)).toBeVisible();
  expect(
    await llamadasA(page, 'allow_model_host'),
    'se autorizó un anfitrión sin confirmar'
  ).toHaveLength(0);
  expect(await llamadasA(page, 'install_ai_model'), 'se descargó sin confirmar').toHaveLength(0);

  await ficha.getByRole('button', { name: 'Autorizar y Descargar' }).click();

  // La red de distribución pide su permiso, con su nombre.
  await expect(ficha.getByText('La descarga sigue desde')).toBeVisible();
  await expect(ficha.getByText('cdn.ejemplo')).toBeVisible();
  expect(await llamadasA(page, 'allow_model_host')).toEqual([{ host: 'huggingface.co' }]);

  await ficha.getByRole('button', { name: 'Autorizar y Seguir' }).click();

  await expect(ficha.getByText('Instalado y verificado.')).toBeVisible();
  expect(await llamadasA(page, 'allow_model_host')).toEqual([
    { host: 'huggingface.co' },
    { host: 'cdn.ejemplo' }
  ]);
  expect(await llamadasA(page, 'install_ai_model')).toHaveLength(2);
});

/**
 * de dónde sale la voz son tres opciones, no un interruptor, y la de
 * aquí dentro **enseña lo suyo antes de descargar nada**: de quién es el modelo,
 * bajo qué licencia, cuánto pesa y que después no sale nada del equipo.
 *
 * Es la misma regla que los modelos de IA aplicada a la voz, y se comprueba igual:
 * pulsar "Descargar" no descarga, enseña.
 */
test('la voz de aquí dentro enseña licencia y tamaño, y no descarga hasta confirmar', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'Voz', exact: true }).click();

  // Las tres opciones, y la de en medio es la que no existía.
  await expect(page.getByRole('radio', { name: 'Del sistema' })).toBeVisible();
  await expect(page.getByRole('radio', { name: 'De un servicio' })).toBeVisible();
  await page.getByRole('radio', { name: 'De aquí dentro' }).click();

  const ficha = page.getByRole('article', { name: 'Kokoro 82M' });
  await expect(ficha).toBeVisible();
  await expect(ficha.getByText('Apache-2.0')).toBeVisible();
  await expect(ficha.getByText('hexgrad')).toBeVisible();
  await expect(ficha.getByText('Dora, Alex, Santa')).toBeVisible();

  await ficha.getByRole('button', { name: 'Descargar', exact: true }).click();
  await expect(ficha.getByText('huggingface.co')).toBeVisible();
  await expect(ficha.getByText(/no sale nada de este equipo/)).toBeVisible();
  expect(
    await llamadasA(page, 'install_ai_model'),
    'se descargó la voz sin confirmar'
  ).toHaveLength(0);

  await ficha.getByRole('button', { name: 'Autorizar y Descargar' }).click();
  expect(await llamadasA(page, 'install_ai_model')).toEqual([{ id: 'kokoro-82m' }]);
});

/**
 * Y con la voz instalada, la tarjeta dice **lo que esta máquina da**: el factor de
 * tiempo real medido. Es la diferencia entre "no se ofrece" y "aquí tarda esto".
 */
test('con la voz instalada, Ajustes enseña lo que tarda en esta máquina', async ({ page }) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbVozNeuronal = 'instalada';
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'Voz', exact: true }).click();
  await page.getByRole('radio', { name: 'De aquí dentro' }).click();

  const ficha = page.getByRole('article', { name: 'Kokoro 82M' });
  await expect(ficha.getByText(/0,4/)).toBeVisible();
  await expect(ficha.getByText('Lista: este equipo la mueve mientras lees.')).toBeVisible();
});

/**
 * el presupuesto de la IA se elige en la misma sección, y un tope de
 * memoria por debajo del modelo **lo dice en la ficha del modelo**, que es donde
 * alguien lo buscaría al ver que la IA no responde. Lo decide el runtime; la
 * interfaz no compara cifras. Y nunca se ofrece ocupar todos los núcleos.
 */
test('el tope de memoria de la IA se elige en Ajustes y avisa si el modelo no cabe', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'IA y Modelos', exact: true }).click();

  const ficha = page.getByRole('article', { name: 'Granite Embedding 107M Multilingual' });
  await expect(ficha).toBeVisible();
  await expect(ficha.getByText(/no se carga/)).toBeHidden();

  const hilos = page.getByRole('radiogroup', { name: 'Hilos' });
  await expect(hilos.getByRole('radio', { name: /Automático/ })).toHaveAttribute(
    'aria-checked',
    'true'
  );
  // Ocho núcleos: 1, 2 y 4, y nunca 8.
  await expect(hilos.getByRole('radio')).toHaveCount(4);

  const memoria = page.getByRole('radiogroup', { name: 'Memoria para un modelo' });
  await memoria.getByRole('radio', { name: '512 MB' }).click();

  expect(await llamadasA(page, 'update_setting')).toContainEqual({
    key: 'ai_memory_limit_mb',
    value: 512
  });
  await expect(ficha.getByText(/no se carga/)).toBeVisible();

  await memoria.getByRole('radio', { name: '1 GB' }).click();
  await expect(ficha.getByText(/no se carga/)).toBeHidden();
});

/**
 * el primer envío a un proveedor externo nuevo enseña qué
 * se va a enviar y exige consentimiento, y lo enviado queda registrado y se puede
 * leer entero.
 *
 * La mitad que importa es la de en medio: con el mensaje delante **no se ha
 * autorizado ni enviado nada** todavía. Y el proveedor activo se ve arriba de la
 * sección antes y después de configurarlo, que es la regla de `ai/privacy.md`.
 */
test('el primer envío a un proveedor externo enseña qué sale y queda registrado', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'IA y Modelos', exact: true }).click();

  const activo = page.getByText('Proveedor activo:');
  await expect(activo).toBeVisible();
  await expect(page.getByText(/solo este equipo/)).toBeVisible();

  await page
    .getByRole('radiogroup', { name: 'Proveedor', exact: true })
    .getByRole('radio', { name: 'Anthropic' })
    .click();
  const servidor = page.getByLabel('Servidor', { exact: true });
  await servidor.fill('api.anthropic.com');
  await servidor.press('Tab');
  const modelo = page.getByLabel('Modelo', { exact: true });
  await modelo.fill('claude-un-modelo');
  await modelo.press('Tab');

  await expect(page.getByText(/sale del equipo hacia ese servicio/)).toBeVisible();

  await page.getByRole('button', { name: 'Probar', exact: true }).click();

  const antes = page.getByRole('region', { name: 'Antes del Primer Envío' });
  await expect(antes).toBeVisible();
  await expect(antes.getByText('api.anthropic.com')).toBeVisible();
  await expect(antes.getByTestId('ia-tamano-envio-prueba')).toHaveText(
    /Tamaño del envío: \d+ caracteres \(~\d+ tokens\)\./
  );
  await expect(antes.locator('pre')).toContainText('claude-un-modelo');
  await expect(antes.locator('pre')).toContainText('prueba de conexión');
  expect(
    await llamadasA(page, 'allow_ai_provider'),
    'se autorizó sin enseñar qué salía'
  ).toHaveLength(0);
  await expect(page.getByText('No se ha enviado nada.')).toBeVisible();

  await antes.getByRole('button', { name: 'Autorizar y Enviar' }).click();

  await expect(page.getByText('Respondió:')).toBeVisible();
  expect(await llamadasA(page, 'allow_ai_provider')).toHaveLength(1);

  // Lo enviado, en el registro y entero.
  const envio = page.locator('.nb-envios li');
  await expect(envio).toHaveCount(1);
  await envio.locator('summary').click();
  await expect(envio.locator('pre')).toContainText('prueba de conexión');
});

/**
 * "Ahora no" no descarga ni autoriza nada: cambiar de opinión delante de la
 * licencia es una respuesta, no un clic perdido.
 */
test('cambiar de opinión delante de la licencia no descarga ni autoriza nada', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Ajustes', exact: true }).click();
  await page.getByRole('button', { name: 'IA y Modelos', exact: true }).click();

  const ficha = page.getByRole('article', { name: 'Granite Embedding 107M Multilingual' });
  await ficha.getByRole('button', { name: 'Descargar', exact: true }).click();
  await ficha.getByRole('button', { name: 'Ahora No' }).click();

  await expect(ficha.getByText('Antes de Descargar')).toBeHidden();
  await expect(ficha.getByRole('button', { name: 'Descargar', exact: true })).toBeVisible();
  expect(await llamadasA(page, 'allow_model_host')).toHaveLength(0);
  expect(await llamadasA(page, 'install_ai_model')).toHaveLength(0);
});

/**
 * **Preparar el capítulo, revisado el 2026-09-22.** Hasta ese día ningún recorrido
 * abría este panel: el simulador no tenía sus comandos. Y tenía cuatro defectos que
 * ninguna prueba podía ver.
 */
async function abrirLaVozNeuronal(
  page: Page,
  audio: Record<string, unknown>,
  voz: 'instalada' | 'lenta' = 'instalada'
) {
  await page.addInitScript(
    ({ estado, voz }) => {
      const global = globalThis as Record<string, unknown>;
      global.__nbProveedorDeVoz = 'neural';
      global.__nbVozNeuronal = voz;
      global.__nbAudio = estado;
    },
    { estado: audio, voz }
  );
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('v');
  const panel = page.getByRole('complementary', { name: 'Lectura en Voz Alta' });
  await expect(panel).toBeVisible();
  return panel;
}

// Aparecía solo si antes se había abierto Estudiar con IA, porque la posición se
// tomaba al abrir ese panel y no este.
test('preparar el capítulo aparece al abrir la voz, sin pasar antes por Estudiar', async ({
  page
}) => {
  const panel = await abrirLaVozNeuronal(page, {});
  await expect(panel.getByRole('heading', { name: 'Preparar Este Capítulo' })).toBeVisible();
  await expect(panel.getByText('Durará')).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Preparar', exact: true })).toBeEnabled();
});

// Una preparación que fallaba volvía al botón sin decir nada.
test('si la preparación falla, el panel dice que se detuvo y por qué', async ({ page }) => {
  const panel = await abrirLaVozNeuronal(page, { falla: 'INTERNAL' });
  await panel.getByRole('button', { name: 'Preparar', exact: true }).click();
  await expect(panel.getByText('La preparación se detuvo', { exact: false })).toBeVisible();
});

// Las marcas se guardaban y solo se contaban: ni se seguía el texto ni se podía
// empezar donde se iba leyendo. Y el audio vivía en el panel, que es momentáneo.
test('lo preparado suena desde donde voy y el lector sigue el texto, también con el panel cerrado', async ({
  page
}) => {
  const panel = await abrirLaVozNeuronal(page, { capitulo: true });
  await expect(panel.getByText('Preparado', { exact: false }).first()).toBeVisible();
  await panel.getByRole('button', { name: 'Escuchar Desde Donde Voy' }).click();
  await expect(panel.getByRole('button', { name: 'Pausar' })).toBeVisible();

  // Se cierra el panel: el audio es de la página y tiene que seguir sonando.
  await page.keyboard.press('Escape');
  await expect(panel).toBeHidden();

  // Al segundo empieza el fragmento del capítulo dos, y el lector va ahí.
  await expect
    .poll(
      async () =>
        ((await llamadasA(page, 'save_progress')) as Array<{ locator: { href: string } }>).some(
          (llamada) => llamada.locator.href === 'OEBPS/c2.xhtml'
        ),
      { timeout: 6000 }
    )
    .toBe(true);
});

// El libro entero preparado no se podía escuchar en ningún sitio.
test('el libro entero preparado se escucha desde el panel de voz del lector', async ({ page }) => {
  const panel = await abrirLaVozNeuronal(page, { libro: true });
  await expect(panel.getByText('El libro entero está preparado')).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Escuchar Desde Donde Voy' })).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Quitar el Libro Preparado' })).toBeVisible();
});

// **La narración en vivo con la voz neuronal nunca se había ejercido en un
// recorrido**: el simulador no tenía `synthesize_neural_speech`. Y en un equipo
// lento, ahora se ofrece igual avisando de que se cortará.
test('la voz neuronal narra en vivo, y en un equipo lento avisa y deja narrar igual', async ({
  page
}) => {
  const panel = await abrirLaVozNeuronal(page, {}, 'lenta');
  await expect(panel.getByText('en vivo se cortará', { exact: false })).toBeVisible();
  await expect(panel.getByText('preparar el capítulo antes', { exact: false })).toBeVisible();

  await panel.getByRole('button', { name: 'Leer en Voz Alta' }).click();
  await expect
    .poll(async () => ((await llamadasA(page, 'synthesize_neural_speech')) as unknown[]).length)
    .toBeGreaterThan(0);
  const primero = (
    (await llamadasA(page, 'synthesize_neural_speech')) as Array<{ text: string }>
  )[0];
  expect(primero?.text.length ?? 0).toBeGreaterThan(0);
});

// **El panel de audiolibros no tenía ningún recorrido.** Se prepara el libro que la
// biblioteca tiene elegido, con su coste delante, y lo que pasa se dice.
test('el panel de audiolibros prepara el libro elegido con el coste delante y dice cómo terminó', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page
    .getByRole('button', { name: /Pedro Páramo/ })
    .first()
    .click();
  await page
    .getByRole('button', { name: /Audiolibros/ })
    .first()
    .click();

  await expect(page.getByText('Durará')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Escuchar una Muestra' })).toBeVisible();
  await page.getByRole('button', { name: 'Preparar', exact: true }).click();
  await expect(page.getByText('escúchalo desde el panel de voz', { exact: false })).toBeVisible();
});

/**
 * Lo preparado dice con qué voz y modelo se hizo.
 * Si no coincide con la voz elegida se marca y se ofrece regenerarlo sin hacerlo solo.
 */
test('lo preparado dice con qué voz y modelo se hizo, y avisa si la voz o modelo cambiaron', async ({
  page
}) => {
  // 1. En el lector, con audio preparado cuya voz no coincide:
  const panel = await abrirLaVozNeuronal(page, {
    capitulo: true,
    voz: 'otra_voz',
    modelRev: 'rev-0',
    matchesCurrent: false
  });
  await expect(panel.getByText('Preparado con voz otra_voz (modelo rev-0)')).toBeVisible();
  await expect(panel.getByText('La voz o el modelo elegidos han cambiado')).toBeVisible();
  const botonRegenerar = panel.getByRole('button', { name: 'Regenerar con la Voz Actual' });
  await expect(botonRegenerar).toBeVisible();

  // No se regenera solo; al pulsar el botón se inicia la preparación:
  await botonRegenerar.click();
  await expect
    .poll(async () => ((await llamadasA(page, 'start_audio')) as unknown[]).length)
    .toBeGreaterThan(0);
});

test('el panel de audiolibros dice con qué voz se preparó y ofrece regenerar si cambió', async ({
  page
}) => {
  await page.addInitScript(() => {
    const global = globalThis as Record<string, unknown>;
    global.__nbAudio = {
      libro: true,
      voz: 'voz_antigua',
      modelRev: 'rev-vieja',
      matchesCurrent: false
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page
    .getByRole('button', { name: /Pedro Páramo/ })
    .first()
    .click();
  await page
    .getByRole('button', { name: /Audiolibros/ })
    .first()
    .click();

  await expect(page.getByText('El libro entero está preparado')).toBeVisible();
  await expect(page.getByText('Preparado con voz voz_antigua (modelo rev-vieja)')).toBeVisible();
  await expect(page.getByText('La voz o el modelo elegidos han cambiado')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Regenerar con la Voz Actual' })).toBeVisible();
});

// **Cerrar el panel mientras genera no pierde lo generado.** El panel dice "puedes
// seguir leyendo" mientras el modelo local trabaja, y seguir leyendo es tocar el
// texto, que lo cierra. Antes el resultado moría con el panel.
test('cerrar el panel de estudio mientras genera no pierde lo generado', async ({ page }) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbEstudio = {
      listo: true,
      externo: false,
      autorizado: false,
      lento: true
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await page.getByRole('button', { name: 'Estudiar con IA (E)' }).click();
  const panel = page.getByRole('complementary', { name: 'Estudiar con IA' });
  await panel.getByRole('button', { name: 'Generar' }).click();
  await expect(panel.getByText('Puedes seguir leyendo', { exact: false })).toBeVisible();

  // Se sigue leyendo: el panel se cierra.
  await page.keyboard.press('Escape');
  await expect(panel).toBeHidden();

  // Termina con el panel cerrado, y se avisa.
  await expect(page.getByText('ya está: ábrelo para verlo', { exact: false })).toBeVisible();

  // Y al volver a abrirlo, lo generado está.
  await page.getByRole('button', { name: 'Estudiar con IA (E)' }).click();
  await expect(panel.getByText('El coronel recuerda el hielo.')).toBeVisible();
});

/*
 * # Estado vivo
 *
 * Los principios 1 y 2 de la sección 7 de la filosofía de producto: ningún cambio
 * exige reiniciar, y un dato que se ve en dos sitios cambia en los dos a la vez.
 * Antes la tabla de esa sección decía "nada todavía" en la fila que
 * los hace cumplir. Estos recorridos son la mitad de la interfaz del mecanismo; la
 * del runtime -que quien escribe lo anuncia- está en `src-tauri/tests/estado_vivo.rs`.
 *
 * Cada recorrido deja una marca en la página al empezar y comprueba al terminar
 * que sigue ahí: una recarga la borraría, así que "se ve sin recargar" no se da por
 * supuesto.
 */
async function marcarLaPagina(page: Page) {
  await page.evaluate(() => {
    (globalThis as Record<string, unknown>).__nbSinRecargar = 'la misma página';
  });
}

async function sigueSiendoLaMismaPagina(page: Page) {
  expect(
    await page.evaluate(() => (globalThis as Record<string, unknown>).__nbSinRecargar),
    'nada de esto puede haber recargado la página'
  ).toBe('la misma página');
}

test('ningún cambio exige reiniciar ni recargar para verse', async ({ page }) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbSinTemaDeExtension = true;
  });
  await montarPuente(page, { conFuenteDeExtension: true });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Cien Años de Soledad' })).toBeVisible();
  await marcarLaPagina(page);

  // 1. Un ajuste cambiado: la escala de la interfaz.
  await page.getByRole('button', { name: 'Ajustes' }).click();
  await page.getByRole('button', { name: 'Accesibilidad', exact: true }).click();
  const escalaAntes = await page.evaluate(() =>
    getComputedStyle(document.documentElement).getPropertyValue('--nb-font-scale').trim()
  );
  await page.getByRole('slider', { name: 'Escala de la interfaz' }).focus();
  await page.keyboard.press('ArrowRight');
  await expect
    .poll(() =>
      page.evaluate(() =>
        getComputedStyle(document.documentElement).getPropertyValue('--nb-font-scale').trim()
      )
    )
    .not.toBe(escalaAntes);

  // 2. Un tema aplicado.
  await page.getByRole('button', { name: 'Apariencia', exact: true }).click();
  await page.getByRole('radio', { name: 'Grafito' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'grafito');

  // 3. Una extensión instalada y retirada: lo que aporta aparece y desaparece.
  await expect(page.getByRole('radio', { name: /Crepúsculo/ })).toHaveCount(0);
  await page.getByRole('button', { name: 'Extensiones', exact: true }).click();
  await page.getByRole('button', { name: 'Instalar una Extensión' }).click();
  await page.getByRole('button', { name: 'Apariencia', exact: true }).click();
  await expect(page.getByRole('radio', { name: /Crepúsculo/ })).toBeVisible();
  await page.getByRole('button', { name: 'Extensiones', exact: true }).click();
  await page
    .locator('li.nb-fila', { hasText: 'Tema Crepúsculo' })
    .getByRole('button', { name: 'Desinstalar' })
    .click();
  await page.getByRole('button', { name: 'Apariencia', exact: true }).click();
  await expect(page.getByRole('radio', { name: /Crepúsculo/ })).toHaveCount(0);

  // 4. Un metadato corregido: la rejilla, la ficha e Inicio dicen el título nuevo.
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page.locator('.nb-tarjeta', { hasText: 'Cien Años de Soledad' }).click();
  const ficha = page.getByRole('complementary', { name: 'Ficha' });
  await ficha.getByRole('button', { name: 'Completar Desde un Catálogo' }).click();
  await ficha.getByRole('button', { name: /Sudamericana/ }).click();
  // Con las mayúsculas exactas: el título nuevo solo se distingue del de antes en
  // ellas, y Playwright compara sin distinguirlas si no se le pide.
  await expect(
    page.locator('.nb-tarjeta .nb-titulo', { hasText: /^Cien años de soledad$/ })
  ).toBeVisible();
  await expect(
    ficha.getByRole('heading', { name: 'Cien años de soledad', exact: true })
  ).toBeVisible();
  await page.getByRole('button', { name: 'Inicio', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Cien años de soledad', exact: true })
  ).toBeVisible();

  // 5. Un documento importado: aparece en la biblioteca sin volver a entrar.
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await expect(page.locator('.nb-tarjeta', { hasText: 'Documento Importado 1' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Importar Documentos', exact: true }).click();
  await page.getByRole('button', { name: 'Elegir Documentos' }).click();
  await expect(page.locator('.nb-tarjeta', { hasText: 'Documento Importado 1' })).toBeVisible();

  // 6. Una colección creada: el menú la tiene, y la ficha la ofrece.
  await page.getByRole('button', { name: 'Colecciones', exact: true }).click();
  await page.getByRole('button', { name: 'Nueva Colección' }).click();
  await page.getByLabel('Nombre de la Colección').fill('Lecturas de Verano');
  await page.getByRole('button', { name: 'Crear', exact: true }).click();
  await expect(page.locator('.nb-coleccion', { hasText: 'Lecturas de Verano' })).toBeVisible();
  await page.keyboard.press('Escape');
  await page.locator('.nb-tarjeta', { hasText: 'Cien años de soledad' }).click();
  await expect(
    ficha.getByRole('button', { name: 'Lecturas de Verano', exact: true })
  ).toBeVisible();

  await sigueSiendoLaMismaPagina(page);
});

/** El porcentaje que pinta el lector, tal y como lo lee un lector de pantalla. */
async function porcentajeDelLector(page: Page): Promise<number> {
  const valor = await page
    .getByRole('progressbar', { name: 'Progreso de la lectura' })
    .getAttribute('aria-valuenow');
  return Number(valor);
}

/** Baja por el capítulo hasta una fracción de su alto, como quien lee. */
async function leerHasta(page: Page, fraccion: number) {
  await page.locator('.nb-superficie').evaluate((superficie, f) => {
    superficie.scrollTop = (superficie.scrollHeight - superficie.clientHeight) * f;
  }, fraccion);
}

test('avanzar en el lector mueve la cifra en la biblioteca y en Inicio sin cerrarlo', async ({
  page
}) => {
  await montarPuente(page, { parrafos: 60 });
  await page.goto('/');
  await marcarLaPagina(page);
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();

  await leerHasta(page, 0.5);
  await expect.poll(() => porcentajeDelLector(page)).toBeGreaterThan(10);
  const enElLector = await porcentajeDelLector(page);

  // La biblioteca, con el lector abierto: la ficha dice la misma cifra.
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page.locator('.nb-tarjeta', { hasText: 'Cien Años de Soledad' }).click();
  const ficha = page.getByRole('complementary', { name: 'Ficha' });
  await expect(ficha.getByText(`${enElLector} %`)).toBeVisible();
  // Y lo que la base sabe de la obra, que la ficha no enseñaba.
  await expect(ficha.getByText('Editorial de la Ficha')).toBeVisible();
  await expect(ficha.getByText('Realismo mágico', { exact: false })).toBeVisible();
  await expect(ficha.getByText(/EPUB · /)).toBeVisible();

  // Inicio, igual.
  await page.getByRole('button', { name: 'Inicio', exact: true }).click();
  await expect(page.locator('.nb-marquesina')).toContainText(`${enElLector} %`);

  // Y el lector sigue abierto donde se dejó, no donde estaba al abrir el libro.
  await page.getByRole('button', { name: 'Lector', exact: true }).click();
  await expect.poll(() => porcentajeDelLector(page)).toBe(enElLector);
  expect((await comandosPedidos(page)).filter((c) => c === 'open_document')).toHaveLength(1);

  await sigueSiendoLaMismaPagina(page);
});

/** El primer párrafo que se ve en la superficie del lector. */
async function primerParrafoVisible(page: Page): Promise<string> {
  return page.locator('.nb-superficie').evaluate((superficie) => {
    const marco = superficie.getBoundingClientRect();
    const p = Array.from(superficie.querySelectorAll('p')).find((el) => {
      const caja = el.getBoundingClientRect();
      return caja.bottom > marco.top && caja.top < marco.bottom;
    });
    return (p?.textContent ?? '').slice(0, 60);
  });
}

test('cerrar con la cruz y reabrir vuelve al párrafo por el que se iba', async ({ page }) => {
  /*
   * Dos defectos a la vez, los dos de "ninguna pérdida". Cerrar con la cruz no
   * pasaba por `salir`, el único que apuntaba la posición, y si se cerraba antes
   * del reposo del desplazamiento la posición se perdía. Y aunque se guardara,
   * reabrir pintaba el capítulo desde arriba: nadie llevaba la vista al locator.
   */
  await montarPuente(page, { parrafos: 60 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();

  await leerHasta(page, 0.6);
  const antes = await primerParrafoVisible(page);
  expect(antes).not.toContain('Muchos años después');

  // Se cierra enseguida, sin esperar al reposo del desplazamiento.
  await page.getByRole('button', { name: 'Cerrar el Documento' }).click();
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();
  await expect.poll(() => primerParrafoVisible(page)).toBe(antes);
});

test('terminar un libro lo saca de lo empezado en Inicio sin cerrar el lector', async ({
  page
}) => {
  await montarPuente(page, { parrafos: 60 });
  await page.goto('/');
  await marcarLaPagina(page);
  await expect(page.locator('.nb-marquesina')).toContainText('Cien Años de Soledad');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();

  // Hasta el último párrafo: el libro queda terminado.
  await leerHasta(page, 1);
  await expect.poll(() => porcentajeDelLector(page)).toBeGreaterThanOrEqual(99);

  await page.getByRole('button', { name: 'Inicio', exact: true }).click();
  // Lo que se retoma ya no es este libro: el siguiente empezado ocupa su sitio.
  await expect(page.locator('.nb-marquesina')).not.toContainText('Cien Años de Soledad');
  await expect(page.locator('.nb-marquesina')).toContainText('Rayuela');

  // Y la ficha lo dice con su palabra, no con un número.
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page.locator('.nb-tarjeta', { hasText: 'Cien Años de Soledad' }).click();
  await expect(
    page.getByRole('complementary', { name: 'Ficha' }).getByText('Terminado')
  ).toBeVisible();

  await sigueSiendoLaMismaPagina(page);
});

test('añadir una obra a una colección cambia su cuenta en el menú sin volver a abrirlo', async ({
  page
}) => {
  await page.goto('/');
  await marcarLaPagina(page);
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();

  await page.getByRole('button', { name: 'Colecciones', exact: true }).click();
  const relecturas = page.locator('.nb-coleccion', { hasText: 'Relecturas' });
  await expect(relecturas.locator('.nb-coleccion-cuenta')).toHaveText('0');
  await page.keyboard.press('Escape');

  await page.locator('.nb-tarjeta', { hasText: 'Pedro Páramo' }).click();
  const ficha = page.getByRole('complementary', { name: 'Ficha' });
  await ficha.getByRole('button', { name: 'Relecturas' }).click();
  await expect(ficha.getByRole('button', { name: 'Relecturas' })).toHaveAttribute(
    'aria-pressed',
    'true'
  );

  await page.getByRole('button', { name: 'Colecciones', exact: true }).click();
  await expect(relecturas.locator('.nb-coleccion-cuenta')).toHaveText('1');

  await sigueSiendoLaMismaPagina(page);
});

test('lo que aparece en una carpeta de la biblioteca se ve al volver a la ventana', async ({
  page
}) => {
  await page.goto('/');
  await marcarLaPagina(page);
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  const nueva = page.locator('.nb-tarjeta', { hasText: 'Documento que Apareció en la Carpeta' });
  await expect(page.locator('.nb-tarjeta').first()).toBeVisible();
  await expect(nueva).toHaveCount(0);

  // Alguien copia un libro en la carpeta con el explorador de archivos y vuelve.
  await page.evaluate(() => {
    (globalThis as Record<string, unknown>).__nbAparecioEnLaCarpeta = true;
    window.dispatchEvent(new Event('focus'));
  });
  await expect(nueva).toBeVisible();
  expect(await comandosPedidos(page)).toContain('rescan_roots');

  await sigueSiendoLaMismaPagina(page);
});

/*
 * # Las fórmulas se leen
 *
 * El marcado es **la salida del saneador del runtime**, no una copia: la fija
 * `src-tauri/tests/epub_lector.rs` contra el mismo archivo, así que si el saneador
 * cambia lo que entrega, esa prueba falla antes que esta.
 */
function capituloConFormulas(): string {
  const saneado = readFileSync(
    new URL('../src-tauri/tests/fixtures/formula.saneada.html', import.meta.url),
    'utf8'
  );
  return /<body>([\s\S]*)<\/body>/.exec(saneado)?.[1] ?? '';
}

test('una fórmula de un EPUB se pinta como fórmula y no como letras sueltas', async ({ page }) => {
  await montarPuente(page, { capitulo: capituloConFormulas() });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByRole('heading', { name: 'Ecuaciones de segundo grado' })).toBeVisible();

  const formula = await page
    .locator('.nb-texto math[display="block"]')
    .first()
    .evaluate((m) => {
      const fraccion = m.querySelector('mfrac');
      const [arriba, abajo] = fraccion ? Array.from(fraccion.children) : [];
      const cajaArriba = arriba?.getBoundingClientRect();
      const cajaAbajo = abajo?.getBoundingClientRect();
      return {
        espacio: m.namespaceURI,
        apilada: Boolean(cajaArriba && cajaAbajo && cajaArriba.bottom <= cajaAbajo.top + 1),
        // La raíz se dibuja con su signo: la caja es más ancha que lo que lleva dentro.
        raiz: (() => {
          const raiz = m.querySelector('msqrt');
          const dentro = raiz?.firstElementChild?.getBoundingClientRect();
          const fuera = raiz?.getBoundingClientRect();
          return Boolean(dentro && fuera && fuera.width > dentro.width);
        })()
      };
    });

  // Es MathML de verdad para el navegador, no etiquetas desconocidas de HTML.
  expect(formula.espacio).toBe('http://www.w3.org/1998/Math/MathML');
  // Y se pinta como una fórmula: el numerador encima del denominador.
  expect(formula.apilada, 'el numerador va encima del denominador').toBe(true);
  expect(formula.raiz, 'la raíz lleva su signo').toBe(true);
  // La anotación en TeX no se enseña: acompaña a la fórmula sin pintarse.
  await expect(page.getByText('\\frac{-b', { exact: false })).toBeHidden();
});

test('la lectura biónica no se mete dentro de una fórmula', async ({ page }) => {
  await montarPuente(page, { capitulo: capituloConFormulas() });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByRole('heading', { name: 'Ecuaciones de segundo grado' })).toBeVisible();
  await page.getByRole('button', { name: /Modo de Lectura/ }).click();
  await page.getByRole('switch', { name: 'Lectura biónica' }).click();

  await expect(page.locator('.nb-texto p b[data-nb-bionica]').first()).toBeVisible();
  expect(await page.locator('.nb-texto math b').count()).toBe(0);
});

/*
 * # Las ilustraciones vectoriales se ven, y lo que no se pinta se dice
 *
 * El marcado vuelve a ser la salida del saneador que fija `tests/epub_lector.rs`,
 * con la imagen del dibujo ya extraída a la caché, que es lo que hace el runtime
 * antes de entregarlo.
 */
function capituloConIlustracion(): string {
  const saneado = readFileSync(
    new URL('../src-tauri/tests/fixtures/ilustracion.saneada.html', import.meta.url),
    'utf8'
  );
  const cuerpo = /<body>([\s\S]*)<\/body>/.exec(saneado)?.[1] ?? '';
  return cuerpo.replace('href="imagenes/portada.jpg"', 'href="/tmp/nb-books/recursos/portada.jpg"');
}

test('una ilustración vectorial de un EPUB se pinta, con su imagen por el protocolo de assets', async ({
  page
}) => {
  await montarPuente(page, { capitulo: capituloConIlustracion() });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByRole('heading', { name: 'El ciclo del agua' })).toBeVisible();

  const dibujo = await page
    .locator('.nb-texto svg')
    .first()
    .evaluate((svg) => {
      const caja = (selector: string) => svg.querySelector(selector)?.getBoundingClientRect();
      return {
        espacio: svg.namespaceURI,
        // El analizador de HTML le devuelve a cada nombre su forma de SVG.
        degradado: svg.querySelector('defs')?.firstElementChild?.localName ?? null,
        fondo: (caja('rect')?.width ?? 0) > 0 && (caja('rect')?.height ?? 0) > 0,
        trazo: (caja('path[marker-end]')?.width ?? 0) > 0
      };
    });
  expect(dibujo.espacio).toBe('http://www.w3.org/2000/svg');
  expect(dibujo.degradado).toBe('linearGradient');
  expect(dibujo.fondo, 'el rectángulo del cielo tiene caja').toBe(true);
  expect(dibujo.trazo, 'el trazo de la evaporación tiene caja').toBe(true);
  await expect(page.locator('.nb-texto svg text', { hasText: 'Mar' })).toBeVisible();

  // La portada compuesta como dibujo: su imagen se pide por el protocolo de
  // assets, no al servidor de la aplicación.
  const imagen = await page.locator('.nb-texto svg image').getAttribute('href');
  expect(imagen).toContain('asset.localhost');
});

test('lo que el capítulo trae y el lector no muestra se dice', async ({ page }) => {
  await montarPuente(page, { omitidos: ['audio', 'video', 'form'] });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await expect(
    page.getByText('Este capítulo trae audio, vídeo y un formulario que el lector no muestra.')
  ).toBeVisible();
});

test('un capítulo sin nada que omitir no dice nada', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await expect(page.getByText('que el lector no muestra', { exact: false })).toHaveCount(0);
});

/*
 * # La narración que trae el propio libro
 *
 * El criterio: el audio suena anclado por Locator, el bloque que suena se resalta
 * con el mismo mecanismo que la voz, y la posición se guarda. Que sobreviva a
 * cerrar y reabrir lo comprueba `tests/narracion_del_libro.rs` contra la base.
 */
test('la narración del libro suena siguiendo el texto y resalta el párrafo que lee', async ({
  page
}) => {
  await montarPuente(page, { narrado: true });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('v');
  const panel = page.getByRole('complementary', { name: 'Lectura en Voz Alta' });
  const narracion = panel.getByRole('region', { name: 'La Narración del Libro' });
  await expect(narracion).toBeVisible();

  await narracion.getByRole('button', { name: 'Escuchar Desde Donde Voy' }).click();
  await expect(narracion.getByRole('button', { name: 'Pausar' })).toBeVisible();

  // Sigue con el panel cerrado: el audio es de la página.
  await page.keyboard.press('Escape');
  await expect(panel).toBeHidden();

  // Al segundo empieza el tramo del segundo párrafo: se resalta **el bloque**, con
  // el mismo atributo que pone la voz en vivo, y la lectura se apunta ahí.
  const segundo = page.locator('.nb-texto [data-nb-bloque="1"]');
  await expect(segundo).toHaveAttribute('data-nb-narrado', 'true', { timeout: 6000 });
  await expect(page.locator('.nb-texto [data-nb-bloque="0"]')).toHaveAttribute(
    'data-nb-narrado',
    'false'
  );
  const apuntadas = (await llamadasA(page, 'save_progress')) as Array<{
    locator: { locations: { fragments: string[] } };
  }>;
  const inicioDelSegundo = 'Muchos años después, frente al pelotón de fusilamiento.'.length + 1;
  expect(
    apuntadas.some((l) => l.locator.locations.fragments[0] === `#char=${inicioDelSegundo}`)
  ).toBe(true);

  // Al acabar el audio se busca el capítulo siguiente que traiga narración; este
  // libro solo la trae en uno, así que se dice que se acabó.
  await page.keyboard.press('v');
  await expect(narracion.getByText('Se acabó la narración del libro.')).toBeVisible({
    timeout: 6000
  });
});

test('sin narración propia, el panel de voz es el de siempre', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('v');
  const panel = page.getByRole('complementary', { name: 'Lectura en Voz Alta' });
  await expect(panel).toBeVisible();
  await expect(panel.getByRole('region', { name: 'La Narración del Libro' })).toHaveCount(0);
  expect(await comandosPedidos(page)).not.toContain('synchronized_audio');
});

/*
 * # El mapa del libro
 */
test('el mapa del libro está en el índice y cada marca lleva a su sitio', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('i');

  const mapa = page.getByRole('region', { name: 'Mapa del Libro' });
  await expect(mapa).toBeVisible();
  // Se abre desde el lector y sin salir de él: el lector sigue ahí debajo.
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  const marcas = mapa.getByRole('toolbar', { name: 'Anotaciones' }).getByRole('button');
  // Las cinco que anclan, en cuatro puntos del libro; la perdida no se pinta.
  await expect(marcas).toHaveCount(4);
  await expect(mapa.getByRole('button', { name: '1 marcador' })).toBeVisible();

  // Con el teclado: un solo punto de tabulación por fila, y las flechas la recorren.
  await marcas.first().focus();
  await page.keyboard.press('ArrowRight');
  await expect(marcas.nth(1)).toBeFocused();

  // Y cada marca lleva a su sitio **por Locator**: el marcador del tercer párrafo.
  await mapa.getByRole('button', { name: '1 marcador' }).click();
  await expect
    .poll(async () =>
      (
        (await llamadasA(page, 'save_progress')) as Array<{
          locator: { locations: { fragments: string[] } };
        }>
      ).some((l) => l.locator.locations.fragments[0] === '#char=113')
    )
    .toBe(true);
});

test('el mapa no crece con lo anotado: mil seiscientas marcas caben en el mismo sitio', async ({
  page
}) => {
  await montarPuente(page, { anotacionesExtra: 1600 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('i');
  const mapa = page.getByRole('region', { name: 'Mapa del Libro' });
  await expect(mapa).toBeVisible();

  const nodos = await mapa.evaluate((m) => m.querySelectorAll('*').length);
  // Cien grupos de marcas, los tramos de capítulo y unos pocos de adorno.
  expect(nodos, `el mapa pintó ${nodos} nodos`).toBeLessThanOrEqual(130);
  // Y no se perdió ninguna: la última del libro también está, en su grupo.
  const ultima = mapa.getByRole('toolbar', { name: 'Anotaciones' }).getByRole('button').last();
  await expect(ultima).toHaveAttribute('aria-label', /resaltados/);
});

/*
 * # Los nombres del libro
 */
test('los nombres del libro se calculan al desplegarlos y cada sitio lleva a su posición', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByText('Macondo era entonces')).toBeVisible();
  await page.keyboard.press('i');
  const nombres = page.locator('details.nb-nombres');
  await expect(nombres).toBeVisible();
  // Abrir el índice no recorre el libro: quien va al capítulo tres no lo pidió.
  expect(await comandosPedidos(page)).not.toContain('book_entities');

  await nombres.getByText('Nombres del Libro').click();
  const macondo = nombres.getByRole('button', { name: /^Macondo/ });
  await expect(macondo).toBeVisible();
  await expect(macondo).toContainText('3 menciones');
  await macondo.click();
  await expect(macondo).toHaveAttribute('aria-expanded', 'true');

  // Al desplegarlo se le pregunta al índice, y sus sitios sustituyen a los del
  // recorrido: el primero pasa a ser el del tercer párrafo.
  const sitios = nombres.getByRole('button', { name: /Ir a este sitio/ });
  await expect(sitios).toHaveCount(2);
  expect(await argumentosDe(page, 'entity_places')).toEqual([
    expect.objectContaining({ name: 'Macondo' })
  ]);
  await sitios.first().click();
  await expect
    .poll(async () =>
      (
        (await llamadasA(page, 'save_progress')) as Array<{
          locator: { locations: { fragments: string[] } };
        }>
      ).some((l) => l.locator.locations.fragments[0] === '#char=113')
    )
    .toBe(true);
});

/*
 * # Lo que se consultó queda
 */
test('el vocabulario consultado se exporta y se convierte en tarjetas desde Estudio', async ({
  page
}) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbVocabulario = true;
  });
  await montarPuente(page, {});
  await page.goto('/');
  await page.getByRole('button', { name: 'Estudio', exact: true }).click();
  await page.getByRole('button', { name: /^Vocabulario/ }).click();
  await expect(page.getByText('aldea').first()).toBeVisible();

  await page.getByRole('button', { name: 'Hacer Tarjetas' }).click();
  await expect(page.getByRole('status', { name: 'Avisos' })).toContainText(
    '1 tarjetas nuevas en Estudio'
  );
  expect(await comandosPedidos(page)).toContain('vocabulary_to_cards');

  await page.getByRole('button', { name: 'Exportar el Vocabulario' }).click();
  await expect(page.getByRole('status', { name: 'Avisos' })).toContainText(
    'Vocabulario guardado: 1 palabras.'
  );
  const guardado = await argumentosDe(page, 'save_vocabulary');
  expect(guardado[0]).toMatchObject({ path: '/tmp/anotaciones.json' });
});

/*
 * # Ritmo de lectura y cuánto falta
 */
test('lo que queda de un libro se dice en tiempo, a tu ritmo, y sin ritmo no se dice', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page.locator('.nb-tarjeta', { hasText: 'Cien Años de Soledad' }).click();
  const ficha = page.getByRole('complementary', { name: 'Ficha' });
  await expect(ficha.getByText('42 %')).toBeVisible();
  // Una instalación sin sesiones cerradas no tiene ritmo: no se inventa nada.
  await expect(ficha.getByText('a tu ritmo', { exact: false })).toHaveCount(0);
});

test('con un ritmo medido, la ficha y el lector dicen cuánto falta', async ({ page }) => {
  await page.addInitScript(() => {
    (globalThis as Record<string, unknown>).__nbRitmo = {
      progressionPerMinute: 0.0025,
      minutes: 50,
      sessions: 2
    };
  });
  await montarPuente(page, {});
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page.locator('.nb-tarjeta', { hasText: 'Cien Años de Soledad' }).click();
  // Le queda el 58 % a 0,25 % por minuto: 232 minutos, unas cuatro horas.
  await expect(
    page.getByRole('complementary', { name: 'Ficha' }).getByText('42 %, unas 4 h más, a tu ritmo')
  ).toBeVisible();

  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();
  // El lector parte de su posición, el principio: 400 minutos, seis horas y media.
  await expect(page.getByText('unas 6,5 h más, a tu ritmo')).toBeVisible();
});

/**
 * Pasar página con movimiento.
 *
 * 1. La animación de pase solo altera transform y opacity (aceleración GPU en el
 *    compositor, sin recalcular disposición).
 * 2. Se puede apagar desde el kit de lectura accesible en el panel de modos de lectura,
 *    guardando la preferencia persistente `book_page_animation`.
 * 3. Con movimiento reducido (`prefers-reduced-motion` o `data-motion="reduced"`),
 *    la animación desaparece por completo (ADR-012, regla 7).
 */
test('pasar página puede tener movimiento y desaparece con movimiento reducido', async ({
  page
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.getByRole('heading', { name: 'Capítulo 1' })).toBeVisible();

  // 1. Las reglas de animación de pase de página solo mueven transform y opacity.
  const propiedadesAnimadas = await page.evaluate(() => {
    const extraerPropiedades = (regla: CSSRule): string[] => {
      if (!(regla instanceof CSSKeyframesRule) || !regla.name.includes('nb-pase')) return [];
      return Array.from(regla.cssRules).flatMap((kf) =>
        kf instanceof CSSKeyframeRule ? Array.from(kf.style) : []
      );
    };

    const reglasDeHoja = (hoja: CSSStyleSheet): CSSRule[] => {
      try {
        return Array.from(hoja.cssRules);
      } catch {
        return [];
      }
    };

    const todas = Array.from(document.styleSheets).flatMap(reglasDeHoja);
    return Array.from(new Set(todas.flatMap(extraerPropiedades)));
  });

  expect(propiedadesAnimadas.length).toBeGreaterThan(0);
  for (const prop of propiedadesAnimadas) {
    expect(['transform', 'opacity']).toContain(prop);
  }

  // 2. El panel de Modo de Lectura permite activar y desactivar la animación.
  await page.getByRole('button', { name: /Modo de Lectura/ }).click();
  const interruptor = page.getByRole('switch', { name: 'Animar el pase de página' });
  await expect(interruptor).toBeVisible();
  await expect(interruptor).toHaveAttribute('aria-checked', 'true');

  await interruptor.click();
  await expect(interruptor).toHaveAttribute('aria-checked', 'false');

  const argumentos = await argumentosDe(page, 'update_setting');
  expect(argumentos).toContainEqual({
    key: 'book_page_animation',
    value: false
  });

  // 3. Con movimiento reducido, la animación se anula en CSS.
  await page.emulateMedia({ reducedMotion: 'reduce' });
  const animacionConReducido = await page.evaluate(() => {
    const texto = document.querySelector('.nb-texto');
    if (!texto) return null;
    texto.classList.add('nb-pase-siguiente');
    const anim = getComputedStyle(texto).animationName;
    texto.classList.remove('nb-pase-siguiente');
    return anim;
  });
  expect(animacionConReducido).toBe('none');
});

/*
 * En paginado la superficie no puede desplazarse en vertical, cada paso
 * mide exactamente una página y la columna no se mueve de sitio al pasar.
 */
test('el modo paginado muestra páginas enteras sin desplazamiento vertical', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 360 });
  await montarPuente(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await page.getByRole('button', { name: /Ajustes del Libro/ }).click();
  const panel = page.getByRole('complementary', { name: 'Ajustes del Libro' });
  await panel.getByRole('switch', { name: 'Pasar páginas en lugar de desplazar' }).click();
  await page.getByRole('button', { name: 'Cerrar panel' }).click();
  const superficie = page.locator('.nb-superficie[data-paginado="true"]');
  await expect(superficie).toHaveCount(1);
  const medida = () =>
    superficie.evaluate((s) => ({
      vertical: s.scrollHeight - s.clientHeight,
      resto: s.scrollWidth % s.clientWidth,
      ancho: s.clientWidth,
      x: s.scrollLeft
    }));
  await expect.poll(async () => (await medida()).vertical).toBeLessThanOrEqual(1);
  const antes = await medida();
  expect(Math.min(antes.resto, antes.ancho - antes.resto)).toBeLessThanOrEqual(1);
  await superficie.evaluate((s) => (s.scrollLeft = s.scrollWidth));
  await expect.poll(async () => (await medida()).x).toBeGreaterThan(0);
  const despues = await medida();
  expect(despues.x % despues.ancho).toBeLessThanOrEqual(1);
});

/*
 * La lista tiene columnas con rótulo, dice formato y tamaño, y pulsar una
 * columna ordena por ella; la segunda pulsación invierte el sentido.
 */
test('la vista de lista tiene columnas que ordenan', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await page.getByRole('button', { name: 'Ver en Lista' }).click();
  await expect(page.locator('.nb-cabecera-lista')).toBeVisible();
  await expect(page.locator('.nb-rejilla .nb-dato').first()).toHaveText('EPUB');
  const titulo = page.locator('.nb-cabecera-lista').getByRole('button', { name: 'Título' });
  await titulo.click();
  await expect(titulo).toHaveAttribute('aria-pressed', 'true');
  const sentido = await titulo.getAttribute('data-sentido');
  await titulo.click();
  await expect(titulo).not.toHaveAttribute('data-sentido', sentido ?? '');
});

/*
 * Con una biblioteca grande y un orden alfabético aparecen las anclas por
 * letra, y una lleva a la página donde empieza esa letra.
 */
test('las anclas alfabéticas aparecen con una biblioteca grande y llevan a su letra', async ({
  page
}) => {
  await montarPuente(page, { obras: 1000 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  const anclas = page.getByRole('navigation', { name: 'Saltar a una letra' });
  // Por fecha de alta no hay letras que ofrecer.
  await expect(anclas).toHaveCount(0);
  await page.getByRole('button', { name: 'Ver en Lista' }).click();
  await page.locator('.nb-cabecera-lista').getByRole('button', { name: 'Título' }).click();
  await expect(anclas).toBeVisible();
  await anclas.getByRole('button', { name: 'M' }).click();
  // Con 50 por página, la posición 300 es la página 7.
  await expect(page.locator('.nb-pagina[aria-current="page"]')).toHaveText('7');
});

/*
 * La ventana se abre antes que la base. Mientras el runtime se prepara -una
 * migración sobre una biblioteca grande- se dice qué pasa, y al terminar la
 * aplicación sigue sin un solo error, porque ninguna llamada salió antes de tiempo.
 */
test('mientras el runtime se prepara se dice que se pone al día la biblioteca', async ({
  page
}) => {
  await montarPuente(page, { runtimeEnEspera: true });
  await page.goto('/');
  const aviso = page.getByRole('status').filter({ hasText: 'Poniendo al día tu biblioteca' });
  await expect(aviso).toBeVisible();
  // La preparación termina después de observar el estado, sin carreras con la carga del equipo.
  await page.evaluate(() => window.dispatchEvent(new Event('nb-test-runtime-listo')));
  await expect(aviso).toHaveCount(0, { timeout: 10_000 });
  await expect(page.getByRole('button', { name: 'Continuar' })).toBeVisible();
});

/*
 * El editor de la nota flota abajo y no puede tapar el párrafo que se anota.
 * Con muchos párrafos y la ventana baja, el último visible cae justo debajo.
 */
test('el editor de la nota no tapa el párrafo que se anota', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 520 });
  await montarPuente(page, { parrafos: 40 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.locator('[data-nb-bloque="0"]')).toBeVisible();
  for (let i = 0; i < 6; i++) await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  const campo = page.getByRole('textbox', { name: 'Nota' });
  await expect(campo).toBeFocused();
  await expect
    .poll(async () => {
      const parrafo = await page.locator('[data-nb-cursor="true"]').last().boundingBox();
      const editor = await page.locator('.nb-nota').boundingBox();
      return parrafo && editor ? parrafo.y + parrafo.height - editor.y : 1;
    })
    .toBeLessThanOrEqual(0);
});

/*
 * Segunda auditoría: el salto a un resultado de búsqueda se quedaba pendiente, y
 * abrir después otro documento le aplicaba la posición del anterior -y la
 * guardaba como suya-. Un PDF se abría en la página 11 con el rótulo de un
 * capítulo de otro libro.
 */
test('abrir otro documento no hereda el salto pendiente del anterior', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('searchbox', { name: 'Buscar' }).fill('soledad');
  const panel = page.getByRole('complementary', { name: 'Búsqueda' });
  await panel.getByRole('tab', { name: 'Dentro de los Libros' }).click();
  await panel.getByRole('button', { name: /frente al pelotón/ }).click();
  await expect(page.locator('.nb-superficie')).toBeVisible();

  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  const antes = await page.evaluate(
    () => ((globalThis as Record<string, unknown>).__nbLlamadas as unknown[]).length
  );
  await page.locator('.nb-rejilla .nb-tarjeta').nth(2).dblclick();
  await expect(page.locator('.nb-superficie')).toBeVisible();
  await page.waitForTimeout(600);
  const guardados = await page.evaluate(
    (desde) =>
      ((globalThis as Record<string, unknown>).__nbLlamadas as Array<{ comando: string }>)
        .slice(desde)
        .filter((l) => l.comando === 'save_progress').length,
    antes
  );
  expect(guardados, 'nadie movió la posición del documento recién abierto').toBe(0);
});
