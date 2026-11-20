# Git Workflow

> **Estado:** Vigente.

Usamos una variante simplificada de Trunk-Based Development combinada con Feature Branches.

## Reglas de Ramas
1. `main`: Siempre debe compilar y ser desplegable. Protegida por revisiones estrictas de CI.
2. `feat/*`: Para nuevas características (ej. `feat/pdf-annotations`).
3. `fix/*`: Para corrección de errores (ej. `fix/sqlite-lock`).
4. `chore/*`: Tareas de mantenimiento o refactor (ej. `chore/update-deps`).

## Flujo
1. Sincroniza `main`: `git pull origin main --rebase`
2. Crea tu rama: `git checkout -b feat/mi-funcionalidad`
3. Commits atómicos (Ver [Commit Conventions](commit-convention.md)).
4. Sube tu rama y abre PR contra `main`.
5. Squash and Merge al aprobarse.
