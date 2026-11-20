/**
 * Lo que cuesta abrir cada panel del lector en WebKitGTK.
 *
 * Se ejecuta con el simulador de la suite, con el capítulo de siempre o con uno
 * largo:
 *
 *   node scripts/exportar-puente-de-pruebas.mjs /tmp/puente.js [--capitulo 1600]
 *   NB_PUENTE=/tmp/puente.js python3 scripts/medir-en-webkitgtk.py \
 *     http://localhost:4174 scripts/guiones/paneles.js
 *
 * Mide desde la tecla hasta **el segundo fotograma**, que es donde se veía el coste:
 * el panel aparece en el primero y el motor paga en el siguiente. Como referencia,
 * dos fotogramas sin hacer nada son unos 30 ms. Devuelve la mediana de siete
 * aperturas por panel; `null` es un panel que no se abrió con esa tecla.
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
  if (!(await hasta(() => document.querySelectorAll('[data-nb-bloque]').length >= 3, 30000)))
    return JSON.stringify({ error: 'no se abrió' });
  await esperar(1500);
  const r = {};
  for (const k of ['i', 'a', 'm', 't', 'v', 'e', 'n']) {
    tecla(k);
    await hasta(() => document.querySelector('.nb-panel'));
    tecla('Escape');
    await hasta(() => !document.querySelector('.nb-panel'));
    await esperar(400);
    const xs = [];
    for (let i = 0; i < 7; i += 1) {
      await fotograma();
      await esperar(50);
      const t0 = performance.now();
      tecla(k);
      await fotograma();
      await fotograma();
      xs.push(performance.now() - t0);
      const abierto = !!document.querySelector('.nb-panel');
      tecla('Escape');
      await hasta(() => !document.querySelector('.nb-panel'));
      await esperar(300);
      if (!abierto) {
        xs.length = 0;
        break;
      }
    }
    r[k] = xs.length ? Math.round(xs.sort((a, b) => a - b)[3]) : null;
  }
  return JSON.stringify(r);
})();
