/**
 * Si una ilustración vectorial se pinta en WebKitGTK.
 *
 * Como `formulas.js`, pero con el capítulo de `ilustracion.saneada.html`. Devuelve
 * el espacio de nombres del dibujo, si el nombre del degradado recuperó su forma
 * de SVG, y el tamaño pintado del fondo y del trazo: un dibujo que el motor no
 * entiende mide cero.
 */
(async () => {
  const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
  const aparece = async (selector, ms = 8000) => {
    const t0 = performance.now();
    while (performance.now() - t0 < ms) {
      const el = document.querySelector(selector);
      if (el) return el;
      await esperar(50);
    }
    return null;
  };
  [...document.querySelectorAll('button')]
    .find((b) => b.textContent.includes('Continuar'))
    ?.click();
  const svg = await aparece('.nb-texto svg');
  if (!svg) return JSON.stringify({ error: 'no se pintó ningún dibujo' });
  await esperar(300);
  const caja = (s) => svg.querySelector(s)?.getBoundingClientRect();
  return JSON.stringify({
    espacio: svg.namespaceURI,
    degradado: svg.querySelector('defs')?.firstElementChild?.localName ?? null,
    fondo: [Math.round(caja('rect')?.width ?? 0), Math.round(caja('rect')?.height ?? 0)],
    trazo: Math.round(caja('path[marker-end]')?.width ?? 0),
    rellenoDelFondo: getComputedStyle(svg.querySelector('rect')).fill,
    motor: navigator.userAgent
  });
})();
