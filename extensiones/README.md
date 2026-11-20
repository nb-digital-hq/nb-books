# Extensiones

Aquí vive lo que se compila **para el cajón de arena** y no para el anfitrión:
módulos WebAssembly con su manifiesto. No es código de la aplicación, así que
tiene su propio espacio de trabajo de Cargo y su propio objetivo de compilación,
`wasm32-wasip1`.

| Directorio | Qué es |
|---|---|
| `propias/` | Las extensiones que publica el proyecto. Tienen su propio README y su propio espacio de trabajo |
| `tema-de-prueba/` | Un tema de ejemplo. **No tiene código**: es un manifiesto y nada más, que es exactamente lo que un tema puede ser |
| `banco/` | El banco: trece módulos. Ocho intentan salirse del cajón de arena, uno es la versión benigna de un par, y cuatro son componentes compilados contra un mundo WIT: `usa-el-mundo`, `formato-de-texto` `fuente-de-metadatos` y `vista-de-estudio` |

## Cómo se construye

```bash
# Los módulos sueltos, que son el camino que usaría un atacante.
cargo build --manifest-path extensiones/banco/Cargo.toml --target wasm32-wasip1

# El componente, que es el camino por el que entran las extensiones de verdad.
cargo build --manifest-path extensiones/banco/Cargo.toml --target wasm32-wasip2 \\
  -p usa-el-mundo -p formato-de-texto -p fuente-de-metadatos -p vista-de-estudio
```

Hace falta el objetivo instalado. En Arch es el paquete `rust-wasm`; con
`rustup`, `rustup target add wasm32-wasip1`. Si no está, las pruebas que
dependen de él **se saltan solas** y lo dicen, igual que las del corpus real.

No hace falta construirlo a mano: la prueba de integración
`src-tauri/tests/banco_de_extensiones.rs` lo construye ella misma antes de
comprobar nada.

## Lo que nunca entra al repositorio

Los `.wasm` compilados. Los ignora `.gitignore` y los rechaza el gancho de
pre-commit, por el innegociable de binarios: un módulo compilado es a la vez un
binario opaco y una copia que puede quedarse vieja respecto a su fuente sin que
nadie lo note.
