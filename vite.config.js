import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

const host = process.env.TAURI_DEV_HOST;

/*
 * La suite de extremo a extremo compila en su propio directorio.
 *
 * Compartirlo con el servidor de desarrollo tiene una consecuencia concreta y
 * reproducible: `pnpm verify` compila, eso reescribe `.svelte-kit/generated/` y
 * `build/` bajo los pies de un `pnpm tauri dev` abierto, y el servidor se queda
 * sin poder servir el CSS de los componentes —"failed to load virtual css
 * module"— hasta que se reinicia. Con dos directorios, las dos cosas conviven.
 */
const sufijo = process.env.NB_E2E ? '-e2e' : '';

/**
 * NB Books opera como aplicación de página única dentro del webview de Tauri.
 * El adaptador estático es obligatorio: no hay servidor, no hay renderizado en
 * servidor (docs/adr/001-tech-stack.md). SvelteKit 3 ya no lee
 * `svelte.config.js`: la configuración va aquí.
 */
export const kit = {
  preprocess: vitePreprocess(),
  outDir: `.svelte-kit${sufijo}`,
  adapter: adapter({
    pages: `build${sufijo}`,
    assets: `build${sufijo}`,
    fallback: 'index.html',
    precompress: false,
    strict: true
  }),
  // `$lib` deja de ser automático en SvelteKit 3; el código entero lo usa.
  alias: {
    $lib: 'src/lib',
    $core: 'src/lib/core',
    $ui: 'src/lib/ui'
  },
  compilerOptions: {
    runes: true
  }
};

export default defineConfig({
  plugins: [sveltekit(kit)],

  // Misma razón que el `outDir` de SvelteKit: dos procesos de Vite sobre la
  // misma caché de dependencias se invalidan el uno al otro.
  cacheDir: process.env.NB_E2E ? 'node_modules/.vite-e2e' : 'node_modules/.vite',

  // Tauri espera un puerto fijo y falla si no está disponible.
  clearScreen: false,
  server: {
    /*
     * Los componentes se transforman al arrancar, antes de que nadie los pida.
     *
     * Sin esto hay una carrera. El navegador pide el CSS de un componente
     * —`Panel.svelte?svelte&type=style&lang.css`— y `vite-plugin-svelte` lo
     * busca en la caché que llena la transformación del JS de ese componente.
     * Si el CSS llega primero, la caché está vacía: avisa "failed to load
     * virtual css module" y devuelve el control, y **otro complemento sirve el
     * CSS correcto de todos modos**. Medido: la respuesta con aviso y la
     * respuesta sin aviso son idénticas byte a byte, 200 las dos.
     *
     * Así que lo que sobra es el aviso, no el CSS. Y sobra de verdad: quince
     * líneas rojas al arrancar enseñan a no leer el registro, y entonces el
     * error que sí importa pasa desapercibido.
     *
     * Sale con el webview de Tauri y no con Chromium porque piden en distinto
     * orden, y solo en desarrollo: al compilar, el CSS va en un archivo.
     *
     * Comprobado con la misma secuencia de peticiones en los dos casos: sin
     * esta línea, dos avisos; con ella, ninguno.
     */
    warmup: { clientFiles: ['./src/**/*.svelte'] },
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: {
      // El árbol de Rust lo vigila Cargo, no Vite. Y la salida de compilación no
      // es código fuente: vigilarla hace que compilar recargue el servidor de
      // desarrollo, que es medio camino andado hacia perder el grafo de módulos.
      ignored: [
        '**/src-tauri/**',
        '**/build/**',
        '**/build-e2e/**',
        // Lo que compila la suite de extremo a extremo. Sin esta línea, Vite ve
        // aparecer su `tsconfig.json`, limpia la caché y fuerza una recarga
        // completa: justo el estado en el que no puede servir el CSS virtual.
        '**/.svelte-kit-e2e/**',
        '**/test-results/**',
        '**/playwright-report/**'
      ]
    }
  },

  test: {
    include: ['src/**/*.{test,spec}.{js,ts}', 'tests/**/*.{test,spec}.{js,ts}'],
    environment: 'node'
  }
});
