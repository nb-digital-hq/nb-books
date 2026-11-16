#!/usr/bin/env python3
"""Comprueba que el motor de verdad —WebKitGTK— hace lo que la interfaz necesita.

# Por qué existe

Porque la voz de un servicio se sintetizaba bien, el archivo se escribía bien, y
**no sonaba nada**, sin ningún error visible. La causa no se ve leyendo el
código ni corriendo la suite:

    WebKitGTK no reproduce medios servidos por un esquema propio.

Las imágenes sí, porque las decodifica el propio motor. El audio se lo entrega a
GStreamer, que no sabe hablar el esquema de recursos de la aplicación y responde
`MediaError` 4 —fuente no soportada—, así que `play()` es rechazado con
`NotSupportedError`. La suite de interfaz corre en Chromium, donde la dirección
directa funciona perfectamente, de modo que puede estar en verde mientras en la
aplicación real no suena nada.

La solución que usa `src/lib/voz/nube.ts` es traer el audio con `fetch` —lo pide
el motor, que sí sabe el esquema— y entregárselo a GStreamer como `blob:`, que
sí sabe leer. Este guion es lo que lo demostró, y sirve para volver a
demostrarlo si alguien duda.

# Qué comprueba

Dos cosas, y las dos por el mismo motivo: **la suite de interfaz corre en
Chromium y no puede ver ninguna**.

1. **Los medios**, que es lo que costó la función entera.
2. **Las capacidades del motor** que la interfaz da por hechas: `Intl.Segmenter`
   parte el texto en frases y sin él la voz volvería a leer párrafos enteros;
   `Intl.Collator` ordena la biblioteca; `Intl.DisplayNames` nombra los idiomas;
   `ResizeObserver` y `structuredClone` los usan el lector y el puente.

Cada una se comprueba **usándola**, no preguntando si existe: un motor puede
declarar `Intl.Segmenter` y segmentar mal, y eso no lo ve un `typeof`.

# Cómo se usa

    python3 scripts/comprobar-el-motor.py [archivo de audio]

Sin argumento busca uno en la caché de voz de la aplicación. Sale con código
distinto de cero si el motor no cumple algo que la interfaz necesita.
"""

import glob
import json
import os
import sys

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.1")
from gi.repository import GLib, Gio, Gtk, WebKit2  # noqa: E402

HTML = """
<body><script>
window.resultado = "pendiente";
async function probar(fuente) {
  const a = new Audio(fuente);
  try {
    await a.play();
    return {resultado: "SUENA", duracion: a.duration, listo: a.readyState};
  } catch (e) {
    return {resultado: "NO SUENA", error: e.name, medio: a.error ? a.error.code : null};
  }
}
function capacidades() {
  const fallos = [];
  // Se comprueban usándolas: declarar la API y hacerlo mal es un fallo que un
  // `typeof` no ve, y es justo el que nos dejaría leyendo párrafos enteros.
  try {
    const trozos = [...new Intl.Segmenter("es", {granularity: "sentence"})
      .segment("Uno. Dos. Tres.")];
    if (trozos.length !== 3) fallos.push("Intl.Segmenter no parte en frases: " + trozos.length);
  } catch (e) { fallos.push("Intl.Segmenter: " + e.message); }

  try {
    const orden = ["ñ", "n", "o"].sort(new Intl.Collator("es").compare);
    if (orden.join("") !== "nño") fallos.push("Intl.Collator ordena mal: " + orden.join(""));
  } catch (e) { fallos.push("Intl.Collator: " + e.message); }

  try {
    if (!new Intl.DisplayNames(["es"], {type: "language"}).of("en")) {
      fallos.push("Intl.DisplayNames no nombra idiomas");
    }
  } catch (e) { fallos.push("Intl.DisplayNames: " + e.message); }

  try {
    // Con 152.080, que es el tamaño de la biblioteca de referencia. Y no con
    // 3412: en español la agrupación empieza en cinco cifras, así que "3412" sin
    // punto es lo correcto. La primera versión de esta comprobación usaba ese
    // número y daba por roto el motor cuando el error era de la comprobación.
    if (!new Intl.NumberFormat("es").format(152080).includes(".")) {
      fallos.push("Intl.NumberFormat no agrupa los millares");
    }
  } catch (e) { fallos.push("Intl.NumberFormat: " + e.message); }

  try {
    if (structuredClone({a: [1, 2]}).a[1] !== 2) fallos.push("structuredClone no clona");
  } catch (e) { fallos.push("structuredClone: " + e.message); }

  if (typeof ResizeObserver !== "function") fallos.push("sin ResizeObserver");
  return fallos;
}

(async () => {
  const directo = await probar("asset://localhost/audio");
  const respuesta = await fetch("asset://localhost/audio");
  const blob = await probar(URL.createObjectURL(await respuesta.blob()));
  window.resultado = JSON.stringify({
    porElEsquemaPropio: directo,
    comoBlob: blob,
    capacidadesQueFaltan: capacidades()
  }, null, 2);
})();
</script></body>
"""


def audio_de_prueba() -> str:
    if len(sys.argv) > 1:
        return sys.argv[1]
    cache = os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache"))
    encontrados = glob.glob(os.path.join(cache, "nb-books", "voz", "*.mp3"))
    if not encontrados:
        sys.exit("no hay audio en la caché de voz: pasa uno como argumento")
    return encontrados[0]


def main() -> None:
    ruta = audio_de_prueba()
    datos = open(ruta, "rb").read()
    print(f"audio: {ruta} ({len(datos)} bytes)")

    contexto = WebKit2.WebContext.get_default()
    contexto.register_uri_scheme(
        "asset",
        lambda peticion: peticion.finish(
            Gio.MemoryInputStream.new_from_data(datos), len(datos), "audio/mpeg"
        ),
    )

    # La misma política que aplica wry, para medir lo que la aplicación tiene y
    # no una configuración más estricta que la suya.
    politicas = WebKit2.WebsitePolicies(autoplay=WebKit2.AutoplayPolicy.ALLOW)
    ventana = Gtk.Window()
    ventana.set_default_size(320, 200)
    vista = WebKit2.WebView(website_policies=politicas, web_context=contexto)
    ventana.add(vista)
    ventana.show_all()

    def terminado(v, res, _):
        try:
            crudo = v.evaluate_javascript_finish(res).to_string()
            print(crudo)
            informe = json.loads(crudo)
            if informe["capacidadesQueFaltan"]:
                print("\nEl motor no cumple algo que la interfaz necesita.")
                globals()["SALIDA"] = 1
            elif informe["comoBlob"]["resultado"] != "SUENA":
                print("\nEl motor no reproduce el audio ni como blob: la voz no sonará.")
                globals()["SALIDA"] = 1
        except (GLib.Error, ValueError, KeyError) as error:
            print("no se pudo leer el resultado:", error)
            globals()["SALIDA"] = 1
        Gtk.main_quit()

    def cuando_carga(v, evento):
        if evento != WebKit2.LoadEvent.FINISHED:
            return
        GLib.timeout_add_seconds(
            5,
            lambda: (
                v.evaluate_javascript("window.resultado", -1, None, None, None, terminado, None),
                False,
            )[1],
        )

    vista.connect("load-changed", cuando_carga)
    vista.load_html(HTML, "asset://localhost/")
    GLib.timeout_add_seconds(30, lambda: (print("tiempo agotado"), Gtk.main_quit())[1])
    Gtk.main()


SALIDA = 0

if __name__ == "__main__":
    main()
    sys.exit(SALIDA)
