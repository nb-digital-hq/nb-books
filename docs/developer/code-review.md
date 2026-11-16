# Revisión de Código (Code Review)

> **Estado:** Vigente.

La revisión de código en NB Books tiene dos objetivos: evitar defectos y difundir conocimiento del sistema. No es
un trámite ni un lugar para discutir estilo, que ya resuelven los linters.

## 1. Reglas del proceso

* Todo cambio entra por Pull Request. No hay excepciones para el mantenedor principal.
* Se requiere al menos una aprobación de un Code Owner del área afectada.
* Los cambios en seguridad, criptografía, migraciones de esquema o el contrato IPC requieren dos aprobaciones.
* Un PR debe ser tan pequeño como sea posible. Si toca más de unos pocos cientos de líneas de lógica, debe
  dividirse o justificar por qué no puede dividirse.
* El autor no aprueba ni fusiona su propio PR sin revisión.

## 2. Responsabilidades del autor

Antes de pedir revisión:

1. La descripción explica **qué problema resuelve** y **por qué así**, con enlace al issue o ADR correspondiente.
2. Los checks automáticos están en verde.
3. Hay pruebas para la lógica nueva y una prueba de regresión para cada bug corregido.
4. La documentación en `docs/` se actualizó si el cambio tiene impacto arquitectónico o de uso
   (Constitución, regla 5).
5. Si el cambio afecta la interfaz, se incluye evidencia visual y confirmación de navegación por teclado.

## 3. Qué busca el revisor

### Corrección
* Casos límite: archivo inexistente, disco lleno, documento corrupto, red caída, base bloqueada.
* Ausencia de `unwrap` y `expect` injustificados en Rust.
* Errores propagados como tipos de dominio, no como cadenas opacas.

### Arquitectura
* Respeto a los límites de responsabilidad: la lógica pesada vive en Rust, no en el webview.
* Dependencias entre componentes a través de contratos, no de implementaciones concretas.
* Ningún componente nuevo introduce un punto único de fallo.

### Seguridad
* Todo comando IPC nuevo valida y canonicaliza rutas y no confía en datos del frontend.
* Ninguna capability nueva de Tauri se concede sin justificación.
* Ningún secreto, ruta de usuario o título de documento llega a los logs.

### Rendimiento
* No se envían colecciones grandes por IPC; se pagina en Rust.
* No se añaden dependencias que rompan el presupuesto de bundle.
* No se introduce trabajo síncrono en el hilo de la interfaz.

### Accesibilidad
* Elementos interactivos reales (`button`, `a`) en lugar de `div` con manejadores.
* Foco visible, orden de tabulación coherente, etiquetas ARIA donde el patrón no es nativo.

### Privacidad
* Ningún flujo nuevo envía datos fuera del dispositivo sin consentimiento explícito.

## 4. Cómo se comenta

* El comentario señala el problema y, cuando es posible, propone una alternativa.
* Se distingue lo bloqueante de lo opcional. Una sugerencia menor se marca como tal y no bloquea el merge.
* Se critica el código, nunca a la persona. El [código de conducta](../legal/governance/code-of-conduct.md) aplica
  íntegramente a las revisiones.
* Ante desacuerdo técnico irresuelto, decide el Code Owner del área y la decisión se documenta.

## 5. Tiempos

Una primera respuesta en un plazo razonable es parte del contrato con los contribuidores externos. Un PR sin
respuesta durante semanas es una forma de rechazo silencioso, y es peor que un rechazo explícito y argumentado.
