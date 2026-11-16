// Inyectado en la página: herramientas de auditoría en window.__qa.
(() => {
  if (window.__qa) return 'ya';
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    return (
      r.width > 0 &&
      r.height > 0 &&
      cs.visibility !== 'hidden' &&
      cs.display !== 'none' &&
      +cs.opacity !== 0
    );
  };
  const nombre = (el) =>
    (
      el.getAttribute('aria-label') ||
      (el.getAttribute('aria-labelledby') &&
        document.getElementById(el.getAttribute('aria-labelledby'))?.innerText) ||
      el.innerText ||
      el.getAttribute('title') ||
      el.value ||
      ''
    )
      .trim()
      .replace(/\s+/g, ' ')
      .slice(0, 80);
  const ruta = (el) => {
    const p = [];
    for (let e = el; e && e !== document.body && p.length < 4; e = e.parentElement) {
      p.unshift(
        e.tagName.toLowerCase() +
          (e.className && typeof e.className === 'string'
            ? '.' +
              e.className
                .trim()
                .split(/\s+/)
                .filter((c) => !c.startsWith('s-'))
                .slice(0, 2)
                .join('.')
            : '')
      );
    }
    return p.join('>');
  };
  const errores = [];
  const oe = console.error,
    ow = console.warn;
  console.error = (...a) => {
    errores.push(`error: ${a.map(String).join(' ').slice(0, 300)}`);
    oe.apply(console, a);
  };
  console.warn = (...a) => {
    errores.push(`warn: ${a.map(String).join(' ').slice(0, 300)}`);
    ow.apply(console, a);
  };
  addEventListener(
    'error',
    (e) =>
      errores.push(
        `excepcion: ${(e.error?.stack || e.message || e.target?.src || '').slice(0, 300)}`
      ),
    true
  );
  addEventListener('unhandledrejection', (e) =>
    errores.push(`promesa: ${String(e.reason?.message || JSON.stringify(e.reason)).slice(0, 300)}`)
  );
  // Fotogramas largos.
  const largos = [];
  try {
    new PerformanceObserver((l) => {
      for (const e of l.getEntries()) largos.push(Math.round(e.duration));
    }).observe({ type: 'longtask', buffered: true });
  } catch {}
  let ultimo = performance.now();
  const huecos = [];
  const bucle = (t) => {
    const d = t - ultimo;
    if (d > 50) huecos.push(Math.round(d));
    ultimo = t;
    requestAnimationFrame(bucle);
  };
  requestAnimationFrame(bucle);

  window.__qa = {
    errores,
    huecos,
    tomarErrores() {
      const c = errores.splice(0);
      return c;
    },
    tomarHuecos() {
      return huecos.splice(0);
    },
    inventario(raiz = document.body) {
      const ctl = [
        ...raiz.querySelectorAll(
          'button,a[href],[role=button],[role=tab],[role=menuitem],[role=option],[role=switch],[role=checkbox],input,select,textarea,[tabindex]:not([tabindex="-1"])'
        )
      ].filter(visible);
      return ctl.map((el) => ({
        tag: el.tagName.toLowerCase(),
        rol: el.getAttribute('role') || '',
        nombre: nombre(el),
        sinNombre: !nombre(el),
        deshabilitado: !!(el.disabled || el.getAttribute('aria-disabled') === 'true'),
        title: el.getAttribute('title') || '',
        ruta: ruta(el)
      }));
    },
    recortes(raiz = document.body) {
      const out = [];
      for (const el of raiz.querySelectorAll('*')) {
        if (!visible(el) || el.children.length > 0) continue;
        const t = el.innerText?.trim();
        if (!t) continue;
        const cs = getComputedStyle(el);
        if (
          (el.scrollWidth > el.clientWidth + 1 && cs.overflowX !== 'visible') ||
          (el.scrollHeight > el.clientHeight + 2 &&
            (cs.overflowY === 'hidden' || cs.webkitLineClamp !== 'none'))
        )
          out.push({
            texto: t.slice(0, 70),
            ruta: ruta(el),
            ellipsis: cs.textOverflow === 'ellipsis' || cs.webkitLineClamp !== 'none'
          });
      }
      return out;
    },
    // Textos visibles iguales repetidos dentro de un mismo contenedor de fila.
    repetidos(selector) {
      const out = [];
      for (const fila of document.querySelectorAll(selector)) {
        const vistos = {};
        for (const t of fila.querySelectorAll('h3,h4,[class*=titulo],[class*=title]')) {
          const k = t.innerText.trim();
          if (k) vistos[k] = (vistos[k] || 0) + 1;
        }
        const rep = Object.entries(vistos).filter(([, n]) => n > 1);
        if (rep.length)
          out.push({ fila: (fila.querySelector('h2')?.innerText || ruta(fila)).slice(0, 60), rep });
      }
      return out;
    },
    porTexto(
      texto,
      sel = 'button,a,[role=button],[role=tab],[role=menuitem],[role=option],[role=switch]'
    ) {
      return [...document.querySelectorAll(sel)]
        .filter(visible)
        .find((el) => nombre(el) === texto || nombre(el).startsWith(texto));
    },
    async click(texto, esperaMs = 600) {
      const el = typeof texto === 'string' ? this.porTexto(texto) : texto;
      if (!el) return { error: `no encontrado: ${texto}` };
      const t0 = performance.now();
      el.scrollIntoView({ block: 'center' });
      el.click();
      await new Promise((r) => setTimeout(r, esperaMs));
      return {
        ms: Math.round(performance.now() - t0),
        errores: this.tomarErrores(),
        huecos: this.tomarHuecos(),
        foco: nombre(document.activeElement || document.body).slice(0, 40)
      };
    },
    nodos: () => document.querySelectorAll('*').length,
    texto: (n = 2000) =>
      document.querySelector('main')?.innerText.slice(0, n) ?? document.body.innerText.slice(0, n),
    contraste(raiz = document.body) {
      // Contraste aproximado de texto contra el fondo opaco más cercano.
      const lum = (c) => {
        const m = c.match(/[\d.]+/g);
        if (!m) return null;
        const [r, g, b] = m.slice(0, 3).map((v) => {
          v /= 255;
          return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
        });
        return { L: 0.2126 * r + 0.7152 * g + 0.0722 * b, a: m[3] === undefined ? 1 : +m[3] };
      };
      const fondo = (el) => {
        for (let e = el; e; e = e.parentElement) {
          const c = getComputedStyle(e).backgroundColor;
          const l = lum(c);
          if (l && l.a > 0.9) return l.L;
        }
        return 0;
      };
      const out = [];
      for (const el of raiz.querySelectorAll('*')) {
        if (
          !visible(el) ||
          !el.childNodes.length ||
          ![...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())
        )
          continue;
        const f = lum(getComputedStyle(el).color);
        if (!f) continue;
        const b = fondo(el);
        const [a, c] = [Math.max(f.L, b), Math.min(f.L, b)];
        const ratio = (a + 0.05) / (c + 0.05);
        const fs = parseFloat(getComputedStyle(el).fontSize);
        const min = fs >= 24 ? 3 : 4.5;
        if (ratio < min)
          out.push({
            texto: el.innerText.trim().slice(0, 40),
            ratio: +ratio.toFixed(2),
            fs,
            ruta: ruta(el)
          });
      }
      return out;
    }
  };
  return 'ok';
})();
(() => {
  // Pulsa y mide hasta que el contenido principal cambia y se queda quieto 300 ms.
  window.__qa.medir = async function (objetivo, { limite = 15000, sel = 'main' } = {}) {
    const el = typeof objetivo === 'string' ? this.porTexto(objetivo) : objetivo;
    if (!el) return { error: `no encontrado: ${objetivo}` };
    const leer = () => (document.querySelector(sel) || document.body).innerText;
    const antes = leer();
    this.tomarHuecos();
    this.tomarErrores();
    const t0 = performance.now();
    el.click();
    let cambio = null,
      ultimo = antes,
      quieto = performance.now();
    while (performance.now() - t0 < limite) {
      await new Promise((r) => setTimeout(r, 25));
      const ahora = leer();
      if (ahora !== ultimo) {
        ultimo = ahora;
        quieto = performance.now();
        if (cambio === null) cambio = Math.round(performance.now() - t0);
      } else if (cambio !== null && performance.now() - quieto > 300) break;
    }
    return {
      primerCambio: cambio,
      estable: cambio === null ? null : Math.round(quieto - t0),
      huecos: this.tomarHuecos(),
      errores: this.tomarErrores()
    };
  };
})();
(() => {
  window.__qa.escribir = async (sel, texto, { esperar = 0 } = {}) => {
    const el = typeof sel === 'string' ? document.querySelector(sel) : sel;
    if (!el) return { error: `no hay ${sel}` };
    el.focus();
    const set = Object.getOwnPropertyDescriptor(
      el.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype,
      'value'
    ).set;
    set.call(el, texto);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    if (esperar) await new Promise((r) => setTimeout(r, esperar));
    return 'ok';
  };
  window.__qa.tecla = (key, el = document.activeElement, extra = {}) => {
    const o = { key, bubbles: true, cancelable: true, ...extra };
    el.dispatchEvent(new KeyboardEvent('keydown', o));
    el.dispatchEvent(new KeyboardEvent('keyup', o));
    return document.activeElement?.outerHTML.slice(0, 160);
  };
  // Espera hasta que el selector tenga texto que cumple, y devuelve el tiempo.
  window.__qa.hasta = async (fn, limite = 20000) => {
    const t0 = performance.now();
    while (performance.now() - t0 < limite) {
      let v;
      try {
        v = fn();
      } catch {}
      if (v) return { ms: Math.round(performance.now() - t0), v };
      await new Promise((r) => setTimeout(r, 20));
    }
    return { ms: null, timeout: true };
  };
})();
(() => {
  window.__qa.buscar = async function (q) {
    const panel = () =>
      [...document.querySelectorAll('[class*=emergente]')].map((x) => x.innerText).join('\n');
    await this.escribir('.nb-buscador-entrada', '');
    await new Promise((r) => setTimeout(r, 400));
    const antes = panel();
    this.tomarHuecos();
    const t0 = performance.now();
    await this.escribir('.nb-buscador-entrada', q);
    let ult = antes,
      cambio = null,
      quieto = t0;
    while (performance.now() - t0 < 8000) {
      await new Promise((r) => setTimeout(r, 25));
      const p = panel();
      if (p !== ult) {
        ult = p;
        quieto = performance.now();
        cambio ??= Math.round(quieto - t0);
      } else if (cambio !== null && performance.now() - quieto > 500) break;
    }
    return {
      q,
      ms: cambio === null ? null : Math.round(quieto - t0),
      huecos: this.tomarHuecos(),
      res: ult.split('\n').slice(2, 10).join(' | ')
    };
  };
})();
(() => {
  // Pulsa sin observar el DOM (leer innerText fuerza maquetación) y devuelve solo
  // los huecos entre fotogramas durante `ventana` ms.
  window.__qa.limpio = async function (objetivo, ventana = 1500) {
    const el = typeof objetivo === 'string' ? this.porTexto(objetivo) : objetivo;
    if (!el) return { error: 'no encontrado' };
    this.tomarHuecos();
    this.tomarErrores();
    const marcas = [];
    let prev = performance.now();
    const _t0 = prev;
    let vivo = true;
    const f = (t) => {
      marcas.push(Math.round(t - prev));
      prev = t;
      if (vivo) requestAnimationFrame(f);
    };
    requestAnimationFrame(f);
    el.click();
    await new Promise((r) => setTimeout(r, ventana));
    vivo = false;
    const largos = marcas.filter((d) => d > 34);
    return {
      fotogramas: marcas.length,
      largos,
      peor: Math.max(...marcas),
      errores: this.tomarErrores()
    };
  };
})();
(() => {
  // Primera línea de texto visible dentro del área de lectura.
  window.__qa.primeraVisible = () => {
    const cands = [...document.querySelectorAll('p,h1,h2,h3,li,blockquote')].filter((e) => {
      const r = e.getBoundingClientRect();
      return (
        r.top >= 60 && r.top < innerHeight && r.left > 250 && r.width > 0 && e.innerText.trim()
      );
    });
    return cands[0]?.innerText.trim().slice(0, 80);
  };
})();
(() => {
  // Traza del puente: invoke no es reescribible, así que se mira la petición
  // que hace por debajo al protocolo ipc://.
  if (window.__qaFetch) return;
  const orig = window.fetch.bind(window);
  window.__qaFetch = true;
  window.__qa.puente = [];
  window.fetch = async (url, opts) => {
    const u = String(url?.url ?? url);
    if (!/^(ipc:|http:\/\/ipc\.localhost)/.test(u)) return orig(url, opts);
    const cmd = decodeURIComponent(u.replace(/^.*localhost\//, ''));
    const t0 = performance.now();
    const r = await orig(url, opts);
    const reg = { cmd, ms: Math.round(performance.now() - t0), status: r.status };
    const tr = r.headers.get('Tauri-Response');
    if (!r.ok || tr === 'error') {
      try {
        reg.error = (await r.clone().text()).slice(0, 300);
      } catch {}
    }
    if (!cmd.startsWith('plugin:event')) window.__qa.puente.push(reg);
    return r;
  };
  window.__qa.tomarPuente = () => window.__qa.puente.splice(0);
})();
(() => {
  window.__qa.rango = async function (etiqueta, valor, espera = 800) {
    const el = [...document.querySelectorAll('input[type=range]')].find(
      (i) => i.getAttribute('aria-label') === etiqueta
    );
    if (!el) return { error: `sin rango ${etiqueta}` };
    this.tomarHuecos();
    const marcas = [];
    let prev = performance.now(),
      vivo = true;
    const f = (t) => {
      marcas.push(Math.round(t - prev));
      prev = t;
      if (vivo) requestAnimationFrame(f);
    };
    requestAnimationFrame(f);
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
    set.call(el, String(valor));
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
    await new Promise((r) => setTimeout(r, espera));
    vivo = false;
    return {
      peor: Math.max(...marcas),
      largos: marcas.filter((d) => d > 34),
      errores: this.tomarErrores()
    };
  };
})();
(() => {
  // Una pulsación como la recibiría la ventana: keydown sobre el elemento con el foco.
  window.__qa.pulsar = async function (key, extra = {}, espera = 400) {
    const el = document.activeElement || document.body;
    const o = { key, bubbles: true, cancelable: true, composed: true, ...extra };
    el.dispatchEvent(new KeyboardEvent('keydown', o));
    el.dispatchEvent(new KeyboardEvent('keyup', o));
    await new Promise((r) => setTimeout(r, espera));
    return {
      foco: (
        document.activeElement?.getAttribute('aria-label') ||
        document.activeElement?.innerText ||
        document.activeElement?.tagName ||
        ''
      ).slice(0, 50),
      errores: this.tomarErrores()
    };
  };
  window.__qa.vivo = () =>
    [...document.querySelectorAll('[aria-live]')].map((e) => e.innerText.trim()).filter(Boolean);
})();
