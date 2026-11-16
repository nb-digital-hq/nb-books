# Convención de Commits

> **Estado:** Vigente.

NB Books sigue estrictamente [Conventional Commits 1.0.0](https://www.conventionalcommits.org/). El CI fallará si los commits no respetan este formato.

## Formato
```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

## Tipos Permitidos
- `feat`: Una nueva característica (genera un MINOR release).
- `fix`: Resolución de un bug (genera un PATCH release).
- `docs`: Solo cambios en documentación.
- `style`: Cambios que no afectan la lógica del código (espacios, formato).
- `refactor`: Cambio de código que ni arregla un bug ni añade una funcionalidad.
- `perf`: Cambio que mejora el rendimiento.
- `test`: Añadir tests faltantes o corregir tests existentes.
- `chore`: Cambios al proceso de build o herramientas auxiliares.

## Ejemplos
- `feat(reader): add continuous scrolling mode`
- `fix(db): resolve database locked error on mass import`
- `docs(adr): document decision to use pdfium`
