#!/usr/bin/env node
/**
 * Toda ruta que llega del webview se canonicaliza antes de usarse.
 *
 * # Por qué existe
 *
 * El innegociable de `AGENTS.md` -"toda ruta que venga de fuera se canonicaliza y
 * se acota"- tenía la utilidad (`security::resolve_within`) y ningún mecanismo, y
 * decía que ningún comando recibía rutas del webview. El 2026-09-24 eran once: las
 * que eligen los diálogos nativos para guardar, importar, instalar o reapuntar. Diez
 * las trataban bien; `relocate_library_root` guardaba la suya tal como llegaba.
 *
 * # Qué comprueba
 *
 * Cada comando del puente (`#[tauri::command]`) con un parámetro que es una ruta
 * tiene que pasarlo, dentro del propio comando, por una de las puertas que
 * canonicalizan. Es una ruta:
 *
 *   * un parámetro de tipo `PathBuf` o `&Path`, se llame como se llame;
 *   * un texto cuyo nombre lo dice (`path`, `ruta`, `destino`, `directory`,
 *     `folder`, `carpeta`, `source`, `origen`, `file`, `archivo` y sus compuestos);
 *   * un campo así dentro de un objeto que el comando recibe: se busca la
 *     estructura que se deserializa y se exige la puerta sobre `objeto.campo`.
 *
 * Las puertas:
 *
 *   * `destino_admisible(&state, &path)` para guardar donde diga el usuario: lo
 *     canonicaliza y no deja escribir dentro de los datos de la aplicación;
 *   * `Path::new(&path).canonicalize()` o `PathBuf::from(&path).canonicalize()`;
 *   * `security::resolve_within(base, ...)`, que además la acota a una base.
 *
 * # Lo que no puede ver, y por eso no se afirma
 *
 * Que una ruta canonicalizada se use después para algo que no debería: acotar a
 * una base es decisión de cada comando, y "guardar donde el usuario elija" no tiene
 * base.
 *
 * Uso: node scripts/check-rutas.mjs
 */

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

// `fileURLToPath` y no `.pathname`: este deja los espacios como `%20` y la ruta no
// existe cuando el repositorio vive en una carpeta con espacios.
const RAIZ = fileURLToPath(new URL('..', import.meta.url));
const FUENTES = join(RAIZ, 'src-tauri', 'src');
// El motor de lectura es un crate aparte y recibe rutas igual que la aplicación.
const MOTOR = join(RAIZ, 'src-tauri', 'lector', 'src');
const NOMBRE =
  /^(?:\w+_)?(path|ruta|destino|directory|dir|folder|carpeta|source|origen|file|archivo)(?:_\w+)?$/;
const TEXTO = /^(String|&str|Option<String>|Option<&str>)$/;
const RUTA = /^(PathBuf|&Path|Option<PathBuf>|std::path::PathBuf)$/;
// Nombres que contienen la palabra y no son rutas: identificadores de la base.
const NO_ES_RUTA = /(_id|_ids|_count|_kind|_type|_hash)$|^file_id$/;
const esRuta = (nombre, tipo) =>
  RUTA.test(tipo) || (TEXTO.test(tipo) && NOMBRE.test(nombre) && !NO_ES_RUTA.test(nombre));

function listar(dir) {
  return readdirSync(dir).flatMap((nombre) => {
    const ruta = join(dir, nombre);
    return statSync(ruta).isDirectory() ? listar(ruta) : ruta.endsWith('.rs') ? [ruta] : [];
  });
}

/** El cuerpo de la función que empieza en `desde`, de su primera llave a la que la cierra. */
function cuerpo(codigo, desde) {
  const abre = codigo.indexOf('{', desde);
  let nivel = 0;
  for (let i = abre; i < codigo.length; i += 1) {
    if (codigo[i] === '{') nivel += 1;
    if (codigo[i] === '}') {
      nivel -= 1;
      if (nivel === 0) return codigo.slice(abre, i + 1);
    }
  }
  return '';
}

const escapar = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

// Estructuras que se deserializan, con sus campos: un comando que recibe una
// puede traer una ruta dentro.
const ESTRUCTURAS = new Map();
for (const archivo of [...listar(FUENTES), ...listar(MOTOR)]) {
  const codigo = readFileSync(archivo, 'utf8');
  for (const m of codigo.matchAll(
    /#\[derive\([^\]]*Deserialize[^\]]*\)\][^{;]*?pub struct (\w+)\s*\{([^}]*)\}/g
  )) {
    const campos = [...m[2].matchAll(/pub (\w+):\s*([^,\n]+)/g)].map((c) => ({
      nombre: c[1],
      tipo: c[2].trim()
    }));
    ESTRUCTURAS.set(m[1], campos);
  }
}

const fallos = [];
let comandos = 0;
let rutas = 0;
for (const archivo of [...listar(FUENTES), ...listar(MOTOR)]) {
  const codigo = readFileSync(archivo, 'utf8');
  for (const marca of codigo.matchAll(
    /#\[tauri::command[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*pub (?:const |async )?fn (\w+)\s*(?:<[^>]*>)?\(/g
  )) {
    comandos += 1;
    const abreFirma = marca.index + marca[0].length;
    const cierraFirma = codigo.indexOf(')', abreFirma);
    // Los parámetros no llevan paréntesis dentro salvo `State<'_, T>`, que no los tiene.
    const firma = codigo.slice(abreFirma, cierraFirma);
    const params = firma
      .split(/,(?![^<]*>)/)
      .map((p) => p.trim())
      .filter(Boolean)
      .map((p) => {
        const [nombre, ...tipo] = p.split(':');
        return { nombre: nombre.replace(/^mut\s+/, '').trim(), tipo: tipo.join(':').trim() };
      });
    const texto = cuerpo(codigo, cierraFirma);
    // Los campos de ruta de un objeto recibido cuentan como `objeto.campo`.
    const candidatos = params.flatMap(({ nombre, tipo }) => {
      const base = tipo.replace(/^(?:Option<|Vec<)?([\w:]+)>?$/, '$1').split('::').pop();
      const campos = ESTRUCTURAS.get(base);
      if (!campos) return [{ nombre, tipo }];
      return campos
        // Dentro de un objeto, `source` y `origen` dicen de dónde salió un dato
        // (un catálogo, un proveedor), no una carpeta.
        .filter((c) => !/^(source|origen)$/.test(c.nombre) && esRuta(c.nombre, c.tipo))
        .map((c) => ({ nombre: `${nombre}.${c.nombre}`, tipo: c.tipo }));
    });
    for (const { nombre, tipo } of candidatos) {
      if (!esRuta(nombre.split('.').pop(), tipo)) continue;
      rutas += 1;
      const p = escapar(nombre);
      const puertas = [
        new RegExp(`destino_admisible\\([^)]*\\b${p}\\b`),
        new RegExp(`(?:Path::new|PathBuf::from)\\(\\s*&?${p}\\s*\\)\\s*\\.canonicalize\\(\\)`),
        new RegExp(`\\b${p}\\s*\\.canonicalize\\(\\)`),
        new RegExp(`resolve_within\\([^)]*\\b${p}\\b`)
      ];
      if (!puertas.some((puerta) => puerta.test(texto))) {
        const linea = codigo.slice(0, marca.index).split('\n').length;
        fallos.push(`${relative(RAIZ, archivo)}:${linea}  ${marca[1]}(${nombre}: ${tipo})`);
      }
    }
  }
}

if (fallos.length > 0) {
  console.error('check-rutas: hay comandos que usan una ruta del webview sin canonicalizarla.\n');
  for (const fallo of fallos) console.error(`  ${fallo}`);
  console.error(
    '\n  Pásala por destino_admisible, por .canonicalize() o por security::resolve_within dentro del comando.'
  );
  process.exit(1);
}
console.log(`Rutas: ${comandos} comandos, ${rutas} reciben una ruta y todas se canonicalizan.`);
