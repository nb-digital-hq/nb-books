# Pruebas End-to-End (E2E) y Regresión

> **Estado:** Implementado: recorridos de interfaz con Playwright sobre la compilación de producción, recorrido en
> WebKitGTK y pruebas de humo del binario en Rust.
>
> Las pruebas E2E validan que NB Books renderiza, entrega las teclas al componente correcto y mueve el foco donde
> dice moverlo, sobre la misma compilación que se empaqueta.

## 1. La herramienta

### 1.1 Lo que hay: Playwright sobre la interfaz compilada

La suite vive en `e2e/` y la configura [`playwright.config.ts`](../../../playwright.config.ts). Se ejecuta con:

```bash
pnpm test:e2e
```

Playwright levanta `pnpm build && pnpm preview` —**la compilación de producción, no el servidor de
desarrollo**— y conduce Chromium contra ella. El runtime se simula dentro del navegador (`e2e/puente.ts`)
inyectando `__TAURI_INTERNALS__` antes de que la aplicación monte, que es exactamente lo que hace el webview de
Tauri.

**Por qué el runtime se simula.** Lo que estas pruebas existen para comprobar es que la interfaz responde: que
una tecla llega, que el foco se mueve, que un panel se abre, que la carcasa monta sin un error en consola. Que
SQLite escribe lo comprueban las pruebas de integración del runtime contra bases de datos reales. El simulador
**responde con los tipos generados desde Rust** (`src/lib/bindings/`), así que un cambio de contrato lo rompe
igual que rompería a la aplicación.

**El navegador.** Por defecto, la compilación de Chromium que descarga Playwright, que es la misma para todo el
mundo. Fuera de Debian y Ubuntu avisa de que baja la compilación de Ubuntu 24.04; funciona, y no dice nada sobre
las plataformas que soporta el producto, porque Playwright no entra en el binario de Tauri. `NB_NAVEGADOR`
permite usar el Chrome del sistema (`chrome`, `msedge`) o un ejecutable por su ruta, para las distribuciones
donde la compilación de Playwright no arranque. Ver [setup.md](../setup.md).

**Compilar no interfiere con desarrollar.** La suite compila en `.svelte-kit-e2e/` y `build-e2e/`, y el servidor
de desarrollo no vigila ninguna salida de compilación; compartir carpeta dejaría la aplicación de desarrollo sin
estilos al verificar. Lo sostienen tres pruebas en
`tests/aislamiento-de-compilacion.test.ts`.

**Por qué no basta con las pruebas de unidad.** Las comprueban una máquina de estado pura: demuestran
que la lógica es correcta, no que el navegador entregue la pulsación al componente ni que `focus` acabe donde
se cree. Los dos defectos que esta suite encontró el día que se escribió —un bucle de efectos que solo aparece
cuando el DOM está compuesto, y una promesa rechazada sin capturar al suscribirse al bus— eran invisibles para
cualquier prueba sin navegador.

### 1.2 El motor real y el binario

* **El motor del producto.** `pnpm check:webkitgtk` recorre la compilación de producción en el WebKitGTK del
  sistema y decide con lo determinista (nodos, píxeles descodificados, capas); se omite donde no hay WebKitGTK ni
  servidor gráfico. Ver [performance-budgets.md](../../architecture/performance-budgets.md), sección 3.
* **El binario.** `src-tauri/tests/arranque_e2e.rs`, `arranque_real.rs` y `cierre_de_sesion.rs` lanzan el runtime de
  verdad: arranque, base creada o migrada, cierre limpio y sucio.
* Playwright no habla WebDriver, así que no puede conducir el binario de Tauri; conducirlo exigiría `tauri-driver`,
  `WebKitWebDriver` y WebdriverIO.

## 2. Suite de regresión

Se ejecuta dentro de `pnpm verify`, así que corre antes de cada commit sin que haya que acordarse.

### Smoke 1: Arranque de la aplicación

| # | Qué se comprueba | Dónde |
|---|---|---|
| 1 | El binario se lanza dentro del presupuesto de arranque | `src-tauri/tests/arranque_e2e.rs` |
| 2 | La base SQLite se crea o migra en el directorio de datos | `src-tauri/tests/arranque_e2e.rs` |
| 3 | Svelte renderiza la carcasa **sin ningún error en consola** | `e2e/flujo.spec.ts` |
| 4 | El comando IPC `ping` responde | `src-tauri/tests/arranque_e2e.rs` |

El punto 3 es una aserción de la suite entera, no de una prueba: cualquier error de consola en cualquier
recorrido falla la prueba que lo produjo. Un error de consola es un defecto, no un aviso.

### Smoke 2: Lectura y anotación (recorrido principal)

Cubierto en `e2e/flujo.spec.ts`, **sin ratón en ninguna de las pruebas salvo donde su nombre lo dice**:

1. La aplicación arranca en Inicio y pinta lo que se está leyendo.
2. La recomendación dice por qué aparece.
3. Se llega a la biblioteca solo con el tabulador y Enter.
4. Abrir, bajar el cursor de bloque, extender la selección y anotar, todo con teclado.
5. Mientras se escribe una nota, las letras son letras y no atajos.
6. La carcasa se retira entera en inmersión y vuelve donde estaba.
7. El Lector no se cierra al ir a la biblioteca (regla 2 de ADR-013).
8. Los controles del lector salen de las capacidades que declara el formato.
9. El tema se cambia y la interfaz entera cambia con él.
10. Sin bus de eventos, la aplicación monta igual y sin errores.

### Smoke 2b: composición

Reglas de [ui-conventions.md](../../design/ui-conventions.md) que no dependen del gusto de nadie:

11-13. **Ningún destino se desplaza en horizontal** a 640, 900 ni 1440 px —ni la ventana, ni ningún contenedor
   con `overflow-x` desplazable, que es el caso que esconde la barra en mitad de la vista—.
14. **El lector con sus paneles tampoco**, a 640 px, con el índice y con los ajustes del libro abiertos.
15. **Los siete temas pintan fondo y texto.** Un token sin definir deja texto invisible sin que nada avise;
   `check-ui.mjs` comprueba que el token exista, y esto comprueba el resultado.

Lo que exige la ventana del sistema operativo (el diálogo nativo, cerrar y reabrir) lo cubren las pruebas del
runtime de la sección 1.2 y la lista manual de cada publicación.

### Smoke 3: Migración de esquema

`src-tauri/tests/humo_de_migracion.rs` parte de una base con el esquema de la primera publicación, arranca la versión
actual y comprueba que no se pierde nada. Es la defensa contra la pérdida de bibliotecas.

### Smoke 4: Accesibilidad

`e2e/accesibilidad.spec.ts`: `axe-core` sobre la carcasa, la biblioteca y el lector, cero violaciones serias o
críticas.

## 3. Entorno aislado

La suite de interfaz no toca disco: el runtime está simulado y no hay ninguna base de datos que ensuciar.

Las pruebas del binario sí son destructivas por naturaleza. Para no borrar la biblioteca real de quien las
ejecuta, el binario de prueba se fuerza a un directorio de datos aislado:

```bash
NB_BOOKS_TEST_DIR=/tmp/nb_books_e2e_test cargo test --test arranque_e2e
```
