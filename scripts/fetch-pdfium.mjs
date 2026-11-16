#!/usr/bin/env node
/**
 * Obtiene la biblioteca nativa de renderizado de PDF.
 *
 * El repositorio **no versiona binarios**: el gancho de pre-commit los rechaza y
 * `.gitignore` los excluye. Esta es la forma de tenerla sin meterla dentro.
 *
 * Descarga la versión fijada, **verifica su hash antes de tocar nada** y la
 * coloca donde el runtime la busca. Si el hash no coincide, no se extrae: se
 * borra y se falla.
 *
 * Se usa la variante **sin V8**. No es un detalle: `pdfium-v8-*` incluye el
 * motor de JavaScript, y el modelo de amenazas prohíbe expresamente que el
 * intérprete de PDF pueda ejecutar script
 * (`docs/audit/threat-model.md`, sección 3.1). La distinción está
 * en el nombre del archivo, así que es verificable y no una esperanza.
 *
 * La aplicación **funciona sin ejecutar esto**: sin la biblioteca, el
 * componente de PDF queda como no instalado y la interfaz oculta lo que dependa
 * de él. Ver `docs/architecture/component-model.md`.
 *
 * Uso: node scripts/fetch-pdfium.mjs [--check]
 *   --check  solo dice si está y si su hash es el esperado
 */

import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const ROOT = resolve(import.meta.dirname, '..');
const DESTINO = join(ROOT, 'src-tauri', 'vendor', 'pdfium');

/**
 * Versión fijada.
 *
 * Tiene que coincidir con la característica `pdfium_NNNN` de `pdfium-render` en
 * `src-tauri/Cargo.toml`: las vinculaciones y el binario hablan una API concreta,
 * y descuadrarlas produce símbolos que faltan en tiempo de ejecución.
 */
const VERSION = 'chromium/7881';

/**
 * Hashes publicados por el proyecto que distribuye los binarios.
 *
 * Fijarlos convierte "descargar de internet" en "obtener exactamente estos
 * bytes". No sustituye a una firma del editor: verifica **inmutabilidad**, no
 * procedencia. Cuando exista una cadena de firma que verificar, se añade aquí.
 */
const PLATAFORMAS = {
  'linux-x64': {
    archivo: 'pdfium-linux-x64.tgz',
    sha256: '1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d',
    biblioteca: 'lib/libpdfium.so'
  },
  'linux-arm64': {
    archivo: 'pdfium-linux-arm64.tgz',
    sha256: 'ee7f7b7d5468958336a818c1cd580bdd20972846b7377b13f9a923d92d1d4674',
    biblioteca: 'lib/libpdfium.so'
  },
  'darwin-x64': {
    archivo: 'pdfium-mac-x64.tgz',
    sha256: '6dedf83990e0e3d6b7c93c9e7589c5a126b0ae14b7464d76120cff7a26afb18b',
    biblioteca: 'lib/libpdfium.dylib'
  },
  'darwin-arm64': {
    archivo: 'pdfium-mac-arm64.tgz',
    sha256: '52e94ca5aa8847934330daf3f8150c190682c5ca93831468794f8b90d4392e40',
    biblioteca: 'lib/libpdfium.dylib'
  },
  'win32-x64': {
    archivo: 'pdfium-win-x64.tgz',
    sha256: '73cc0de638ac2095e7445bf56a38200a5b7c7ca0e9f4ba144598f2457377ac08',
    biblioteca: 'bin/pdfium.dll'
  }
};

const clave = `${process.platform}-${process.arch}`;
const plataforma = PLATAFORMAS[clave];

if (!plataforma) {
  console.error(
    `\nNo hay una versión fijada de la biblioteca de PDF para ${clave}.\n\n` +
      'Añadirla es añadir una entrada a PLATAFORMAS con el hash que publica el\n' +
      'proyecto de binarios. Deliberadamente no se descarga nada sin hash fijado.\n'
  );
  process.exit(1);
}

const nombreLocal = plataforma.biblioteca.split('/').pop();
const rutaFinal = join(DESTINO, nombreLocal);
const rutaSello = join(DESTINO, 'version.txt');
// Las licencias viajan con la biblioteca: `bundle.resources` copia la carpeta entera al paquete, y redistribuir
// el binario sin sus textos (PDFium, FreeType, ICU, libjpeg-turbo, OpenJPEG y el resto) incumpliría lo que piden.
const rutaLicencia = join(DESTINO, 'LICENSE-pdfium');
const rutaLicencias = join(DESTINO, 'licenses');

const selloEsperado = `${VERSION} ${plataforma.sha256}\n`;
const yaEsta =
  existsSync(rutaFinal) &&
  existsSync(rutaSello) &&
  existsSync(rutaLicencia) &&
  existsSync(join(rutaLicencias, 'pdfium.txt')) &&
  readFileSync(rutaSello, 'utf8') === selloEsperado;

if (process.argv.includes('--check')) {
  if (yaEsta) {
    console.log(`Biblioteca de PDF: presente y al día (${VERSION}).`);
    process.exit(0);
  }
  console.log(
    'Biblioteca de PDF: ausente. La aplicación arranca igual y oculta el PDF.\n' +
      'Para tenerla: pnpm pdfium'
  );
  process.exit(0);
}

if (yaEsta) {
  console.log(`Biblioteca de PDF: ya presente (${VERSION}). Nada que hacer.`);
  process.exit(0);
}

const url =
  `https://github.com/bblanchon/pdfium-binaries/releases/download/` +
  `${VERSION}/${plataforma.archivo}`;

console.log(`Descargando la biblioteca de PDF para ${clave}...`);

const respuesta = await fetch(url, { redirect: 'follow' });
if (!respuesta.ok) {
  console.error(`\nLa descarga falló con estado ${respuesta.status}.\n`);
  process.exit(1);
}

const bytes = Buffer.from(await respuesta.arrayBuffer());
const obtenido = createHash('sha256').update(bytes).digest('hex');

// Antes de escribir nada en disco y antes de extraer nada.
if (obtenido !== plataforma.sha256) {
  console.error(
    `\nEl hash no coincide. No se extrae nada.\n\n` +
      `  esperado: ${plataforma.sha256}\n  obtenido: ${obtenido}\n\n` +
      'O el archivo publicado ha cambiado, o la descarga se manipuló por el camino.\n' +
      'En ninguno de los dos casos se instala.\n'
  );
  process.exit(1);
}

mkdirSync(DESTINO, { recursive: true });
const comprimido = join(DESTINO, plataforma.archivo);
writeFileSync(comprimido, bytes);

// `tar` está en Linux, en macOS y en Windows 10 en adelante. Preferirlo a
// implementar la extracción evita cien líneas de formato para un solo uso.
const extraccion = spawnSync('tar', ['-xzf', comprimido, '-C', DESTINO, plataforma.biblioteca, 'LICENSE', 'licenses'], {
  stdio: 'inherit'
});
rmSync(comprimido, { force: true });

if (extraccion.status !== 0) {
  console.error('\nNo se pudo extraer el archivo descargado.\n');
  process.exit(1);
}

// La biblioteca queda plana en el destino, no dentro de lib/ o bin/.
const extraida = join(DESTINO, plataforma.biblioteca);
if (extraida !== rutaFinal) {
  writeFileSync(rutaFinal, readFileSync(extraida));
  rmSync(join(DESTINO, plataforma.biblioteca.split('/')[0]), {
    recursive: true,
    force: true
  });
}

renameSync(join(DESTINO, 'LICENSE'), rutaLicencia);
writeFileSync(rutaSello, selloEsperado);

console.log(`Biblioteca de PDF instalada: ${rutaFinal}`);
console.log(`Versión ${VERSION}, sin motor de JavaScript.`);
