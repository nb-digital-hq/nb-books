# Seguridad

> **Estado:** Implementado.

NB Books opera bajo confianza cero local: aunque se ejecute en la máquina del usuario, asume que los documentos
importados, los catálogos remotos y las extensiones pueden ser hostiles, y que el equipo puede estar compartido.

| Documento | Contenido |
|---|---|
| [Modelo de amenazas](../../audit/threat-model.md) | Activos, actores, superficie de ataque y límites declarados |
| [Credenciales](authentication.md) | Almacén de credenciales del sistema |
| [Autorización](authorization.md) | Capacidades del webview y permisos de extensiones |
| [Cifrado](encryption.md) | Datos en reposo y en tránsito, y firmas |
| [Privacidad](../../audit/privacy.md) | Inventario de datos, consentimiento y derechos |
| [Cadena de suministro](../../audit/supply-chain.md) | Firma, dependencias y procedencia de los artefactos |

Las vulnerabilidades se reportan en privado según `SECURITY.md`, en la raíz del repositorio.
