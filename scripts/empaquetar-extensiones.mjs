#!/usr/bin/env node
/**
 * Construye las extensiones de primera parte y deja cada una lista para instalar.
 *
 * # Por qué existe
 *
 * Porque la carpeta de una extensión en `extensiones/propias/` **son sus
 * fuentes**: el manifiesto, la licencia y el código en Rust, sin el `.wasm` que el
 * manifiesto declara. El `.wasm` no se comita -lo impiden `.gitignore` y el gancho
 * de pre-commit- y se construye en `target/`, que no es la carpeta del paquete.
 *
 * El 2026-09-12 se instalaron las extensiones propias apuntando a esas carpetas.
 * Entraron sin queja, y cada uso respondía "no se encontró el archivo". Ahora  * la instalación rechaza un paquete al que le falta un módulo que declara; esto es
 * la otra mitad: la forma de tener el paquete completo sin montarlo a mano.
 *
 * # Qué hace
 *
 * 1. Compila el espacio de trabajo a componentes (`wasm32-wasip2`), en release.
 * 2. Por cada extensión, copia a `extensiones/propias/dist/<nombre>/` su
 *    manifiesto, su licencia, lo que no es código que haga falta (una extensión
 *    sin proveedores, como un tema, es solo su manifiesto) y **cada módulo que el
 *    manifiesto declara**, con el nombre que declara.
 * 3. Falla si un módulo declarado no salió de la compilación: un paquete a medias
 *    es justo lo que esto existe para no producir.
 *
 * `dist/` no entra al repositorio. Firmar sigue siendo aparte, con la clave del
 * proyecto, que no vive aquí: ver `extensiones/propias/README.md`.
 *
 * Uso:
 *   node scripts/empaquetar-extensiones.mjs
 *   pnpm extensiones:empaquetar
 */

import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';

const RAIZ = resolve(import.meta.dirname, '..', 'extensiones', 'propias');
const DESTINO = join(RAIZ, 'dist');
const OBJETIVO = 'wasm32-wasip2';
const COMPILADOS = join(RAIZ, 'target', OBJETIVO, 'release');

const compilacion = spawnSync(
  'cargo',
  ['build', '--release', '--target', OBJETIVO, '--target-dir', 'target'],
  { cwd: RAIZ, stdio: 'inherit' }
);
if (compilacion.status !== 0) {
  console.error(
    `\nNo se pudieron compilar las extensiones. ¿Está instalado el objetivo?\n  rustup target add ${OBJETIVO}\n`
  );
  process.exit(1);
}

rmSync(DESTINO, { recursive: true, force: true });
mkdirSync(DESTINO, { recursive: true });

const fallos = [];
const hechas = [];

for (const nombre of readdirSync(RAIZ, { withFileTypes: true })) {
  if (!nombre.isDirectory() || ['dist', 'target'].includes(nombre.name)) continue;
  const carpeta = join(RAIZ, nombre.name);
  const archivoDeManifiesto = join(carpeta, 'nb-extension.json');
  if (!existsSync(archivoDeManifiesto)) continue;

  const manifiesto = JSON.parse(readFileSync(archivoDeManifiesto, 'utf8'));
  const paquete = join(DESTINO, nombre.name);
  mkdirSync(paquete, { recursive: true });
  cpSync(archivoDeManifiesto, join(paquete, 'nb-extension.json'));
  if (existsSync(join(carpeta, 'LICENSE'))) cpSync(join(carpeta, 'LICENSE'), join(paquete, 'LICENSE'));
  // El código fuente y su receta viajan con el binario: la revisión del
  // registro (R8) no acepta un módulo que no se pueda reconstruir.
  for (const fuente of ['src', 'Cargo.toml']) {
    if (existsSync(join(carpeta, fuente))) cpSync(join(carpeta, fuente), join(paquete, fuente), { recursive: true });
  }

  const modulos = Object.values(manifiesto.contributes?.providers ?? {});
  for (const modulo of new Set(modulos)) {
    const compilado = join(COMPILADOS, modulo);
    if (!existsSync(compilado)) {
      fallos.push(`${nombre.name}: declara ${modulo} y la compilación no lo produjo`);
      continue;
    }
    cpSync(compilado, join(paquete, modulo));
  }
  hechas.push(`${nombre.name} (${modulos.length} módulos)`);
}

if (fallos.length > 0) {
  console.error('\nPaquetes incompletos:\n');
  for (const fallo of fallos) console.error(`  ${fallo}`);
  process.exit(1);
}

console.log(`\nListas para instalar en ${DESTINO}:`);
for (const hecha of hechas) console.log(`  ${hecha}`);
