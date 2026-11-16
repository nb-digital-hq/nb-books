/*
 * En qué se van los 313 ms de pasar de página.
 *
 * La pregunta J8 pedía una caché. Antes de construirla hay que saber qué
 * ahorraría: si el coste está en pedirle los datos al runtime, una caché lo
 * quita entero; si está en rehacer el DOM y volver a descodificar cincuenta
 * portadas, una caché de datos no quita nada y la respuesta es otra.
 *
 * Se miden tres tramos sobre el mismo cambio de página:
 *   1. lo que tarda el runtime en contestar `browse_library`
 *   2. lo que tarda la rejilla en tener nodos nuevos
 *   3. lo que tarda en que esas portadas estén descodificadas
 */
(async () => {
  const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
  try {
    [...document.querySelectorAll('button')]
      .find(
        (b) =>
          (
            b.querySelector('.nb-destino-texto')?.textContent ??
            b.getAttribute('aria-label') ??
            b.textContent
          ).trim() === 'Biblioteca'
      )
      ?.click();
    await esperar(2500);

    // El puente ya está montado: se envuelve para cronometrar solo esta llamada.
    const interno = window.__TAURI_INTERNALS__;
    const original = interno.invoke;
    let msRuntime = 0;
    interno.invoke = (comando, args) => {
      if (comando !== 'browse_library') return original(comando, args);
      const t = performance.now();
      return Promise.resolve(original(comando, args)).then((r) => {
        msRuntime = performance.now() - t;
        return r;
      });
    };

    const visibles = () =>
      [...document.querySelectorAll('.nb-tarjeta .nb-portada img')].filter((img) => {
        const c = img.getBoundingClientRect();
        return c.bottom > 0 && c.top < window.innerHeight;
      });
    const fuentes = (imgs) => imgs.map((i) => i.currentSrc || i.src).join('|');

    /*
     * Una pausa entre páginas, porque sin ella se mide otro escenario.
     *
     * La primera versión pulsaba "siguiente" en cuanto la anterior terminaba de
     * pintarse. Eso no se parece a nadie usando la aplicación, y además hace
     * imposible que sirva de nada adelantar la página siguiente: el adelanto
     * necesita un hueco de inactividad, y ahí no lo hay. Con el adelanto puesto
     * salían lecturas de 115 ms y de 564 en la misma tanda, según si el trabajo
     * adelantado caía encima del cambio o no.
     *
     * `NB_PAUSA` la controla; por defecto milisegundo y medio, que es menos de lo
     * que cualquiera tarda en mirar una página de cincuenta portadas.
     */
    const PAUSA = Number(window.__PAUSA ?? 1500);

    const medida = [];
    for (let p = 0; p < 5; p += 1) {
      await esperar(PAUSA);
      const botones = [...document.querySelectorAll('.nb-paginacion button')];
      const siguiente = botones[botones.length - 1];
      if (!siguiente || siguiente.disabled) break;

      const antes = fuentes(visibles());
      msRuntime = 0;
      const t0 = performance.now();
      siguiente.click();

      let nuevas = [];
      for (let i = 0; i < 400; i += 1) {
        nuevas = visibles();
        if (nuevas.length > 0 && fuentes(nuevas) !== antes) break;
        await esperar(5);
        nuevas = [];
      }
      if (nuevas.length === 0) break;
      const tDom = performance.now();

      await Promise.all(nuevas.map((img) => img.decode().catch(() => 'error')));
      const tFin = performance.now();

      medida.push({
        runtime: Math.round(msRuntime),
        hastaElDom: Math.round(tDom - t0),
        descodificar: Math.round(tFin - tDom),
        total: Math.round(tFin - t0),
        portadas: nuevas.length
      });
    }

    interno.invoke = original;
    return JSON.stringify(medida);
  } catch (e) {
    return JSON.stringify({ excepcion: String(e) });
  }
})();
