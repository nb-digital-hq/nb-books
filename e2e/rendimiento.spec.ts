/**
 * El presupuesto de rendimiento, medido sobre la aplicación montada.
 *
 * Contrato: `docs/architecture/performance-budgets.md`.
 *
 * # Por qué esto no lo cubre `paginacion.test.ts`
 *
 * Aquella prueba demuestra que **ningún tamaño de página ofrecido pasa del
 * presupuesto de nodos**, y lo demuestra enumerando. Lo que no puede ver es si
 * el componente pide de verdad una página en lugar de todo.
 *
 * El 2026-08-31 la biblioteca perdió cuatro declaraciones de CSS —entre ellas el
 * `overflow: auto` de su contenedor— y con eso perdió la virtualización entera,
 * sin que ninguna prueba de aritmética se enterara. Sobre una biblioteca real
 * eso eran ciento cincuenta mil tarjetas de una vez.
 *
 * De ahí que estas pruebas midan **nodos del documento de verdad**, sobre la
 * aplicación montada y con un catálogo del tamaño del real. Una regla sin
 * mecanismo no es una regla.
 *
 * # Y las dos últimas, por qué están aquí
 *
 * Porque el fotograma no se gasta solo pintando de más. Un `:hover` que mueve la
 * caja del elemento que lo recibe, y una capa de fondo que desborda al
 * contenedor que se desplaza, cuestan poco en nodos y se ven como que la
 * aplicación tiembla. Las dos aparecieron en la revisión del 2026-09-02 y las
 * dos las encontró alguien mirando la pantalla, no una prueba.
 */

import { expect, test } from '@playwright/test';

import { paginas } from '../src/lib/ui/paginacion';
import { argumentosDe, montarPuente, vistaDeEstudio } from './puente';

/**
 * El tamaño del catálogo real de referencia.
 *
 * No es un número redondo por gusto: son los EPUB del corpus con el que se
 * prueba esto de verdad. Comprobar con cien obras no comprueba nada, porque cien
 * caben.
 */
const CATALOGO_REAL = 152_080;

/** De `performance-budgets.md`, y repetido en `src/lib/ui/paginacion.ts`. */
const PRESUPUESTO_DE_NODOS = 1500;

/**
 * Las cifras se agrupan en el idioma de la interfaz, no en el del navegador.
 * Aquí se formatean igual para poder buscarlas: comprobar contra `152,080`
 * pasaría en una máquina en inglés y fallaría en una en español, y eso no es una
 * prueba, es una lotería.
 */
const cifra = new Intl.NumberFormat('es');

async function irALaBiblioteca(page: import('@playwright/test').Page) {
  await page.getByRole('button', { name: 'Biblioteca', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Biblioteca' })).toBeVisible();
  // La cuenta se pinta con el total del runtime, así que sirve de señal de que
  // la primera página ya llegó: sin esperarla se mediría la pantalla de carga.
  await expect(page.getByText(`${cifra.format(CATALOGO_REAL)} documentos`)).toBeVisible();
}

async function montada(page: import('@playwright/test').Page) {
  await page.setViewportSize({ width: 1600, height: 900 });
  await montarPuente(page, { obras: CATALOGO_REAL });
  await page.goto('/');
  await irALaBiblioteca(page);
}

const nodosDeLaRejilla = (page: import('@playwright/test').Page) =>
  page.locator('.nb-rejilla').evaluate((el) => el.querySelectorAll('*').length);

test('ciento cincuenta mil obras caben en el presupuesto de nodos', async ({ page }) => {
  await montada(page);

  const nodos = await nodosDeLaRejilla(page);

  expect(nodos, `la rejilla materializó ${nodos} nodos`).toBeLessThanOrEqual(PRESUPUESTO_DE_NODOS);
});

test('el mayor tamaño de página tampoco se sale del presupuesto', async ({ page }) => {
  await montada(page);

  // El presupuesto tiene que aguantar lo que el usuario **puede** elegir, no
  // solo lo que viene puesto. Con el mayor de la lista es cuando aprieta. El
  // control está en la barra de arriba, con lo demás que decide qué se ve.
  await page
    .getByRole('toolbar', { name: 'Controles de la Biblioteca' })
    .getByRole('button', { name: /Por página/ })
    .click();
  await page.getByRole('option', { name: '100', exact: true }).click();
  await expect(page.getByText('1–100 de')).toBeVisible();

  const nodos = await nodosDeLaRejilla(page);
  expect(nodos, `cien por página materializaron ${nodos} nodos`).toBeLessThanOrEqual(
    PRESUPUESTO_DE_NODOS
  );
});

test('el contenedor de la biblioteca se desplaza él, y no la ventana entera', async ({ page }) => {
  await montada(page);

  /*
   * Quien se desplaza tiene que ser el lienzo. Si se desplazara un antepasado,
   * la página crecería hacia abajo y se llevaría por delante el pie de
   * paginación, que es justo el control con el que se sale de ahí.
   */
  const lienzo = page.locator('.nb-lienzo');
  const medidas = await lienzo.evaluate((el) => ({
    seDesplaza: el.scrollHeight > el.clientHeight + 1,
    visible: el.clientHeight,
    total: el.scrollHeight
  }));

  expect(medidas.seDesplaza, 'el lienzo no tiene nada que desplazar').toBe(true);
  expect(medidas.visible).toBeLessThan(medidas.total);
  // Y el alto visible es el de una ventana, no el del catálogo entero.
  expect(medidas.visible).toBeLessThan(2000);
  // El pie de paginación sigue a la vista: es la prueba de que lo que se
  // desplaza es el lienzo y no la vista entera.
  await expect(page.getByRole('navigation', { name: 'Páginas de la Biblioteca' })).toBeVisible();
});

test('desplazarse por la biblioteca no hace crecer lo que hay pintado', async ({ page }) => {
  await montada(page);

  const lienzo = page.locator('.nb-lienzo');
  const alPrincipio = await nodosDeLaRejilla(page);

  await lienzo.evaluate((el) => el.scrollTo({ top: el.scrollHeight }));
  await expect.poll(() => nodosDeLaRejilla(page)).toBeLessThanOrEqual(PRESUPUESTO_DE_NODOS);
  expect(await nodosDeLaRejilla(page)).toBe(alPrincipio);

  // Y hay obras pintadas, no un hueco: llegar al final y no ver nada es el otro
  // modo de fallo, y se ve igual de bien en una captura.
  await expect(page.locator('.nb-tarjeta').first()).toBeVisible();
});

test('cambiar de página trae otras obras y sigue costando lo mismo', async ({ page }) => {
  await montada(page);

  const primerTitulo = () => page.locator('.nb-rejilla .nb-titulo').first().innerText();
  const antes = await primerTitulo();
  const nodosAntes = await nodosDeLaRejilla(page);

  await page.getByRole('button', { name: 'Página Siguiente' }).click();

  await expect.poll(primerTitulo).not.toBe(antes);
  // No se compara con igualdad estricta: la marca de "no disponible" o la de
  // copias aparece en unas obras y no en otras, así que dos páginas del mismo
  // tamaño pueden diferir en unos pocos nodos. Lo que no puede es crecer con el
  // número de página, que es lo que distingue paginar de acumular.
  expect(await nodosDeLaRejilla(page)).toBeLessThanOrEqual(nodosAntes + 20);
  // Y se dice dónde se está, que es la mitad de lo que aporta paginar.
  await expect(page.getByText('51–100 de')).toBeVisible();
});

test('la última página se alcanza y tiene obras dentro', async ({ page }) => {
  await montada(page);

  // Con 152.080 obras y 50 por página son 3042 páginas: el salto a la última es
  // el que no existe con un desplazamiento continuo. El número se formatea igual
  // que en la interfaz porque en español los grupos de cuatro cifras no llevan
  // separador y los de seis sí: `3042` y `152.080`.
  const ultima = paginas(CATALOGO_REAL, 50);
  await page.getByRole('button', { name: cifra.format(ultima), exact: true }).click();

  await expect(
    page.getByText(`${cifra.format(CATALOGO_REAL - 29)}–${cifra.format(CATALOGO_REAL)} de`)
  ).toBeVisible();
  await expect(page.locator('.nb-tarjeta').first()).toBeVisible();
});

test('la biblioteca no deja una capa de composición por cada portada', async ({ page }) => {
  await montada(page);

  /*
   * `will-change` fija una capa de composición y no la suelta. Puesto en una
   * regla que alcanza a cada portada, eso son tantas capas como portadas quepan:
   * medido el 2026-09-01, cincuenta y cuatro vivas en reposo sobre una ventana
   * de 1600x900, con el fotograma mediano por encima del presupuesto.
   *
   * `check-ui.mjs` lo rechaza en el código fuente. Esto lo comprueba **en la
   * página compuesta**, que es donde además se vería si llegara desde una hoja
   * de estilos que el guardián no mira.
   */
  const capas = await page.evaluate(
    () =>
      Array.from(document.querySelectorAll<HTMLElement>('*')).filter((el) => {
        const wc = getComputedStyle(el).willChange;
        return wc !== '' && wc !== 'auto';
      }).length
  );

  expect(capas, `${capas} elementos anuncian un cambio permanente`).toBe(0);
});

test('en reposo no hay nada animándose', async ({ page }) => {
  await montada(page);

  /*
   * Antes esta prueba comprobaba que el fondo vivo de la carcasa se animara
   * **componiendo**, con `transform` y no con `background-position`. Comprobaba
   * lo correcto sobre una premisa equivocada: que el fondo tenía que estar.
   *
   * Medido el 2026-09-02 con un desplazamiento continuo sobre la biblioteca, ese
   * fondo costaba trece fotogramas largos de setenta y siete y un peor caso de
   * 31,6 ms; sin él, tres de ochenta y siete y 25,1. Componer es más barato que
   * repintar y sigue sin ser gratis: compite por el mismo fotograma que el
   * desplazamiento, y a partir de veinticinco obras por página el
   * desplazamiento es continuo. `performance-budgets.md` ya decía qué hacer con
   * una animación que no cabe.
   *
   * Así que ahora se comprueba lo contrario, y es una garantía más fuerte:
   * **con la biblioteca quieta, nada se mueve**. Se ignoran los indicadores de
   * carga, que se animan mientras tapan a lo que esperan.
   */
  const animados = await page.evaluate(() => {
    const salida: string[] = [];
    for (const el of Array.from(document.querySelectorAll<HTMLElement>('*'))) {
      const nombre = (el.className?.toString() || el.tagName).split(' ')[0];
      if (nombre.includes('spinner') || nombre.includes('cargando')) continue;
      for (const pseudo of [null, '::before', '::after']) {
        if (getComputedStyle(el, pseudo).animationName !== 'none') {
          salida.push(`${nombre}${pseudo ?? ''}`);
        }
      }
    }
    return salida;
  });

  expect(animados, `se animan en reposo: ${animados.join(', ')}`).toEqual([]);
});

test('un tablero lleno cabe en el presupuesto de nodos', async ({ page }) => {
  /*
   * **La otra mitad del criterio**, con la forma: ocho columnas
   * con las ciento veinte tarjetas que caben. Un lienzo infinito no puede
   * cumplirlo, siempre cabe una nota más; el tablero es finito, así que lo que se
   * pinta está acotado por construcción, y aquí se cuenta. Se cuenta el documento
   * entero y no solo el tablero: lo que ahoga al webview es el total.
   */
  await montarPuente(page, { tableroLleno: true });
  await page.goto('/');
  await page.getByRole('button', { name: 'Estudio' }).click();
  await vistaDeEstudio(page, /Tableros/).click();
  await page
    .getByRole('group', { name: 'Tableros' })
    .getByRole('button', { name: /Capítulo tres/ })
    .click();
  await expect(page.getByRole('button', { name: /Nota número 119,/ })).toBeVisible();

  const nodos = await page.evaluate(() => document.querySelectorAll('*').length);

  expect(nodos, `nodos con un tablero lleno: ${nodos}`).toBeLessThanOrEqual(PRESUPUESTO_DE_NODOS);
});

test('ningún destino ni panel pinta un número de nodos que decida la biblioteca', async ({
  page
}) => {
  /*
   * **El criterio, regla 2 del contrato de carga.**
   *
   * El presupuesto de 1.500 nodos lo cumplía la biblioteca por construcción
   *, y **nada cubría a los demás**: un panel de facetas, la lista
   * de notas o el vocabulario podían pintar lo que hubiera. Con ocho archivos de
   * prueba ninguno se nota; el 2026-09-09, con 152.080 documentos dentro, abrir
   * el panel de autores congelaba la ventana.
   *
   * Se cuenta el documento entero y no cada región por separado, que es lo que
   * de verdad tiene que caber: lo que ahoga al webview es el total.
   */
  await montarPuente(page, { obras: CATALOGO_REAL, conCatalogo: true });
  await page.goto('/');

  const nodosDelDocumento = () => page.evaluate(() => document.querySelectorAll('*').length);

  const destinos = ['Inicio', 'Biblioteca', 'Descubrir', 'Estudio', 'Ajustes'];
  for (const destino of destinos) {
    await page.getByRole('button', { name: destino, exact: true }).click();
    const nodos = await nodosDelDocumento();
    expect(nodos, `${destino} pintó ${nodos} nodos`).toBeLessThanOrEqual(PRESUPUESTO_DE_NODOS);
  }

  // Las cuatro vistas de Estudio, que son listas acotadas.
  await page.getByRole('button', { name: 'Estudio', exact: true }).click();
  for (const vista of [/Notas/, /Anotaciones/, /Vocabulario/, /Tableros/]) {
    await vistaDeEstudio(page, vista).click();
    const nodos = await nodosDelDocumento();
    expect(nodos, `la vista ${vista} pintó ${nodos} nodos`).toBeLessThanOrEqual(
      PRESUPUESTO_DE_NODOS
    );
  }

  // Y el panel de estanterías, que es el que se congelaba.
  await irALaBiblioteca(page);
  await page.locator('.nb-banner').getByRole('button', { name: 'Estanterías' }).click();
  await expect(page.getByRole('complementary', { name: 'Estanterías' })).toBeVisible();
  const conEstanterias = await nodosDelDocumento();
  expect(
    conEstanterias,
    `el panel de estanterías pintó ${conEstanterias} nodos`
  ).toBeLessThanOrEqual(PRESUPUESTO_DE_NODOS);
});

test('un fondo sonoro no le quita fotogramas a la lectura', async ({ page }) => {
  /*
   * **El criterio**, y la parte que solo se puede comprobar aquí:
   * reproducir no degrada el presupuesto de fotograma.
   *
   * Lo que lo hace posible no es esta prueba, es cómo está hecho: el ruido se
   * genera **una vez** en un búfer de diez segundos que se repite, en lugar de
   * muestra a muestra con un `ScriptProcessorNode`, que corre en el hilo
   * principal y compite con el fotograma de la lectura.
   */
  await montarPuente(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Macondo era entonces')).toBeVisible();

  const fotogramas = () =>
    page.evaluate(
      () =>
        new Promise<{ largos: number; total: number }>((listo) => {
          let anterior = performance.now();
          let largos = 0;
          let total = 0;
          const hasta = anterior + 1200;
          const paso = () => {
            const ahora = performance.now();
            if (ahora - anterior > 20) largos += 1;
            total += 1;
            anterior = ahora;
            if (ahora < hasta) requestAnimationFrame(paso);
            else listo({ largos, total });
          };
          requestAnimationFrame(paso);
        })
    );

  const enSilencio = await fotogramas();

  await page.getByRole('button', { name: /Fondo Sonoro/ }).click();
  await page.getByRole('button', { name: 'Lluvia' }).click();
  await expect(page.getByRole('button', { name: 'Lluvia' })).toHaveAttribute(
    'aria-pressed',
    'true'
  );

  // Se mide **después** de arrancar: generar el búfer cuesta una vez, al
  // pulsar, y lo que el criterio protege es leer con el ruido puesto.
  const sonando = await fotogramas();

  /*
   * Se compara contra un **presupuesto**, no contra el silencio, y el motivo
   * está medido: con la suite entera corriendo en cuatro procesos, el silencio
   * ya da cinco fotogramas largos de setenta y dos y el ruido once. La
   * diferencia no es del fondo sonoro, es de la máquina; una aserción de "no
   * más de dos por encima del silencio" fallaba según lo ocupado que estuviera
   * el equipo, y una prueba que depende de eso no mide lo que dice medir.
   *
   * El criterio de la ficha es que reproducir **no degrade el presupuesto de
   * fotograma de la lectura**, así que se comprueba eso: con el ruido puesto,
   * menos de una cuarta parte de los fotogramas puede pasarse. En una máquina
   * en reposo la cifra real ronda el cinco por ciento.
   */
  const proporcion = sonando.largos / sonando.total;
  expect(
    proporcion,
    `fotogramas largos: ${enSilencio.largos}/${enSilencio.total} en silencio, ` +
      `${sonando.largos}/${sonando.total} con ruido`
  ).toBeLessThan(0.25);
});

/**
 * El fondo que el lector pinta detrás de su barra no lleva `filter`.
 *
 * Lo llevaba, para teñirse igual que la superficie, y con él abrir cualquier panel
 * del lector costaba 90-110 ms en WebKitGTK contra 30: el panel se apoya encima y
 * el motor volvía a filtrar el fondo entero. Es un color liso, así que el color
 * filtrado se calcula una vez (`fondoFiltrado`). Se comprueba en el modo sepia, que
 * es el que filtra: en el de día no hay filtro que quitar.
 */
test('el fondo del lector detrás de la barra es un color, no un filtro', async ({ page }) => {
  await montarPuente(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Continuar' }).click();
  await expect(page.locator('[data-nb-bloque]').first()).toBeVisible();

  const capa = await page.evaluate(() => {
    const lector = document.querySelector('.nb-lector');
    if (!lector) return null;
    const estilo = getComputedStyle(lector, '::before');
    const superficie = document.querySelector('.nb-superficie');
    return {
      filtro: estilo.filter,
      fondo: estilo.backgroundColor,
      filtroDeLaSuperficie: superficie ? getComputedStyle(superficie).filter : null
    };
  });
  expect(capa).not.toBeNull();
  // La superficie sí filtra: si no, el modo no sería sepia y la prueba no diría nada.
  expect(capa?.filtroDeLaSuperficie).toContain('sepia');
  expect(capa?.filtro).toBe('none');
  // El sepia de fábrica, #f3e9d8 con brillo 0,97 y temperatura 0,35.
  expect(capa?.fondo).toBe('rgb(255, 242, 210)');
});

test('mover el cursor no cuesta más en un capítulo largo', async ({ page }) => {
  /*
   * **Regresión del 2026-09-11**, encontrada midiendo en WebKitGTK.
   *
   * Cada flecha sustituía el estado del lector, y dos cosas que leían ese estado
   * entero volvían a trabajar con el capítulo completo: el narrador lo partía otra
   * vez en frases, y la superficie volvía a colocar todas las marcas. En WebKitGTK,
   * mover el cursor costaba 18 ms con cincuenta párrafos y 115 con mil seiscientos.
   * Ahora cuesta lo mismo con cualquier largo.
   *
   * **Se compara un capítulo largo con uno corto, en la misma prueba**, y no contra
   * un número fijo de milisegundos. La primera versión tenía un umbral de 20 ms y
   * falló con el código correcto, 29,3 ms, en una pasada con otro programa ocupando
   * más de un núcleo: una prueba que depende de lo ocupada que esté la máquina no
   * mide lo que dice medir. La carga afecta igual a los dos capítulos, y la
   * proporción entre ellos es lo que la propiedad afirma.
   *
   * Se mide el trabajo de JavaScript que dispara la tecla, desde que se pulsa hasta
   * que se vacía la cola, y no el fotograma, que depende del compositor.
   */
  const costePorFlecha = async (pagina: import('@playwright/test').Page, parrafos: number) => {
    await montarPuente(pagina, { parrafos });
    await pagina.goto('/');
    await pagina.getByRole('button', { name: 'Continuar' }).click();
    // Esto es el **montaje**, no la medida, y por eso lleva un plazo propio.
    //
    // Pintar seis mil cuatrocientos bloques con la suite entera corriendo en
    // paralelo pasa de los cinco segundos que trae `toHaveCount` de fábrica, y
    // el 2026-09-12 la prueba falló por eso con el código correcto: dijo que
    // había cero bloques porque se cansó de esperar, no porque no llegaran.
    // Esperar más aquí **no afloja nada**: la propiedad que esta prueba afirma
    // es la proporción entre el capítulo largo y el corto, y esa proporción no
    // cambia porque el montaje tarde. Es lo contrario de bajar un umbral, que es
    // lo que la sección 4.12.2 de presupuestos prohíbe.
    await expect(pagina.locator('[data-nb-bloque]')).toHaveCount(parrafos, { timeout: 30_000 });
    return pagina.evaluate(async () => {
      const tiempos: number[] = [];
      for (let i = 0; i < 20; i += 1) {
        const t0 = performance.now();
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
        await new Promise((r) => setTimeout(r, 0));
        tiempos.push(performance.now() - t0);
        await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      }
      tiempos.sort((a, b) => a - b);
      // El **mínimo**, no la mediana: la suite corre en paralelo, y una flecha a la
      // que el planificador le quita la CPU mide la máquina y no el código. De veinte
      // repeticiones, la más barata es la que no la perdió. Con la mediana, esta misma
      // prueba daba 2,1 dentro de la suite y 1,2 sola, con el mismo código.
      return tiempos[0] ?? 0;
    });
  };

  const corto = await costePorFlecha(page, 50);
  const largo = await costePorFlecha(await page.context().newPage(), 6400);
  // Un suelo para el corto: por debajo, el reloj y la cola de tareas dan cifras de
  // ruido, y dividir por ruido no dice nada. Con el mínimo el suelo baja a 1 ms; con
  // 2 comprimía la proporción hasta dejar pasar el defecto que esto vigila.
  const proporcion = largo / Math.max(corto, 1);

  // Medido el 2026-09-12 con seis mil cuatrocientos párrafos, que es el largo donde crecer se
  // nota: 3,0 con el código actual y 10,2 devolviendo el recorrido entero de atributos, que era
  // lo que quedaba de coste lineal. Devolviendo cualquiera de las dos causas grandes —el narrador
  // o las marcas— daba entre 11 y 16 ya con mil seiscientos.
  //
  // **Con mil seiscientos la diferencia no sobrevivía a la suite.** Eran 1,2 contra 3,6, y con la
  // máquina a 14 de carga el código correcto llegó a 2,1: la prueba falló sin que nadie hubiera
  // roto nada. Antes que afinar el umbral hasta que quepa el ruido, se hace la señal más grande.
  expect(
    proporcion,
    `${largo.toFixed(1)} ms por flecha con seis mil cuatrocientos párrafos y ${corto.toFixed(1)} con cincuenta`
  ).toBeLessThan(6);
});
test('el cromo que se apoya sobre lo que se desplaza es opaco', async ({ page }) => {
  await montada(page);

  /*
   * La segunda mitad de "todo parpadea al hacer scroll", y la que queda después
   * de retirar el fondo animado.
   *
   * Un color translúcido hay que **mezclarlo con lo que hay debajo**, y debajo
   * del banner pasa la biblioteca entera: la mezcla se rehace cada vez que lo de
   * debajo se mueve. Es el mismo problema del desenfoque en menor grado, y por
   * eso la respuesta es la misma: el cromo permanente que se apoya sobre una
   * zona que se desplaza va opaco. Medido, el peor fotograma de un
   * desplazamiento continuo bajaba de 25,1 a 22,7 ms al hacerlo.
   *
   * Se comprueba el alfa computado y no la regla de CSS porque el color sale de
   * un token con `color-mix`, y lo que importa es el color que acaba pintándose.
   *
   * Esto no vale para un panel **momentáneo**, que se abre encima y se va: ahí
   * la translucidez dura lo que dura el panel y dice que lo de debajo sigue ahí.
   */
  for (const selector of ['.nb-banner', '.nb-lateral']) {
    const alfa = await page.locator(selector).evaluate((el) => {
      const bg = getComputedStyle(el).backgroundColor;
      const m = /\/\s*([\d.]+)\s*\)/.exec(bg) || /rgba\([^)]*,\s*([\d.]+)\)/.exec(bg);
      return m ? Number.parseFloat(m[1]) : 1;
    });
    expect(alfa, `${selector} es translúcido sobre lo que se desplaza`).toBe(1);
  }
});

test('la barra lateral no se desplaza cuando sus destinos caben', async ({ page }) => {
  await montada(page);

  /*
   * La regresión del 2026-09-02, y su causa exacta.
   *
   * La capa del fondo vivo se agranda con `scale` para poder moverse sin dejar
   * bordes, y una caja transformada **cuenta para el área desplazable** de quien
   * la contiene. Con el fondo dentro del elemento que se desplaza, la barra
   * tenía siempre trescientos píxeles de recorrido de más y enseñaba una barra
   * de desplazamiento permanente que no llevaba a ninguna parte.
   *
   * Siete destinos en una ventana de 900 caben de sobra. Que haya recorrido es
   * el defecto, no un detalle de estilo.
   */
  // La barra no se desplaza: recorta. Lo que se desplaza, si hace falta, es su
  // interior, y con siete destinos no hace falta.
  const lateral = page.locator('.nb-lateral');
  expect(await lateral.evaluate((el) => getComputedStyle(el).overflowY)).toBe('hidden');

  const interior = page.locator('.nb-lateral-interior');
  const recorrido = await interior.evaluate((el) => el.scrollHeight - el.clientHeight);
  expect(recorrido, `los destinos tienen ${recorrido}px de recorrido de más`).toBeLessThanOrEqual(
    1
  );

  // Y ninguno de los dos enseña una barra de desplazamiento, que es lo que se
  // veía: una barra permanente que no llevaba a ninguna parte.
  for (const zona of [lateral, interior]) {
    const ancho = await zona.evaluate((el) => el.offsetWidth - el.clientWidth);
    expect(ancho, 'hay una barra de desplazamiento a la vista').toBeLessThanOrEqual(1);
  }
});

test('pasar el ratón por una obra no la saca de debajo del ratón', async ({ page }) => {
  await montada(page);

  /*
   * El parpadeo del 2026-09-02, y por qué era un parpadeo de verdad.
   *
   * `transform: translateY(-4px)` en `:hover` mueve la caja **y su área
   * sensible**. Con el puntero en los últimos píxeles del elemento, levantarlo
   * lo saca de debajo, el hover se apaga, el elemento vuelve y el hover se
   * enciende otra vez: a sesenta fotogramas por segundo eso se ve temblar.
   *
   * La propiedad que lo impide es que el rectángulo con el ratón encima
   * **contenga** al de reposo: `scale` por encima de 1 solo agranda, así que el
   * ciclo no puede darse.
   *
   * # Por qué esto y `check-ui.mjs` no son la misma prueba
   *
   * Porque cubren cosas distintas, y se comprobó rompiéndolas. El guardián mira
   * la regla de CSS y rechaza **cualquier** desplazamiento en un `:hover` cuyo
   * sujeto sea el elemento hovereado; esto mide la caja ya compuesta. Un
   * `translateY(-2px) scale(1.01)` sobre una tarjeta alta lo detecta el guardián
   * y no lo detecta esta prueba, porque el escalado devuelve al borde inferior
   * justo a donde estaba; un `translateY(-4px)` los detectan los dos. Así que el
   * guardián es el que impide escribirlo y este el que comprueba que lo que se
   * ve en pantalla cumple, venga la regla de donde venga.
   */
  const tarjeta = page.locator('.nb-tarjeta').first();
  const reposo = await tarjeta.boundingBox();
  await tarjeta.hover();
  // La transición dura lo que dura: se mide cuando ha terminado de moverse.
  await page.waitForTimeout(400);
  const encima = await tarjeta.boundingBox();

  expect(reposo).not.toBeNull();
  expect(encima).not.toBeNull();
  if (!reposo || !encima) return;

  const holgura = 0.5;
  expect(encima.y, 'el borde de arriba bajó').toBeLessThanOrEqual(reposo.y + holgura);
  expect(encima.x, 'el borde de la izquierda se movió a la derecha').toBeLessThanOrEqual(
    reposo.x + holgura
  );
  expect(
    encima.y + encima.height,
    'el borde de abajo subió: el hover parpadea'
  ).toBeGreaterThanOrEqual(reposo.y + reposo.height - holgura);
  expect(encima.x + encima.width, 'el borde de la derecha se encogió').toBeGreaterThanOrEqual(
    reposo.x + reposo.width - holgura
  );
});

/**
 * El banco tiene que poder enseñar el coste de las portadas.
 *
 * # Por qué esta prueba existe
 *
 * Porque durante meses no pudo. El simulador no daba `coverPath` a ninguna
 * edición, así que la rejilla pintaba portadas generadas por CSS y **no cargaba
 * ni una imagen**; y el arnés de WebKitGTK resolvía todas las portadas a la
 * misma URL. El motor descodifica por URL: cincuenta `<img>` al mismo sitio son
 * una descodificación, y cincuenta obras con su portada son cincuenta. Medido el
 * 2026-09-06 en el motor real, pasar de página cuesta 2 ms con una portada
 * repetida y unos 100 con cincuenta distintas.
 *
 * Con el banco colapsado, la pregunta "¿cuánto cuesta pasar de página?" solo
 * podía contestarse que nada, y así se concluyó que la caché de la interfaz no
 * hacía falta. No era una prueba optimista: era **una prueba incapaz de fallar**.
 *
 * Esto no mide tiempo a propósito. El tiempo aquí es Chromium y el producto corre
 * en WebKitGTK, que es la pregunta abierta I13; lo que sí es cierto en los dos
 * motores es cuántos recursos distintos pide el documento.
 */
test('cada obra de la página pide su propia portada, no la de todas', async ({ page }) => {
  await montada(page);

  const portadas = page.locator('.nb-tarjeta .nb-portada img');
  await expect(portadas.first()).toBeAttached();

  const fuentes = await portadas.evaluateAll((imgs) =>
    imgs.map((img) => (img as HTMLImageElement).getAttribute('src') ?? '')
  );
  const distintas = new Set(fuentes).size;

  expect(fuentes.length, 'la página no pintó ninguna portada').toBeGreaterThan(1);
  expect(
    distintas,
    `${fuentes.length} obras en la página y solo ${distintas} portadas distintas: ` +
      'el banco no puede enseñar lo que cuesta descodificarlas'
  ).toBe(fuentes.length);
});

/**
 * Pasar el ratón no puede cambiar la caja de nada que haya dentro.
 *
 * # Por qué hacen falta las dos, el guardián y esta
 *
 * `check-ui.mjs` lee la regla de CSS y rechaza un `:hover` que toque `width`,
 * `padding`, `font-size` y compañía. Lo que no puede ver es una caja que cambia
 * porque la cambia JavaScript, o porque una regla que por separado parece
 * inocente compone con otra. Esto mide la caja ya dispuesta.
 *
 * **Comprobado rompiéndolo, y el reparto no es el que parecía.** Con
 * `padding-right: 6px` en el `:hover`, el guardián lo caza y **esta prueba no**:
 * el título es un bloque, así que su caja exterior la fija el contenedor y el
 * relleno solo encoge la de contenido. Con `width: max-content` —el defecto que
 * de verdad había— esta prueba falla (189 píxeles a 0) y el guardián también. El
 * guardián cubre más casos porque lee la intención; esta cubre los que llegan a
 * la pantalla vengan de donde vengan, incluida una caja que mueva JavaScript.
 *
 * La razón es el coste, no el sitio: rehacer la disposición de un subárbol al
 * entrar el ratón **y otra vez al salir** costaba 6,5 ms medidos por vez en
 * WebKitGTK, sobre un presupuesto de fotograma de 16. Con la rueda girando el
 * contenido pasa por debajo del puntero quieto, así que eso se paga una vez por
 * cada tarjeta que se roce sin que nadie mueva el ratón adrede.
 */
test('pasar el ratón por una obra no cambia la caja de su texto', async ({ page }) => {
  await montada(page);

  const tarjeta = page.locator('.nb-tarjeta').first();
  const titulo = tarjeta.locator('.nb-titulo');
  await expect(titulo).toBeVisible();

  /*
   * Se mide `offsetWidth`, que es la caja **de disposición**, y no
   * `boundingBox()`, que es la ya compuesta.
   *
   * La diferencia importa y costó una prueba equivocada: la tarjeta tiene
   * permitido agrandarse con `scale` —lo dice la sección 11, porque escalar solo
   * puede ampliar el área sensible—, así que su caja compuesta **sí** cambia. La
   * primera versión de esto medía eso y fallaba con 189,16 a 191,99 píxeles, que
   * es exactamente el 1,5 % de escala. Lo que esta prueba vigila es otra cosa: que
   * el motor no tenga que **rehacer la disposición**, que es lo que cuesta 6,5 ms.
   * Una escala se compone y no dispone; un `width` distinto dispone.
   */
  const caja = () => titulo.evaluate((el) => ({ ancho: el.offsetWidth, alto: el.offsetHeight }));

  const reposo = await caja();
  await tarjeta.hover();
  // La transición dura lo que dura: se mide cuando ha terminado de moverse.
  await page.waitForTimeout(400);
  const encima = await caja();

  expect(
    encima.ancho,
    `el título pasó de ${reposo.ancho} a ${encima.ancho} píxeles de ancho de disposición al recibir el ratón`
  ).toBe(reposo.ancho);
  expect(encima.alto, 'el título cambió de alto de disposición al recibir el ratón').toBe(
    reposo.alto
  );
});

/**
 * La página siguiente se pide antes de que nadie la pida.
 *
 * # Por qué esto y no una caché de páginas
 *
 * Porque está medido en qué se va el tiempo de cambiar de página, y no es en
 * pedir los datos: son menos de 10 ms de unos 265. Lo caro es rehacer el DOM y
 * descodificar las portadas, y de esas dos solo la segunda se puede adelantar.
 * Con la página siguiente traída y sus portadas visibles ya descodificadas, el
 * cambio baja de 265 ms de mediana a 112. Ver `performance-budgets.md` 4.3.6.
 *
 * Lo que se comprueba aquí es lo estructural —que se pide por adelantado y que al
 * pulsar no se vuelve a pedir—, no el tiempo: el tiempo aquí es Chromium y el
 * producto corre en WebKitGTK.
 */
test('la página siguiente se pide antes de que nadie pulse', async ({ page }) => {
  await montada(page);

  const desplazamientos = async () =>
    (await argumentosDe(page, 'browse_library')).map(
      (args) => ((args as { query?: { offset?: number } })?.query?.offset ?? 0) as number
    );

  // El adelanto va detrás de `requestIdleCallback`, así que se espera a que
  // ocurra en lugar de darlo por hecho al instante.
  await expect.poll(async () => (await desplazamientos()).some((o) => o > 0)).toBe(true);

  const antes = await desplazamientos();
  const adelantado = antes.find((o) => o > 0) ?? 0;
  const vecesAntes = antes.filter((o) => o === adelantado).length;

  await page.getByRole('button', { name: 'Página Siguiente' }).click();
  await expect(page.locator('.nb-tarjeta').first()).toBeVisible();

  const despues = await desplazamientos();
  const vecesDespues = despues.filter((o) => o === adelantado).length;

  expect(
    vecesDespues,
    `la página ya adelantada se volvió a pedir: ${vecesAntes} veces antes y ${vecesDespues} después`
  ).toBe(vecesAntes);
});
