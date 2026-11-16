/**
 * Lo que cuesta Inicio en el motor del producto: desplazarse por sus filas y pasar
 * de página en una.
 *
 * Es el criterio 2, "fotogramas medidos en WebKitGTK". Se mide como
 * `desplazar.js`, en huecos entre fotogramas, y con el aviso de la sección 4.3.0 de
 * performance-budgets.md: dentro de una ventana GTK el hueco lo marca también el
 * compositor, así que las cifras sirven para comparar dentro de una ejecución, y
 * lo que no depende del reloj —nodos, filas— se cuenta aparte.
 *
 * Pasar de página es lo nuevo de este Inicio: cada pulsación rehace el carril de la
 * fila con su animación de entrada, y es donde podía aparecer el coste.
 */
(async () => {
  const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
  const fotograma = () => new Promise((r) => requestAnimationFrame(r));

  if (!document.querySelector('.nb-inicio')) {
    return JSON.stringify({ error: 'Inicio no llegó a pintarse' });
  }
  await esperar(1500);

  /** Huecos entre fotogramas mientras dura `accion`. */
  const medir = async (accion) => {
    const huecos = [];
    let anterior = performance.now();
    let seguir = true;
    const bucle = () => {
      const ahora = performance.now();
      huecos.push(ahora - anterior);
      anterior = ahora;
      if (seguir) requestAnimationFrame(bucle);
    };
    requestAnimationFrame(bucle);
    await accion();
    seguir = false;
    await esperar(80);
    // Los tres primeros incluyen el arranque del propio bucle.
    const utiles = huecos.slice(3).sort((a, b) => a - b);
    const cuantil = (q) => Math.round((utiles[Math.floor(utiles.length * q)] ?? 0) * 10) / 10;
    return {
      fotogramas: utiles.length,
      mediana: cuantil(0.5),
      p95: cuantil(0.95),
      peor: Math.round((utiles.at(-1) ?? 0) * 10) / 10,
      largos: utiles.filter((t) => t > 20).length
    };
  };

  const principal = document.querySelector('main');
  const desplazar = await medir(async () => {
    const t0 = performance.now();
    while (performance.now() - t0 < 1500) {
      principal.scrollTop += 12;
      await fotograma();
    }
  });
  principal.scrollTop = 0;
  await esperar(300);

  const botones = (prefijo) =>
    [...document.querySelectorAll('button[aria-label]')].filter((b) =>
      b.getAttribute('aria-label').startsWith(prefijo)
    );
  const filasConPaginas = botones('Siguientes: ').length;
  let pulsaciones = 0;
  const paginar = await medir(async () => {
    const t0 = performance.now();
    while (performance.now() - t0 < 2000) {
      const boton =
        botones('Siguientes: ').find((b) => !b.disabled) ??
        botones('Anteriores: ').find((b) => !b.disabled);
      if (!boton) break;
      boton.click();
      pulsaciones += 1;
      await esperar(250);
    }
  });

  return JSON.stringify({
    desplazar,
    paginar: { ...paginar, pulsaciones, filasConPaginas },
    filas: document.querySelectorAll('.nb-fila').length,
    nodos: document.querySelectorAll('*').length
  });
})();
