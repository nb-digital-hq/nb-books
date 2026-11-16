# Estrategia de Testing (Testing Strategy)

> **Estado:** Parcialmente implementado. Existen las capas 1 a 4, 5 (solo humo del binario) y 7;
> faltan la 6 y la 8, y la medición de cobertura. El estado real de cada capa está en la tabla de abajo.
> Un producto diseñado para durar 10 años no sobrevive sin pruebas de regresión.

La arquitectura de NB Books (Tauri) presenta desafíos únicos de testing, ya que combina un backend de sistemas (Rust), un puente de comunicación asíncrono (IPC) y un frontend reactivo renderizado por el motor nativo del SO (WebKitGTK, WebView2, WKWebView).

Nuestra estrategia se divide en capas que priorizan la estabilidad del Core.

---

## La Pirámide de NB Books

| # | Capa | Qué cubre | Estado hoy |
|---|---|---|---|
| 1 | Unit Tests (Rust) | Lógica pura de cada módulo. Cobertura objetivo mayor al 90% en el módulo de persistencia y en el motor de PDF cuando exista | 324 pruebas unitarias, 754 en total, medidas el 2026-09-07. **Sin medición de cobertura**: requiere la integración continua (deuda) |
| 2 | Unit/Component Tests (Svelte/TS) | Stores y componentes aislados | 529 pruebas en 29 archivos, medidas el 2026-09-07: capa de acceso al puente, sistema de diseño y aritmética de la lista virtualizada. **Los componentes no se montan**: su accesibilidad la hacen cumplir los avisos del compilador de Svelte, bloqueantes, y `scripts/check-ui.mjs` |
| 3 | Integration Tests | Comandos de Tauri sobre el estado compartido, sin abrir ventana | 4 pruebas |
| 4 | Contract Tests | Contrato IPC y esquema de eventos entre Rust y TypeScript | Tipos generados con `ts-rs`; deriva comprobada por `pnpm check:bindings` |
| 5 | End-to-End | La compilación de producción en un navegador real, con el runtime simulado; y el binario compilado con directorio de datos aislado | Diez recorridos de teclado, foco y modos degradados (`e2e/`), más el humo del arranque. Conducir el **binario** sigue siendo deuda |
| 6 | Performance Tests | Presupuestos de rendimiento | `pnpm baseline` mide; **todavía no falla la compilación** al excederse |
| 7 | Chaos Tests | Caídas de componentes, base bloqueada o corrupta, entrada hostil | 15 escenarios, medidos el 2026-09-07. Los cuatro últimos son del cajón de arena de extensiones, sobre el banco adverso |
| 8 | Accessibility Tests | WCAG sobre el shell y el lector | Pendiente: no hay interfaz que auditar |

---

## Contenido de la Sección

* [**Unit Tests**](unit.md): Frameworks (Cargo Test, Vitest) y reglas de mocking.
* [**Integration Tests**](integration.md): Cómo testear la capa de comandos Tauri sin abrir ventanas reales.
* [**Contract Tests**](contract.md): Pruebas de esquema para el puente IPC, los eventos y `nb-extension.json`.
* [**End-to-End y Regresión**](regression.md): Flujos críticos (Smoke Tests) sobre el binario compilado.
* [**Performance Testing**](../../audit/performance/testing.md): Verificación de arranque, RAM y rendimiento de renderizado.
* [**Chaos Testing**](chaos.md): Degradación elegante sometida a prueba automatizada.
* [**Lectores de pantalla**](screen-readers.md): La lista que **no** se puede automatizar, y el registro de
  cada pasada. Ninguna herramienta sustituye a una persona con el lector encendido.

---

## Criterio de "hecho" (Definition of Done)

Ningún PR que introduzca lógica nueva se fusiona sin:

1. Pruebas unitarias de la lógica pura añadida.
2. Un test de contrato si el PR modifica un tipo que cruza el puente IPC.
3. Una prueba de regresión si el PR corrige un bug reportado (el test debe fallar antes del fix).
