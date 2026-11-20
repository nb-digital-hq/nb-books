(async () => {
  [...document.querySelectorAll('button')]
    .find(
      (b) =>
        (
          b.querySelector('.nb-destino-texto')?.textContent ??
          b.getAttribute('aria-label') ??
          b.textContent
        ).trim() === 'Biblioteca'
    )
    .click();
  await new Promise((r) => setTimeout(r, 2500));
  const lienzo = document.querySelector('.nb-lienzo');

  // Trabajo del hilo principal, sin depender del vsync: cuántas veces se puede
  // desplazar y forzar el recálculo de la disposición en un segundo.
  const rendimiento = () => {
    lienzo.scrollTop = 0;
    const t0 = performance.now();
    let vueltas = 0;
    while (performance.now() - t0 < 800) {
      lienzo.scrollTop += 7;
      void lienzo.scrollHeight;
      void document.querySelector('.nb-tarjeta')?.getBoundingClientRect().height;
      vueltas += 1;
    }
    return vueltas;
  };

  const imgs = [...document.querySelectorAll('.nb-portada img')];
  const px = imgs.reduce((a, i) => a + (i.naturalWidth || 0) * (i.naturalHeight || 0), 0);

  return JSON.stringify({
    vueltasPorSegundo: Math.round(rendimiento() / 0.8),
    imagenes: imgs.length,
    pixelesDeOrigen: px,
    memoriaDeMapasDeBitsMB: Math.round((px * 4) / 1048576),
    natural: imgs[0] ? `${imgs[0].naturalWidth}x${imgs[0].naturalHeight}` : 'sin imagen',
    pintado: imgs[0]
      ? `${Math.round(imgs[0].getBoundingClientRect().width)}x${Math.round(imgs[0].getBoundingClientRect().height)}`
      : '-'
  });
})();
