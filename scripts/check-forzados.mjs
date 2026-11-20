#!/usr/bin/env node
/**
 * Ningún `as unknown as` sin escribir por qué.
 *
 * # Por qué existe
 *
 * Los tipos del puente se generan desde Rust para que la interfaz no pueda
 * mandar algo que el runtime no entiende. Un `as unknown as` se salta esa
 * garantía en silencio: antes el audio preparado no funcionaba en
 * ningún sitio porque se mandaba `kind: 'Chapter'` forzado a `AudioScope`, que
 * espera `chapter`, y ni TypeScript ni la suite podían verlo.
 *
 * Se admite solo con un comentario `forzado:` en las tres líneas anteriores que
 * diga por qué no hay otra forma.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = fileURLToPath(new URL('..', import.meta.url));

/**
 * @param {string} dir
 * @returns {string[]}
 */
function archivos(dir) {
  /** @type {string[]} */
  const out = [];
  for (const n of readdirSync(dir)) {
    const r = join(dir, n);
    if (statSync(r).isDirectory()) out.push(...archivos(r));
    else if (/\.(ts|svelte)$/.test(n) && !/\.test\.ts$/.test(n)) out.push(r);
  }
  return out;
}

/** @returns {string[]} */
export function revisar(raiz = RAIZ) {
  /** @type {string[]} */
  const fallos = [];
  for (const f of archivos(join(raiz, 'src'))) {
    if (f.includes(`${join('src', 'lib', 'bindings')}`)) continue;
    const lineas = readFileSync(f, 'utf8').split('\n');
    lineas.forEach((l, i) => {
      if (!/\bas unknown as\b/.test(l) || /^\s*(\/\/|\*)/.test(l)) return;
      const antes = lineas.slice(Math.max(0, i - 3), i).join('\n');
      if (!/forzado:/.test(antes)) fallos.push(`${relative(raiz, f)}:${i + 1}`);
    });
  }
  return fallos;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const fallos = revisar();
  if (fallos.length) {
    console.error('`as unknown as` sin un comentario `forzado:` que diga por qué:');
    for (const f of fallos) console.error(`  ${f}`);
    process.exit(1);
  }
  console.log('Forzados de tipo: ninguno sin justificar.');
}
