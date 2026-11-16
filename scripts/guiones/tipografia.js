/*
 * ¿Está la interfaz pintada con las tipografías del producto, o con las del
 * sistema?
 *
 * Se comprueba en WebKitGTK porque es el motor real en Linux, y porque la
 * pregunta nació de que durante mucho tiempo la respuesta fue "con las del
 * sistema" sin que nadie lo viera.
 *
 * No basta con `document.fonts.check`: eso dice si la fuente está cargada, no
 * si el texto se está pintando con ella. Se mide además el ancho de una misma
 * cadena con la familia declarada y con una de respaldo: si coinciden al
 * píxel, la declarada no se está aplicando.
 */
(async () => {
  await document.fonts.ready;

  function ancho(familia) {
    const lienzo = document.createElement('canvas').getContext('2d');
    lienzo.font = `16px ${familia}`;
    return lienzo.measureText('Biblioteca — ñáéíóú Wíñdóws 152.080').width;
  }

  const respaldo = ancho('monospace');
  return JSON.stringify(
    {
      cargadas: {
        'IBM Plex Sans': document.fonts.check('16px "IBM Plex Sans"'),
        Literata: document.fonts.check('16px "Literata"')
      },
      // Si el ancho con la familia declarada es el del respaldo, no se aplicó.
      seAplican: {
        'IBM Plex Sans': ancho('"IBM Plex Sans", monospace') !== respaldo,
        Literata: ancho('"Literata", monospace') !== respaldo
      },
      deLaInterfaz: getComputedStyle(document.body).fontFamily,
      caras: document.fonts.size
    },
    null,
    2
  );
})();
