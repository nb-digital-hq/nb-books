# Cumplimiento y protección de datos

> **Audiencia:** áreas jurídicas, de cumplimiento, protección de datos y seguridad de la información.
> **Estado:** Implementado. Describe el comportamiento verificable de la aplicación.

## 1. Qué datos trata la aplicación

| Categoría | Ejemplos | Dónde viven | ¿Salen del equipo? |
|---|---|---|---|
| Biblioteca | Rutas relativas, metadatos bibliográficos, portadas | Base SQLite local y caché | No |
| Actividad de lectura | Progreso, sesiones de lectura | Base SQLite local | No |
| Producción intelectual | Anotaciones, notas, tableros, vocabulario, tarjetas | Base SQLite local | Solo si la persona exporta |
| Preferencias | Tema, idioma, ajustes de lectura | Base SQLite local | No |
| Credenciales de servicios opcionales | Claves de proveedores de IA | Llavero del sistema | Solo hacia el proveedor que la persona configuró |
| Registros técnicos | Eventos del sistema, redactados | Carpeta de datos local | No |

La aplicación no tiene cuentas de usuario, no recoge telemetría y no dispone de ningún servidor del proyecto que
reciba datos. Ver la [declaración de privacidad](../audit/privacy.md) y el
[modelo de amenazas](../audit/threat-model.md).

## 2. Lo que la institución puede afirmar ante una auditoría

* La aplicación **no transmite** datos de estudiantes, docentes ni personal a la institución, al proyecto ni a
  terceros por iniciativa propia.
* Toda conexión saliente pasa por un único módulo y requiere el consentimiento expreso de la persona por servidor
  y propósito; el modo sin red la impide por completo. Esto lo hace cumplir una comprobación automática sobre el
  código en cada cambio.
* Los registros técnicos no contienen rutas, títulos, autores ni texto de los documentos.
* El código es público y auditable, y cada publicación está firmada y acompañada de su procedencia.

## 3. Roles en protección de datos

Al no transmitir datos, la aplicación por sí sola no convierte al proyecto en encargado del tratamiento. Cuando
una institución combina la aplicación con servicios de la Suite Institucional, los roles se definen en el contrato
correspondiente; el marco está en [legal/data-protection](../legal/data-protection.md).

## 4. Salud y seguridad en el trabajo, bienestar y analítica

La aplicación no mide ni reporta indicadores individuales de uso. La analítica institucional del ecosistema,
cuando se contrata, es **agregada y anónima**: solo se publican cifras de grupos que superan un umbral mínimo de
tamaño ($k$-anonimato), de modo que ningún indicador permite identificar a una persona. Ver
[integración institucional](integration.md).

## 5. Accesibilidad y derechos de autor

* Marco de accesibilidad: [legal/accessibility-compliance](../legal/accessibility-compliance.md).
* La aplicación no elude medidas técnicas de protección de obras: [legal/copyright-and-drm](../legal/copyright-and-drm.md).
