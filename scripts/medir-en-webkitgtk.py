#!/usr/bin/env python3
"""Mide la interfaz en el motor de verdad: WebKitGTK, el mismo que usa Tauri.

# Por qué existe

Porque la suite de interfaz mide en **Chromium** y la aplicación corre en
**WebKitGTK**, y no se parecen en lo que importa. El 2026-09-02, con la
biblioteca desplazándose:

    Chromium    17 ms por fotograma
    WebKitGTK   40 ms por fotograma

Con los mismos nodos, el mismo CSS y los mismos datos. Todo lo que la suite dice
sobre rendimiento está dicho sobre un motor que el producto no usa, y por eso
podía estar en verde mientras la aplicación se veía lenta.

Playwright trae una compilación de WebKit, pero no arranca en Arch sin
bibliotecas del sistema que no vienen con el paquete. Esto usa el WebKitGTK que
**ya está instalado** —el mismo contra el que enlaza Tauri—, así que mide el
motor exacto y no uno parecido.

# Qué hace falta

    python-gobject y webkit2gtk-4.1, que ya son dependencias de Tauri en Linux.

Por eso no está en `pnpm verify`: exige un enlace con GTK que no todo el mundo
que contribuye va a tener, y una verificación que no se puede ejecutar no sirve
para decidir si algo está roto. Es una herramienta de diagnóstico, y su lugar
está documentado en `docs/architecture/performance-budgets.md`.

# Cómo se usa

    NB_E2E=1 pnpm build && NB_E2E=1 pnpm preview --port 4174 &
    python3 scripts/medir-en-webkitgtk.py http://localhost:4174 scripts/guiones/desplazar.js

**En el 4174 y no en el 4173**, que es el de la suite de interfaz. Playwright
reutiliza el servidor que encuentre en su puerto, así que con uno de medida vivo
en el 4173 los recorridos prueban **la compilación de la medida** y no la del
código que se está comprobando. Pasó el 2026-09-25: una mutación "pasó" porque la
prueba nunca vio el código mutado.

El segundo argumento es un guion de JavaScript que devuelve una promesa con lo
que se quiera medir; lo que resuelva se imprime por la salida estándar.

**Nunca se recompila debajo de un servidor en marcha.** `vite preview` indexa sus
archivos al arrancar: recompilado debajo, sigue sirviendo la compilación anterior
o se cae pidiendo un trozo borrado, y lo que se mide no es lo que se cree. Se para
el servidor, se compila y se arranca otro. Pasó el 2026-09-11 y dio por buena una
conclusión falsa: `performance-budgets.md`, sección 4.12.

Con `NB_PUENTE` apuntando a lo que escribe `scripts/exportar-puente-de-pruebas.mjs`
se mide con el simulador de la suite de interfaz, que abre documentos y trae
catálogos.

El puente al runtime se simula aquí, igual que en `e2e/puente.ts`: lo que se mide
es cómo pinta la interfaz, no qué contesta SQLite.
"""

import json
import os
import re
import sys

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.1")
from gi.repository import GLib, Gtk, WebKit2  # noqa: E402

URL = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:4174"
GUION = open(sys.argv[2], encoding="utf8").read() if len(sys.argv) > 2 else "Promise.resolve('ok')"

# Con qué portada se pinta la rejilla.
#
# De fábrica, **una distinta por obra**, generada en la página: es la única forma
# de que el coste de descodificar portadas cuente lo que cuenta. Antes se servía
# una sola para las 152.080, y con eso el instrumento no podía medir lo que se le
# estaba preguntando: ver `performance-budgets.md` sección 4.3.3.
#
# `NB_PORTADA` sigue admitiendo una imagen concreta —o un archivo con su data
# URI— para comparar contra un original de verdad. `NB_PORTADAS_IGUALES=1`
# reproduce a propósito el banco viejo, que es como se comprueba que la
# diferencia existe.
_portada = os.environ.get("NB_PORTADA", "")
if _portada and os.path.isfile(_portada):
    _portada = open(_portada, encoding="utf8").read().strip()
PORTADA = _portada
IGUALES = os.environ.get("NB_PORTADAS_IGUALES", "") == "1"

# Los comandos que devuelven una lista, leídos del puente de verdad.
#
# Sin esto, cada comando nuevo contestaba `null`, y la vista que espera una lista
# se rompía antes de pintarse: el arnés dejaba de medir sin decir por qué. Leerlos
# de `ipc.ts` es lo que impide que vuelva a quedarse atrás.
# Desde que la capa se partió en varios archivos vive en `ipc/index.ts`; con la ruta
# de antes, el arnés no arrancaba.
_IPC = os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "src", "lib", "core", "ipc", "index.ts"
)
LISTAS = sorted(set(re.findall(r"call<[^>]*\[\][^>]*>\(\s*'([a-z_]+)'", open(_IPC, encoding="utf8").read())))

# Un runtime simulado: lo justo para que la biblioteca se pinte llena. Las
# portadas se sirven como una imagen de un píxel salvo que el guion ponga otra
# cosa en `window.__PORTADA`, que es como se mide el coste de una portada real.
PUENTE = (
    'window.__PORTADA = "' + PORTADA + '";\n'
    "window.__IGUALES = " + ("true" if IGUALES else "false") + ";\n"
    "window.__PAUSA = " + os.environ.get("NB_PAUSA", "1500") + ";\n"
    "window.__LISTAS = " + json.dumps(LISTAS) + ";\n"
) + r"""
(() => {
  const OBRAS = 152080;
  const LISTAS = window.__LISTAS;
  const PIXEL = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAf'
    + 'FcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';
  const item = (i) => ({
    editionId: `e${i}`, fileId: `f${i}`, workId: `w${i}`,
    title: `Obra Generada ${i + 1}`,
    authors: i % 3 === 0 ? 'Autor Ejemplo' : 'Otra Persona',
    seriesTitle: null, seriesIndex: null, addedAt: '2026-08-01T00:00:00Z',
    progression: i % 5 === 0 ? 0.42 : null,
    coverPath: `p${i}.jpg`,
    availability: i % 40 === 0 ? 'missing' : 'available',
    copies: i % 30 === 0 ? 2 : 1,
    mediaType: 'application/epub+zip'
  });
  /*
   * Una portada distinta por obra, del tamano al que se sirven de verdad.
   *
   * El motor descodifica **por URL**: cincuenta <img> al mismo sitio son una
   * descodificacion, y cincuenta obras con su portada son cincuenta. Servir una
   * sola no hacia el banco optimista, lo hacia incapaz de fallar.
   */
  const CACHE = new Map();
  let BASE = null;

  /*
   * Una portada distinta por obra, y barata de fabricar.
   *
   * El motor descodifica **por URL**: cincuenta <img> al mismo sitio son una
   * descodificacion, y cincuenta obras con su portada son cincuenta. Servir una
   * sola no hacia el banco optimista, lo hacia incapaz de fallar.
   *
   * Se codifica **un** JPEG con el lienzo y se derivan los demas anadiendole
   * bytes al final, que el decodificador ignora: cada uno es un recurso distinto
   * que hay que descodificar aparte, y fabricarlo cuesta un blob en vez de una
   * codificacion. Codificar uno por obra tardaba mas que el propio arnes.
   */
  const bytesBase = () => {
    if (BASE) return BASE;
    const lienzo = document.createElement('canvas');
    lienzo.width = 400;
    lienzo.height = 600;
    const ctx = lienzo.getContext('2d');
    // Ruido de verdad: un color liso se comprime a nada y no se parece a una
    // cubierta, que es lo que hay que descodificar.
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
    if (window.__PORTADA) return window.__PORTADA;
    const n = window.__IGUALES ? 0 : Number(/p(\d+)\.jpg/.exec(ruta || '')?.[1] ?? 0);
    if (CACHE.has(n)) return CACHE.get(n);
    const cola = new Uint8Array(8);
    for (let i = 0; i < 8; i += 1) cola[i] = (n >> (i * 4)) & 0xff;
    const url = URL.createObjectURL(new Blob([bytesBase(), cola], { type: 'image/jpeg' }));
    CACHE.set(n, url);
    return url;
  };

  const RESPUESTAS = {
    health_check: { ok: true, checks: [] }, component_states: [], recovery_report: null,
    settings_snapshot: [], list_catalogs: [], list_notes: [], library_annotations: [],
    vocabulary: [], reading_stats: { minutesToday: 0, minutesWeek: 0, streakDays: 0, inProgress: 0, documents: OBRAS },
    recommendations: [], library_roots: [], indexing_progress: null,
    search_status: { state: 'ready' },
    network_report: { offline: false, consents: [], log: [], pending: [] },
    voice_status: { available: false, reason: 'sin-sintetizador', voices: [] },
    installed_dictionaries: [], library_shelves: { shelves: [], total: 0 }
  };
  window.__TAURI_INTERNALS__ = {
    invoke: (comando, args) => {
      if (comando === 'browse_library') {
        const q = args?.query ?? {};
        const off = q.offset ?? 0, lim = q.limit ?? 50;
        const items = [];
        for (let i = off; i < Math.min(off + lim, OBRAS); i += 1) items.push(item(i));
        return Promise.resolve({ total: OBRAS, items });
      }
      if (comando in RESPUESTAS) return Promise.resolve(RESPUESTAS[comando]);
      return Promise.resolve(LISTAS.includes(comando) ? [] : null);
    },
    transformCallback: (cb) => { const id = Math.random(); window[`_cb${id}`] = cb; return id; },
    convertFileSrc: (ruta) => portada(ruta)
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
})();
"""

# El runtime simulado de la suite de interfaz, en lugar del de arriba.
#
# El de arriba solo sabe llenar la biblioteca: no abre documentos ni tiene
# catálogos, así que con él no se puede medir el lector ni Descubrir. Con
# `NB_PUENTE` apuntando a lo que escribe `scripts/exportar-puente-de-pruebas.mjs`
# se usa el mismo simulador que `e2e/puente.ts`, y no una copia que se quede atrás.
if os.environ.get("NB_PUENTE"):
    PUENTE = open(os.environ["NB_PUENTE"], encoding="utf8").read()

ventana = Gtk.Window()
ventana.set_default_size(1600, 900)
gestor = WebKit2.UserContentManager()
gestor.add_script(
    WebKit2.UserScript.new(
        PUENTE,
        WebKit2.UserContentInjectedFrames.TOP_FRAME,
        WebKit2.UserScriptInjectionTime.START,
        None,
        None,
    )
)
# **Un contexto efímero, sin caché en disco.** Con el de fábrica, una ejecución podía
# pedir recursos de la compilación anterior que el motor recordaba, y lo que se
# medía no era necesariamente lo que estaba servido. Cada medida parte de cero.
contexto = WebKit2.WebContext.new_ephemeral()
vista = WebKit2.WebView(web_context=contexto, user_content_manager=gestor)
vista.get_settings().set_enable_developer_extras(True)
ventana.add(vista)
ventana.show_all()

estado = {"listo": False}


def evaluar(codigo, al_terminar):
    def cb(fuente, resultado, _datos):
        try:
            valor = fuente.evaluate_javascript_finish(resultado)
            al_terminar(valor.to_string() if valor else None)
        except Exception as error:  # noqa: BLE001
            al_terminar(f"ERROR {error}")

    vista.evaluate_javascript(codigo, -1, None, None, None, cb, None)


def al_cargar(_vista, evento):
    if evento != WebKit2.LoadEvent.FINISHED or estado["listo"]:
        return
    estado["listo"] = True

    def arrancar():
        # El guion es asíncrono y `evaluate_javascript` no sabe devolver una
        # promesa: se lanza, deja lo que resuelva en `window.__r`, y se sondea.
        # El guion es **una expresión** que devuelve una promesa, así que se
        # envuelve entre paréntesis: sin ellos, un punto y coma final lo partiría
        # en dos sentencias y el `then` se colgaría de la nada.
        evaluar(
            f"window.__r = null; Promise.resolve({GUION.strip().rstrip(';')})"
            ".then(v => { window.__r = v; }); 'lanzado'",
            lambda _: None,
        )
        GLib.timeout_add(500, sondear)
        return False

    def sondear():
        def visto(valor):
            if valor and valor != "null":
                print(valor)
                Gtk.main_quit()
            else:
                GLib.timeout_add(500, sondear)

        evaluar("window.__r", visto)
        return False

    # Se espera antes de arrancar: la primera pintada incluye el arranque de la
    # aplicación, y medirla diría más del arranque que del desplazamiento.
    GLib.timeout_add(2500, arrancar)


vista.connect("load-changed", al_cargar)
vista.load_uri(URL)
GLib.timeout_add(120_000, lambda: (print("TIMEOUT"), Gtk.main_quit(), False)[2])
Gtk.main()
