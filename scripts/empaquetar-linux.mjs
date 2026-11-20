#!/usr/bin/env node
/**
 * Empaqueta NB Books para Linux x86-64 con las dos ediciones en un solo paquete.
 *
 * # Por qué
 *
 * La IA local va diez veces más rápida compilada con AVX2 (`x86-64-v3`), y esa
 * compilación no arranca en un procesador sin ella. Antes eran dos
 * descargas, y quien tenía la universal en un equipo con AVX2 esperaba veinte
 * minutos por un resumen. Ahora el paquete lleva las dos: la universal es
 * `nb-books` y la rápida `nb-books-avx2`, y al arrancar la universal salta a la
 * rápida si el procesador puede (`src-tauri/src/main.rs`).
 *
 * Uso: node scripts/empaquetar-linux.mjs [argumentos de tauri build]
 * Necesita NO_STRIP=1 en una distribución moderna: ver deployment/linux.md, 1.1.
 */
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = fileURLToPath(new URL('..', import.meta.url));
// Por `scripts/tauri.mjs` y no por el CLI directo: ese envoltorio pone en el PATH
// el `patchelf` que necesita linuxdeploy cuando el sistema no lo tiene, y sin él
// el AppImage falla con un "failed to run linuxdeploy" que no dice nada.
const tauri = join(RAIZ, 'scripts', 'tauri.mjs');

function correr(argumentos, entorno) {
  const r = spawnSync(process.execPath, [tauri, ...argumentos], { stdio: 'inherit', cwd: RAIZ, env: { ...process.env, ...entorno } });
  if (r.status !== 0) process.exit(r.status ?? 1);
}

// 1. La rápida, sin empaquetar y en su propio directorio de compilación: con las
//    mismas RUSTFLAGS que la universal, cargo reutilizaría lo compilado sin AVX2.
const destinoRapida = join(RAIZ, 'src-tauri', 'target', 'avx2');
correr(['build', '--no-bundle'], {
  RUSTFLAGS: '-C target-cpu=x86-64-v3',
  CARGO_TARGET_DIR: destinoRapida
});
const binarios = join(RAIZ, 'src-tauri', 'target', 'ediciones');
mkdirSync(binarios, { recursive: true });
copyFileSync(join(destinoRapida, 'release', 'nb-books'), join(binarios, 'nb-books-avx2'));

// 2. GStreamer filtrado y `patchelf`, como pide deployment/linux.md, 2.1: sin las
//    dos variables el AppImage copiaría los trescientos complementos del sistema.
const gst = spawnSync(process.execPath, [join(RAIZ, 'scripts', 'preparar-gstreamer.mjs')], {
  cwd: RAIZ,
  encoding: 'utf8'
});
if (gst.status !== 0) {
  process.stderr.write(gst.stderr ?? '');
  process.exit(gst.status ?? 1);
}
const variablesGst = Object.fromEntries(
  (gst.stdout ?? '')
    .split('\n')
    .filter((l) => l.startsWith('GSTREAMER_'))
    .map((l) => [l.slice(0, l.indexOf('=')), l.slice(l.indexOf('=') + 1)])
);

// 3. La universal, empaquetada con la rápida al lado.
const junto = { '/usr/bin/nb-books-avx2': join(binarios, 'nb-books-avx2') };
const config = {
  bundle: {
    linux: { appimage: { files: junto }, deb: { files: junto }, rpm: { files: junto } }
  }
};
correr(['build', '--config', JSON.stringify(config), ...process.argv.slice(2)], variablesGst);

/*
 * 4. El `.tar.gz` (2026-11-14): el programa solo, sin WebKitGTK dentro.
 *
 * Es el formato que le corresponde a una distribución que no es de `.deb` ni de
 * `.rpm` -Arch y derivadas, la de quien desarrolla esto-: usa las bibliotecas
 * del sistema, pesa lo que el `.deb` y no los 140 MB del AppImage, y se
 * descomprime donde se quiera. El binario busca su motor de PDF y su edición
 * rápida **respecto a sí mismo** (`reader/pdf.rs`, `main.rs`), así que no hace
 * falta lanzador ni variable de entorno.
 *
 * Sale del árbol que el empaquetador ya montó para el `.deb`, no de una lista
 * de archivos propia: con dos listas, el día que el `.deb` gane un recurso el
 * `.tar.gz` saldría sin él y nadie lo vería hasta abrir un PDF.
 */
const { version } = JSON.parse(readFileSync(join(RAIZ, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const paquetes = join(RAIZ, 'src-tauri', 'target', 'release', 'bundle');
const arbolDeb = join(paquetes, 'deb', `NB Books_${version}_amd64`, 'data');
if (!existsSync(join(arbolDeb, 'usr', 'bin', 'nb-books-avx2'))) {
  console.error('El árbol del .deb no está o no trae la edición rápida: no se puede hacer el .tar.gz.');
  process.exit(1);
}
const carpetaTar = join(paquetes, 'targz');
rmSync(carpetaTar, { recursive: true, force: true });
mkdirSync(carpetaTar, { recursive: true });
const raizTar = `nb-books-${version}-linux-x86_64`;
const tar = spawnSync(
  'tar',
  [
    // Orden y dueño fijos: el mismo árbol da el mismo archivo, sea quien sea
    // quien empaquete.
    '--sort=name',
    '--owner=0',
    '--group=0',
    '--numeric-owner',
    '-C',
    arbolDeb,
    `--transform=s,^usr,${raizTar},`,
    '-czf',
    join(carpetaTar, `nb-books_${version}_linux-x86_64.tar.gz`),
    'usr'
  ],
  { stdio: 'inherit' }
);
if (tar.status !== 0) process.exit(tar.status ?? 1);
console.log(`    Bundling nb-books_${version}_linux-x86_64.tar.gz`);
