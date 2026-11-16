# Política de Seguridad y Respuesta a Incidentes

> **Estado:** Aplicable. Su resumen operativo, en inglés, es `SECURITY.md` en la raíz del repositorio.

## 1. Reporte de vulnerabilidades

* **No se reportan vulnerabilidades como issues públicos.** Un issue público es una divulgación completa antes de
  que exista corrección.
* El canal privado es el correo **nbdigitalsas@gmail.com**, con un asunto que empiece por
  `[NB Books][Seguridad]`; un mensaje sin ese prefijo es muy probable que no se lea. El reporte privado de GitHub
  del repositorio público también se admite.
* Se acusa recibo en un plazo de siete días.
* Se mantiene informado a quien reporta durante el análisis y la corrección.

## 2. Divulgación coordinada

El proyecto adopta divulgación coordinada:

1. Recepción y confirmación del reporte.
2. Evaluación de impacto y asignación de severidad.
3. Desarrollo de la corrección en privado.
4. Publicación de la versión corregida.
5. Publicación del aviso de seguridad, con crédito a quien reportó si lo desea.

Quien reporta puede divulgar públicamente a los 90 días del acuse de recibo, haya o no corrección. Sin ese plazo,
el proyecto podría retener indefinidamente un problema conocido.

## 3. Severidad y respuesta

| Severidad | Ejemplo | Objetivo de corrección |
|---|---|---|
| Crítica | Ejecución de código a partir de un documento importado; compromiso de la clave de firma | Inmediata, con release fuera de ciclo |
| Alta | Exfiltración de datos por una extensión; elusión del modelo de permisos | Release prioritaria |
| Media | Fuga de información en logs o diagnósticos | Siguiente release planificada |
| Baja | Endurecimiento defensivo | Según planificación |

## 4. Avisos de seguridad

Cada corrección de seguridad publicada incluye: componente afectado, versiones afectadas, impacto, mitigación
temporal si existe y versión que corrige. Las instituciones necesitan esta información para sus propios procesos
de gestión de vulnerabilidades; ocultarla es un obstáculo para la adopción.

## 5. Incidentes con datos personales

Si un incidente implica acceso no autorizado a datos personales:

1. Contención y evaluación del alcance.
2. Notificación a quien corresponda dentro de los plazos que exija la normativa aplicable.
3. Comunicación a los usuarios afectados cuando corresponda.
4. Informe posterior con causa raíz y medidas correctivas.

Ver [legal/data-protection.md](../data-protection.md).

## 6. Compromiso de la cadena de suministro

Si se sospecha compromiso de una clave de firma o del actualizador:

1. Revocación inmediata de la credencial.
2. Retirada de los artefactos afectados del canal de actualización.
3. Aviso público inmediato, incluso antes de tener el análisis completo.
4. Reemisión de credenciales y republicación firmada.

La clave pública de las publicaciones viaja compilada en la aplicación, así que sustituirla exige publicar una
versión nueva firmada con la clave anterior que incorpore la nueva, o pedir a quien la instaló que descargue la
nueva versión a mano y la verifique contra la clave publicada en el repositorio.

## 7. Alcance

Están en alcance: la aplicación de escritorio, su actualizador, el registro de extensiones y la infraestructura de
compilación y distribución.

Está fuera de alcance: el software de terceros del sistema operativo, las extensiones de terceros no publicadas
en el registro oficial y los despliegues modificados por el usuario.
