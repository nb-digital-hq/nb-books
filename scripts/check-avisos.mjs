#!/usr/bin/env node
/**
 * Los avisos de seguridad de las dependencias de Rust.
 *
 * # Qué comprueba
 *
 * Ejecuta `cargo deny check advisories` sobre `src-tauri`, que lee la base de
 * RustSec: vulnerabilidades, crates retirados de crates.io y crates sin
 * mantenimiento. La política está en `deny.toml`.
 *
 * # Por qué no está en `pnpm verify`
 *
 * Porque `cargo-deny` **no es parte de la cadena de herramientas**: quien clone
 * el repositorio no lo tiene, y una verificación que falla por una herramienta
 * ausente enseña a saltarse la verificación entera. Es el mismo trato que
 * `check:pdfium`.
 *
 * Lo que sí hace es **decir que falta**, con la orden para instalarla, en lugar
 * de pasar en silencio. Pasar en silencio sería lo peor de las dos opciones: la
 * verificación en verde sin haber mirado nada.
 *
 * # Por qué existe
 *
 * Porque `dependency-policy.md` decía que la auditoría corría "en cada PR" y no
 * corría: el CI nunca ha llegado a ejecutarse, y antes
 * `cargo-deny` ni siquiera estaba instalado. La primera vez que se ejecutó
 * encontró un crate retirado que Dependabot no reportaba.
 *
 * Uso: node scripts/check-avisos.mjs
 */

import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const RAIZ = resolve(import.meta.dirname, '..');

// Se invoca como subcomando de `cargo` y no por su nombre: `cargo-deny` vive en
// `~/.cargo/bin`, que no siempre está en el PATH, y `cargo` sí lo está y busca
// ahí sus subcomandos.
const disponible = spawnSync('cargo', ['deny', '--version'], { encoding: 'utf8' });
if (disponible.error || disponible.status !== 0) {
  console.error(
    '\ncargo-deny no está instalado, así que los avisos de seguridad de Rust\n' +
      '**no se han comprobado**. Se instala una vez con:\n\n' +
      '  cargo install cargo-deny --locked\n\n' +
      'Ver docs/audit/supply-chain.md, sección 3.2.\n'
  );
  process.exit(1);
}

const revision = spawnSync('cargo', ['deny', 'check', 'advisories'], {
  cwd: resolve(RAIZ, 'src-tauri'),
  encoding: 'utf8'
});

// `cargo-deny` escribe el detalle por el error estándar, incluso cuando pasa.
const salida = `${revision.stdout ?? ''}${revision.stderr ?? ''}`;
if (revision.status !== 0) {
  console.error(salida);
  console.error(
    '\nHay avisos abiertos. Los que no se pueden cerrar cambiando algo de este\n' +
      'proyecto se anotan en docs/audit/supply-chain.md con su\n' +
      'motivo y su fecha; los demás se arreglan.\n'
  );
  process.exit(1);
}

console.log('Avisos de seguridad de Rust: ninguno abierto.');
