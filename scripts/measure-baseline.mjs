#!/usr/bin/env node
/**
 * Medición base de rendimiento.
 *
 * Convierte los presupuestos de docs/architecture/performance-budgets.md en
 * números medidos sobre el binario de release.
 *
 * Mide sobre la aplicación **vacía**, que es la línea base contra la que se
 * comparan las regresiones. Los escenarios con biblioteca cargada los miden
 * e2e/rendimiento.spec.ts y las pruebas de Rust.
 *
 * Uso: node scripts/measure-baseline.mjs [--json]
 */

import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const ROOT = resolve(import.meta.dirname, '..');
const BINARIO = join(ROOT, 'src-tauri', 'target', 'release', 'nb-books');
const REPETICIONES = 5;

/** Presupuestos declarados. La medición no falla el build todavía: primero se calibra. */
const PRESUPUESTOS = {
  arranque_runtime_ms: 200,
  bundle_frontend_kb: 250,
  binario_mb: 50
};

function medianaDe(valores) {
  const orden = [...valores].sort((a, b) => a - b);
  const mitad = Math.floor(orden.length / 2);
  return orden.length % 2 === 0 ? (orden[mitad - 1] + orden[mitad]) / 2 : orden[mitad];
}

/**
 * Tiempo desde el lanzamiento hasta que el runtime declara la base lista.
 *
 * Se mide contra el registro, no contra la aparición de la ventana: lo que este
 * presupuesto cubre es el arranque del runtime, no el del webview del sistema.
 */
async function medirArranque() {
  if (!existsSync(BINARIO)) {
    return { error: 'binario de release no compilado (pnpm tauri build)' };
  }

  const muestras = [];

  for (let i = 0; i < REPETICIONES; i++) {
    const dir = mkdtempSync(join(tmpdir(), 'nb-baseline-'));
    const inicio = performance.now();

    const proceso = spawn(BINARIO, {
      env: { ...process.env, NB_BOOKS_TEST_DIR: dir, NB_BOOKS_LOG: 'info' },
      stdio: 'ignore'
    });

    let transcurrido = null;
    const limite = Date.now() + 20_000;

    while (Date.now() < limite) {
      const logs = join(dir, 'logs');
      if (existsSync(logs)) {
        const contenido = readdirSync(logs)
          .map((f) => readFileSync(join(logs, f), 'utf8'))
          .join('');
        if (contenido.includes('base de datos lista')) {
          transcurrido = performance.now() - inicio;
          break;
        }
      }
      await new Promise((r) => setTimeout(r, 5));
    }

    proceso.kill();
    rmSync(dir, { recursive: true, force: true });

    if (transcurrido === null) return { error: 'el runtime no completó el arranque' };
    muestras.push(transcurrido);
  }

  return { mediana_ms: Math.round(medianaDe(muestras)), muestras: muestras.map(Math.round) };
}

/**
 * El paquete lo mide `check-paquete.mjs`, que separa lo que se carga al arrancar
 * de lo que se carga al abrir cada destino. Antes esto sumaba todo
 * el JavaScript, que era lo mismo mientras no había nada diferido.
 */
function medirBundle() {
  try {
    const salida = execFileSync('node', [join(ROOT, 'scripts', 'check-paquete.mjs'), '--json'], {
      encoding: 'utf8'
    });
    const { arranque_kb, total_kb } = JSON.parse(salida);
    return { kb: Math.round(arranque_kb), total_kb };
  } catch {
    return { error: 'no se pudo medir el paquete (scripts/check-paquete.mjs)' };
  }
}

function medirBinario() {
  if (!existsSync(BINARIO)) return { error: 'binario de release no compilado' };
  return { mb: Math.round((statSync(BINARIO).size / 1024 / 1024) * 10) / 10 };
}

const resultados = {
  fecha: new Date().toISOString().slice(0, 10),
  arranque_runtime: await medirArranque(),
  bundle_frontend: medirBundle(),
  binario_release: medirBinario(),
  presupuestos: PRESUPUESTOS
};

if (process.argv.includes('--json')) {
  console.log(JSON.stringify(resultados, null, 2));
} else {
  console.log('\nMedición base de NB Books\n');
  const fila = (etiqueta, valor, presupuesto, unidad) => {
    if (valor === undefined) return console.log(`  ${etiqueta.padEnd(28)} sin medir`);
    const estado = valor <= presupuesto ? 'dentro' : 'EXCEDE';
    console.log(
      `  ${etiqueta.padEnd(28)} ${String(valor).padStart(6)} ${unidad}  (presupuesto ${presupuesto} ${unidad}) ${estado}`
    );
  };

  fila('Arranque del runtime', resultados.arranque_runtime.mediana_ms, PRESUPUESTOS.arranque_runtime_ms, 'ms');
  fila('Paquete de arranque', resultados.bundle_frontend.kb, PRESUPUESTOS.bundle_frontend_kb, 'KB');
  fila('Binario de release', resultados.binario_release.mb, PRESUPUESTOS.binario_mb, 'MB');

  for (const [clave, valor] of Object.entries(resultados)) {
    if (valor?.error) console.log(`\n  ${clave}: ${valor.error}`);
  }
  console.log('');
}
