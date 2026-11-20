#!/usr/bin/env node
/**
 * Ninguna plantilla de IA se queda sin medir en silencio.
 *
 * # Qué comprueba
 *
 * Cruza las plantillas de `src-tauri/prompts/` con los resultados publicados en
 * `docs/architecture/ai/resultados/`. Cada plantilla tiene que estar en uno de
 * dos sitios y en ninguno más:
 *
 * * En un resultado publicado, **con la misma versión que tiene hoy**. Si el
 *   texto de la plantilla cambia, su versión sube -eso lo fija una prueba de
 *   `plantillas.rs`- y el resultado publicado deja de corresponderle: hay que
 *   volver a ejecutar la suite y publicar.
 * * En la tabla "Plantillas sin medir" de `evaluation.md`, **con su versión y
 *   su motivo**. Subir la versión también invalida la declaración, para que
 *   nadie cambie una plantilla amparándose en una excusa escrita para otra.
 *
 * # Por qué existe
 *
 * Porque la regla 1 de la sección 4 de `evaluation.md` dice que "toda
 * modificación de plantilla ejecuta la suite", y sin mecanismo eso es una
 * intención. Sin CI y con modelos de un gigabyte, este guion no puede ejecutar
 * la suite; lo que puede, y hace, es **impedir que el cambio pase sin que los
 * resultados se vuelvan a publicar o sin que alguien escriba por qué no**.
 *
 * # Qué no comprueba
 *
 * Que los resultados publicados sean buenos, ni que los produjera esta máquina.
 * Eso lo dice la suite y lo juzga quien lea los números. Esto solo impide que el
 * documento y las plantillas dejen de hablar del mismo mundo.
 *
 * Uso: node scripts/check-evaluacion.mjs
 */

import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { join, resolve } from 'node:path';

const ROOT = resolve(import.meta.dirname, '..');
const PLANTILLAS = 'src-tauri/prompts';
const RESULTADOS = 'docs/architecture/ai/resultados';
const EVALUACION = 'docs/architecture/ai/evaluation.md';

const problemas = [];

/** Las plantillas, con la versión que declaran en su encabezado. */
function plantillas() {
  const dir = join(ROOT, PLANTILLAS);
  const encontradas = new Map();
  for (const archivo of readdirSync(dir)) {
    if (!archivo.endsWith('.md') || archivo === 'README.md') continue;
    const texto = readFileSync(join(dir, archivo), 'utf8');
    const id = texto.match(/^id:\s*(\S+)\s*$/m)?.[1];
    const version = texto.match(/^version:\s*(\d+)\s*$/m)?.[1];
    if (!id || !version) {
      problemas.push(`${PLANTILLAS}/${archivo}: sin id o sin versión en su encabezado`);
      continue;
    }
    encontradas.set(id, Number(version));
  }
  return encontradas;
}

/** Lo publicado: qué plantilla se midió, en qué versión y cuándo. */
function publicados() {
  const dir = join(ROOT, RESULTADOS);
  const medidas = new Map();
  if (!existsSync(dir)) {
    problemas.push(`${RESULTADOS}: no existe, así que no hay nada publicado que auditar`);
    return medidas;
  }
  for (const archivo of readdirSync(dir)) {
    if (!archivo.endsWith('.json')) continue;
    const ruta = `${RESULTADOS}/${archivo}`;
    let datos;
    try {
      datos = JSON.parse(readFileSync(join(dir, archivo), 'utf8'));
    } catch (error) {
      problemas.push(`${ruta}: no se puede leer como JSON (${error.message})`);
      continue;
    }
    for (const campo of ['fecha', 'suite']) {
      if (!datos[campo]) problemas.push(`${ruta}: le falta "${campo}"`);
    }
    for (const [id, version] of Object.entries(datos.plantillas ?? {})) {
      medidas.set(id, { version: Number(version), donde: ruta, fecha: datos.fecha });
    }
  }
  return medidas;
}

/** La tabla de plantillas declaradas sin medir, con su versión y su motivo. */
function sinMedir() {
  const texto = readFileSync(join(ROOT, EVALUACION), 'utf8');
  const seccion = texto.split(/^#+\s*Plantillas sin medir\s*$/m)[1];
  const declaradas = new Map();
  if (!seccion) return declaradas;
  for (const linea of seccion.split('\n')) {
    if (!linea.startsWith('|')) continue;
    const celdas = linea
      .split('|')
      .slice(1, -1)
      .map((c) => c.trim());
    if (celdas.length < 3) continue;
    // El identificador va en la tabla entre comillas de código, que es como se
    // escriben los nombres en el resto del documento.
    const id = celdas[0].replace(/`/g, '').trim();
    const [, version, motivo] = celdas;
    if (!/^\d+$/.test(version)) continue;
    if (motivo.length < 20) {
      problemas.push(`${EVALUACION}: "${id}" se declara sin medir con un motivo de una palabra`);
    }
    declaradas.set(id, Number(version));
  }
  return declaradas;
}

const tiene = plantillas();
const medidas = publicados();
const declaradas = sinMedir();

for (const [id, version] of tiene) {
  const medida = medidas.get(id);
  const declarada = declaradas.get(id);
  if (medida && medida.version === version) continue;
  if (declarada === version) continue;
  if (medida) {
    problemas.push(
      `la plantilla "${id}" va por la versión ${version} y lo publicado en ${medida.donde} ` +
        `es de la ${medida.version}: vuelve a ejecutar la suite y publica los resultados`
    );
  } else if (declarada !== undefined) {
    problemas.push(
      `la plantilla "${id}" va por la versión ${version} y está declarada sin medir en la ` +
        `${declarada}: confirma el motivo para la versión de ahora, o mide`
    );
  } else {
    problemas.push(
      `la plantilla "${id}" no tiene resultados publicados ni está declarada sin medir`
    );
  }
}

for (const [id, medida] of medidas) {
  if (!tiene.has(id)) {
    problemas.push(`${medida.donde}: publica resultados de "${id}", que ya no es una plantilla`);
  }
}
for (const id of declaradas.keys()) {
  if (!tiene.has(id)) {
    problemas.push(`${EVALUACION}: declara sin medir "${id}", que ya no es una plantilla`);
  }
}

if (problemas.length > 0) {
  console.error('\nLa evaluación y las plantillas no hablan del mismo mundo:\n');
  for (const problema of problemas) console.error(`  ${problema}`);
  console.error(
    '\nCambiar una plantilla obliga a volver a medir: es la regla 1 de la sección 4 de\n' +
      'evaluation.md. Si no se puede medir, se escribe por qué en "Plantillas sin medir"\n' +
      'con la versión de ahora, que es lo que impide que una excusa vieja tape un cambio\n' +
      'nuevo.\n'
  );
  process.exit(1);
}

const publicadas = [...tiene.keys()].filter((id) => medidas.has(id)).length;
console.log(
  `Evaluación: ${tiene.size} plantillas, ${publicadas} con resultados publicados y ` +
    `${tiene.size - publicadas} declaradas sin medir.`
);
