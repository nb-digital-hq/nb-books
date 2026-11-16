/**
 * Suite de extremo a extremo sobre la interfaz real.
 *
 * # Qué conduce esta suite, y qué no
 *
 * Conduce la **interfaz real en un navegador real**, con eventos de teclado y de
 * ratón de verdad: es la mitad que las pruebas declaraban como no
 * cubierta, porque una máquina de estado pura demuestra la lógica y no que el
 * navegador entregue las teclas.
 *
 * Lo que **no** conduce es el binario compilado. Eso necesita `tauri-driver` y
 * `WebKitWebDriver`, y ese segundo no existe empaquetado para WebKitGTK 4.1 en
 * Arch ni en AUR: habría que compilar WebKitGTK entero. Queda declarado en el
 * plan con su causa, no disimulado.
 *
 * El puente al runtime se simula en el navegador (`e2e/puente.ts`). No es una
 * carencia de esta suite: el runtime ya tiene 521 pruebas propias contra bases
 * de datos reales, y lo que aquí hace falta comprobar es que la interfaz
 * responde, no que SQLite escribe.
 *
 * Contrato: `docs/developer/testing/regression.md`.
 */

import { defineConfig, devices } from '@playwright/test';

/*
 * # El navegador, y por qué es una decisión y no un detalle
 *
 * Playwright descarga su propia compilación de Chromium. En Debian y Ubuntu es
 * la oficial; en Arch, Fedora o cualquier otra avisa —"your OS is not officially
 * supported"— y baja la compilación de Ubuntu 24.04, que funciona porque las
 * bibliotecas del sistema son compatibles. Ese aviso **no dice nada sobre el
 * producto**: Playwright es dependencia de desarrollo y no entra en el binario
 * de Tauri, así que las plataformas que NB Books soporta las decide esta parte y no
 * esto.
 *
 * Lo que sí decide es si quien contribuye desde otra distribución puede
 * verificar. Por defecto se usa la compilación de Playwright, que es la misma
 * para todo el mundo y hace la suite reproducible. `NB_NAVEGADOR` permite salir
 * de ahí cuando esa compilación no arranca:
 *
 *   NB_NAVEGADOR=chrome            usa el Chrome del sistema (también `msedge`)
 *   NB_NAVEGADOR=/usr/bin/chromium usa ese ejecutable
 *
 * Se prefiere la variable explícita a detectar un navegador del sistema por su
 * cuenta: dos personas con versiones distintas de Chrome verían resultados
 * distintos sin saber por qué, y una suite que no es reproducible no sirve para
 * decidir si algo está roto.
 */
const navegador = process.env.NB_NAVEGADOR;
const conNavegador =
  navegador === undefined
    ? {}
    : navegador.includes('/')
      ? { launchOptions: { executablePath: navegador } }
      : { channel: navegador };

export default defineConfig({
  testDir: './e2e',
  // Una interfaz que solo pasa cuando se reintenta es una interfaz con una
  // condición de carrera, y esconderla con reintentos es peor que verla fallar.
  retries: 0,
  fullyParallel: true,
  reporter: process.env.CI ? 'github' : 'list',

  use: {
    baseURL: 'http://localhost:4173',
    // Se guarda el rastro solo del primer intento fallido: en verde no aporta
    // nada y llena el disco.
    trace: 'retain-on-failure'
  },

  /*
   * # El motor, y el aviso que cuesta caro no leer
   *
   * Esta suite corre en Chromium y **la aplicación corre en WebKitGTK**. No se
   * parecen en lo que importa para el rendimiento: el mismo desplazamiento sobre
   * la misma biblioteca daba 17 ms por fotograma en uno y 40 en el otro. Todo lo
   * que esta suite afirma sobre fotogramas está afirmado sobre un motor que el
   * producto no usa, y por eso podía estar en verde mientras la aplicación se
   * veía lenta al usarla.
   *
   * `NB_WEBKIT=1` añade el WebKit de Playwright, que es el mismo proyecto que
   * WebKitGTK aunque no la misma compilación, y es lo más cerca que se puede
   * estar sin salir de aquí. **No arranca en cualquier distribución**: necesita
   * bibliotecas del sistema que su paquete no instala, y en Arch faltan. Por eso
   * va detrás de una variable y no por defecto: una suite que no arranca en la
   * máquina de quien contribuye es una suite que se acaba ignorando entera.
   *
   * Para medir en el motor de verdad está `scripts/medir-en-webkitgtk.py`, que
   * usa el WebKitGTK instalado. Qué se hace para que esto sea un mecanismo y no
   * una herramienta es la pregunta I13.
   */
  projects: [
    { name: 'escritorio', use: { ...devices['Desktop Chrome'], ...conNavegador } },
    ...(process.env.NB_WEBKIT ? [{ name: 'webkit', use: { ...devices['Desktop Safari'] } }] : [])
  ],

  webServer: {
    // Contra la compilación de producción y no contra el servidor de
    // desarrollo: lo que se prueba tiene que ser lo que se publica.
    // `NB_E2E` desvía la compilación a `.svelte-kit-e2e/` y `build-e2e/`, para
    // que verificar no le quite el suelo a un `pnpm tauri dev` abierto.
    command: 'NB_E2E=1 pnpm build && NB_E2E=1 pnpm preview --port 4173 --strictPort',
    url: 'http://localhost:4173',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000
  }
});
