/**
 * Compilar no puede quitarle el suelo a un servidor de desarrollo abierto.
 *
 * El fallo que motiva estas pruebas se reprodujo el 2026-08-28: con
 * `pnpm tauri dev` levantado, ejecutar `pnpm verify` —que compila -- reescribía
 * `.svelte-kit/generated/` y `build/`, el servidor recargaba su grafo de módulos
 * a media petición y se quedaba sin poder servir el CSS de los componentes.
 * Quince líneas de "failed to load virtual css module" y una aplicación sin
 * estilos, sin ningún error que dijera por qué.
 *
 * Se arregla por construcción y no con una prueba de carrera: dos directorios
 * distintos, y la salida de compilación fuera de lo que el servidor vigila. Lo
 * que estas pruebas sostienen es que esas dos condiciones sigan ahí, no que la
 * carrera no exista.
 */

import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

/**
 * Lee las dos configuraciones tal y como las lee la herramienta: en un proceso
 * aparte, con el entorno puesto.
 *
 * No se importan aquí porque las dos dependen de `process.env` en el momento de
 * evaluarse, y un módulo ya cargado no se vuelve a evaluar: la segunda lectura
 * devolvería la primera respuesta, y la prueba pasaría sin comprobar nada.
 */
function configuracion(e2e: boolean) {
  const guion = `
    const { default: vite, kit } = await import('./vite.config.js');
    console.log(JSON.stringify({
      outDir: kit.outDir,
      cacheDir: vite.cacheDir,
      ignorados: vite.server.watch.ignored,
      precalentados: vite.server.warmup?.clientFiles ?? []
    }));
  `;
  const salida = execFileSync('node', ['--input-type=module', '-e', guion], {
    cwd: fileURLToPath(new URL('..', import.meta.url)),
    env: { ...process.env, NB_E2E: e2e ? '1' : '' },
    encoding: 'utf8'
  });
  return JSON.parse(salida) as {
    outDir: string;
    cacheDir: string;
    ignorados: string[];
    precalentados: string[];
  };
}

describe('la compilación de las pruebas y la de desarrollo no comparten sitio', () => {
  const normal = configuracion(false);
  const pruebas = configuracion(true);

  it('escriben en directorios distintos', () => {
    expect(normal.outDir).toBe('.svelte-kit');
    expect(pruebas.outDir).toBe('.svelte-kit-e2e');
  });

  it('tampoco comparten la caché de dependencias de Vite', () => {
    expect(pruebas.cacheDir).not.toBe(normal.cacheDir);
  });

  it('los componentes se transforman al arrancar y no cuando se piden', () => {
    // Si el CSS de un componente se pide antes de que su JS se haya
    // transformado, `vite-plugin-svelte` no lo encuentra en su caché y avisa.
    // Sirve el CSS correcto igualmente, pero quince avisos al arrancar enseñan
    // a no leer el registro. Ver el comentario en vite.config.js.
    expect(normal.precalentados).toContain('./src/**/*.svelte');
  });

  it('el servidor de desarrollo no vigila ninguna salida de compilación', () => {
    // Vigilar la salida hace que compilar recargue el servidor, que es medio
    // camino andado hacia perder el grafo de módulos.
    for (const salida of ['build', 'build-e2e', '.svelte-kit-e2e']) {
      expect(normal.ignorados, `${salida} tiene que estar fuera de la vigilancia`).toContain(
        `**/${salida}/**`
      );
    }
  });
});
