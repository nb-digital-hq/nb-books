/**
 * Lo que cuesta pasar página de un EPUB en WebKitGTK.
 *
 * Contra el control de dos fotogramas sin hacer nada, como `paneles.js`. Para el modo
 * paginado se añade la preferencia al simulador exportado:
 *
 *   node scripts/exportar-puente-de-pruebas.mjs /tmp/puente.js --capitulo 1600
 *   sed "s/{ key: 'book_font_size', value: 18,/{ key: 'book_pagination', value: 'paged', \
 *     affectsFirstPaint: false, plane: 'book' }, &/" /tmp/puente.js > /tmp/paginado.js
 *   NB_PUENTE=/tmp/paginado.js python3 scripts/medir-en-webkitgtk.py \
 *     http://localhost:4174 scripts/guiones/pasar-pagina.js
 */
(async () => {
  const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
  const fotograma = () => new Promise((r) => requestAnimationFrame(r));
  const tecla = (key) => window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
  const hasta = async (c, ms = 8000) => {
    const t0 = performance.now();
    while (performance.now() - t0 < ms) {
      if (c()) return true;
      await fotograma();
    }
    return false;
  };
  [...document.querySelectorAll('button')]
    .find((b) => b.textContent.includes('Continuar'))
    ?.click();
  if (!(await hasta(() => document.querySelectorAll('[data-nb-bloque]').length > 100, 30000)))
    return JSON.stringify({ error: 'no se abrió' });
  await esperar(2000);
  const sup = document.querySelector('.nb-superficie');
  const paginado = sup?.dataset.paginado;
  const medir = async (accion) => {
    const xs = [];
    for (let i = 0; i < 15; i += 1) {
      await fotograma();
      await esperar(80);
      const t0 = performance.now();
      accion();
      await fotograma();
      await fotograma();
      xs.push(performance.now() - t0);
    }
    xs.sort((a, b) => a - b);
    return { mediana: Math.round(xs[7]), max: Math.round(xs[14]) };
  };
  const antes = paginado === 'true' ? sup.scrollLeft : sup.scrollTop;
  const nada = await medir(() => tecla('F24'));
  const siguiente = await medir(() => tecla('ArrowRight'));
  const despues = paginado === 'true' ? sup.scrollLeft : sup.scrollTop;
  return JSON.stringify({
    paginado,
    bloques: document.querySelectorAll('[data-nb-bloque]').length,
    avanzo: Math.round(despues - antes),
    dosFotogramasSinNada: nada,
    pasarPagina: siguiente
  });
})();
