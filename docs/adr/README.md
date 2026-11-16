# Registro de Decisiones Arquitectónicas (ADR)

Un ADR documenta una decisión estructural: su contexto, las alternativas evaluadas, lo decidido y sus
consecuencias. Su valor no está en la decisión, sino en preservar **por qué** se tomó.

## Reglas

* Los ADR son **inmutables**. Una decisión que cambia no se edita: se escribe un ADR nuevo que supersede al
  anterior, y el anterior se marca como superado con enlace al sucesor.
* Un ADR sin alternativas rechazadas y sin sus motivos es un comunicado, no una decisión.
* Un ADR en estado Propuesto no autoriza a implementar.

## Índice

| ADR | Título | Estado |
|---|---|---|
| [001](001-tech-stack.md) | Selección del stack tecnológico | Aceptado |
| [002](002-critical-bottlenecks.md) | Mitigación de cuellos de botella críticos | Aceptado |
| [003](003-licensing-model.md) | Modelo de licenciamiento y procedencia de contribuciones | **Aceptado** |
| [004](004-reader-locator-model.md) | Modelo de posición y anclaje de anotaciones | **Aceptado** |
| [007](007-vector-store.md) | Almacén vectorial para búsqueda semántica: ninguno de terceros, fuerza bruta sobre candidatos | **Aceptado** |
| [010](010-settings-persistence.md) | Persistencia de las preferencias de usuario | **Aceptado** |
| [012](012-ui-paradigm.md) | Paradigma de la interfaz: aplicación convencional de paneles y vistas | **Aceptado** |
| [013](013-information-architecture.md) | Arquitectura de información: destinos de la interfaz y qué no lo es | **Aceptado** |
| [014](014-design-system.md) | Sistema de diseño visual: Material 3 con m3-svelte | **Aceptado** |
| [015](015-ui-stack-retention.md) | Continuidad del stack de interfaz (Tauri v2 + Svelte 5) | **Aceptado** |

Plantilla: [ADR-000-template.md](000-template.md).

## Cuándo escribir un ADR

Cuando la decisión afecte a la estructura, las dependencias, la seguridad, el modelo de datos, un contrato
público o la viabilidad legal o comercial del proyecto. La regla práctica: si será difícil de revertir dentro de
seis meses, necesita ADR.

Ver [governance/decision-process.md](../legal/governance/decision-process.md).
