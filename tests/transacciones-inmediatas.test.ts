/**
 * **El criterio**, comprobado rompiéndolo.
 *
 * La regla ya se incumplió dos veces: la primera dejó 12.156 «database is
 * locked» en una sesión y títulos con el nombre del archivo; la segunda, con la
 * misma forma en otro módulo, perdió 1.207 documentos de una importación real
 * que `storage.md` daba por imposible.
 *
 * Lo que hace que una regla así vuelva a entrar es que **no se ve**: `BEGIN`
 * parece completo, y `.transaction()` parece la forma normal de abrir una. Por
 * eso el guardián mira las dos.
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
    const texto = execFileSync(
      'node',
      [join(RAIZ, 'scripts', 'check-transacciones.mjs'), fuentes],
      {
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'pipe']
      }
    );
    return { salida: 0, texto };
  } catch (error) {
    const fallo = error as { status: number; stdout: string; stderr: string };
    return { salida: fallo.status, texto: `${fallo.stdout}${fallo.stderr}` };
  }
}

function arbol(contenido: string): string {
  const directorio = mkdtempSync(join(tmpdir(), 'nb-transacciones-'));
  creados.push(directorio);
  writeFileSync(join(directorio, 'boards.rs'), contenido);
  return directorio;
}

/** Un módulo que abre su transacción como se debe, para que el guardián vea algo. */
const CORRECTO = `
pub fn mover(conn: &mut Connection) -> NbResult<()> {
    let tx = crate::db::write_transaction(conn)?;
    tx.commit().map_err(|e| fallo(&e))
}
`;

describe('ninguna transacción de escritura se abre en diferido', () => {
  it('el runtime real no tiene ninguna', () => {
    const { salida, texto } = guardian(join(RAIZ, 'src-tauri', 'src'));

    expect(salida).toBe(0);
    expect(texto).toContain('ninguna de escritura abierta en diferido');
  });

  it('reintroducir un `BEGIN` suelto hace fallar la verificación', () => {
    const { salida, texto } = guardian(
      arbol(
        CORRECTO +
          `
pub fn intercambiar(conn: &Connection) -> NbResult<()> {
    conn.execute_batch("BEGIN").map_err(|e| fallo(&e))?;
    Ok(())
}
`
      )
    );

    expect(salida).toBe(1);
    expect(texto).toContain('un `BEGIN` a secas');
  });

  it('`BEGIN TRANSACTION` tampoco cuela, que es lo mismo con otro nombre', () => {
    const { salida } = guardian(
      arbol(`${CORRECTO}\n    conn.execute_batch("BEGIN TRANSACTION")?;\n`)
    );

    expect(salida).toBe(1);
  });

  it('`.transaction()` tampoco, que es la forma en la que rusqlite lo esconde', () => {
    const { salida, texto } = guardian(arbol(`${CORRECTO}\n    let tx = conn.transaction()?;\n`));

    expect(salida).toBe(1);
    expect(texto).toContain('rusqlite abre diferida');
  });

  it('un comentario que explica el problema no lo comete', () => {
    // Los documentos y los comentarios de este proyecto citan `BEGIN DEFERRED`
    // para explicar por qué no se usa. Un guardián que los contara obligaría a
    // dejar de explicarlo, que es lo contrario de lo que se quiere.
    const { salida } = guardian(
      arbol(`${CORRECTO}\n// Esto no abre un "BEGIN": explica por qué no se hace.\n`)
    );

    expect(salida).toBe(0);
  });

  it('un árbol sin transacciones no pasa en verde', () => {
    const { salida, texto } = guardian(arbol('pub fn nada() {}\n'));

    expect(salida).toBe(1);
    expect(texto).toContain('No se encontró ninguna transacción de escritura');
  });
});
