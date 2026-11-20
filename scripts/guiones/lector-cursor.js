/**
 * Lo que cuesta mover el cursor de lectura con el teclado: el trabajo del
 * manejador de la tecla, y si lo que provoca cabe en un fotograma.
 *
 * Existe porque `destinos-fotogramas.js` encontró el 2026-09-11 que, sobre un
 * capítulo de cuatrocientos párrafos, cada flecha dejaba un fotograma de unos
 * 48 ms. Leer con el teclado es un recorrido de primera clase (ADR-012), así que
 * hay que saber dónde está el coste y si crece con el capítulo: se ejecuta con
 * simuladores de capítulos de distinto largo, exportados con
 * `scripts/exportar-puente-de-pruebas.mjs --capitulo N`.
 *
 * **La tecla se pulsa al principio de un fotograma, y no es un detalle.** La
 * primera versión la pulsaba treinta milisegundos después del fotograma anterior
 * —justo por debajo de dos intervalos a 60 Hz— y contaba dos fotogramas. Según en
 * qué lado de la sincronía cayera la tecla, la cifra ganaba o perdía un fotograma
 * entero: con el mismo código dio 18 ms una tarde y 33 otra, en todos los largos
 * de capítulo por igual. Pulsada al principio del fotograma, lo que cabe se ve en
 * el siguiente, y lo que no cabe se cuenta. Ver `performance-budgets.md` 4.12.
 *
 * - `intervalo`: el hueco entre fotogramas sin tocar nada, en la misma ventana.
 *   Es la unidad, porque el compositor no tiene por qué ir a 60 Hz.
 * - `manejador`: lo que tarda en volver `dispatchEvent`, el JavaScript síncrono
 *   de la tecla.
 * - `efectos`: desde que vuelve hasta que se vacía la primera tanda de
 *   microtareas, que es donde Svelte aplica los efectos del cambio de estado. No
 *   entran en `manejador`, y la primera versión los daba por dentro.
 * - `hastaElSiguiente`: desde la tecla hasta el fotograma siguiente. Lo que no es
 *   manejador ni efectos es estilo, disposición y pintado. Si todo cupo en el
 *   fotograma en que se pulsó, vale algo menos de un intervalo.
 * - `tarde`: cuántas de las cuarenta flechas tardaron más de intervalo y medio,
 *   es decir, se vieron al menos un fotograma tarde.
 */
(async () => {
  const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
  const fotograma = () => new Promise((r) => requestAnimationFrame(r));
  const aparece = async (selector, ms = 8000) => {
    const t0 = performance.now();
    while (performance.now() - t0 < ms) {
      const el = document.querySelector(selector);
      if (el) return el;
      await fotograma();
    }
    return null;
  };

  if (!(await aparece('.nb-inicio')))
    return JSON.stringify({ error: 'Inicio no llegó a pintarse' });
  await esperar(1200);
  const continuar = [...document.querySelectorAll('button')].find(
    (b) => b.textContent.trim() === 'Continuar'
  );
  if (!continuar) return JSON.stringify({ error: 'no hay botón Continuar' });
  continuar.click();
  if (!(await aparece('[data-nb-bloque]')))
    return JSON.stringify({ error: 'el texto no apareció' });
  await esperar(1000);

  const tecla = (key) => window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
  const resumen = (valores) => {
    const orden = [...valores].sort((a, b) => a - b);
    const q = (p) => Math.round((orden[Math.floor(orden.length * p)] ?? 0) * 10) / 10;
    return { mediana: q(0.5), p95: q(0.95), peor: Math.round((orden.at(-1) ?? 0) * 10) / 10 };
  };

  const huecos = [];
  await fotograma();
  let previo = performance.now();
  for (let i = 0; i < 60; i += 1) {
    await fotograma();
    const ahora = performance.now();
    huecos.push(ahora - previo);
    previo = ahora;
  }
  const intervalo = resumen(huecos).mediana;

  const manejador = [];
  const efectos = [];
  const hastaElSiguiente = [];
  let tarde = 0;
  for (let i = 0; i < 40; i += 1) {
    await esperar(30);
    await fotograma();
    const t0 = performance.now();
    tecla('ArrowDown');
    const t1 = performance.now();
    await new Promise((r) => queueMicrotask(r));
    const t2 = performance.now();
    await fotograma();
    const transcurrido = performance.now() - t0;
    manejador.push(t1 - t0);
    efectos.push(t2 - t1);
    hastaElSiguiente.push(transcurrido);
    if (transcurrido > 1.5 * intervalo) tarde += 1;
  }

  return JSON.stringify({
    bloques: document.querySelectorAll('[data-nb-bloque]').length,
    cursor: document.activeElement?.getAttribute('data-nb-bloque') ?? null,
    intervalo,
    manejador: resumen(manejador),
    efectos: resumen(efectos),
    hastaElSiguiente: resumen(hastaElSiguiente),
    tarde: `${tarde} de 40`
  });
})();
