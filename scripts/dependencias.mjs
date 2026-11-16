/**
 * Lo que viaja dentro de NB Books y no es nuestro: una sola lista.
 *
 * La usan dos guiones que tienen que decir lo mismo: `generate-notices.mjs`,
 * que escribe los avisos de terceros, y `procedencia.mjs`, que escribe el
 * listado de componentes de cada publicación. Con dos listas, la
 * primera dependencia nueva dejaría un documento diciendo una cosa y el otro
 * otra, y los dos se publican juntos.
 */

import { execFileSync } from 'node:child_process';
import { join, resolve } from 'node:path';

const ROOT = resolve(import.meta.dirname, '..');

function ejecutar(cmd, args, cwd) {
  return execFileSync(cmd, args, { cwd, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
}

/** Dependencias de Rust que acaban en el binario. */
export function dependenciasRust() {
  const salida = ejecutar('cargo', ['metadata', '--format-version', '1'], join(ROOT, 'src-tauri'));
  const { packages, resolve: grafo, workspace_members: miembros } = JSON.parse(salida);
  const propios = new Set(miembros);

  return grafo.nodes
    .filter((n) => !propios.has(n.id))
    .map((n) => packages.find((p) => p.id === n.id))
    .filter(Boolean)
    .map((p) => ({
      nombre: p.name,
      version: p.version,
      licencia: p.license ?? (p.license_file ? 'ver archivo de licencia' : 'SIN DECLARAR'),
      repositorio: p.repository ?? ''
    }))
    .sort((a, b) => a.nombre.localeCompare(b.nombre));
}

/** Dependencias de Node incluidas en el bundle del frontend. */
export function dependenciasNode() {
  const salida = ejecutar('pnpm', ['licenses', 'list', '--json', '--prod'], ROOT);
  const porLicencia = JSON.parse(salida);

  const paquetes = [];
  for (const [licencia, entradas] of Object.entries(porLicencia)) {
    for (const entrada of entradas) {
      paquetes.push({
        nombre: entrada.name,
        version: Array.isArray(entrada.versions) ? entrada.versions.join(', ') : entrada.version,
        licencia,
        repositorio: entrada.homepage ?? ''
      });
    }
  }
  return paquetes.sort((a, b) => a.nombre.localeCompare(b.nombre));
}

/**
 * Código de terceros **copiado dentro** del repositorio, no traído por un gestor
 * de paquetes.
 *
 * `cargo metadata` no lo ve, así que sin esta lista la atribución que exigen MIT
 * y Apache-2.0 se quedaría fuera del archivo que se distribuye. Cada entrada
 * apunta a la carpeta donde vive su `PROCEDENCIA.md`, y más abajo se comprueba
 * que no haya ninguna carpeta vendorizada sin entrada: una lista a mano que nadie
 * comprueba se queda corta en la segunda vez.
 */
export const VENDORIZADOS = [
  {
    nombre: 'any-tts',
    version: '0.1.3',
    licencia: 'MIT OR Apache-2.0',
    repositorio: 'https://github.com/TM9657/any-tts',
    // No hay aviso de copyright que reproducir: el repositorio no publica archivo
    // de licencia y el paquete tampoco. La licencia la declara su `Cargo.toml`,
    // que es la declaración del editor y lo que muestra crates.io. Se dice como
    // es en lugar de inventar un titular.
    copyright: 'Sin aviso publicado; la licencia la declara su Cargo.toml',
    carpeta: 'src-tauri/src/voice/kokoro'
  }
];
