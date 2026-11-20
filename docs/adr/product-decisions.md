# Decisiones de producto

> **Estado:** Vigente. Registra los acuerdos sobre qué hace NB Books para la persona, derivados de la
> [filosofía de producto](../design/product-philosophy.md).

## 1. Experiencia individual

La aplicación permite crear y usar una biblioteca local moderna y robusta, completa sin ninguna edición de pago.

* **Gestión sin fricción:** importación de carpetas y de bibliotecas de Calibre, metadatos internos y en línea con
  consentimiento, portadas, estanterías, colecciones manuales y por regla, etiquetas y duplicados.
* **Lectura y trabajo integrados:** resaltado, subrayado, trazos, notas ancladas, consulta de diccionarios,
  enciclopedia y traducción, lectura en voz alta y vista dividida, sin saltar entre aplicaciones.
* **Progreso y estadísticas locales:** posición guardada por Locator, sesiones de lectura, ritmo propio y tiempo
  restante.
* **Estudio:** notas enlazadas con retroenlaces, tableros, vocabulario y repaso espaciado.
* **Independencia total:** todo el flujo central funciona sin internet, sin cuentas y sin suscripciones.

## 2. Inteligencia artificial

La IA es una capacidad transversal de **enriquecimiento opcional**, nunca la identidad del producto, con
privacidad por diseño.

* **Capacidades:** resúmenes, tarjetas, preguntas de autoevaluación, conceptos y búsqueda por significado.
* **Motores:** inferencia local con Candle, en Rust puro, y proveedores externos que la persona configura y autoriza
  de forma consciente. Ver [local-models.md](../architecture/ai/local-models.md).
* **Degradación:** si no hay modelo o el equipo no lo ejecuta, la función se oculta sin afectar a la lectura.
* **Distribución:** los motores y modelos son componentes opcionales de primera parte, descargados por la aplicación
  y verificados por huella, nunca extensiones de terceros. Ver la sección 6 de
  [plugin-system.md](../architecture/plugin-system.md).

## 3. Fuera de esta aplicación

La integración con sistemas de gestión del aprendizaje, la identidad corporativa, el despliegue gestionado y la
analítica agregada pertenecen a la Suite Institucional de NB Digital, que se distribuye por separado. Ver
[institution](../institution/README.md).
