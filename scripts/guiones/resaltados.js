/**
 * Lo que cuesta cambiar el documento con marcas en un capítulo de 1.600 párrafos,
 * en WebKitGTK.
 *
 * # Qué mide
 *
 * 1. `sinMarcas`: 0 marcas en `.nb-capa-marcas` y 0 en `CSS.highlights`.
 * 2. `capaPropia_20`: 20 marcas pintadas en `.nb-capa-marcas` (el mecanismo de
 * los resaltados en `ReadingSurface.svelte`), con `CSS.highlights.size === 0`.
 * 3. `cssHighlights_1`, `cssHighlights_5`, `cssHighlights_20`: el mecanismo
 *    anterior (`CSS.highlights`), donde cualquier cambio de texto o inserción de
 *    nodo costaba en proporción al número de rangos y al tamaño del capítulo.
 *
 * Uso:
 *   node scripts/exportar-puente-de-pruebas.mjs /tmp/puente.js --capitulo 1600
 *   NB_PUENTE=/tmp/puente.js python3 scripts/medir-en-webkitgtk.py \
 *     http://localhost:4174 scripts/guiones/resaltados.js
 */
(async () => {
  const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
  const fotograma = () => new Promise((r) => requestAnimationFrame(r));
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
  await hasta(() => document.querySelectorAll('[data-nb-bloque]').length >= 3, 30000);
  await esperar(1500);

  const reg = CSS.highlights;
  const highlightsAlAbrir = reg.size;
  const set0 = reg.set.bind(reg);
  const lienzo = document.querySelector('.nb-texto');
  const bloques = [...document.querySelectorAll('[data-nb-bloque]')];
  const rango = (el) => {
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    const t = walker.nextNode();
    const r = document.createRange();
    r.setStart(t, 0);
    r.setEnd(t, Math.min(25, t.length));
    return r;
  };
  const medir = async (accion) => {
    const xs = [];
    for (let i = 0; i < 7; i += 1) {
      await fotograma();
      await esperar(60);
      const t0 = performance.now();
      accion(i);
      await fotograma();
      await fotograma();
      xs.push(performance.now() - t0);
    }
    return Math.round(xs.sort((a, b) => a - b)[3]);
  };

  let capa = lienzo.querySelector('.nb-capa-marcas');
  if (!capa) {
    capa = document.createElement('div');
    capa.className = 'nb-capa-marcas';
    capa.setAttribute('aria-hidden', 'true');
    lienzo.append(capa);
  }

  const pintarEnCapaPropia = (n) => {
    capa.replaceChildren();
    if (!n) return;
    const cajaLienzo = lienzo.getBoundingClientRect();
    const elegidos = bloques.filter((_, i) => i % 50 === 0).slice(0, n);
    elegidos.forEach((el, idx) => {
      const rg = rango(el);
      const rect = rg.getBoundingClientRect();
      const span = document.createElement('span');
      span.className = 'nb-marca-item';
      span.dataset.nbMarca = idx % 3 === 0 ? 'note' : idx % 3 === 1 ? 'underline' : 'highlight';
      span.dataset.nbMarcaId = `m-${idx}`;
      span.style.left = `${rect.left - cajaLienzo.left + lienzo.scrollLeft}px`;
      span.style.top = `${rect.top - cajaLienzo.top + lienzo.scrollTop}px`;
      span.style.width = `${rect.width}px`;
      span.style.height = `${rect.height}px`;
      capa.append(span);
    });
  };

  const caja = document.createElement('div');
  document.body.append(caja);
  const r = {
    parrafos: bloques.length,
    highlightsAlAbrir
  };

  // 1. Sin marcas (0 en capa propia, 0 en CSS.highlights)
  reg.clear();
  pintarEnCapaPropia(0);
  r.insertarNodo_0 = await medir(() => caja.append(document.createElement('span')));
  r.cambiarTexto_0 = await medir((i) => {
    caja.textContent = String(i);
  });
  r.cambiarAtributo_0 = await medir((i) => {
    caja.dataset.x = String(i);
  });
  r.cambiarEstiloFijo_0 = await medir((i) => {
    caja.style.width = `${i % 2 ? 10 : 20}px`;
  });

  // 2. Con 20 marcas en `.nb-capa-marcas` y 0 en `CSS.highlights`
  reg.clear();
  pintarEnCapaPropia(20);
  r.marcasEnCapaPropia = capa.querySelectorAll('.nb-marca-item').length;
  r.highlightsConCapaPropia = reg.size;
  r.insertarNodo_capa20 = await medir(() => caja.append(document.createElement('span')));
  r.cambiarTexto_capa20 = await medir((i) => {
    caja.textContent = String(i);
  });
  r.cambiarAtributo_capa20 = await medir((i) => {
    caja.dataset.x = String(i);
  });
  r.cambiarEstiloFijo_capa20 = await medir((i) => {
    caja.style.width = `${i % 2 ? 10 : 20}px`;
  });

  // 3. Comparación contra `CSS.highlights` con 1, 5 y 20 rangos
  pintarEnCapaPropia(0);
  for (const n of [1, 5, 20]) {
    reg.clear();
    set0(
      'prueba',
      new Highlight(
        ...bloques
          .filter((_, i) => i % 50 === 0)
          .slice(0, n)
          .map(rango)
      )
    );
    r[`insertarNodo_highlights${n}`] = await medir(() =>
      caja.append(document.createElement('span'))
    );
    r[`cambiarTexto_highlights${n}`] = await medir((i) => {
      caja.textContent = String(i);
    });
  }
  reg.clear();

  return JSON.stringify(r);
})();
