/**
 * Cuánto tarda en aparecer cada destino al abrirlo, la primera vez y la segunda.
 *
 * Ahora Estudio, Ajustes, Descubrir y el lector no viajan en el
 * paquete de arranque: su trozo se lee y se compila al abrirlos por primera vez.
 * La sección 4.11 de performance-budgets.md dejó escrito que cuánto cuesta eso en
 * WebKitGTK no se sabía, y esto lo mide: desde la pulsación hasta que dentro de
 * `main` hay algo que no es Inicio, más dos fotogramas para que se pinte. La
 * segunda visita, con el trozo ya cargado, es la referencia; Biblioteca, que va en
 * el arranque, es el control.
 *
 * Lo que no reproduce: aquí los trozos llegan por HTTP desde `vite preview`, y en
 * la aplicación por el protocolo propio de Tauri desde el binario. El motor que los
 * compila sí es el mismo.
 */
(async () => {
  const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
  const fotograma = () => new Promise((r) => requestAnimationFrame(r));
  // Por el nombre y no por el texto del botón: Biblioteca lleva al lado cuántas
  // obras hay, y el texto entero ya no es solo el nombre.
  const destino = (nombre) =>
    [...document.querySelectorAll('.nb-destino')].find(
      (b) =>
        (
          b.querySelector('.nb-destino-texto')?.textContent ??
          b.getAttribute('aria-label') ??
          ''
        ).trim() === nombre
    );

  if (!document.querySelector('.nb-inicio')) {
    return JSON.stringify({ error: 'Inicio no llegó a pintarse' });
  }
  await esperar(1500);

  const abrir = async (nombre) => {
    const boton = destino(nombre);
    if (!boton) return null;
    const t0 = performance.now();
    boton.click();
    for (;;) {
      await fotograma();
      const primero = document.querySelector('main')?.firstElementChild;
      if (primero && !primero.classList.contains('nb-inicio')) break;
      if (performance.now() - t0 > 5000) return -1;
    }
    await fotograma();
    await fotograma();
    const ms = Math.round((performance.now() - t0) * 10) / 10;
    destino('Inicio')?.click();
    await esperar(600);
    return ms;
  };

  const resultado = {};
  for (const nombre of ['Biblioteca', 'Ajustes', 'Estudio']) {
    const primera = await abrir(nombre);
    const segunda = await abrir(nombre);
    resultado[nombre] = { primera, segunda };
  }
  return JSON.stringify(resultado);
})();
