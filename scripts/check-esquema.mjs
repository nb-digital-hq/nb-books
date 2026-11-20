#!/usr/bin/env node
/**
 * Comprobación del esquema publicado del manifiesto de extensión.
 *
 * Es, junto al validador de `manifest.rs`, el mecanismo del requisito **R1** de
 * la política del registro (`docs/legal/governance/extension-policy.md`).
 *
 * El esquema de `docs/architecture/api/nb-extension.schema.json` **no se escribe a mano**:
 * lo genera la tabla `CAMPOS` de `src-tauri/src/extensions/manifest.rs`, que es
 * la misma que usa el validador del anfitrión. Este guion hace dos cosas, y la
 * segunda es la que de verdad importa.
 *
 * 1. **Deriva.** Lee los dos archivos, los regenera y compara el contenido. Se
 *    compara **por contenido y no por el estado de git**, y la diferencia
 *    importa: `check-bindings.mjs` descuenta lo que ya estaba sucio antes de
 *    regenerar, para no culpar al commit equivocado, y con eso se le escapa
 *    justo el caso de que alguien edite a mano el archivo generado dentro del
 *    mismo cambio. Comprobado el 2026-09-07 rompiéndolo: una edición a mano del
 *    esquema pasaba sin que nadie dijera nada. Comparar bytes no depende de en
 *    qué estado esté el árbol.
 *
 * 2. **La costura.** El anfitrión valida con una función de Rust; el esquema
 *    publica una expresión regular. Son dos escrituras de la misma regla,
 *    porque el proyecto no incorpora un motor de expresiones regulares: cabe en
 *    cien líneas propias, y ese es el listón de `dependency-policy.md`. Aquí se
 *    ejecuta **la expresión del esquema** sobre los mismos ejemplos que la
 *    prueba de Rust ejecuta contra **la función**, y se falla si no dicen lo
 *    mismo.
 *
 *    Sin esta segunda parte, el día que las dos escrituras se separaran, un
 *    autor validaría su manifiesto contra el esquema publicado, le saldría
 *    bien, y NB Books se lo rechazaría. El coste lo pagaría él y la causa
 *    estaría aquí.
 *
 * Uso: node scripts/check-esquema.mjs
 */

import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const ROOT = resolve(import.meta.dirname, '..');
const ESQUEMA = 'docs/architecture/api/nb-extension.schema.json';
const EJEMPLOS = 'docs/architecture/api/nb-extension.ejemplos.json';

function ejecutar(comando, args, opciones = {}) {
  return spawnSync(comando, args, { cwd: ROOT, encoding: 'utf8', ...opciones });
}

/** Lo que hay hoy en disco, o `null` si el archivo no existe. */
function leer(ruta) {
  try {
    return readFileSync(join(ROOT, ruta), 'utf8');
  } catch {
    return null;
  }
}

// --- 1. Deriva -------------------------------------------------------------

const antes = new Map([ESQUEMA, EJEMPLOS].map((ruta) => [ruta, leer(ruta)]));

const generacion = ejecutar(
  'cargo',
  [
    'test',
    '--quiet',
    '--lib',
    'extensions::manifest::pruebas::el_esquema_y_los_ejemplos_se_publican'
  ],
  { cwd: resolve(ROOT, 'src-tauri'), stdio: 'pipe' }
);

if (generacion.status !== 0) {
  console.error('\nNo se pudo regenerar el esquema del manifiesto:\n');
  console.error(generacion.stderr || generacion.stdout);
  process.exit(1);
}

const derivados = [...antes.entries()]
  .filter(([ruta, contenido]) => contenido !== leer(ruta))
  .map(([ruta]) => ruta);

if (derivados.length > 0) {
  console.error('\nEl esquema publicado del manifiesto no era el que genera Rust:\n');
  for (const ruta of derivados) console.error(`  ${ruta}`);
  console.error(
    '\nSe ha regenerado. Revisa el cambio y añádelo al commit.\n' +
      'Estos archivos no se editan a mano: salen de la tabla CAMPOS de\n' +
      'src-tauri/src/extensions/manifest.rs. El esquema es un contrato público,\n' +
      'así que comprueba la sección 2 de\n' +
      'docs/architecture/versioning.md antes de continuar.\n'
  );
  process.exit(1);
}

// --- 2. La costura ---------------------------------------------------------

const esquema = JSON.parse(readFileSync(join(ROOT, ESQUEMA), 'utf8'));
const ejemplos = JSON.parse(readFileSync(join(ROOT, EJEMPLOS), 'utf8'));

const problemas = [];

for (const [campo, casos] of Object.entries(ejemplos.campos)) {
  const propiedad = esquema.properties[campo];
  if (!propiedad) {
    problemas.push(`${campo}: hay ejemplos publicados y el esquema no describe el campo`);
    continue;
  }

  const admite = admisor(propiedad);
  if (!admite) {
    problemas.push(
      `${campo}: el esquema no publica ni patrón ni lista de valores, así que ` +
        'sus ejemplos no comprueban nada'
    );
    continue;
  }

  for (const valor of casos.aceptados) {
    if (!admite(valor)) {
      problemas.push(
        `${campo}: se publica ${JSON.stringify(valor)} como aceptado y el esquema lo rechaza`
      );
    }
  }
  for (const valor of casos.rechazados) {
    if (admite(valor)) {
      problemas.push(
        `${campo}: se publica ${JSON.stringify(valor)} como rechazado y el esquema lo acepta`
      );
    }
  }
}

/**
 * De una propiedad del esquema, la función que decide si acepta un valor.
 *
 * Cubre las tres formas que la tabla de campos sabe producir. Una forma nueva
 * sin su rama aquí devuelve `null`, y eso falla: es a propósito, porque un
 * campo cuyo esquema no se sabe ejecutar es un campo cuya regla deja de estar
 * comprobada.
 */
function admisor(propiedad) {
  if (propiedad.pattern) {
    const expresion = new RegExp(propiedad.pattern, 'u');
    return (valor) => expresion.test(valor);
  }
  if (Array.isArray(propiedad.enum)) {
    return (valor) => propiedad.enum.includes(valor);
  }
  if (propiedad.items && Array.isArray(propiedad.items.enum)) {
    return (valor) => propiedad.items.enum.includes(valor);
  }
  if (propiedad.type === 'string' && typeof propiedad.minLength === 'number') {
    return (valor) => valor.length >= propiedad.minLength;
  }
  return null;
}

if (problemas.length > 0) {
  console.error('\nEl esquema publicado y el validador del anfitrión no dicen lo mismo:\n');
  for (const problema of problemas) console.error(`  ${problema}`);
  console.error(
    '\nLos dos salen de la tabla CAMPOS de src-tauri/src/extensions/manifest.rs.\n' +
      'La expresión del esquema y la función de Rust son dos escrituras de la\n' +
      'misma regla: corrige la que esté mal, no los ejemplos.\n'
  );
  process.exit(1);
}

const campos = Object.keys(esquema.properties).length;
const casos = Object.values(ejemplos.campos).reduce(
  (total, c) => total + c.aceptados.length + c.rechazados.length,
  0
);
console.log(
  `Esquema del manifiesto: al día, ${campos} campos, y ${casos} ejemplos donde el ` +
    'esquema y el anfitrión coinciden.'
);
