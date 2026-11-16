/**
 * **El criterio**, comprobado rompiéndolo.
 *
 * La regla es la 1 del contrato de carga: ninguna respuesta del runtime trae un
 * número de filas que dependa del tamaño de la biblioteca. Su mecanismo es
 * `scripts/check-listas.mjs`, y un guardián vale lo que vale su reacción a lo
 * que tiene que rechazar.
 *
 * Hay una diferencia deliberada con `check-comandos.mjs` y conviene verla aquí:
 * allí la excepción se puede comprobar mirando el código -o el comando toca la
 * ventana o no-, y por eso se exige. Aquí la cota declarada es una afirmación
 * sobre el mundo -"las extensiones instaladas"- y **no se puede verificar**. Lo
 * que el guardián garantiza es que esté escrita, que es lo que la hace
 * revisable.
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

function guardian(fuentes: string): { salida: number; texto: string } {
  try {
    const texto = execFileSync('node', [join(RAIZ, 'scripts', 'check-listas.mjs'), fuentes], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe']
    });
    return { salida: 0, texto };
  } catch (error) {
    const fallo = error as { status: number; stdout: string; stderr: string };
    return { salida: fallo.status, texto: `${fallo.stdout}${fallo.stderr}` };
  }
}

function arbol(contenido: string): string {
  const directorio = mkdtempSync(join(tmpdir(), 'nb-listas-'));
  creados.push(directorio);
  writeFileSync(join(directorio, 'ipc.rs'), contenido);
  return directorio;
}

describe('ninguna lista del puente crece con la biblioteca', () => {
  it('el puente real no tiene ninguna sin acotar ni sin declarar', () => {
    const { salida, texto } = guardian(join(RAIZ, 'src-tauri', 'src'));

    expect(salida).toBe(0);
    expect(texto).toMatch(/acotados por petición y \d+ por construcción declarada/);
  });

  it('quitarle el límite a un comando hace fallar la verificación', () => {
    const { salida, texto } = guardian(
      arbol(`
/// Las notas del usuario.
#[tauri::command(async)]
pub fn list_notes(state: State<'_, AppState>) -> NbResult<Vec<notes::Note>> {
    notes::list(&state.db.connection()?)
}
`)
    );

    expect(salida).toBe(1);
    expect(texto).toContain('list_notes devuelve una lista sin');
  });

  it('un comando con límite pasa', () => {
    const { salida } = guardian(
      arbol(`
/// Las notas del usuario.
#[tauri::command(async)]
pub fn list_notes(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> NbResult<Vec<notes::Note>> {
    notes::list(&state.db.connection()?, limit, offset)
}
`)
    );

    expect(salida).toBe(0);
  });

  it('una consulta que lleva la cota dentro también', () => {
    // `BrowseQuery` trae `limit` y `offset`: la cota viaja en el tipo, y pedir
    // además un parámetro suelto sería tener dos sitios donde discrepar.
    const { salida } = guardian(
      arbol(`
/// La biblioteca, por páginas.
#[tauri::command(async)]
pub fn browse_library(
    state: State<'_, AppState>,
    query: crate::browse::BrowseQuery,
) -> NbResult<Vec<BrowseItem>> {
    Ok(Vec::new())
}
`)
    );

    expect(salida).toBe(0);
  });

  it('declarar la cota basta cuando la lista está acotada por construcción', () => {
    const { salida } = guardian(
      arbol(`
/// Estado de los componentes.
///
/// Cota: los componentes del runtime, que son una lista cerrada.
#[tauri::command(async)]
pub fn component_states(state: State<'_, AppState>) -> NbResult<Vec<ComponentStatus>> {
    Ok(state.monitor.snapshot())
}
`)
    );

    expect(salida).toBe(0);
  });

  it('un árbol sin listas no pasa en verde', () => {
    const { salida, texto } = guardian(arbol('pub fn nada() {}\n'));

    expect(salida).toBe(1);
    expect(texto).toContain('Un guardián que no vigila nada no es un guardián');
  });
});
