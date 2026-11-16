/**
 * Fotogramas destino a destino: lo que cuesta usar Ajustes, Estudio y el lector
 * en el motor del producto.
 *
 * Es el criterio 2, "los mismos tres criterios del piloto, destino a
 * destino". Se mide con el simulador de la suite de interfaz, que abre documentos
 * y tiene datos en cada destino: ver `scripts/exportar-puente-de-pruebas.mjs`, y
 * exportarlo con `--capitulo` para que el lector tenga algo que recorrer.
 *
 * Mismo aviso que `inicio.js`: el hueco entre fotogramas lo marca también el
 * compositor, así que esto compara, no certifica.
 */
(async () => {
  const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
  const fotograma = () => new Promise((r) => requestAnimationFrame(r));
  const destino = (nombre) =>
    [...document.querySelectorAll('.nb-destino')].find(
      (b) =>
        (
          b.querySelector('.nb-destino-texto')?.textContent ??
          b.getAttribute('aria-label') ??
          ''
        ).trim() === nombre
    );
  const aparece = async (selector, ms = 5000) => {
    const t0 = performance.now();
    while (performance.now() - t0 < ms) {
      const el = document.querySelector(selector);
      if (el) return el;
      await fotograma();
    }
    return null;
  };

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

  const desplazar = (contenedor) =>
    medir(async () => {
      const t0 = performance.now();
      while (performance.now() - t0 < 1500) {
        contenedor.scrollTop += 12;
        await fotograma();
      }
    });

  const pulsarTodos = (botones) =>
    medir(async () => {
      for (const boton of botones) {
        boton.click();
        await esperar(300);
      }
    });

  const resultado = {};
  if (!(await aparece('.nb-inicio')))
    return JSON.stringify({ error: 'Inicio no llegó a pintarse' });
  await esperar(1200);
  const principal = document.querySelector('main');

  // Ajustes: recorrer sus secciones y desplazar la primera.
  destino('Ajustes')?.click();
  const nav = await aparece('.nb-ajustes-nav');
  if (nav) {
    await esperar(500);
    const secciones = [...nav.querySelectorAll('button')];
    resultado.ajustes = {
      secciones: secciones.length,
      cambiarDeSeccion: await pulsarTodos(secciones),
      desplazar: (secciones[0]?.click(), await esperar(400), await desplazar(principal)),
      nodos: document.querySelectorAll('*').length
    };
  } else {
    resultado.ajustes = { error: 'no se pintó' };
  }

  // Estudio: recorrer sus vistas y desplazar.
  destino('Estudio')?.click();
  const pestanas = await aparece('[role="tablist"]');
  if (pestanas) {
    await esperar(500);
    const vistas = [...pestanas.querySelectorAll('[role="tab"]')];
    resultado.estudio = {
      vistas: vistas.length,
      cambiarDeVista: await pulsarTodos(vistas),
      desplazar: (vistas[0]?.click(), await esperar(400), await desplazar(principal)),
      nodos: document.querySelectorAll('*').length
    };
  } else {
    resultado.estudio = { error: 'no se pintó' };
  }

  // Descubrir, si el simulador trae catálogo: abrirlo, entrar en una sección y
  // desplazar. Sin catálogo el destino no se anuncia, así que se dice y se sigue.
  const boton = (texto) =>
    [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === texto);
  const descubrir = destino('Descubrir');
  if (descubrir) {
    descubrir.click();
    const t0 = performance.now();
    while (!boton('Biblioteca de ejemplo') && performance.now() - t0 < 5000) await fotograma();
    const catalogo = boton('Biblioteca de ejemplo');
    if (catalogo) {
      resultado.descubrir = {
        abrirCatalogo: await medir(async () => {
          catalogo.click();
          await esperar(800);
        }),
        entrarEnSeccion: await medir(async () => {
          boton('Abrir')?.click();
          await esperar(800);
        }),
        desplazar: await desplazar(principal),
        nodos: document.querySelectorAll('*').length
      };
    } else {
      resultado.descubrir = { error: 'no apareció el catálogo' };
    }
  } else {
    resultado.descubrir = { omitido: 'sin catálogo: exportar con conCatalogo y catalogoAbierto' };
  }

  // El lector: abrir, desplazar el texto, mover el cursor y abrir y cerrar un panel.
  destino('Inicio')?.click();
  await aparece('.nb-inicio');
  await esperar(600);
  const continuar = [...document.querySelectorAll('button')].find(
    (b) => b.textContent.trim() === 'Continuar'
  );
  if (continuar) {
    const t0 = performance.now();
    continuar.click();
    const bloque = await aparece('[data-nb-bloque]', 8000);
    if (bloque) {
      await fotograma();
      await fotograma();
      const abrir = Math.round(performance.now() - t0);
      await esperar(800);
      let texto = bloque.parentElement;
      while (
        texto &&
        !(
          texto.scrollHeight > texto.clientHeight + 10 &&
          /auto|scroll/.test(getComputedStyle(texto).overflowY)
        )
      ) {
        texto = texto.parentElement;
      }
      const tecla = (key) =>
        window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
      resultado.lector = {
        bloques: document.querySelectorAll('[data-nb-bloque]').length,
        abrirHastaVerTexto: abrir,
        desplazar: texto ? await desplazar(texto) : 'sin contenedor desplazable',
        moverElCursor: await medir(async () => {
          for (let i = 0; i < 40; i += 1) {
            tecla('ArrowDown');
            await esperar(50);
          }
        }),
        abrirYCerrarPanel: await medir(async () => {
          for (let i = 0; i < 4; i += 1) {
            tecla('a');
            await esperar(350);
            tecla('Escape');
            await esperar(350);
          }
        }),
        nodos: document.querySelectorAll('*').length
      };
    } else {
      resultado.lector = { error: 'el texto no apareció' };
    }
  } else {
    resultado.lector = { error: 'no hay botón Continuar' };
  }

  return JSON.stringify(resultado);
})();
