#!/usr/bin/env node
/**
 * Genera el archivo de avisos de terceros.
 *
 * La sección 4 de la licencia Apache 2.0, y prácticamente todas las licencias
 * permisivas de nuestras dependencias, obligan a conservar el aviso de copyright
 * y el texto de la licencia en las redistribuciones. Incumplirlo es una
 * infracción, con independencia de que el proyecto sea gratuito.
 *
 * Se genera automáticamente: un archivo mantenido a mano queda obsoleto en la
 * primera actualización de dependencias.
 *
 * Uso: node scripts/generate-notices.mjs [--check]
 *   --check  falla si el archivo generado difiere del versionado
 *
 * Ver docs/legal/third-party-notices.md
 */

import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';
import { dependenciasNode, dependenciasRust, VENDORIZADOS } from './dependencias.mjs';

const ROOT = resolve(import.meta.dirname, '..');
const SALIDA = join(ROOT, 'THIRD-PARTY-NOTICES.md');

/**
 * Licencias admitidas. Debe coincidir con `deny.toml` y con la política de
 * dependencias, y que coincida **no depende de que nadie se acuerde**:
 * `comprobarListasAlineadas` lo verifica más abajo. Tres listas que dicen lo
 * mismo en tres sitios distintos divergen solas, y la que se queda corta deja
 * pasar una licencia que la política prohíbe.
 */
const PERMITIDAS = new Set([
  'Apache-2.0',
  'Apache-2.0 WITH LLVM-exception',
  'MIT',
  'MIT-0',
  'BSD-2-Clause',
  'BSD-3-Clause',
  'ISC',
  'Zlib',
  'Unicode-3.0',
  'CC0-1.0',
  'Unlicense',
  'BSL-1.0',
  'MPL-2.0',
  'Python-2.0',
  '0BSD',
  // Licencia de datos, no de código: la trae el catálogo de raíces de confianza
  // de Mozilla. Permisiva y sin reciprocidad. Revisión registrada en
  // docs/architecture/dependency-policy.md, sección 4.1.
  'CDLA-Permissive-2.0',
  // Licencia de tipografías, no de código: la traen IBM Plex Sans y Literata,
  // que viajan dentro de la aplicación. Su única obligación es distribuir el
  // texto de la licencia, y está en `static/fuentes/LICENCIAS.md`.
  'OFL-1.1'
]);

/**
 * Comprueba que `deny.toml` admite exactamente las mismas licencias que este
 * archivo.
 *
 * Existe porque la política obliga a tocar tres sitios al ampliar la lista, y
 * una regla que exige acordarse de tres sitios se incumple al segundo cambio.
 * Ver docs/architecture/dependency-policy.md, sección 4.1.
 */
function comprobarListasAlineadas() {
  const deny = readFileSync(join(ROOT, 'deny.toml'), 'utf8');
  const bloque = deny.slice(deny.indexOf('allow = ['), deny.indexOf(']', deny.indexOf('allow = [')));
  const enDeny = new Set([...bloque.matchAll(/"([^"]+)"/g)].map((m) => m[1]));

  const soloAqui = [...PERMITIDAS].filter((l) => !enDeny.has(l));
  const soloEnDeny = [...enDeny].filter((l) => !PERMITIDAS.has(l));

  if (soloAqui.length > 0 || soloEnDeny.length > 0) {
    console.error('Las listas de licencias admitidas no coinciden.\n');
    for (const l of soloAqui) console.error(`  solo en generate-notices.mjs: ${l}`);
    for (const l of soloEnDeny) console.error(`  solo en deny.toml: ${l}`);
    console.error('\nVer docs/architecture/dependency-policy.md, sección 4.1');
    process.exit(1);
  }
}


/** Carpetas con un `PROCEDENCIA.md`: ahí vive código de fuera. */
function carpetasVendorizadas(raiz) {
  const encontradas = [];
  const recorrer = (dir) => {
    for (const entrada of readdirSync(dir)) {
      if (entrada === 'node_modules' || entrada === 'target' || entrada.startsWith('.')) continue;
      const ruta = join(dir, entrada);
      if (statSync(ruta).isDirectory()) recorrer(ruta);
      else if (entrada === 'PROCEDENCIA.md') encontradas.push(relative(raiz, dir));
    }
  };
  recorrer(join(raiz, 'src-tauri', 'src'));
  return encontradas;
}

function comprobarVendorizados(problemas) {
  const declaradas = new Set(VENDORIZADOS.map((v) => v.carpeta));
  for (const carpeta of carpetasVendorizadas(ROOT)) {
    if (!declaradas.has(carpeta)) {
      problemas.push(
        `Vendorizado: ${carpeta} trae código de fuera y no está en VENDORIZADOS de este script`
      );
    }
  }
}

function tablaVendorizados() {
  const filas = VENDORIZADOS.map(
    (v) =>
      `| \`${v.nombre}\` | ${v.version} | ${v.licencia} | ${v.repositorio} | ${v.copyright} | \`${v.carpeta}\` |`
  );
  return [
    '| Componente | Versión | Licencia | Origen | Copyright | Dónde vive aquí |',
    '|---|---|---|---|---|---|',
    ...filas
  ].join('\n');
}

function tabla(paquetes) {
  const filas = paquetes.map(
    (p) => `| \`${p.nombre}\` | ${p.version} | ${p.licencia} | ${p.repositorio} |`
  );
  return ['| Componente | Versión | Licencia | Origen |', '|---|---|---|---|', ...filas].join('\n');
}

function verificarLicencias(paquetes, origen, problemas) {
  for (const p of paquetes) {
    const licencias = p.licencia.split(/\s+(?:OR|AND)\s+|\//).map((l) => l.trim().replace(/[()]/g, ''));
    if (!licencias.some((l) => PERMITIDAS.has(l))) {
      problemas.push(`${origen}: ${p.nombre}@${p.version} declara "${p.licencia}"`);
    }
  }
}

const rust = dependenciasRust();
const node = dependenciasNode();

const problemas = [];
verificarLicencias(rust, 'Rust', problemas);
verificarLicencias(node, 'Node', problemas);
verificarLicencias(VENDORIZADOS, 'Vendorizado', problemas);
comprobarVendorizados(problemas);

const contenido = `# Avisos de Terceros

<!-- GENERADO AUTOMÁTICAMENTE por scripts/generate-notices.mjs. No editar a mano. -->

NB Books se distribuye bajo Apache License 2.0 e incluye software de terceros. Este archivo enumera esos
componentes y sus licencias, en cumplimiento de las obligaciones de atribución de cada una.

Regenerar: \`node scripts/generate-notices.mjs\`

## Componentes de Rust (${rust.length})

${tabla(rust)}

## Componentes de JavaScript (${node.length})

${tabla(node)}

## Código de terceros copiado en este repositorio (${VENDORIZADOS.length})

Adaptado, no enlazado: vive dentro del árbol y se compila con el proyecto. Cada carpeta lleva su
\`PROCEDENCIA.md\` con qué se tomó, qué no y por qué.

${tablaVendorizados()}
`;

/**
 * Comprueba que `LICENSE`, `NOTICE` y `THIRD-PARTY-NOTICES.md` viajan dentro del
 * paquete (`bundle.resources` de `src-tauri/tauri.conf.json`) y existen en disco
 *.
 *
 * Ver docs/legal/third-party-notices.md, secciones 3 y 5.
 */
function comprobarEmpaquetado() {
  const rutaConfig = join(ROOT, 'src-tauri', 'tauri.conf.json');
  const config = JSON.parse(readFileSync(rutaConfig, 'utf8'));
  const recursos = config?.bundle?.resources ?? {};
  const requeridos = ['../LICENSE', '../NOTICE', '../THIRD-PARTY-NOTICES.md'];
  const faltan = [];

  for (const rel of requeridos) {
    if (!(rel in recursos)) {
      faltan.push(`falta "${rel}" en bundle.resources de src-tauri/tauri.conf.json`);
      continue;
    }
    const rutaReal = resolve(join(ROOT, 'src-tauri'), rel);
    try {
      if (!statSync(rutaReal).isFile()) {
        faltan.push(`"${rel}" no es un archivo regular (${rutaReal})`);
      }
    } catch {
      faltan.push(`"${rel}" está en bundle.resources pero no existe en ${rutaReal}`);
    }
  }

  if (faltan.length > 0) {
    console.error('Los avisos legales no viajan completos en el paquete:\n');
    for (const f of faltan) console.error(`  ${f}`);
    console.error('\nVer docs/legal/third-party-notices.md');
    process.exit(1);
  }
}

comprobarListasAlineadas();
comprobarEmpaquetado();

if (process.argv.includes('--check')) {
  let versionado = '';
  try {
    versionado = readFileSync(SALIDA, 'utf8');
  } catch {
    console.error('THIRD-PARTY-NOTICES.md no existe. Ejecuta: node scripts/generate-notices.mjs');
    process.exit(1);
  }
  if (versionado !== contenido) {
    console.error('THIRD-PARTY-NOTICES.md está desactualizado. Ejecuta: node scripts/generate-notices.mjs');
    process.exit(1);
  }
  console.log('Avisos de terceros: al día.');
} else {
  writeFileSync(SALIDA, contenido);
  console.log(`Avisos generados: ${rust.length} componentes de Rust, ${node.length} de JavaScript.`);
}

if (problemas.length > 0) {
  console.error('\nLicencias no permitidas por la política de dependencias:\n');
  for (const p of problemas) console.error(`  ${p}`);
  console.error('\nVer docs/architecture/dependency-policy.md');
  process.exit(1);
}
