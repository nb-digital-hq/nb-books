# Cómo Contribuir

> **Estado:** Vigente.
> Este documento es la puerta de entrada para colaboradores. Para el primer PR paso a paso, ver
> [first-pr.md](first-pr.md).

NB Books es un proyecto de código abierto. Antes de escribir código, conviene
entender qué se acepta y qué no.

## 1. Antes de escribir código

1. Lee la [Filosofía de Producto](../design/product-philosophy.md) y la
   [Constitución](../architecture/constitution.md). Una contribución técnicamente correcta que contradiga
   estas reglas será rechazada.
2. Busca si ya existe un issue. Si no existe, ábrelo antes de implementar.
3. Para cambios con impacto arquitectónico, abre una propuesta antes de codificar. Ver
   [governance/decision-process.md](../legal/governance/decision-process.md).

Esta regla protege tu tiempo: nada es más frustrante que un PR grande rechazado por razones de dirección de
producto que podían discutirse antes.

## 2. Tipos de contribución bienvenidos

* Corrección de defectos reproducibles.
* Mejoras de accesibilidad y de rendimiento con medición.
* Traducciones y localización.
* Documentación: correcciones, ejemplos, guías de usuario.
* Soporte de formatos y conectores, preferentemente como extensiones.
* Informes de seguridad, a través del canal privado descrito en
  [governance/security-policy.md](../legal/governance/security-policy.md), nunca como issue público.

## 3. Qué no se acepta

* Código de terceros sin licencia compatible o sin atribución.
* Funcionalidad para eliminar protecciones técnicas de obras protegidas. Ver
  [legal/copyright-and-drm.md](../legal/copyright-and-drm.md).
* Telemetría, analítica o publicidad.
* Dependencias que incumplan la [política de dependencias](../architecture/dependency-policy.md).
* Cambios masivos de estilo o refactorizaciones amplias sin acuerdo previo.
* Archivos con copyright vigente añadidos al repositorio como material de prueba.

## 4. Licencia y procedencia de las contribuciones

NB Books se publica bajo **Apache License 2.0**. Tu contribución se licencia bajo esos mismos términos y **tú
conservas el copyright**: el proyecto no pide cesión de derechos ni firma de ningún acuerdo.

Lo único que se exige es una **declaración de origen (DCO)**: cada commit debe llevar la línea `Signed-off-by`,
que se añade automáticamente con `git commit -s`.

```bash
git commit -s -m "fix(library): corregir orden de la cuadrícula al importar"
```

Al firmar así afirmas que tienes derecho a aportar ese código bajo la licencia del proyecto. El CI verifica la
presencia de la línea y rechaza los commits que no la tengan. Si olvidaste firmar, `git rebase --signoff` corrige
los commits de tu rama.

**No aportes código del que no seas autor** salvo que su licencia sea compatible y lo indiques explícitamente en
el PR, con la atribución correspondiente. Ver [legal/licensing.md](../legal/licensing.md).

## 5. Flujo de trabajo

1. Haz fork y crea una rama siguiendo [git-workflow.md](git-workflow.md).
2. Haz commits atómicos siguiendo [commit-convention.md](commit-convention.md).
3. Ejecuta linters y pruebas localmente antes de abrir el PR.
4. Abre el PR contra `main` con la plantilla completa.
5. Atiende la revisión descrita en [code-review.md](code-review.md).

## 6. Comunicación

Todas las interacciones se rigen por el [Código de Conducta](../legal/governance/code-of-conduct.md). El idioma
principal del proyecto es el español; se aceptan issues y PR en inglés.
