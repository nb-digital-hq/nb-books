#!/usr/bin/env node
/**
 * Ninguna capacidad se exporta al invitado sin un permiso detrás.
 *
 * Es también la mitad estructural de R3 (sin carga remota de código) y de R5
 * (permisos declarados): el mundo WIT no puede ganar una función, tampoco una
 * que cargue o instancie código, sin una capacidad del manifiesto que la conceda.
 *
 * # Qué comprueba
 *
 * Cruza las funciones que el anfitrión ofrece en el mundo WIT
 * (`src-tauri/wit/nb-extension.wit`) con las capacidades que el manifiesto
 * admite, y falla si alguna función se queda sin capacidad que la respalde o
 * nombra una que no existe.
 *
 * Las capacidades se leen del **esquema publicado**, no de una lista propia:
 * ese esquema lo genera la misma tabla que valida los manifiestos, así que
 * aquí no hay una tercera copia que se pueda quedar vieja.
 *
 * Es la mitad automática del requisito **R5** de la política del registro
 * (`docs/legal/governance/extension-policy.md`): ninguna capacidad se ejerce sin
 * estar declarada. La otra mitad -declarar de más- la comprueba la puerta de
 * revisión.
 *
 * # Por qué existe
 *
 * Porque sin esto, añadir una función al mundo sería el equivalente exacto de
 * una salida de red sin consentimiento: una capacidad que el usuario nunca vio
 * porque no había nada que se la enseñara. Esa regla tiene mecanismo desde
 * cada una tiene su comprobación y esta es la suya.
 *
 * # Por qué no basta con la revisión
 *
 * Porque el archivo WIT es un contrato público y crece con cada punto de
 * contribución. Una regla que solo vive en la cabeza de quien
 * programa no es una regla.
 *
 * Uso: node scripts/check-capacidades.mjs
 */

import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const ROOT = resolve(import.meta.dirname, '..');
const MUNDO = 'src-tauri/wit/nb-extension.wit';
const ESQUEMA = 'docs/architecture/api/nb-extension.schema.json';

/** El bloque `interface host { ... }`, que es lo que el anfitrión ofrece. */
function interfazDelAnfitrion(texto) {
  const inicio = texto.indexOf('interface host {');
  if (inicio === -1) return null;

  let profundidad = 0;
  for (let i = inicio; i < texto.length; i += 1) {
    if (texto[i] === '{') profundidad += 1;
    if (texto[i] === '}') {
      profundidad -= 1;
      if (profundidad === 0) return texto.slice(inicio, i);
    }
  }
  return null;
}

/**
 * Las funciones de la interfaz, con la capacidad que cada una declara.
 *
 * La capacidad se escribe en un comentario de documentación, `Capacidad: x`,
 * justo encima de la función. Va ahí y no en un archivo aparte porque es lo que
 * lee quien escribe una extensión: separarlos garantizaría que un día digan
 * cosas distintas.
 */
function funciones(interfaz) {
  const salida = [];
  let capacidad = null;

  for (const bruta of interfaz.split('\n')) {
    const linea = bruta.trim();

    const declara = linea.match(/^\/\/\/\s*Capacidad:\s*(\S+)/);
    if (declara) {
      capacidad = declara[1];
      continue;
    }
    if (linea.startsWith('///') || linea.startsWith('//') || linea.length === 0) continue;

    const funcion = linea.match(/^([a-z][a-z0-9-]*)\s*:\s*func\s*\(/);
    if (funcion) {
      salida.push({ nombre: funcion[1], capacidad });
    }
    // Cualquier otra línea con contenido cierra el comentario anterior: una
    // capacidad declarada encima de un `record` no respalda a la función que
    // venga después.
    capacidad = null;
  }
  return salida;
}

const texto = readFileSync(join(ROOT, MUNDO), 'utf8');
const interfaz = interfazDelAnfitrion(texto);
if (!interfaz) {
  console.error(`\nNo se encontró "interface host" en ${MUNDO}.\n`);
  process.exit(1);
}

const esquema = JSON.parse(readFileSync(join(ROOT, ESQUEMA), 'utf8'));
const admitidas = esquema.properties?.capabilities?.items?.enum ?? [];
if (admitidas.length === 0) {
  console.error('\nEl esquema publicado no declara ninguna capacidad: algo se ha roto.\n');
  process.exit(1);
}

const declaradas = funciones(interfaz);
const problemas = [];

if (declaradas.length === 0) {
  problemas.push('el mundo no ofrece ninguna función: o falta el archivo, o esto no sabe leerlo');
}

for (const { nombre, capacidad } of declaradas) {
  if (!capacidad) {
    problemas.push(
      `${nombre}: el anfitrión la ofrece y no declara capacidad. ` +
        'Escribe "Capacidad: <nombre>" encima de la función'
    );
    continue;
  }
  if (!admitidas.includes(capacidad)) {
    problemas.push(
      `${nombre}: declara la capacidad "${capacidad}", que el manifiesto no admite. ` +
        `Las que existen son: ${admitidas.join(', ')}`
    );
  }
}

if (problemas.length > 0) {
  console.error('\nHay capacidades del anfitrión sin permiso detrás:\n');
  for (const problema of problemas) console.error(`  ${problema}`);
  console.error(
    '\nUna función que el invitado puede llamar sin que nadie la haya concedido es\n' +
      'una salida sin consentimiento con otro nombre. Añade la capacidad al\n' +
      'manifiesto (tabla CAMPOS de src-tauri/src/extensions/manifest.rs) o quita la\n' +
      'función del mundo.\n'
  );
  process.exit(1);
}

console.log(
  `Capacidades del anfitrión: ${declaradas.length} función(es) en el mundo, ` +
    'todas con permiso detrás.'
);
