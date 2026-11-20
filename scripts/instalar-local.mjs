#!/usr/bin/env node
/**
 * Instala en este equipo el `.tar.gz` recién empaquetado, fuera de la carpeta de
 * datos.
 *
 * Antes la aplicación estaba extraída **dentro** de
 * `~/.local/share/nb-books`, junto a la base: `AppRun` y `usr/` mezclados con 51
 * GB de datos del usuario. Aquí el programa va a `~/.local/opt/nb-books` y los
 * datos se quedan solos donde estaban: este guion no los abre ni los mueve.
 *
 * Ahora lo que se instala es el `.tar.gz` y no el AppImage
 * extraído: en una distribución que ya trae WebKitGTK, llevar otro dentro son
 * 380 MB de bibliotecas que además no son las del sistema.
 *
 * Se extrae primero a una carpeta hermana y se cambia de un solo `rename`: una
 * instalación a medias no puede dejar al usuario sin aplicación. La anterior se
 * borra solo cuando la nueva ya está en su sitio.
 *
 * Uso: pnpm instalar:local   (después de `NO_STRIP=1 pnpm empaquetar:linux`)
 */
import { execFileSync } from 'node:child_process';
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readdirSync,
  renameSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const raiz = join(dirname(fileURLToPath(import.meta.url)), '..');
const carpetaDelPaquete = join(raiz, 'src-tauri/target/release/bundle/targz');
const paquete = existsSync(carpetaDelPaquete)
  ? readdirSync(carpetaDelPaquete).find((n) => n.endsWith('.tar.gz'))
  : undefined;
if (!paquete) {
  console.error('No hay .tar.gz: ejecuta antes NO_STRIP=1 pnpm empaquetar:linux');
  process.exit(1);
}

const opt = join(homedir(), '.local/opt');
const destino = join(opt, 'nb-books');
const nueva = join(opt, 'nb-books.nueva');
const anterior = join(opt, 'nb-books.anterior');
mkdirSync(opt, { recursive: true });
rmSync(nueva, { recursive: true, force: true });
mkdirSync(nueva);

// Sin la carpeta de primer nivel del archivo: su nombre lleva la versión, y la
// instalación vive siempre en el mismo sitio.
execFileSync('tar', ['-xzf', join(carpetaDelPaquete, paquete), '--strip-components=1', '-C', nueva]);
if (!existsSync(join(nueva, 'bin', 'nb-books'))) {
  console.error('El .tar.gz no trae bin/nb-books: no se instala.');
  process.exit(1);
}
/*
 * El lanzador de NB Digital apunta a `AppRun`, que es como se llamaba la entrada
 * cuando esto era un AppImage extraído. Se deja un enlace con ese nombre para no
 * romperle la configuración a otro programa por cambiar el formato de este.
 */
symlinkSync('bin/nb-books', join(nueva, 'AppRun'));

rmSync(anterior, { recursive: true, force: true });
if (existsSync(destino)) renameSync(destino, anterior);
renameSync(nueva, destino);
rmSync(anterior, { recursive: true, force: true });

/*
 * Lo que la instalación antigua dejó dentro de la carpeta de datos. Se borra por
 * nombre, uno a uno: en esa carpeta vive la base del usuario, y un borrado por
 * patrón es justo lo que no se puede permitir ahí.
 */
const datos = join(homedir(), '.local/share/nb-books');
for (const nombre of [
  'AppRun',
  'AppRun.wrapped',
  'apprun-hooks',
  'usr',
  '.DirIcon',
  'NB Books.desktop',
  'NB Books.png',
  'nb-books.png'
]) {
  rmSync(join(datos, nombre), { recursive: true, force: true });
}

const lanzador = join(homedir(), '.local/share/applications/nb-books.desktop');
mkdirSync(dirname(lanzador), { recursive: true });
writeFileSync(
  lanzador,
  [
    '[Desktop Entry]',
    'Type=Application',
    'Name=NB Books',
    'Comment=Plataforma de lectura y aprendizaje digital',
    `Exec="${join(destino, 'bin', 'nb-books')}" %U`,
    `Icon=${join(destino, 'share/icons/hicolor/256x256@2/apps/nb-books.png')}`,
    'Terminal=false',
    'Categories=Education;',
    'StartupWMClass=nb-books',
    ''
  ].join('\n')
);
chmodSync(lanzador, 0o755);
console.log(`Instalada en ${destino}. Los datos siguen en ${datos}.`);
