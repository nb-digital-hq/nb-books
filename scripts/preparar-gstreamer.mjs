#!/usr/bin/env node
/**
 * Prepara los complementos de GStreamer que viajan dentro del AppImage.
 *
 * # Por qué existe
 *
 * WebKitGTK reproduce todo lo que suena con GStreamer, y el AppImage tiene que
 * llevarlo dentro para funcionar sin instalar nada (2026-09-22: la puerta de
 * empaquetado encontró `GStreamer element appsink not found`). El complemento de
 * `linuxdeploy` que lo mete **copia todos los complementos del sistema**, y en una
 * distribución como Arch eso son cerca de trescientos, incluidos `gst-plugins-ugly`
 * y `gst-libav`, que son GPL. La política de licencias no admite GPL en lo que se
 * distribuye (`dependency-policy.md`, sección 4).
 *
 * Así que se le da una carpeta con **solo los que la aplicación usa**, todos de
 * `gstreamer`, `gst-plugins-base` y `gst-plugins-good` (LGPL-2.1 o posterior,
 * cargados dinámicamente, que es el caso que la política admite con revisión
 * registrada: está en la sección 3.9).
 *
 * # Qué hace
 *
 * Copia la lista cerrada de abajo a `src-tauri/target/gstreamer/plugins`, y el
 * explorador de complementos a `.../helpers`, y dice las dos variables que el
 * complemento de `linuxdeploy` lee. En el CI las escribe en `$GITHUB_ENV`. Falla
 * si falta alguno: un AppImage sin el decodificador de Opus arrancaría y no sonaría,
 * que es justo el defecto que esto existe para impedir.
 *
 * Uso: node scripts/preparar-gstreamer.mjs
 */

import { execFileSync, spawnSync } from 'node:child_process';
import { appendFileSync, chmodSync, copyFileSync, existsSync, mkdirSync, rmSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';

const ROOT = resolve(import.meta.dirname, '..');
const DESTINO = join(ROOT, 'src-tauri/target/gstreamer');

/**
 * Los complementos que la aplicación usa, y para qué. Añadir uno exige comprobar
 * de qué paquete viene: solo `gstreamer`, `-base` y `-good`.
 */
const NECESARIOS = {
  coreelements: 'los elementos básicos de cualquier tubería',
  app: 'appsrc y appsink, por donde WebKit entrega y recoge el audio',
  playback: 'playbin y decodebin, que es lo que monta el <audio>',
  typefindfunctions: 'averiguar qué formato es cada archivo',
  audioconvert: 'convertir entre formatos de muestra',
  audioresample: 'cambiar la frecuencia de muestreo',
  volume: 'el volumen del <audio>',
  audiomixer: 'mezclar, que usa WebAudio para los fondos sonoros',
  interleave: 'intercalar canales, que también usa WebAudio',
  ogg: 'el contenedor del audio preparado y de la voz neuronal',
  opus: 'el códec del audio preparado y de la voz neuronal',
  audioparsers: 'analizar MP3, que es lo que devuelve la voz de un servicio',
  mpg123: 'decodificar MP3',
  id3demux: 'las etiquetas que traen los MP3',
  wavparse: 'WAV, por si un servicio de voz lo devuelve así',
  autodetect: 'elegir la salida de audio del sistema',
  pulseaudio: 'la salida por PulseAudio o PipeWire',
  alsa: 'la salida por ALSA, cuando no hay servidor de sonido'
};

const CARPETAS = [
  process.env.GST_PLUGIN_SYSTEM_PATH,
  '/usr/lib/x86_64-linux-gnu/gstreamer-1.0',
  '/usr/lib/aarch64-linux-gnu/gstreamer-1.0',
  '/usr/lib64/gstreamer-1.0',
  '/usr/lib/gstreamer-1.0'
].filter(Boolean);

const EXPLORADORES = [
  '/usr/lib/x86_64-linux-gnu/gstreamer1.0/gstreamer-1.0/gst-plugin-scanner',
  '/usr/lib/aarch64-linux-gnu/gstreamer1.0/gstreamer-1.0/gst-plugin-scanner',
  '/usr/libexec/gstreamer-1.0/gst-plugin-scanner',
  '/usr/lib/gstreamer-1.0/gst-plugin-scanner'
];

const origen = CARPETAS.find((c) => existsSync(join(c, 'libgstcoreelements.so')));
if (!origen) {
  console.error('No se encuentra la carpeta de complementos de GStreamer del sistema.');
  console.error(`Buscada en: ${CARPETAS.join(', ')}`);
  process.exit(1);
}
const explorador = EXPLORADORES.find((e) => existsSync(e));
if (!explorador) {
  console.error('No se encuentra gst-plugin-scanner, que GStreamer necesita para cargar complementos.');
  process.exit(1);
}

rmSync(DESTINO, { recursive: true, force: true });
const complementos = join(DESTINO, 'plugins');
const ayudantes = join(DESTINO, 'helpers');
mkdirSync(complementos, { recursive: true });
mkdirSync(ayudantes, { recursive: true });

const faltan = [];
for (const nombre of Object.keys(NECESARIOS)) {
  const archivo = `libgst${nombre}.so`;
  const de = join(origen, archivo);
  if (!existsSync(de)) {
    faltan.push(`${archivo} (${NECESARIOS[nombre]})`);
    continue;
  }
  copyFileSync(de, join(complementos, archivo));
}
if (faltan.length > 0) {
  console.error('\nFaltan complementos de GStreamer en el sistema que construye:\n');
  for (const f of faltan) console.error(`  ${f}`);
  console.error('\nSin ellos el AppImage arrancaría y no sonaría. Instala gst-plugins-base y -good.\n');
  process.exit(1);
}
copyFileSync(explorador, join(ayudantes, 'gst-plugin-scanner'));

// `linuxdeploy-plugin-gstreamer.sh` exige `patchelf` en el `PATH` del anfitrión,
// aunque el propio `linuxdeploy-x86_64.AppImage` lo lleva dentro en `usr/bin/patchelf`.
// Si el sistema no tiene `patchelf` instalado aparte, se extrae de ahí a
// `src-tauri/target/gstreamer/bin/patchelf`, que `scripts/tauri.mjs` añade al `PATH`
//.
const tienePatchelf = spawnSync('patchelf', ['--version'], { stdio: 'ignore' }).status === 0;
if (!tienePatchelf) {
  const linuxdeploy = join(homedir(), '.cache', 'tauri', 'linuxdeploy-x86_64.AppImage');
  if (existsSync(linuxdeploy)) {
    const carpetaBin = join(DESTINO, 'bin');
    mkdirSync(carpetaBin, { recursive: true });
    execFileSync(linuxdeploy, ['--appimage-extract', 'usr/bin/patchelf'], {
      cwd: DESTINO,
      stdio: 'ignore'
    });
    const extraido = join(DESTINO, 'squashfs-root', 'usr', 'bin', 'patchelf');
    if (existsSync(extraido)) {
      const destinoPatchelf = join(carpetaBin, 'patchelf');
      copyFileSync(extraido, destinoPatchelf);
      chmodSync(destinoPatchelf, 0o755);
    }
    rmSync(join(DESTINO, 'squashfs-root'), { recursive: true, force: true });
  }
}

const variables = [`GSTREAMER_PLUGINS_DIR=${complementos}`, `GSTREAMER_HELPERS_DIR=${ayudantes}`];
if (process.env.GITHUB_ENV) {
  appendFileSync(process.env.GITHUB_ENV, `${variables.join('\n')}\n`);
}
console.log(
  `GStreamer: ${Object.keys(NECESARIOS).length} complementos de ${origen}, y el explorador.`
);
for (const v of variables) console.log(v);

