# Constitución (Constitution)

> **Estado:** Vigente. Modificable únicamente mediante ADR aprobado.

Este documento representa el conjunto de reglas absolutas, técnicas y de producto de NB Books. A diferencia de la filosofía (que marca una dirección), **la Constitución dicta leyes**. Ningún Pull Request (PR), nueva arquitectura o feature debe fusionarse si rompe alguno de estos mandamientos.

---

## 1. Regla de Resiliencia Arquitectónica
**"Un fallo local debe producir consecuencias locales."**
* Ningún componente no esencial debe derribar la aplicación completa (*Graceful Degradation*).
* Si el servicio de Inteligencia Artificial falla, solo las herramientas de IA dejan de funcionar; el libro sigue siendo legible.
* Si un proveedor externo (metadatos, catálogo, diccionario, voz) no responde, la biblioteca y la lectura siguen intactas.
* Los *Single Points of Failure (SPOF)* deben ser identificados y erradicados desde la base del diseño.

## 2. Regla de Posesión y Disponibilidad
**"Offline First y Local First no son funciones; son la base estructural."**
* La experiencia *Offline* debe permitir lectura, búsqueda, anotación, organización y exportación sin conexión a Internet.
* La información pertenece al dispositivo del usuario (*Local First*). No existe cuenta ni servidor propio del que dependa nada.
* Internet se utiliza únicamente para metadatos, portadas, catálogos abiertos, descarga de modelos y voces, consulta de diccionarios y traducción, IA externa opcional y comprobación de actualizaciones, siempre con consentimiento y a través de `net.rs`.

## 3. Reglas de Seguridad y Privacidad
* **Privacy by Design:** Queda estrictamente prohibido subir libros, enviar telemetría invasiva o analizar documentos en la nube sin consentimiento explícito del usuario.
* **Secure by Default:** Las configuraciones iniciales deben ser siempre seguras sin requerir que el usuario comprenda ingeniería de seguridad.
* **Least Privilege (Principio de Menor Privilegio):** Cada componente solo tendrá acceso a las carpetas, redes o recursos del sistema operativo estrictamente necesarios para su funcionamiento.

## 4. Regla de la Biblioteca Personal
* La biblioteca, las anotaciones, el progreso y las estadísticas son de la persona y no salen del equipo, salvo
  hacia los teléfonos que la propia persona empareja con él, por su red local y mientras no los revoque
  ([ADR-016](../adr/016-mobile-companion.md)).
* Ningún tercero (institución, empleador, proveedor) recibe, bajo ninguna circunstancia, acceso a ellos a través de esta aplicación.

## 5. Regla de Documentación como Producto
**"Si una decisión importante solo vive en la cabeza de una persona, esa decisión todavía no forma parte del proyecto."**
* El código no se considera terminado si su impacto arquitectónico o de uso no está reflejado en esta carpeta (`docs/`).
* La documentación debe ser tratada con el mismo rigor que el código fuente, versionada en Git.

## 6. Regla de Evolución a Largo Plazo
* NB Books se diseña para durar **diez años o más**.
* La arquitectura no debe depender rígidamente de un proveedor Cloud (Vendor Lock-in), de un framework efímero o de un único formato.
* Las abstracciones deben primar sobre las implementaciones rígidas (Ej: *Reader* debe depender de un contrato, no directamente del motor de renderizado).

