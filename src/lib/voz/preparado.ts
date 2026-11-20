/**
 * Escuchar lo preparado: dónde va el texto mientras suena, y por dónde empezar.
 *
 * Contrato: `docs/architecture/voice-services.md`. El audio preparado se guarda
 * con una marca por fragmento -desde qué milisegundo hasta cuál, y su `Locator`-,
 * que es lo que permite **seguir el texto** mientras suena y **empezar donde se
 * va leyendo**. Antes las marcas se guardaban y solo se contaban: el
 * panel decía "N fragmentos marcados" y no hacía nada con ellas.
 *
 * Aparte del panel para poder probarlo sin la ventana ni el audio.
 */

import { rutaDeImagen } from '$core/ipc';
import type { AudioMark, Locator } from '$lib/bindings';

/**
 * La marca que está sonando en `ms`, o -1 si ninguna.
 *
 * Las marcas van en orden y sin huecos -cada una empieza donde acabó la anterior,
 * lo fija `audio_preparado.rs`-, así que una búsqueda binaria basta y no hay que
 * recorrer las tres mil de un libro cada vez que avanza el reproductor.
 */
export function marcaEn(marcas: readonly AudioMark[], ms: number): number {
  let bajo = 0;
  let alto = marcas.length - 1;
  while (bajo <= alto) {
    const medio = (bajo + alto) >> 1;
    const marca = marcas[medio];
    if (!marca) return -1;
    if (ms < marca.startMs) alto = medio - 1;
    else if (ms >= marca.endMs) bajo = medio + 1;
    else return medio;
  }
  return -1;
}

function sinAncla(href: string): string {
  return href.split('#')[0] ?? href;
}

/**
 * La marca por la que empezar para seguir **donde se va leyendo**.
 *
 * La primera del mismo recurso cuyo avance llega a la posición; si ninguna llega,
 * la última de ese recurso, que es la que la contiene; y si el recurso no está en
 * lo preparado, el principio. En un PDF, donde todo es un recurso, manda la página.
 */
export function marcaDesde(marcas: readonly AudioMark[], actual: Locator): number {
  const pagina = actual.locations.position;
  if (typeof pagina === 'number') {
    const indice = marcas.findIndex(
      (m) =>
        typeof m.locator.locations.position === 'number' && m.locator.locations.position >= pagina
    );
    return Math.max(0, indice);
  }
  const recurso = sinAncla(actual.href);
  const avance = actual.locations.progression ?? 0;
  const delRecurso = marcas
    .map((m, i) => ({ m, i }))
    .filter(({ m }) => sinAncla(m.locator.href) === recurso);
  if (delRecurso.length === 0) return 0;
  const llega = delRecurso.find(
    ({ m }) => (m.locator.locations.progression ?? 0) >= avance - 0.001
  );
  // La que contiene la posición es la anterior a la primera que la pasa.
  if (!llega) return delRecurso[delRecurso.length - 1]?.i ?? 0;
  const antes = delRecurso.filter(({ i }) => i < llega.i).pop();
  const empezada = (llega.m.locator.locations.progression ?? 0) > avance + 0.001;
  return empezada && antes ? antes.i : llega.i;
}

/**
 * El audio de un archivo, **como `blob:`**, que es lo único que WebKitGTK reproduce.
 *
 * WebKitGTK entrega el audio a GStreamer, que no sabe hablar el esquema propio de
 * la aplicación y responde "fuente no soportada" (performance-budgets.md 4.3.1). El
 * panel del capítulo lo reproducía con un `<audio src>` directo hasta el
 * 2026-09-22, y por eso **lo preparado no sonaba en la aplicación** aunque en la
 * suite de interfaz, que corre en Chromium, sí. Quien lo pide tiene que soltarlo
 * con `URL.revokeObjectURL` al terminar.
 */
export async function enBlob(ruta: string): Promise<string> {
  const respuesta = await fetch(rutaDeImagen(ruta));
  if (!respuesta.ok) throw new Error(`no se pudo leer el audio: ${respuesta.status}`);
  return URL.createObjectURL(await respuesta.blob());
}
