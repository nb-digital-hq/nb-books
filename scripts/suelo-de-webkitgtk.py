#!/usr/bin/env python3
"""El suelo del motor: lo que gasta WebKitGTK pintando una sola línea.

# Por qué existe

Porque el presupuesto de memoria en reposo de la Fase 0 -100 MB, "webview ~70 MB,
Rust ~30 MB"- se escribió sin medir el motor, y el motor vacío ya gasta más de
eso. Medido el 2026-09-23 en esta máquina: una ventana de WebKitGTK que solo pinta
"hola" suma **219 MB** de PSS entre sus tres procesos. Con un total a secas no se
distingue una regresión nuestra del ruido del motor -una versión nueva de WebKitGTK,
otro controlador gráfico-, así que K12 se respondió con dos cifras: el total y **lo
nuestro**, que es la diferencia contra este suelo medido el mismo día en la misma
máquina. Ver `docs/architecture/performance-budgets.md`, sección 1.

# Qué imita, y qué no

Lo que Tauri monta en Linux: GTK 3, WebKitGTK 4.1 -el mismo que enlaza la
aplicación-, una ventana del mismo tamaño que la de `tauri.conf.json` y un contexto
con sus datos en disco, que es lo que arranca el proceso de red. Los datos van a un
directorio temporal que se borra al salir: esto no escribe en el perfil de nadie.

No imita nada nuestro: ni el runtime de Rust, ni la interfaz, ni sus fuentes. Esa es
justamente la parte que se quiere aislar.

# Cómo se usa

No se usa solo: lo lanza `scripts/medir-memoria-en-reposo.mjs`, que espera a que
diga `listo` y le mide el PSS igual que a la aplicación. Necesita python-gobject y
webkit2gtk-4.1, que ya son dependencias de Tauri en Linux.
"""

import shutil
import signal
import sys
import tempfile

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.1")
from gi.repository import GLib, Gtk, WebKit2  # noqa: E402

# El mismo tamaño que la ventana de la aplicación: la superficie que se compone
# cuesta memoria, y comparar contra una ventana más pequeña inflaría lo nuestro.
ANCHO, ALTO = 1100, 720

datos = tempfile.mkdtemp(prefix="nb-suelo-")
gestor = WebKit2.WebsiteDataManager(
    base_data_directory=f"{datos}/datos", base_cache_directory=f"{datos}/cache"
)
contexto = WebKit2.WebContext.new_with_website_data_manager(gestor)

ventana = Gtk.Window(title="Suelo de WebKitGTK")
ventana.set_default_size(ANCHO, ALTO)
vista = WebKit2.WebView.new_with_context(contexto)
ventana.add(vista)


def al_cargar(_vista, evento):
    if evento == WebKit2.LoadEvent.FINISHED:
        print("listo", flush=True)


vista.connect("load-changed", al_cargar)
ventana.connect("destroy", Gtk.main_quit)
# Quien mide lo para con SIGTERM. Sin esto, el directorio temporal se quedaría: el
# manejador de Python no llega a correr mientras manda el bucle de GTK.
try:
    # GLib 2.84 lo movió a su propio espacio; el nombre viejo sigue, con aviso.
    gi.require_version("GLibUnix", "2.0")
    from gi.repository import GLibUnix  # noqa: E402

    senal = GLibUnix.signal_add
except (ImportError, ValueError):
    senal = GLib.unix_signal_add
senal(GLib.PRIORITY_DEFAULT, signal.SIGTERM, lambda: Gtk.main_quit() or False)
vista.load_html("<!doctype html><html><body><p>hola</p></body></html>", "about:blank")
ventana.show_all()

try:
    Gtk.main()
finally:
    shutil.rmtree(datos, ignore_errors=True)
    sys.exit(0)
