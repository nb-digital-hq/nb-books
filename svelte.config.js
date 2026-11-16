import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/**
 * NB Books opera como aplicación de página única dentro del webview de Tauri.
 * El adaptador estático es obligatorio: no hay servidor, no hay renderizado en servidor.
 * Ver docs/adr/001-tech-stack.md
 * @type {import('@sveltejs/kit').Config}
 */
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

const config = {
  preprocess: vitePreprocess(),
  kit: {
    outDir: `.svelte-kit${sufijo}`,
    adapter: adapter({
      pages: `build${sufijo}`,
      assets: `build${sufijo}`,
      fallback: 'index.html',
      precompress: false,
      strict: true
    }),
    alias: {
      $core: 'src/lib/core',
      $ui: 'src/lib/ui'
    }
  },
  compilerOptions: {
    runes: true
  }
};

export default config;
