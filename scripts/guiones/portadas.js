/*
 * Lo que cuesta pasar de página de la biblioteca, con las portadas dentro.
 *
 * Se mide desde que se pide la página hasta que **todas** sus portadas han
 * resuelto `decode()`: descodificar es el trabajo, y `load` llega antes de que
 * ocurra. Sin las portadas, este recorrido cuesta un par de milisegundos y no
 * dice nada; con ellas es donde se ve el coste real de recorrer una biblioteca
 * grande.
 *
 * Se mide también **volver a una página ya vista**, porque es donde una caché de
 * la interfaz se notaría y donde se decide si hace falta.
 *
 *   NB_E2E=1 pnpm build && NB_E2E=1 pnpm preview --port 4174 &
 *   python3 scripts/medir-en-webkitgtk.py http://localhost:4174 scripts/guiones/portadas.js
 *
 * Con `NB_PORTADAS_IGUALES=1` el arnés vuelve a servir una sola portada para
 * todas, que es como estaba el banco antes: sirve para comprobar
 * que la diferencia existe, no para medir nada.
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

    // El ultimo boton de la paginacion es "siguiente": los de en medio son los
    // numeros y el primero es "anterior".
    const siguiente = () => {
      const botones = [...document.querySelectorAll('.nb-paginacion button')];
      return botones[botones.length - 1];
    };
    const anterior = () => document.querySelectorAll('.nb-paginacion button')[0];

    const visiblesAhora = () =>
      [...document.querySelectorAll('.nb-tarjeta .nb-portada img')].filter((img) => {
        const c = img.getBoundingClientRect();
        return c.bottom > 0 && c.top < window.innerHeight;
      });

    const fuentes = (imgs) => imgs.map((i) => i.currentSrc || i.src).join('|');

    /**
     * Lo que cuesta pasar de página, contando desde la pulsación hasta que las
     * portadas que se ven están descodificadas.
     *
     * # Dos trampas que este guion tuvo y que dejaron números falsos
     *
     * La primera: medir en cuanto hay imágenes. Al pulsar, las de la página
     * anterior **siguen montadas**, así que se medían esas y salía cero. Hay que
     * esperar a que el conjunto de fuentes cambie de verdad.
     *
     * La segunda: `decode()` sobre una imagen que Svelte ya desmontó no se resuelve
     * ni se rechaza **nunca**, así que esperarlas colgaba el arnés; con un tope, el
     * tope se convertía en el número que se leía. Midiendo sobre las nuevas no hace
     * falta ningún tope, y si aparece uno es que algo va mal.
     *
     * Y solo las que se ven: las portadas llevan `loading="lazy"`, así que una
     * fuera de la ventana no se carga y `decode()` sobre ella tampoco resuelve.
     */
    const pasarDePagina = async (accion) => {
      const antes = fuentes(visiblesAhora());
      const t0 = performance.now();
      accion();

      let nuevas = [];
      for (let i = 0; i < 400; i += 1) {
        nuevas = visiblesAhora();
        if (nuevas.length > 0 && fuentes(nuevas) !== antes) break;
        await esperar(5);
        nuevas = [];
      }
      if (nuevas.length === 0) return { ms: null, motivo: 'la página no cambió' };

      await Promise.all(nuevas.map((img) => img.decode().catch(() => 'error')));
      return {
        ms: Math.round(performance.now() - t0),
        visibles: nuevas.length,
        distintas: new Set(nuevas.map((i) => i.currentSrc || i.src)).size
      };
    };

    const ida = [];
    for (let p = 0; p < 6; p += 1) {
      const boton = siguiente();
      if (!boton || boton.disabled) break;
      ida.push(await pasarDePagina(() => boton.click()));
    }

    // Volver sobre lo andado: páginas que se acaban de ver, que es donde una caché
    // de la interfaz se notaría y donde se decide si hace falta.
    const vuelta = [];
    for (let p = 0; p < 3; p += 1) {
      const boton = anterior();
      if (!boton || boton.disabled) break;
      vuelta.push(await pasarDePagina(() => boton.click()));
    }

    return JSON.stringify({ ida, vuelta });
  } catch (error) {
    return JSON.stringify({ excepcion: String(error) });
  }
})();
