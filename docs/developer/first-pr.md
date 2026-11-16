# Tu Primer Pull Request

> **Estado:** Vigente.
> Guía práctica para pasar de un repositorio recién clonado a un PR abierto.

## 1. Prepara el entorno

Sigue [setup.md](setup.md). Verifica que la aplicación arranca antes de cambiar nada:

```bash
pnpm install
pnpm tauri dev
```

Si algo falla en este paso, ya encontraste tu primera contribución: abre un issue describiendo tu sistema
operativo, versiones y el error exacto.

## 2. Elige una tarea

Los issues etiquetados como `good first issue` están acotados y tienen contexto suficiente. Comenta en el issue
que vas a trabajarlo para evitar trabajo duplicado.

Si es tu primera vez, evita: migraciones de base de datos, el puente IPC, el sistema de ventanas y cualquier cosa
etiquetada como `security`.

## 3. Crea la rama

```bash
git checkout main
git pull origin main --rebase
git checkout -b fix/descripcion-corta
```

## 4. Trabaja en cambios pequeños

Un buen primer PR toca pocos archivos y hace una sola cosa. Antes de commitear:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
pnpm biome check .
pnpm svelte-check
pnpm test
```

## 5. Escribe el commit

```bash
git commit -s -m "fix(library): corregir orden de la cuadrícula al importar"
```

El formato está definido en [commit-convention.md](commit-convention.md). La opción `-s` añade la línea
`Signed-off-by` requerida por la declaración de origen.

## 6. Abre el PR

La descripción debe responder tres preguntas:

1. ¿Qué problema resuelve? Enlaza el issue.
2. ¿Cómo lo resuelve? Una o dos frases sobre el enfoque.
3. ¿Cómo se verifica? Pasos manuales o el nombre del test añadido.

Si el cambio afecta la interfaz, adjunta una captura y confirma que funciona solo con teclado.

## 7. Durante la revisión

* Responde a los comentarios; si no estás de acuerdo, explica tu razonamiento con datos.
* Añade commits nuevos en lugar de reescribir la historia mientras la revisión está en curso.
* Cuando esté aprobado, el mantenedor hará Squash and Merge.

## 8. Qué esperar

La primera revisión puede tener bastantes comentarios. Es normal y no es una valoración sobre ti: el proyecto
mantiene un estándar alto porque los datos de los usuarios (sus bibliotecas y anotaciones) no son recuperables si
los perdemos.
