#!/usr/bin/env node
/**
 * `pnpm tauri`, con la edición rápida en desarrollo si el procesador la admite.
 *
 * # Por qué existe
 *
 * Los núcleos cuantizados de Candle eligen su versión vectorizada **al compilar**
 * (`#[cfg(target_feature = "avx2")]`), no al ejecutar, y sin `target-cpu` el
 * compilador apunta al x86-64 de 2003. Así que `pnpm tauri dev` compilaba siempre
 * la edición universal: la aplicación de desarrollo generaba diez veces más
 * despacio que la rápida y avisaba en Ajustes de que "esta edición no usa AVX2",
 * en la misma máquina donde se mide la rápida. Se observó el 2026-09-23.
 *
 * # Qué hace
 *
 * Con `dev`, en Linux x86-64 con AVX2 y FMA, compila con `x86-64-v3` -lo mismo que
 * la edición rápida que se empaqueta, no `native`: `native` en una máquina con
 * AVX-512 compilaría algo que ninguna edición publicada es- y **en su propia
 * carpeta**, `target/desarrollo`. Cambiar `RUSTFLAGS` sobre `target/debug`
 * invalidaría lo que comparten `cargo build` y las pruebas, y cada vuelta entre
 * uno y otro recompilaría Tauri, wasmtime y Candle enteros.
 *
 * Todo lo demás pasa tal cual: `pnpm tauri build` sigue construyendo la edición
 * universal, y las otras se piden explícitamente (ver `.github/workflows/ediciones.yml`).
 *
 * # Cómo no usarla
 *
 * `NB_EDICION=universal pnpm tauri dev` compila la universal, para ver lo que ve
 * quien no tiene AVX2. Un `RUSTFLAGS` puesto a mano también manda sobre esto.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { arch, platform } from 'node:os';
import { delimiter, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const raiz = join(dirname(fileURLToPath(import.meta.url)), '..');
const argumentos = process.argv.slice(2);
const entorno = { ...process.env };

const binGstreamer = join(raiz, 'src-tauri', 'target', 'gstreamer', 'bin');
if (existsSync(binGstreamer)) {
  entorno.PATH = `${binGstreamer}${delimiter}${entorno.PATH ?? ''}`;
}

/**
 * Si este procesador ejecuta lo compilado con `x86-64-v3`. Solo se sabe leerlo en
 * Linux; en otro sistema se responde que no y se compila la universal, que
 * funciona en todos.
 */
function admiteLaRapida() {
  if (platform() !== 'linux' || arch() !== 'x64') return false;
  try {
    const banderas = readFileSync('/proc/cpuinfo', 'utf8').match(/^flags\s*:(.*)$/m)?.[1] ?? '';
    const tiene = new Set(banderas.trim().split(/\s+/));
    // Las de x86-64-v3 que no tiene ya todo x86-64-v2: sin cualquiera de ellas, el
    // binario muere con una instrucción ilegal al primer cálculo.
    return ['avx', 'avx2', 'fma', 'bmi1', 'bmi2', 'f16c', 'movbe', 'abm', 'xsave'].every((b) =>
      tiene.has(b)
    );
  } catch {
    return false;
  }
}

if (
  argumentos[0] === 'dev' &&
  entorno.RUSTFLAGS === undefined &&
  entorno.NB_EDICION !== 'universal' &&
  admiteLaRapida()
) {
  entorno.RUSTFLAGS = '-C target-cpu=x86-64-v3';
  entorno.CARGO_TARGET_DIR ??= join(raiz, 'src-tauri', 'target', 'desarrollo');
  console.log(
    'NB Books: desarrollo con la edición rápida (AVX2). NB_EDICION=universal para la otra.'
  );
}

const cli = join(raiz, 'node_modules', '.bin', platform() === 'win32' ? 'tauri.cmd' : 'tauri');
const resultado = spawnSync(cli, argumentos, {
  stdio: 'inherit',
  env: entorno,
  shell: platform() === 'win32'
});
process.exit(resultado.status ?? 1);
