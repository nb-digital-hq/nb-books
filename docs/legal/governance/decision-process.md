# Proceso de Decisión

> **Estado:** Vigente.

## 1. Tres niveles de decisión

| Nivel | Ejemplos | Instrumento |
|---|---|---|
| Operativa | Nombre de una función, orden de los elementos de un menú | Pull Request |
| Significativa | Nueva dependencia, cambio de esquema, nuevo comando IPC, cambio de comportamiento visible | Issue de propuesta + PR |
| Estructural | Stack tecnológico, licencia, modelo de datos, protocolo de integración, política de privacidad | ADR |

La regla práctica: si la decisión será difícil de revertir dentro de seis meses, es estructural y necesita ADR.

## 2. Registro de decisiones arquitectónicas (ADR)

Un ADR documenta el contexto, las alternativas evaluadas, la decisión y sus consecuencias. Su valor no está en la
decisión, sino en preservar **por qué** se tomó, para que quien la revise en tres años no repita el análisis ni la
revierta por desconocimiento.

* Los ADR son inmutables. Una decisión que cambia no se edita: se escribe un ADR nuevo que supersede al anterior,
  y el anterior se marca como superado con enlace al sucesor.
* Un ADR debe registrar las alternativas **rechazadas** y el motivo. Un ADR sin alternativas es un comunicado, no
  una decisión.
* Formato en [ADR-000-template.md](../../adr/000-template.md).

## 3. Propuestas

Para decisiones significativas y estructurales, el flujo es:

1. **Propuesta:** issue con problema, restricciones, alternativas y recomendación.
2. **Discusión:** periodo abierto de comentarios con plazo declarado.
3. **Resolución:** la decisión se toma y se registra. Si es estructural, se escribe el ADR.
4. **Implementación:** el PR referencia el ADR.

Nadie debería descubrir que su trabajo no será aceptado después de haberlo escrito.

## 4. Quién decide

Mientras el proyecto tenga un mantenedor, decide el mantenedor, pero **con el registro escrito completo**. La
obligación de documentar el razonamiento es lo que hace auditable una decisión unipersonal y lo que permite que
otras personas se incorporen sin arqueología.

A medida que se incorporen mantenedores, la decisión pasa a requerir acuerdo entre los responsables del área
afectada, con el mantenedor principal como desempate.

## 5. Desacuerdos

* El desacuerdo técnico se resuelve con evidencia: medición, prototipo o cita del estándar.
* Si no hay evidencia disponible, se elige la opción más fácil de revertir.
* La decisión y su motivo se escriben. Un desacuerdo cerrado sin registro reaparece.

## 6. Filtro constitucional

Toda decisión, en cualquier nivel, debe superar la prueba de la
[Constitución](../../architecture/constitution.md). Una propuesta que contradiga una regla constitucional solo
puede avanzar modificando primero la Constitución mediante un ADR explícito. Las reglas absolutas que se
incumplen en silencio dejan de ser reglas.
