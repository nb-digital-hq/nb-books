/**
 * Lo que cuesta abrir el mapa del libro en WebKitGTK.
 *
 * Se ejecuta con el simulador de la suite exportado con un capítulo largo y muchas
 * anotaciones:
 *
 *   node scripts/exportar-puente-de-pruebas.mjs /tmp/puente.js --capitulo 1600 \
 *     --opciones '{"anotacionesExtra":1600}'
 *
 * Abre el índice una vez para que el trozo de los paneles ya esté cargado -lo que
 * se mide es pintar, no descargar-, lo cierra, y lo vuelve a abrir diez veces
 * midiendo desde la tecla hasta el primer fotograma con el mapa pintado. Devuelve
 * el intervalo entre fotogramas en reposo, que es la unidad, y la mediana y el
 * máximo de las diez aperturas.
 */
(async () => {
  const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
  const fotograma = () => new Promise((r) => requestAnimationFrame(r));
  const tecla = (key) => window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
  const hasta = async (condicion, ms = 8000) => {
    const t0 = performance.now();
    while (performance.now() - t0 < ms) {
      if (condicion()) return true;
      await fotograma();
    }
    return false;
  };
  [...document.querySelectorAll('button')]
    .find((b) => b.textContent.includes('Continuar'))
    ?.click();
  if (!(await hasta(() => document.querySelectorAll('[data-nb-bloque]').length > 100, 30000)))
    return JSON.stringify({ error: 'no se abrió el capítulo' });
  await esperar(1500);

  const intervalos = [];
  let antes = await new Promise((r) => requestAnimationFrame(r));
  for (let i = 0; i < 30; i += 1) {
    const ahora = await new Promise((r) => requestAnimationFrame(r));
    intervalos.push(ahora - antes);
    antes = ahora;
  }
  intervalos.sort((a, b) => a - b);

  tecla('i');
  await hasta(() => document.querySelector('.nb-mapa'));
  tecla('Escape');
  await hasta(() => !document.querySelector('.nb-mapa'));
  await esperar(500);

  const tiempos = [];
  for (let i = 0; i < 10; i += 1) {
    await fotograma();
    const t0 = performance.now();
    tecla('i');
    await hasta(() => document.querySelector('.nb-mapa'));
    await fotograma();
    tiempos.push(performance.now() - t0);
    tecla('Escape');
    await hasta(() => !document.querySelector('.nb-mapa'));
    await esperar(300);
  }
  tiempos.sort((a, b) => a - b);
  return JSON.stringify({
    intervalo: Math.round(intervalos[15] * 10) / 10,
    mediana: Math.round(tiempos[5] * 10) / 10,
    maximo: Math.round(tiempos[9] * 10) / 10,
    marcas: document.querySelectorAll('[data-nb-bloque]').length,
    motor: navigator.userAgent.slice(-40)
  });
})();
