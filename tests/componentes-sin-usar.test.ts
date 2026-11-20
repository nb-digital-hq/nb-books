/**
 * **Ningún componente del sistema de diseño existe sin una pantalla que lo use**.
 *
 * Comprueba que `scripts/check-ui.mjs` detecte componentes huérfanos en `src/lib/ui/`
 * en ambas direcciones:
 *   1. Falla cuando existe un componente en `lib/ui/` que ninguna pantalla consume.
 *   2. Pasa cuando el componente tiene al menos un consumidor fuera de pruebas.
 *   3. Pasa sobre el árbol real del proyecto.
 */

import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';

const RAIZ = fileURLToPath(new URL('..', import.meta.url));

function guardian(fuentes: string): { salida: number; texto: string } {
  try {
    const texto = execFileSync('node', [join(RAIZ, 'scripts', 'check-ui.mjs'), fuentes], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe']
    });
    return { salida: 0, texto };
  } catch (error) {
    const fallo = error as { status: number; stdout: string; stderr: string };
    return { salida: fallo.status, texto: `${fallo.stdout}${fallo.stderr}` };
  }
}

describe('el guardián de componentes del sistema de diseño', () => {
  const temporales: string[] = [];

  afterEach(() => {
    for (const t of temporales) {
      try {
        rmSync(t, { recursive: true, force: true });
      } catch {
        // Ignorar limpieza de temporales.
      }
    }
    temporales.length = 0;
  });

  function crearArbol(): { dir: string; uiDir: string } {
    const dir = mkdtempSync(join(tmpdir(), 'nb-ui-check-'));
    temporales.push(dir);
    const uiDir = join(dir, 'lib', 'ui');
    mkdirSync(uiDir, { recursive: true });
    writeFileSync(join(dir, 'app.css'), ':root { color-scheme: dark; }\n');
    return { dir, uiDir };
  }

  it('detiene un componente huérfano que ninguna pantalla consume', () => {
    const { dir, uiDir } = crearArbol();

    writeFileSync(
      join(uiDir, 'Fantasma.svelte'),
      '<script lang="ts">export let texto = "";</script><p>{texto}</p>\n'
    );

    const resultado = guardian(dir);
    expect(resultado.salida).toBe(1);
    expect(resultado.texto).toContain(
      'Fantasma.svelte: componente del sistema de diseño sin usar en ninguna pantalla (R1)'
    );
  });

  it('acepta el componente cuando una vista lo consume', () => {
    const { dir, uiDir } = crearArbol();

    writeFileSync(
      join(uiDir, 'Fantasma.svelte'),
      '<script lang="ts">export let texto = "";</script><p>{texto}</p>\n'
    );

    const vistasDir = join(uiDir, 'vistas');
    mkdirSync(vistasDir, { recursive: true });
    writeFileSync(
      join(vistasDir, 'UnaVista.svelte'),
      '<script lang="ts">import Fantasma from "../Fantasma.svelte";</script><Fantasma texto="hola" />\n'
    );

    const resultado = guardian(dir);
    expect(resultado.salida).toBe(0);
    expect(resultado.texto).toContain('Sistema de diseño: componentes conformes con ADR-012.');
  });

  it('el árbol real de src cumple la regla sin componentes huérfanos', () => {
    const resultado = guardian(join(RAIZ, 'src'));
    expect(resultado.salida).toBe(0);
    expect(resultado.texto).toContain('Sistema de diseño: componentes conformes con ADR-012.');
  }, 20_000);
});
