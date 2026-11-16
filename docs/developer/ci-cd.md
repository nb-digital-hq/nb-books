# Integración continua

> **Estado:** Implementado. Flujos: `.github/workflows/ci.yml` (cada push a `main` y cada pull request) y
> `.github/workflows/ediciones.yml` (manual, compilaciones de publicación). El proceso humano de revisión está en
> [code-review.md](code-review.md) y el corte de versiones en [releases.md](releases.md).

La verificación de referencia es local: `just verify` (ver `AGENTS.md`, sección 7). La integración continua repite
lo esencial en una máquina limpia, para que un cambio no dependa del equipo de quien lo escribió.

## 1. Trabajos de `ci.yml`

| Trabajo | Qué comprueba |
|---|---|
| Procedencia y convenciones | Conventional Commits y firma DCO de cada commit del pull request |
| Integridad de la documentación | `scripts/check-docs.mjs` |
| Frontend | `pnpm lint`, `pnpm check`, `pnpm test`, contrato de errores y `pnpm build` |
| Backend | `cargo fmt`, `cargo clippy -D warnings`, `cargo test` y que los tipos generados estén al día |
| Dependencias y licencias | `pnpm audit`, `cargo deny` (avisos, licencias, prohibiciones y orígenes) y avisos de terceros al día |
| Humo del binario | Arranque real bajo `xvfb` y suite de caos |

Todo corre en Ubuntu: Linux es la única plataforma de publicación.

## 2. Reglas de la rama principal

`main` se protege con un conjunto de reglas del repositorio (`just github-public`): sin borrado, sin reescritura de
historia y con cambios por pull request.

## 3. Firma

Ningún flujo firma. La clave de publicación vive fuera del repositorio y de GitHub, y la firma se hace en local con
`pnpm procedencia` (ver [deployment/releases.md](deployment/releases.md)). Así un flujo disparado desde una
bifurcación no tiene ningún secreto de firma que ver. Ver [supply-chain.md](../audit/supply-chain.md).
