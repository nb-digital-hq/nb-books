# Protección de datos personales

> **Estado:** Aplicable.
> **Aviso:** no constituye asesoría legal.

## 1. El punto de partida

NB Books es local-first y no tiene telemetría: **el proyecto no recibe datos personales** de quienes usan la
aplicación. No hay cuentas, ni servidores del proyecto que almacenen bibliotecas, anotaciones o actividad, ni
analítica de uso. Es una posición que se preserva a propósito.

## 2. Roles

| Situación | Quién trata los datos |
|---|---|
| Uso de la aplicación en el propio equipo | La propia persona. Los datos viven en sus carpetas de usuario |
| Despliegue en equipos de una institución | La institución, como responsable de sus equipos y de las cuentas de sistema de sus usuarios. La aplicación no le envía nada |
| Un proveedor externo que la persona configura (IA, voz, traducción, catálogos) | Ese proveedor, bajo sus propias condiciones, para lo que la persona decidió enviarle |
| Servicios de la Suite Institucional contratados aparte | Se rigen por su propio contrato de encargo de tratamiento; esta aplicación no forma parte de ellos |

## 3. Marcos aplicables

El criterio es adoptar el más estricto como línea base, en lugar de cumplir varias normativas con implementaciones
distintas:

* **Colombia:** régimen general de protección de datos personales, con protección reforzada de los datos de menores
  de edad.
* **Unión Europea:** RGPD, con minimización, base legal explícita y derechos del interesado.
* **Estados Unidos:** normativa sobre registros educativos y sobre menores en línea.

## 4. Reglas de diseño

1. **Minimización.** Nada sale del equipo sin consentimiento por anfitrión y propósito, y cada propósito dice qué
   envía. Ver [privacy.md](../audit/privacy.md).
2. **Sin perfilado.** No existe ningún mecanismo de perfilado ni de publicidad.
3. **Registros redactados.** Los registros técnicos no contienen rutas, títulos, autores ni texto.
4. **Derechos operables.** Desde Ajustes, la persona exporta la biblioteca entera y borra por alcances (el
   historial de lectura, las anotaciones y notas, o la biblioteca entera); cada acción dice antes qué borra y qué
   no, y los archivos del usuario no se borran nunca. El historial de envíos a proveedores de IA también se borra.
5. **Proveedores externos declarados.** Si se activa un proveedor de IA externo, la aplicación muestra antes del
   primer envío qué sale y a quién, registra cada envío en `ai_sends` y permite borrar ese historial. En contextos
   con menores, la opción razonable por defecto es la inferencia local.
6. **Incidentes.** Procedimiento en [governance/security-policy.md](governance/security-policy.md).
