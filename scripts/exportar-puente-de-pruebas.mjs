#!/usr/bin/env node
/**
 * Escribe el runtime simulado de la suite de interfaz como un script suelto, para
 * que `scripts/medir-en-webkitgtk.py` mida con él.
 *
 * # Por qué existe
 *
 * El arnés de WebKitGTK traía su propio simulador, y solo sabía llenar la
 * biblioteca: no abría documentos ni tenía catálogos, así que el lector y
 * Descubrir no se podían medir en el motor del producto. Escribir otro simulador
 * para ellos habría sido la tercera copia del mismo contrato, y la que tenía ya
 * se había quedado atrás en silencio —contestaba `null` a los comandos nuevos—.
 *
 * Así que no se copia: se llama a `montarPuente` de `e2e/puente.ts` con una página
 * de mentira que guarda lo que le instalan. Lo que sale es exactamente lo que
 * recibe Playwright.
 *
 * # Lo que no viaja
 *
 * Las imágenes. En la suite las sirve una ruta interceptada de Playwright, que el
 * arnés no tiene: aquí piden `asset.localhost` y no llegan. Para medir el lector,
 * Ajustes o Estudio da igual; para medir portadas está `desplazar.js` con el
 * simulador del propio arnés, que las fabrica.
 *
 * # El capítulo largo
 *
 * El capítulo de la suite tiene tres párrafos, que es lo que necesitan sus pruebas
 * y no dice nada de lo que cuesta leer. `--capitulo N` pasa la opción `parrafos` del
 * simulador, la misma que usa `e2e/rendimiento.spec.ts`: N párrafos, los tres de
 * siempre y detrás texto propio de largos distintos. No sale de ningún libro.
 *
 * Uso:
 *   node scripts/exportar-puente-de-pruebas.mjs <salida.js> [--capitulo 400] [--opciones '{"conCatalogo":true}']
 */

import { writeFileSync } from 'node:fs';

import { montarPuente } from '../e2e/puente.ts';

const [salida, ...resto] = process.argv.slice(2);
if (!salida) {
  console.error('Uso: node scripts/exportar-puente-de-pruebas.mjs <salida.js> [--capitulo N] [--opciones JSON]');
  process.exit(2);
}
const valor = (nombre) => {
  const i = resto.indexOf(nombre);
  return i === -1 ? undefined : resto[i + 1];
};
const parrafos = Number(valor('--capitulo') ?? 0);
const opciones = JSON.parse(valor('--opciones') ?? '{}');
if (parrafos > 0) opciones.parrafos = parrafos;

const instalado = [];
const pagina = {
  route: async () => {},
  addInitScript: async (funcion, argumento) => {
    instalado.push(`(${funcion.toString()})(${JSON.stringify(argumento)});`);
  }
};
await montarPuente(pagina, opciones);
if (instalado.length === 0) {
  console.error('montarPuente no instaló nada');
  process.exit(1);
}

// Todos, en el orden en que se registran: el simulador se monta en
// varios guiones -los modelos y el bus, el proveedor externo, el estudio, el
// audio- y el principal usa lo que dejan los anteriores. Antes esto
// exigía uno solo, y desde entonces no exportaba nada.
const script = instalado.join('\n');

writeFileSync(salida, script);
console.log(`Simulador escrito en ${salida}: ${script.length} caracteres${parrafos ? `, capítulo de ${parrafos} párrafos` : ''}.`);
