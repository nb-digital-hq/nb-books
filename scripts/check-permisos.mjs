#!/usr/bin/env node
/**
 * Lo que la interfaz llama de un plugin de Tauri tiene su permiso concedido.
 *
 * # Por qué existe
 *
 * Antes ninguna exportación funcionaba en el binario: la interfaz
 * llamaba a `save()` del plugin de diálogos y la capacidad de la ventana solo
 * concedía `dialog:allow-open`. La suite no podía verlo porque simula el puente
 * entero, y el binario contesta "not allowed by ACL" en tiempo de ejecución.
 *
 * El guardián recorre `src/` buscando las funciones de plugin que se usan y
 * exige que el permiso correspondiente esté en `capabilities/`. La tabla de
 * abajo es la lista cerrada de lo que la interfaz puede usar: una función nueva
 * que no esté en ella hace fallar la verificación hasta que alguien escriba qué
 * permiso pide.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = fileURLToPath(new URL('..', import.meta.url));

/**
 * módulo → función → permiso que exige.
 * @type {Record<string, Record<string, string>>}
 */
const TABLA = {
  '@tauri-apps/plugin-dialog': { open: 'dialog:allow-open', save: 'dialog:allow-save' },
  '@tauri-apps/api/window': { setFullscreen: 'core:window:allow-set-fullscreen' }
};

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

export function concedidos(raiz = RAIZ) {
  const dir = join(raiz, 'src-tauri', 'capabilities');
  const set = new Set();
  for (const n of readdirSync(dir).filter((x) => x.endsWith('.json'))) {
    for (const p of JSON.parse(readFileSync(join(dir, n), 'utf8')).permissions ?? []) {
      set.add(typeof p === 'string' ? p : p.identifier);
    }
  }
  return set;
}

/** @returns {string[]} */
export function revisar(raiz = RAIZ) {
  const tiene = concedidos(raiz);
  /** @type {string[]} */
  const fallos = [];
  for (const f of archivos(join(raiz, 'src'))) {
    const texto = readFileSync(f, 'utf8');
    for (const [modulo, funciones] of Object.entries(TABLA)) {
      if (!texto.includes(modulo)) continue;
      for (const [fn, permiso] of Object.entries(funciones)) {
        const usa = new RegExp(`(?:\\b|\\.)${fn}\\s*\\(`).test(texto);
        if (usa && !tiene.has(permiso)) {
          fallos.push(`${relative(raiz, f)} usa ${fn}() de ${modulo} y falta "${permiso}" en capabilities/`);
        }
      }
    }
    const importados = [...texto.matchAll(/from '(@tauri-apps\/plugin-[a-z-]+)'/g)].map((m) => m[1] ?? '');
    for (const m of importados) {
      if (!TABLA[m]) fallos.push(`${relative(raiz, f)} importa ${m}, que no está en la tabla de check-permisos.mjs`);
    }
  }
  return fallos;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const fallos = revisar();
  if (fallos.length) {
    console.error('Permisos de plugin que la interfaz usa y nadie concedió:');
    for (const f of fallos) console.error(`  ${f}`);
    process.exit(1);
  }
  console.log('Permisos: todo lo que la interfaz llama de un plugin está concedido.');
}
