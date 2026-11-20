# ADR-010: Persistencia de las Preferencias de Usuario

**Fecha:** 2026-08-25
**Decidido:** 2026-08-26
**Estado:** Aceptado

## Contexto

Las preferencias del usuario (tema, fondo, transparencia, filtro de lectura, escala tipográfica, alto contraste,
movimiento reducido, velocidad de lectura en voz alta, proveedor de IA y límite de memoria asignado) se
almacenan actualmente en el almacenamiento local del webview.

Esto contradice una regla ya escrita en el manual de ingeniería del proyecto: todo estado que deba sobrevivir a
un reinicio se persiste en SQLite. Además tiene consecuencias prácticas:

* Se pierden al limpiar los datos del webview o al cambiar la versión del motor.
* No entran en el respaldo ni en la exportación de datos del usuario.
* No pueden sincronizarse entre dispositivos.
* Las preferencias de accesibilidad, que son las que más importa preservar, son las más expuestas a perderse.

## Alternativas Consideradas

1. **Mantener el almacenamiento del webview.** Sin coste inmediato, con la deuda intacta.
2. **Migrar todo a SQLite.** Coherente con la arquitectura declarada, respaldable y sincronizable. Requiere
   lectura al arrancar antes de pintar la interfaz.
3. **Modelo mixto:** SQLite como fuente de verdad y una copia en el almacenamiento del webview como caché de
   arranque, para evitar el parpadeo del tema al iniciar.

## Decisión

Se adopta la **alternativa 3**: SQLite como única fuente de verdad, con una caché de solo lectura en el
almacenamiento del webview usada exclusivamente para pintar la primera pantalla sin parpadeo. Ante discrepancia,
SQLite gana y la caché se reescribe.

### Por qué no la alternativa 2, que sería más limpia

La alternativa 2 (solo SQLite) es conceptualmente más pura y tiene una única fuente de verdad, sin riesgo de
divergencia. Se descarta por una razón concreta del entorno: **el webview se pinta antes de que el runtime
responda**. Leer las preferencias por el puente IPC antes del primer pintado obliga a elegir entre dos males:

* Bloquear el montaje de la interfaz hasta que llegue la respuesta, lo que suma latencia al tiempo hasta que la
  aplicación es interactiva, contra un presupuesto que ya está declarado.
* Pintar con valores por defecto y corregir después, es decir, un destello de tema claro para quien usa tema
  oscuro, o de tipografía pequeña para quien usa escala aumentada.

El segundo mal no es cosmético. Las preferencias que más importa preservar son precisamente las de
accesibilidad, y un destello de alto contraste desactivado, de movimiento no reducido o de tipografía sin
escalar es una barrera real para quien depende de ellas.

La lectura del almacenamiento del webview es síncrona e inmediata, y elimina ambos males.

### Alcance de la caché: deliberadamente estrecho

La caché **no es un espejo de las preferencias**. Contiene únicamente lo que afecta al primer pintado:

| En la caché | Fuera de la caché |
|---|---|
| Tema | Proveedor de IA y su presupuesto de memoria |
| Alto contraste | Modo de almacenamiento de la biblioteca |
| Movimiento reducido | Velocidad de lectura en voz alta |
| Escala tipográfica | Filtro del lector |
| Idioma | Cualquier otra preferencia |

Acotarla así reduce la superficie de divergencia a cinco valores en lugar de a toda la configuración, y hace que
el riesgo residual sea proporcional al problema que resuelve. Todo lo demás se lee de SQLite cuando la interfaz
ya está en pantalla, donde la latencia del puente es irrelevante.

### Reglas de coherencia

1. Escribir una preferencia escribe **primero** en SQLite; la caché se actualiza después.
2. Al arrancar, la caché pinta; cuando llega el valor de SQLite, si difiere, gana SQLite y la caché se reescribe.
3. La caché ausente o corrupta no es un error: se pinta con los valores por defecto y se reconstruye.
4. Las preferencias que contienen secretos (claves de API) quedan fuera de ambos sitios: viven en el almacén de
   credenciales del sistema operativo.

## Consecuencias

* **Positivas:** las preferencias sobreviven a la limpieza del webview; entran en el respaldo y la exportación;
  quedan disponibles para sincronización futura; se elimina una contradicción entre la documentación y el código.
* **Negativas:** requiere migración de los valores existentes de los usuarios actuales, un esquema de
  preferencias versionado y cuidado en el orden de arranque.
* **Riesgos:** una migración descuidada perdería las preferencias de accesibilidad del usuario, que es
  precisamente el caso más sensible. La migración debe conservar los valores previos y ser idempotente. Una
  prueba debe demostrarlo antes de dar la iteración por terminada.
