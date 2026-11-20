/**
 * **El criterio**, comprobado rompiéndolo.
 *
 * La regla es que ningún comando del puente corra en el hilo que atiende la
 * ventana, y su mecanismo es `scripts/check-comandos.mjs`. Un guardián solo
 * vale lo que vale su reacción a lo que tiene que rechazar, así que aquí se le
 * dan árboles de Rust fabricados —uno por forma de incumplimiento— y se
 * comprueba que falla en cada uno, además de que el puente real está en verde.
 *
 * El caso 4 es el que sostiene la parte difícil: la excepción del hilo
 * principal existe porque en Linux la biblioteca gráfica no es segura entre
 * hilos, y una excepción que se concede sola deja de ser una excepción. Por eso
 * declararla no basta, hay que usarla.
 */

import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, describe, expect, it } from 'vitest';

// Lo que cada prueba crea en el temporal se borra al acabar: sin esto se
// acumulaba una carpeta por ejecución.
const creados: string[] = [];
afterAll(() => {
  for (const d of creados) rmSync(d, { recursive: true, force: true });
});

const RAIZ = fileURLToPath(new URL('..', import.meta.url));

/** Ejecuta el guardián sobre un árbol de fuentes y devuelve cómo terminó. */
function guardian(fuentes: string): { salida: number; texto: string } {
  try {
    const texto = execFileSync('node', [join(RAIZ, 'scripts', 'check-comandos.mjs'), fuentes], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe']
    });
    return { salida: 0, texto };
  } catch (error) {
    const fallo = error as { status: number; stdout: string; stderr: string };
    return { salida: fallo.status, texto: `${fallo.stdout}${fallo.stderr}` };
  }
}

/** Escribe un `.rs` suelto en un directorio nuevo y devuelve el directorio. */
function arbol(contenido: string): string {
  const directorio = mkdtempSync(join(tmpdir(), 'nb-comandos-'));
  creados.push(directorio);
  writeFileSync(join(directorio, 'ipc.rs'), contenido);
  return directorio;
}

describe('ningún comando del puente corre en el hilo de la ventana', () => {
  it('el puente real está entero fuera de ese hilo', () => {
    const { salida, texto } = guardian(join(RAIZ, 'src-tauri', 'src'));

    expect(salida).toBe(0);
    // Los dos números del resumen son el mismo: no queda ninguno dentro.
    expect(texto).toMatch(/(\d+) en el puente, \1 fuera del hilo de la ventana/);
  });

  it('revertir un comando a síncrono hace fallar la verificación', () => {
    const { salida, texto } = guardian(
      arbol(`
/// Comprobación de vida.
#[tauri::command]
pub fn health_check(state: State<'_, AppState>) -> NbResult<HealthReport> {
    Ok(HealthReport { schema_version: state.db.schema_version()? })
}
`)
    );

    expect(salida).toBe(1);
    expect(texto).toContain('health_check es síncrono');
  });

  it('declarar la excepción no basta para acogerse a ella', () => {
    const { salida, texto } = guardian(
      arbol(`
/// Comprobación de vida.
///
/// Se queda en el hilo principal: porque sí.
#[tauri::command]
pub fn health_check(state: State<'_, AppState>) -> NbResult<HealthReport> {
    Ok(HealthReport { schema_version: state.db.schema_version()? })
}
`)
    );

    expect(salida).toBe(1);
    expect(texto).toContain('la excepción no le aplica');
  });

  it('un comando que sí toca la ventana se queda donde está, si lo declara', () => {
    const { salida } = guardian(
      arbol(`
/// Enseña la ventana.
///
/// Se queda en el hilo principal: GTK solo se puede tocar desde el hilo que lo
/// creó, y llamarlo desde otro no da lentitud, aborta el proceso.
#[tauri::command]
pub fn mostrar(ventana: WebviewWindow) -> NbResult<()> {
    ventana.show().map_err(|_| NbError::ComponentUnavailable)
}
`)
    );

    expect(salida).toBe(0);
  });

  it('`async fn` cuenta igual que el atributo', () => {
    const { salida } = guardian(
      arbol(`
/// Busca.
#[tauri::command]
pub async fn search(state: State<'_, AppState>) -> NbResult<Vec<Hit>> {
    Ok(Vec::new())
}
`)
    );

    expect(salida).toBe(0);
  });

  it('la forma corta del atributo tampoco escapa', () => {
    // `use tauri::command;` y el atributo pasa a escribirse `#[command]`. Un
    // guardián que solo conociera la forma larga dejaría de vigilar el archivo
    // entero por un `use`, y seguiría informando de que todo está bien.
    const { salida, texto } = guardian(
      arbol(`
/// Comprobación de vida.
#[command]
pub fn health_check(state: State<'_, AppState>) -> NbResult<HealthReport> {
    Ok(HealthReport { schema_version: state.db.schema_version()? })
}
`)
    );

    expect(salida).toBe(1);
    expect(texto).toContain('health_check es síncrono');
  });

  it('un árbol sin comandos no pasa en verde', () => {
    // Un guardián que deja de encontrar lo que vigila da por bueno lo que ya no
    // mira. Si `ipc.rs` se mueve o el atributo cambia de forma, esto avisa en
    // lugar de seguir informando de que todo está bien.
    const { salida, texto } = guardian(arbol('pub fn nada() {}\n'));

    expect(salida).toBe(1);
    expect(texto).toContain('Un guardián que no vigila nada no es un guardián');
  });
});
