/**
 * Si una fórmula de MathML se pinta como fórmula en WebKitGTK.
 *
 * La suite de interfaz lo comprueba en Chromium, y el producto corre en WebKitGTK:
 * que uno pinte MathML no dice nada del otro. Se ejecuta con el simulador de la
 * suite y el capítulo de fórmulas que fija `src-tauri/tests/epub_lector.rs`:
 *
 *   node scripts/exportar-puente-de-pruebas.mjs /tmp/puente.js \
 *     --opciones "$(node -e "...")"   # ver performance-budgets.md o fase-6
 *   NB_PUENTE=/tmp/puente.js python3 scripts/medir-en-webkitgtk.py \
 *     http://localhost:4174 scripts/guiones/formulas.js
 *
 * Devuelve el espacio de nombres de la fórmula, si el numerador queda encima del
 * denominador, si la raíz lleva su signo y cuánto más alta es la fórmula en
 * bloque que una línea de texto: letras sueltas medirían una línea.
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
  const continuar = [...document.querySelectorAll('button')].find((b) =>
    b.textContent.includes('Continuar')
  );
  continuar?.click();
  const formula = await aparece('.nb-texto math[display="block"]');
  if (!formula) return JSON.stringify({ error: 'no se pintó ninguna fórmula' });
  await esperar(300);

  const fraccion = formula.querySelector('mfrac');
  const [arriba, abajo] = fraccion ? [...fraccion.children] : [];
  const raiz = formula.querySelector('msqrt');
  const linea = document.querySelector('.nb-texto p')?.getBoundingClientRect();
  const caja = formula.getBoundingClientRect();
  return JSON.stringify({
    espacio: formula.namespaceURI,
    apilada: arriba.getBoundingClientRect().bottom <= abajo.getBoundingClientRect().top + 1,
    raiz: raiz.getBoundingClientRect().width > raiz.firstElementChild.getBoundingClientRect().width,
    altoDeLaFormula: Math.round(caja.height),
    altoDeUnaLinea: linea
      ? Math.round(
          Number.parseFloat(getComputedStyle(document.querySelector('.nb-texto p')).lineHeight) || 0
        )
      : null,
    motor: navigator.userAgent
  });
})();
