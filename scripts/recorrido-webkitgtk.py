#!/usr/bin/env python3
"""Recorrido de rendimiento en el motor real del producto: WebKitGTK en Linux.

# Por qué existe

La suite de interfaz corre en Chromium y la aplicación en WebKitGTK. No se parecen
en lo que importa para el rendimiento: el mismo desplazamiento sobre la misma
biblioteca daba 17 ms por fotograma en uno y 40 en el otro (sección 4.3.0 de
`docs/architecture/performance-budgets.md`).

Este recorrido comprueba las aserciones deterministas de rendimiento en el motor
de verdad:
  1. Nodos del DOM (< 1500 por vista, sección 2 de presupuestos).
  2. Píxeles de imagen descodificados acotados (miniaturas de 400 px y no
     originales de 600x900 / 800x1200 que inflan la memoria a 183 MB).
  3. Capas de composición permanentes (cero elementos con will-change en reposo).
  4. Fotogramas: se registran y formatean sin decidir aprobado/suspenso, ya que
     el intervalo entre fotogramas lo marca también el compositor del escritorio
     (pregunta abierta I14).

# Dónde corre

Solo en Linux con WebKitGTK (python-gobject y webkit2gtk-4.1) y servidor gráfico.
En otras plataformas o sin las dependencias necesarias se salta de forma limpia
explicando el motivo, de forma análoga a las pruebas de corpus (pregunta abierta I13).

Uso:
  python3 scripts/recorrido-webkitgtk.py
  python3 scripts/recorrido-webkitgtk.py --json
  python3 scripts/recorrido-webkitgtk.py --resumen
  python3 scripts/recorrido-webkitgtk.py --datos-simulados '{"inicio":...}'
"""

import http.server
import json
import os
import subprocess
import sys
import tempfile
import threading

# 1. Comprobación temprana de entorno y dependencias (se salta limpiamente donde no hay WebKitGTK)
if sys.platform != "linux":
    print("Omitiendo recorrido de rendimiento en WebKitGTK: solo aplica en Linux.")
    sys.exit(0)

try:
    import gi

    gi.require_version("Gtk", "3.0")
    gi.require_version("WebKit2", "4.1")
    from gi.repository import GLib, Gtk, WebKit2
except (ImportError, ValueError) as err:
    print(f"Omitiendo recorrido de rendimiento en WebKitGTK: dependencias no disponibles ({err}).")
    sys.exit(0)

# Verificar si hay servidor gráfico disponible
try:
    if not Gtk.init_check()[0]:
        print(
            "Omitiendo recorrido de rendimiento en WebKitGTK: "
            "no se puede abrir el servidor gráfico (DISPLAY / WAYLAND_DISPLAY no disponibles)."
        )
        sys.exit(0)
except Exception as err:
    print(f"Omitiendo recorrido de rendimiento en WebKitGTK: fallo al inicializar GTK ({err}).")
    sys.exit(0)

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
BUILD_DIR = os.path.join(RAIZ, "build-e2e")

# Presupuestos deterministas (performance-budgets.md §2 y §4.3.0)
PRESUPUESTO_NODOS = 1500
PRESUPUESTO_PIXELES_INICIO = 15_000_000
PRESUPUESTO_PIXELES_BIBLIOTECA = 20_000_000
PRESUPUESTO_PIXELES_LECTOR = 5_000_000
PRESUPUESTO_CAPAS_WILL_CHANGE = 0

salida_datos = {"datos": None, "error": None}

if "--datos-simulados" in sys.argv:
    idx = sys.argv.index("--datos-simulados")
    if idx + 1 < len(sys.argv):
        try:
            salida_datos["datos"] = json.loads(sys.argv[idx + 1])
        except Exception as ex:  # noqa: BLE001
            salida_datos["error"] = f"JSON simulado inválido: {ex}"


def ejecutar_recorrido_en_ventana():
    # Compilar build-e2e si no existe todavía
    if not os.path.isdir(BUILD_DIR):
        print("Compilando frontend para pruebas (build-e2e)...", file=sys.stderr)
        env = os.environ.copy()
        env["NB_E2E"] = "1"
        # La salida de la compilación va a la de errores: con `--json`, la estándar
        # es el resultado, y en un árbol recién clonado -sin `build-e2e`- lo que
        # imprime Vite llegaba delante del JSON y quien lo leía fallaba al
        # interpretarlo. No se veía porque la carpeta casi siempre existe.
        subprocess.run(["pnpm", "build"], cwd=RAIZ, env=env, check=True, stdout=sys.stderr)

    # Exportar puente de pruebas con simulador
    puente_fd, puente_path = tempfile.mkstemp(prefix="nb-puente-", suffix=".js")
    os.close(puente_fd)

    try:
        subprocess.run(
            [
                "node",
                os.path.join(RAIZ, "scripts", "exportar-puente-de-pruebas.mjs"),
                puente_path,
                "--capitulo",
                "100",
            ],
            cwd=RAIZ,
            check=True,
            stdout=subprocess.DEVNULL,
        )
        with open(puente_path, "r", encoding="utf-8") as f:
            puente_codigo = f.read()
    finally:
        if os.path.exists(puente_path):
            os.unlink(puente_path)

    # Generador de miniaturas sintéticas para `convertFileSrc` (400x600 px)
    generador_portadas = r"""
    (() => {
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
    })();
    """

    codigo_inyectado = puente_codigo + "\n" + generador_portadas

    class ManejadorE2E(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=BUILD_DIR, **kwargs)

        def log_message(self, format, *args):
            pass

    servidor = http.server.ThreadingHTTPServer(("127.0.0.1", 0), ManejadorE2E)
    hilo_servidor = threading.Thread(target=servidor.serve_forever, daemon=True)
    hilo_servidor.start()
    puerto = servidor.server_address[1]
    url_local = f"http://127.0.0.1:{puerto}/"

    ventana = Gtk.Window()
    ventana.set_default_size(1600, 900)
    gestor_contenido = WebKit2.UserContentManager()
    gestor_contenido.add_script(
        WebKit2.UserScript.new(
            codigo_inyectado,
            WebKit2.UserContentInjectedFrames.TOP_FRAME,
            WebKit2.UserScriptInjectionTime.START,
            None,
            None,
        )
    )
    contexto = WebKit2.WebContext.new_ephemeral()
    vista = WebKit2.WebView(web_context=contexto, user_content_manager=gestor_contenido)
    vista.get_settings().set_enable_developer_extras(True)
    ventana.add(vista)
    ventana.show_all()

    guion_recorrido = r"""
    (async () => {
      const esperar = (ms) => new Promise(r => setTimeout(r, ms));
      const fotograma = () => new Promise(r => requestAnimationFrame(r));
      const aparece = async (selector, ms = 8000) => {
        const t0 = performance.now();
        while (performance.now() - t0 < ms) {
          const el = document.querySelector(selector);
          if (el) return el;
          await fotograma();
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
        await esperar(80);
        const utiles = huecos.slice(3).sort((a, b) => a - b);
        const cuantil = (q) => Math.round((utiles[Math.floor(utiles.length * q)] ?? 0) * 10) / 10;
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
        const pixelesDescodificados = imgs.reduce((acc, img) => acc + (img.naturalWidth || 0) * (img.naturalHeight || 0), 0);
        const capasWillChange = Array.from(document.querySelectorAll('*')).filter((el) => {
          const wc = getComputedStyle(el).willChange;
          return wc !== '' && wc !== 'auto';
        }).length;
        return { nodos, pixelesDescodificados, imagenes: imgs.length, capasWillChange };
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

      // --- 1. INICIO ---
      if (!(await aparece('.nb-inicio'))) {
        return JSON.stringify({ error: 'Inicio no apareció' });
      }
      await esperar(1200);

      const inicioRecursos = contarRecursos();
      const main = document.querySelector('main');
      const inicioFrames = await medirFotogramas(async () => {
        const t0 = performance.now();
        while (performance.now() - t0 < 1000) {
          if (main) main.scrollTop += 12;
          await fotograma();
        }
        if (main) main.scrollTop = 0;
      });
      resultados.inicio = { ...inicioRecursos, fotogramas: inicioFrames };

      // --- 2. BIBLIOTECA ---
      const btnBib = destino('Biblioteca');
      if (btnBib) {
        btnBib.click();
        await aparece('.nb-rejilla');
        await esperar(1500);

        const visibles = [...document.querySelectorAll('.nb-tarjeta .nb-portada img')].filter(img => {
          const r = img.getBoundingClientRect();
          return r.bottom > 0 && r.top < window.innerHeight;
        });
        await Promise.all(visibles.map(img => img.decode().catch(() => 'error')));

        const bibRecursos = contarRecursos();
        const lienzo = document.querySelector('.nb-lienzo') || main;
        const bibFrames = await medirFotogramas(async () => {
          const t0 = performance.now();
          while (performance.now() - t0 < 1000) {
            if (lienzo) lienzo.scrollTop += 12;
            await fotograma();
          }
          if (lienzo) lienzo.scrollTop = 0;
        });
        resultados.biblioteca = { ...bibRecursos, fotogramas: bibFrames };
      } else {
        resultados.biblioteca = { error: 'botón Biblioteca no encontrado' };
      }

      // --- 3. LECTOR ---
      destino('Inicio')?.click();
      await aparece('.nb-inicio');
      await esperar(800);

      const btnContinuar = [...document.querySelectorAll('button')].find(
        (b) => b.textContent.trim() === 'Continuar'
      );
      if (btnContinuar) {
        btnContinuar.click();
        await aparece('[data-nb-bloque]');
        await esperar(1200);

        const lectorRecursos = contarRecursos();
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
        const lectorFrames = await medirFotogramas(async () => {
          const t0 = performance.now();
          while (performance.now() - t0 < 1000) {
            if (texto) texto.scrollTop += 12;
            await fotograma();
          }
        });
        resultados.lector = { ...lectorRecursos, fotogramas: lectorFrames };
      } else {
        resultados.lector = { error: 'botón Continuar no encontrado' };
      }

      return JSON.stringify(resultados);
    })()
    """

    def evaluar_js(codigo, cb_resultado):
        def respuesta(fuente, res, _):
            try:
                val = fuente.evaluate_javascript_finish(res)
                cb_resultado(val.to_string() if val else None)
            except Exception as error:  # noqa: BLE001
                cb_resultado(f"ERROR: {error}")

        vista.evaluate_javascript(codigo, -1, None, None, None, respuesta, None)

    def al_terminar_carga(_vista, evento):
        if evento != WebKit2.LoadEvent.FINISHED:
            return

        def iniciar_recorrido():
            evaluar_js(
                f"window.__r = null; Promise.resolve({guion_recorrido.strip().rstrip(';')})"
                ".then(v => { window.__r = v; }).catch(e => { window.__r = JSON.stringify({excepcion: String(e)}); }); 'lanzado'",
                lambda _: None,
            )
            GLib.timeout_add(400, sondear)
            return False

        def sondear():
            def recibido(valor):
                if valor and valor != "null":
                    try:
                        salida_datos["datos"] = json.loads(valor)
                    except Exception as ex:  # noqa: BLE001
                        salida_datos["error"] = f"Fallo al parsear JSON: {ex} ({valor})"
                    servidor.shutdown()
                    Gtk.main_quit()
                else:
                    GLib.timeout_add(400, sondear)

            evaluar_js("window.__r", recibido)
            return False

        GLib.timeout_add(1500, iniciar_recorrido)

    vista.connect("load-changed", al_terminar_carga)
    vista.load_uri(url_local)

    # Timeout de seguridad de 60 segundos
    GLib.timeout_add(
        60_000,
        lambda: (
            salida_datos.update({"error": "Tiempo de espera agotado (60s)"}),
            servidor.shutdown(),
            Gtk.main_quit(),
            False,
        )[3],
    )

    Gtk.main()


if salida_datos["datos"] is None and salida_datos["error"] is None:
    ejecutar_recorrido_en_ventana()

if salida_datos["error"]:
    print(f"Error en el recorrido WebKitGTK: {salida_datos['error']}", file=sys.stderr)
    sys.exit(1)

datos = salida_datos["datos"]
if not datos:
    print("Error: no se recibieron datos del recorrido WebKitGTK", file=sys.stderr)
    sys.exit(1)

if "--json" in sys.argv:
    print(json.dumps(datos, indent=2))
    sys.exit(0)

# Verificación de presupuestos y reporte
errores = []

vistas_esperadas = [
    ("inicio", "Inicio", PRESUPUESTO_PIXELES_INICIO),
    ("biblioteca", "Biblioteca", PRESUPUESTO_PIXELES_BIBLIOTECA),
    ("lector", "Lector", PRESUPUESTO_PIXELES_LECTOR),
]

lineas_informe = []
lineas_informe.append("=== Recorrido de Rendimiento en WebKitGTK ===")

for clave, etiqueta, presupuesto_px in vistas_esperadas:
    vista_data = datos.get(clave)
    if not vista_data or "error" in vista_data:
        errores.append(f"{etiqueta}: {vista_data.get('error', 'no se obtuvo datos')}")
        continue

    nodos = vista_data["nodos"]
    pixeles = vista_data["pixelesDescodificados"]
    capas = vista_data["capasWillChange"]
    fotogramas = vista_data.get("fotogramas", {})

    # 1. Comprobar presupuesto de nodos DOM (< 1500)
    if nodos > PRESUPUESTO_NODOS:
        errores.append(f"{etiqueta}: {nodos} nodos excede el presupuesto ({PRESUPUESTO_NODOS})")

    # 2. Comprobar presupuesto de píxeles descodificados
    if pixeles > presupuesto_px:
        errores.append(
            f"{etiqueta}: {pixeles:,} píxeles descodificados excede el presupuesto ({presupuesto_px:,})"
        )

    # 3. Comprobar capas de composición permanentes (will-change)
    if capas > PRESUPUESTO_CAPAS_WILL_CHANGE:
        errores.append(f"{etiqueta}: {capas} elementos con will-change permanente en reposo")

    # Registro de fotogramas (informativo, sin decidir aprobado/suspenso)
    total_f = fotogramas.get("total", 0)
    mediana_f = fotogramas.get("mediana", 0)
    p95_f = fotogramas.get("p95", 0)
    peor_f = fotogramas.get("peor", 0)
    largos_f = fotogramas.get("largos", 0)

    mb_mapas = round((pixeles * 4) / (1024 * 1024), 1)
    lineas_informe.append(
        f"  [{etiqueta}]\n"
        f"    Nodos DOM: {nodos} (límite: {PRESUPUESTO_NODOS})\n"
        f"    Píxeles descodificados: {pixeles:,} (~{mb_mapas} MB de mapas de bits, límite: {presupuesto_px:,})\n"
        f"    Capas will-change en reposo: {capas} (límite: {PRESUPUESTO_CAPAS_WILL_CHANGE})\n"
        f"    Fotogramas (registro sin decisión): {total_f} fotogramas, "
        f"mediana: {mediana_f} ms, p95: {p95_f} ms, peor: {peor_f} ms, largos (>20ms): {largos_f}"
    )

if "--resumen" not in sys.argv:
    for linea in lineas_informe:
        print(linea)

if errores:
    print("\nFallo en el presupuesto de rendimiento en WebKitGTK:", file=sys.stderr)
    for err in errores:
        print(f"  - {err}", file=sys.stderr)
    sys.exit(1)

print("\nWebKitGTK: Inicio, Biblioteca y Lector cumplen los presupuestos de nodos, píxeles y capas.")
sys.exit(0)

