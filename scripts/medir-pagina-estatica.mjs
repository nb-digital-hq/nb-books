#!/usr/bin/env node
/**
 * Comparativa reproducible de la página estática en Chromium y WebKitGTK.
 *
 * Mide dos escenarios sin backend de Rust ni SQLite por medio:
 *   1. Página HTML estática con una cuadrícula de 50 tarjetas (356 nodos DOM):
 *      - Tiempo de construcción del árbol DOM (`DocumentFragment` + `appendChild`).
 *      - Tiempo de disposición síncrona (`offsetHeight`).
 *      - Tiempo hasta el primer fotograma pintado (`requestAnimationFrame`).
 *      - Intervalos entre fotogramas durante 1 segundo de desplazamiento continuo.
 *   2. Compilación estática de la aplicación (`build-e2e`) en Inicio, Biblioteca y Lector:
 *      - Nodos DOM, píxeles de imagen descodificados, capas `will-change` y fotogramas.
 *
 * Uso:
 *   node scripts/medir-pagina-estatica.mjs [--json] [--datos-simulados <json>]
 */

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { extname, join, resolve } from 'node:path';

const RAIZ = resolve(import.meta.dirname, '..');
const BUILD_DIR = join(RAIZ, 'build-e2e');

const HTML_ESTATICO = `<!DOCTYPE html>
<html lang="es">
<head>
  <meta charset="utf-8" />
  <title>Página estática de referencia (50 obras)</title>
  <style>
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: system-ui, -apple-system, sans-serif;
      background: #121316;
      color: #e3e2e6;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
      gap: 16px;
      padding: 24px;
      height: 100vh;
      overflow-y: auto;
    }
    .card {
      background: #1e1f23;
      border-radius: 12px;
      padding: 12px;
      display: flex;
      flex-direction: column;
      gap: 8px;
      box-shadow: 0 2px 4px rgba(0, 0, 0, 0.24);
    }
    .thumb {
      width: 100%;
      aspect-ratio: 2 / 3;
      background: #2b2d31;
      border-radius: 8px;
    }
    .title {
      font-size: 14px;
      font-weight: 600;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .author {
      font-size: 12px;
      opacity: 0.72;
    }
    .meta {
      display: flex;
      justify-content: space-between;
      font-size: 11px;
      opacity: 0.56;
    }
  </style>
</head>
<body>
  <div id="grid" class="grid"></div>
  <script>
    window.__medirEstatica = async () => {
      const grid = document.getElementById('grid');
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));

      const t0 = performance.now();
      const frag = document.createDocumentFragment();
      for (let i = 0; i < 50; i += 1) {
        const card = document.createElement('div');
        card.className = 'card';
        card.innerHTML =
          '<div class="thumb"></div>' +
          '<div class="title">Libro de Prueba ' + (i + 1) + '</div>' +
          '<div class="author">Autor Ejemplar</div>' +
          '<div class="meta"><span>EPUB</span><span>' + ((i * 7) % 100) + '%</span></div>';
        frag.appendChild(card);
      }
      grid.appendChild(frag);
      const domMs = Math.round((performance.now() - t0) * 100) / 100;

      const tLayout0 = performance.now();
      void grid.offsetHeight;
      const layoutMs = Math.round((performance.now() - tLayout0) * 100) / 100;

      const tPaint0 = performance.now();
      await new Promise((r) => requestAnimationFrame(r));
      const paintMs = Math.round((performance.now() - tPaint0) * 100) / 100;

      const totalFrioMs = Math.round((domMs + layoutMs + paintMs) * 100) / 100;
      const nodos = document.querySelectorAll('*').length;

      const huecos = [];
      let anterior = performance.now();
      let animando = true;
      const bucle = () => {
        const ahora = performance.now();
        huecos.push(ahora - anterior);
        anterior = ahora;
        if (animando) requestAnimationFrame(bucle);
      };
      requestAnimationFrame(bucle);

      const tInicioScroll = performance.now();
      while (performance.now() - tInicioScroll < 1000) {
        grid.scrollTop += 12;
        await new Promise((r) => requestAnimationFrame(r));
      }
      animando = false;
      await new Promise((r) => setTimeout(r, 50));

      const utiles = huecos.slice(3).sort((a, b) => a - b);
      const cuantil = (q) =>
        Math.round((utiles[Math.floor(utiles.length * q)] ?? 0) * 10) / 10;

      return {
        nodos,
        domMs,
        layoutMs,
        paintMs,
        totalFrioMs,
        fotogramas: {
          total: utiles.length,
          mediana: cuantil(0.5),
          p95: cuantil(0.95),
          peor: Math.round((utiles.at(-1) ?? 0) * 10) / 10,
          largos: utiles.filter((t) => t > 20).length
        }
      };
    };
  </script>
</body>
</html>`;

const SCRIPT_WEBKIT_ESTATICA = `
import gi, json, sys
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib

ok, _ = Gtk.init_check(sys.argv)
if not ok:
    print(json.dumps({"omitido": "sin pantalla gráfica"}))
    sys.exit(0)

html = sys.stdin.read()
win = Gtk.Window()
win.set_default_size(1280, 800)
ctx = WebKit2.WebContext.new_ephemeral()
view = WebKit2.WebView(web_context=ctx)
win.add(view)
win.show_all()

def sondear():
    def cb(fuente, res, _):
        try:
            val = fuente.evaluate_javascript_finish(res)
            s = val.to_string() if val and not val.is_null() else None
            if s and s != 'null':
                print(s)
                Gtk.main_quit()
            else:
                GLib.timeout_add(80, sondear)
        except Exception as e:
            print(json.dumps({"error": str(e)}))
            Gtk.main_quit()
    view.evaluate_javascript('window.__r', -1, None, None, None, cb, None)
    return False

def al_cargar(v, ev):
    if ev == WebKit2.LoadEvent.FINISHED:
        v.evaluate_javascript(
            'window.__r = null; window.__medirEstatica().then(r => { window.__r = JSON.stringify(r); });',
            -1, None, None, None, lambda *a: None, None
        )
        GLib.timeout_add(100, sondear)

view.connect('load-changed', al_cargar)
view.load_html(html, 'http://localhost')
GLib.timeout_add(8000, lambda: (print(json.dumps({"error": "timeout"})), Gtk.main_quit(), False)[2])
Gtk.main()
`;

async function medirEstaticaChromium() {
  const { chromium } = await import('@playwright/test');
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    await page.setContent(HTML_ESTATICO);
    return await page.evaluate(() => window.__medirEstatica());
  } finally {
    await browser.close();
  }
}

function medirEstaticaWebKitGTK() {
  try {
    const salida = execFileSync('python3', ['-c', SCRIPT_WEBKIT_ESTATICA], {
      input: HTML_ESTATICO,
      encoding: 'utf8',
      timeout: 12_000,
      stdio: ['pipe', 'pipe', 'ignore']
    });
    return JSON.parse(salida.trim());
  } catch (error) {
    return { omitido: error instanceof Error ? error.message : String(error) };
  }
}

async function medirAplicacionChromium() {
  if (!existsSync(join(BUILD_DIR, 'index.html'))) {
    return { omitido: 'build-e2e no compilado' };
  }
  const { chromium } = await import('@playwright/test');
  const { montarPuente } = await import('../e2e/puente.ts');

  const MIME = {
    '.html': 'text/html',
    '.js': 'application/javascript',
    '.css': 'text/css',
    '.svg': 'image/svg+xml',
    '.woff2': 'font/woff2'
  };

  const srv = createServer((req, res) => {
    const urlPath = (req.url ?? '/').split('?')[0];
    let p = join(BUILD_DIR, urlPath === '/' ? 'index.html' : urlPath);
    if (!existsSync(p) || statSync(p).isDirectory()) {
      p = join(BUILD_DIR, 'index.html');
    }
    res.writeHead(200, { 'Content-Type': MIME[extname(p)] ?? 'application/octet-stream' });
    res.end(readFileSync(p));
  });

  await new Promise((r) => srv.listen(0, '127.0.0.1', r));
  const addr = srv.address();
  const port = typeof addr === 'object' && addr ? addr.port : 0;

  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
    await montarPuente(page, { parrafos: 100 });
    await page.addInitScript(() => {
      let BASE = null;
      const CACHE = new Map();
      const bytesBase = () => {
        if (BASE) return BASE;
        const lienzo = document.createElement('canvas');
        lienzo.width = 400;
        lienzo.height = 600;
        const ctx = lienzo.getContext('2d');
        for (let k = 0; k < 900; k += 1) {
          ctx.fillStyle = `hsl(${(k * 37) % 360} 70% ${15 + (k % 70)}%)`;
          ctx.fillRect((k * 53) % 400, (k * 71) % 600, 24, 24);
        }
        const b64 = lienzo.toDataURL('image/jpeg', 0.82).split(',')[1];
        const crudo = atob(b64);
        BASE = new Uint8Array(crudo.length);
        for (let i = 0; i < crudo.length; i += 1) BASE[i] = crudo.charCodeAt(i);
        return BASE;
      };
      const portada = (ruta) => {
        const n = Number(/p(\d+)\.jpg/.exec(ruta || '')?.[1] ?? 0);
        if (CACHE.has(n)) return CACHE.get(n);
        const cola = new Uint8Array(8);
        for (let i = 0; i < 8; i += 1) cola[i] = (n >> (i * 4)) & 0xff;
        const url = URL.createObjectURL(new Blob([bytesBase(), cola], { type: 'image/jpeg' }));
        CACHE.set(n, url);
        return url;
      };
      const conectar = () => {
        if (window.__TAURI_INTERNALS__) {
          window.__TAURI_INTERNALS__.convertFileSrc = (ruta) => portada(ruta);
        } else {
          setTimeout(conectar, 10);
        }
      };
      conectar();
    });

    await page.goto(`http://127.0.0.1:${port}/`);
    return await page.evaluate(async () => {
      const esperar = (ms) => new Promise((r) => setTimeout(r, ms));
      const fotograma = () => new Promise((r) => requestAnimationFrame(r));
      const aparece = async (selector, ms = 5000) => {
        const t0 = performance.now();
        while (performance.now() - t0 < ms) {
          const el = document.querySelector(selector);
          if (el) return el;
          await esperar(20);
        }
        return null;
      };
      const medirFotogramas = async (accion) => {
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
        await esperar(50);
        const utiles = huecos.slice(3).sort((a, b) => a - b);
        const cuantil = (q) =>
          Math.round((utiles[Math.floor(utiles.length * q)] ?? 0) * 10) / 10;
        return {
          total: utiles.length,
          mediana: cuantil(0.5),
          p95: cuantil(0.95),
          peor: Math.round((utiles.at(-1) ?? 0) * 10) / 10,
          largos: utiles.filter((t) => t > 20).length
        };
      };
      const contarRecursos = () => {
        const nodos = document.querySelectorAll('*').length;
        const imgs = [...document.querySelectorAll('img')];
        const pixelesDescodificados = imgs.reduce(
          (acc, img) => acc + (img.naturalWidth || 0) * (img.naturalHeight || 0),
          0
        );
        const capasWillChange = Array.from(document.querySelectorAll('*')).filter((el) => {
          const wc = getComputedStyle(el).willChange;
          return wc !== '' && wc !== 'auto';
        }).length;
        return {
          nodos,
          pixelesDescodificados,
          imagenes: imgs.length,
          capasWillChange
        };
      };
      const destino = (nombre) =>
        [...document.querySelectorAll('.nb-destino')].find(
          (b) =>
            (
              b.querySelector('.nb-destino-texto')?.textContent ??
              b.getAttribute('aria-label') ??
              ''
            ).trim() === nombre
        );

      const resultados = {};
      await aparece('.nb-inicio');
      await esperar(400);
      const main = document.querySelector('main');
      resultados.inicio = {
        ...contarRecursos(),
        fotogramas: await medirFotogramas(async () => {
          const t0 = performance.now();
          while (performance.now() - t0 < 800) {
            if (main) main.scrollTop += 12;
            await fotograma();
          }
          if (main) main.scrollTop = 0;
        })
      };

      destino('Biblioteca')?.click();
      await aparece('.nb-rejilla');
      await esperar(500);
      const lienzo = document.querySelector('.nb-lienzo') || main;
      resultados.biblioteca = {
        ...contarRecursos(),
        fotogramas: await medirFotogramas(async () => {
          const t0 = performance.now();
          while (performance.now() - t0 < 800) {
            if (lienzo) lienzo.scrollTop += 12;
            await fotograma();
          }
          if (lienzo) lienzo.scrollTop = 0;
        })
      };

      destino('Inicio')?.click();
      await aparece('.nb-inicio');
      await esperar(400);
      const btnContinuar = [...document.querySelectorAll('button')].find(
        (b) => b.textContent.trim() === 'Continuar'
      );
      btnContinuar?.click();
      await aparece('[data-nb-bloque]');
      await esperar(500);
      let texto = document.querySelector('[data-nb-bloque]')?.parentElement;
      while (
        texto &&
        !(
          texto.scrollHeight > texto.clientHeight + 10 &&
          /auto|scroll/.test(getComputedStyle(texto).overflowY)
        )
      ) {
        texto = texto.parentElement;
      }
      resultados.lector = {
        ...contarRecursos(),
        fotogramas: await medirFotogramas(async () => {
          const t0 = performance.now();
          while (performance.now() - t0 < 800) {
            if (texto) texto.scrollTop += 12;
            await fotograma();
          }
          if (texto) texto.scrollTop = 0;
        })
      };
      return resultados;
    });
  } finally {
    await browser.close();
    srv.close();
  }
}

function medirAplicacionWebKitGTK() {
  try {
    const salida = execFileSync(
      'python3',
      [join(RAIZ, 'scripts', 'recorrido-webkitgtk.py'), '--json'],
      {
        encoding: 'utf8',
        timeout: 20_000,
        stdio: ['ignore', 'pipe', 'ignore']
      }
    );
    return JSON.parse(salida.trim());
  } catch (error) {
    return { omitido: error instanceof Error ? error.message : String(error) };
  }
}

async function main() {
  const args = process.argv.slice(2);
  const modoJson = args.includes('--json');
  const idxSim = args.indexOf('--datos-simulados');

  let resultado;
  if (idxSim !== -1 && args[idxSim + 1]) {
    resultado = JSON.parse(args[idxSim + 1]);
  } else {
    const estaticaChromium = await medirEstaticaChromium();
    const estaticaWebkitgtk = medirEstaticaWebKitGTK();
    const appChromium = await medirAplicacionChromium();
    const appWebkitgtk = medirAplicacionWebKitGTK();

    resultado = {
      paginaEstatica: {
        chromium: estaticaChromium,
        webkitgtk: estaticaWebkitgtk
      },
      aplicacion: {
        chromium: appChromium,
        webkitgtk: appWebkitgtk
      }
    };
  }

  if (modoJson) {
    console.log(JSON.stringify(resultado, null, 2));
    return;
  }

  console.log('=== Comparativa de Página Estática y Aplicación: Chromium vs WebKitGTK ===');
  const ec = resultado.paginaEstatica?.chromium;
  const ew = resultado.paginaEstatica?.webkitgtk;
  if (ec) {
    console.log(
      `  [Página estática 50 tarjetas (${ec.nodos} nodos)]` +
        `\n    Chromium  -> DOM: ${ec.domMs} ms, Disposición: ${ec.layoutMs} ms, Pintado: ${ec.paintMs} ms (Total frío: ${ec.totalFrioMs} ms) | Scroll mediana: ${ec.fotogramas?.mediana} ms`
    );
  }
  if (ew && !ew.omitido && !ew.error) {
    console.log(
      `    WebKitGTK -> DOM: ${ew.domMs} ms, Disposición: ${ew.layoutMs} ms, Pintado: ${ew.paintMs} ms (Total frío: ${ew.totalFrioMs} ms) | Scroll mediana: ${ew.fotogramas?.mediana} ms`
    );
  } else if (ew) {
    console.log(`    WebKitGTK -> Omitido (${ew.omitido ?? ew.error})`);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
