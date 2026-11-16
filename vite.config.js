import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [sveltekit()],

  // Misma razón que el `outDir` de svelte.config.js: dos procesos de Vite sobre
  // la misma caché de dependencias se invalidan el uno al otro.
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
