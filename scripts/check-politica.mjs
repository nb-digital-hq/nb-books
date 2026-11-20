#!/usr/bin/env node
/**
 * La política del registro dice lo que de verdad se comprueba.
 *
 * # Qué comprueba
 *
 * Lee los requisitos de `docs/legal/governance/extension-policy.md` y exige de cada
 * uno: identificador consecutivo, los campos obligatorios, un estado de
 * comprobación de la lista cerrada, y coherencia entre ese estado y su
 * mecanismo. Un requisito `automática` o `parcial` tiene que nombrar archivos
 * que **existan** y que **citen su identificador**; uno `manual` o `ninguna`
 * tiene que decir qué haría falta y no puede nombrar mecanismo.
 *
 * # Por qué existe
 *
 * Porque una política de registro envejece en una dirección concreta: se
 * escribe con buenas intenciones, el código que las cumplía se mueve o se
 * borra, y el documento sigue prometiendo. Nadie lo nota hasta el día del
 * primer envío dudoso, que es el peor día para enterarse.
 *
 * La cita en el archivo es lo que le da fuerza. Borrar `revisar_tema` de
 * `manifest.rs` no rompe ninguna prueba de este guion salvo esa: el archivo
 * deja de decir R7 y la verificación falla, en vez de dejar la política
 * prometiendo una comprobación que ya no ocurre.
 *
 * # Qué no comprueba
 *
 * Que el mecanismo citado haga bien su trabajo. Eso son las pruebas de cada
 * módulo. Esto solo comprueba que el documento y el código sigan hablando del
 * mismo mundo.
 *
 * Uso: node scripts/check-politica.mjs
 */

import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const ROOT = resolve(import.meta.dirname, '..');
const POLITICA = 'docs/legal/governance/extension-policy.md';

/** Los cuatro estados posibles. Cualquier otro es un requisito sin declarar. */
const ESTADOS = ['automática', 'parcial', 'manual', 'ninguna'];
/** Los que obligan a nombrar mecanismo, y por tanto a citarlo desde el código. */
const CON_MECANISMO = ['automática', 'parcial'];
/** Los que obligan a decir qué haría falta para comprobarlo solo. */
const CON_DEUDA = ['parcial', 'manual', 'ninguna'];

/** El guion largo con el que un requisito declara que no tiene mecanismo. */
const SIN_MECANISMO = '—';

/**
 * Los requisitos del documento, con sus campos.
 *
 * Se leen de las secciones `### R<n> — Título` y de sus viñetas
 * `* **Campo:** valor`, que es como está escrito el documento. Un campo puede
 * ocupar varias líneas: la continuación va indentada, como en el resto de la
 * documentación del proyecto.
 */
function requisitos(texto) {
  const encontrados = [];
  let actual = null;
  let campo = null;

  for (const linea of texto.split('\n')) {
    const cabecera = linea.match(/^### (R\d+) —\s*(.+)$/);
    if (cabecera) {
      actual = { id: cabecera[1], titulo: cabecera[2].trim(), campos: {} };
      encontrados.push(actual);
      campo = null;
      continue;
    }
    if (!actual) continue;

    // Otra sección de nivel dos o tres cierra la lista de requisitos.
    if (/^#{2,3} /.test(linea)) {
      actual = null;
      campo = null;
      continue;
    }

    const vinneta = linea.match(/^\* \*\*(.+?):\*\*\s*(.*)$/);
    if (vinneta) {
      campo = vinneta[1];
      actual.campos[campo] = vinneta[2].trim();
      continue;
    }
    if (campo && /^\s+\S/.test(linea)) {
      actual.campos[campo] = `${actual.campos[campo]} ${linea.trim()}`.trim();
      continue;
    }
    if (linea.trim().length === 0) campo = null;
  }
  return encontrados;
}

/** Las rutas que un campo **Mecanismo** nombra, entre acentos graves. */
function rutas(valor) {
  return [...valor.matchAll(/`([^`]+)`/g)].map((coincidencia) => coincidencia[1]);
}

const texto = readFileSync(join(ROOT, POLITICA), 'utf8');
const lista = requisitos(texto);
const problemas = [];
/** Los archivos que algún requisito nombra, para la comprobación inversa. */
const citadas = new Map();

if (lista.length === 0) {
  console.error(`\nNo se encontró ningún requisito en ${POLITICA}.\n`);
  process.exit(1);
}

lista.forEach((requisito, indice) => {
  const { id, campos } = requisito;
  const esperado = `R${indice + 1}`;
  if (id !== esperado) {
    problemas.push(
      `${id}: los identificadores van seguidos desde R1 y aquí tocaba ${esperado}. ` +
        'No se renumeran: se citan desde el código y desde los avisos a los autores'
    );
  }

  for (const obligatorio of ['Exige', 'Comprobación', 'Si falla']) {
    if (!campos[obligatorio]) problemas.push(`${id}: le falta el campo "${obligatorio}"`);
  }

  const estado = campos['Comprobación'];
  if (estado && !ESTADOS.includes(estado)) {
    problemas.push(
      `${id}: declara la comprobación como "${estado}", que no es ninguno de los ` +
        `cuatro estados: ${ESTADOS.join(', ')}`
    );
    return;
  }
  if (!estado) return;

  const mecanismo = campos['Mecanismo'] ?? '';

  if (CON_MECANISMO.includes(estado)) {
    const nombradas = rutas(mecanismo);
    if (nombradas.length === 0) {
      problemas.push(
        `${id}: dice comprobarse de forma ${estado} y no nombra ningún mecanismo. ` +
          'Nombra el archivo entre acentos graves, o baja el estado a manual o ninguna'
      );
    }
    for (const ruta of nombradas) {
      if (!existsSync(join(ROOT, ruta))) {
        problemas.push(`${id}: nombra como mecanismo ${ruta}, que no existe`);
        continue;
      }
      const codigo = readFileSync(join(ROOT, ruta), 'utf8');
      if (!new RegExp(`\\b${id}\\b`).test(codigo)) {
        problemas.push(
          `${id}: ${ruta} es su mecanismo y no lo cita. ` +
            `Escribe ${id} en el archivo, donde se aplique el requisito`
        );
      }
      citadas.set(ruta, codigo);
    }
  } else if (mecanismo !== SIN_MECANISMO) {
    problemas.push(
      `${id}: se comprueba de forma ${estado} y aun así nombra mecanismo. ` +
        `Si hay código que lo comprueba, el estado no es ${estado}; si no lo hay, ` +
        `el campo va con ${SIN_MECANISMO}`
    );
  }

  if (CON_DEUDA.includes(estado) && !campos['Qué haría falta']) {
    problemas.push(
      `${id}: se comprueba de forma ${estado} y no dice qué haría falta para ` +
        'comprobarlo solo. Un requisito sin mecanismo y sin deuda declarada es ' +
        'una promesa que nadie va a cumplir'
    );
  }
});

// Y la dirección contraria: un archivo que cita un requisito que no existe.
// Pasa al renumerar, y el resultado es un aviso que manda al autor de una
// extensión a buscar en la política algo que no está escrito en ninguna parte.
const definidos = new Set(lista.map((requisito) => requisito.id));
for (const [ruta, codigo] of citadas) {
  for (const [, citado] of codigo.matchAll(/\bR(\d+)\b/g)) {
    const id = `R${citado}`;
    if (!definidos.has(id)) {
      problemas.push(`${ruta}: cita el requisito ${id}, que la política no define`);
    }
  }
}

if (problemas.length > 0) {
  console.error('\nLa política del registro promete cosas que no se sostienen:\n');
  for (const problema of problemas) console.error(`  ${problema}`);
  console.error(
    '\nCada requisito de la sección 3 lleva pegado su mecanismo, y el mecanismo cita\n' +
      'el requisito. Es lo que impide que la política siga prometiendo una\n' +
      'comprobación que se movió, se renombró o se borró.\n'
  );
  process.exit(1);
}

const porEstado = ESTADOS.map(
  (estado) => `${lista.filter((r) => r.campos['Comprobación'] === estado).length} ${estado}`
);
console.log(`Política del registro: ${lista.length} requisitos (${porEstado.join(', ')}).`);
