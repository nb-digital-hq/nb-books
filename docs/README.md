# NB Books: documentación

Este directorio es la fuente única de verdad de NB Books. Lo que no está aquí no es oficial, y si el código y un
documento discrepan hay un defecto que se corrige en uno de los dos.

## Cómo está organizada

| Sección | Pregunta que responde | Audiencia |
|---|---|---|
| [user](user/README.md) | ¿Cómo se usa la aplicación? | Personas que leen y estudian |
| [institution](institution/README.md) | ¿Qué garantiza a una institución y cómo se despliega? | Dirección, TI, áreas legales y de cumplimiento |
| [architecture](architecture/README.md) | ¿Cómo está construido el sistema y qué contratos rigen? | Ingeniería |
| [adr](adr/README.md) | ¿Qué se decidió y por qué? | Ingeniería |
| [developer](developer/README.md) | ¿Cómo se trabaja en el código, se prueba y se empaqueta? | Contribuidores |
| [audit](audit/README.md) | ¿Qué garantías de privacidad, seguridad y rendimiento se pueden comprobar? | Auditores, ingeniería |
| [design](design/README.md) | ¿Cómo se ve, se siente y se usa la interfaz? | Diseño, producto, ingeniería |
| [legal](legal/README.md) | ¿Bajo qué condiciones se usa, se contribuye y se distribuye? | Todos |
| [assets](assets/) | Recursos gráficos de la documentación | Todos |

## Documentos de entrada recomendados

- Para **usar** la aplicación: [primeros pasos](user/getting-started.md).
- Para **evaluarla** en una institución: [visión institucional](institution/README.md) y
  [cumplimiento](institution/compliance.md).
- Para **construir**: [Constitución](architecture/constitution.md), [arquitectura](architecture/README.md) y
  [handbook de ingeniería](developer/handbook.md).
- Para **contribuir**: [cómo contribuir](developer/contributing.md) y [tu primer PR](developer/first-pr.md).
- Para **auditar**: [privacidad](audit/privacy.md), [modelo de amenazas](audit/threat-model.md) y
  [cadena de suministro](audit/supply-chain.md).

## Convención de estados

Todo documento técnico declara su estado en sus primeras líneas, para distinguir lo implementado de lo diseñado.

| Etiqueta | Significado |
|---|---|
| **Implementado** | El código refleja lo descrito y puede verificarse en el repositorio |
| **Parcialmente implementado** | Existe código que no cubre toda la especificación |
| **Diseño** | Especificación acordada que el código todavía no refleja |
| **Propuesta** | Recomendación que requiere aprobación |
| **Aplicable** | Norma vigente |

## Principios de esta documentación

- **Fuente única de verdad.** Cada hecho tiene un documento propietario; el resto enlaza.
- **Texto plano versionado.** Markdown en Git, revisable como el código. Diagramas en Mermaid.
- **Honestidad de estado.** Describir como existente algo que no existe es peor que no documentarlo.
- **Registro impersonal y profesional**, sin emoticonos. La documentación profunda está en español; los archivos
  de comunidad de la raíz del repositorio, en inglés.
- **La documentación forma parte del criterio de terminación.** Un cambio con impacto arquitectónico o de uso no
  está hecho hasta que su documento lo refleja (Constitución, regla 5). `pnpm check:docs` comprueba enlaces,
  duplicados, archivos vacíos y truncamientos.

## Licencia de esta documentación

Apache License 2.0, igual que el código. Ver `LICENSE` y `NOTICE` en la raíz del repositorio, y
[ADR-003](adr/003-licensing-model.md) para el razonamiento.
