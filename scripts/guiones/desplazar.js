/**
 * Cuánto cuesta desplazar la biblioteca, medido en huecos entre fotogramas.
 *
 * Se mide el hueco y no el tiempo de una función porque lo que se busca es si el
 * navegador llega a los sesenta fotogramas por segundo: 16,7 ms es el
 * presupuesto, y lo que se pinta o se descodifica de más aparece aquí y no en
 * ningún perfil de JavaScript.
 *
 * Con `window.__PORTADA` puesta a una imagen se mide el coste de esa portada.
 * Es lo que descubrió que servir el original de 800x1200 a una rejilla que lo
 * pinta a 188 costaba veinticuatro milisegundos por fotograma.
 */
(async () => {
  const boton = [...document.querySelectorAll('button')].find(
    (b) =>
      (
        b.querySelector('.nb-destino-texto')?.textContent ??
        b.getAttribute('aria-label') ??
        b.textContent
      ).trim() === 'Biblioteca'
  );
  if (!boton) return JSON.stringify({ error: 'no se encontró el destino Biblioteca' });
  boton.click();
  await new Promise((r) => setTimeout(r, 1800));

  const lienzo = document.querySelector('.nb-lienzo');
  if (!lienzo) return JSON.stringify({ error: 'la biblioteca no llegó a pintarse' });

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

  const inicio = performance.now();
  while (performance.now() - inicio < 1500) {
    lienzo.scrollTop += 12;
    await new Promise((r) => requestAnimationFrame(r));
  }
  seguir = false;
  await new Promise((r) => setTimeout(r, 80));

  // Los tres primeros se descartan: incluyen el arranque del propio bucle.
  const utiles = huecos.slice(3).sort((a, b) => a - b);
  const cuantil = (q) => Math.round((utiles[Math.floor(utiles.length * q)] ?? 0) * 10) / 10;

  return JSON.stringify({
    fotogramas: utiles.length,
    mediana: cuantil(0.5),
    p95: cuantil(0.95),
    peor: Math.round((utiles.at(-1) ?? 0) * 10) / 10,
    largos: utiles.filter((t) => t > 20).length,
    nodos: document.querySelectorAll('*').length,
    tarjetas: document.querySelectorAll('.nb-tarjeta').length
  });
})();
