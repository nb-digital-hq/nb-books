# Roles y Responsabilidades

> **Estado:** Vigente. Hoy un solo mantenedor ejerce todos los roles; la tabla dice cómo se accede a cada uno.

## 1. Roles

| Rol | Responsabilidad | Cómo se accede |
|---|---|---|
| Contribuidor | Aporta código, documentación, traducciones o reportes | Abrir un PR o un issue |
| Revisor | Revisa PR en un área concreta sin poder de fusión | Invitación tras contribuciones sostenidas de calidad |
| Code Owner | Responsable de un área; su aprobación es obligatoria para fusionar en ella | Designación por los mantenedores |
| Mantenedor | Acceso de fusión, gestión de releases, decisiones estructurales | Designación por los mantenedores existentes |
| Responsable de seguridad | Recepción de reportes, coordinación de la respuesta | Designación explícita, con suplente |

## 2. Áreas y propiedad

Las áreas se declaran en un archivo de propietarios de código en el repositorio:

* Núcleo Rust y puente IPC
* Motores de formato y lector
* Almacenamiento y migraciones
* Interfaz y sistema de ventanas
* Subsistema de IA
* Seguridad y cadena de suministro
* Documentación
* Traducciones

Un área sin propietario declarado es un área sin responsable real de su calidad.

## 3. Responsabilidades de un mantenedor

* Responder a issues y PR en un plazo razonable, aunque sea para decir que no.
* Aplicar el código de conducta.
* Mantener verde la rama principal.
* No fusionar código propio sin revisión.
* Custodiar las credenciales de firma según [supply-chain.md](../../audit/supply-chain.md).

## 4. Incorporación y salida

* La incorporación es gradual: contribuidor, revisor, propietario de área, mantenedor.
* Un mantenedor inactivo durante un periodo prolongado pasa a emérito y se le retiran los accesos, sin que ello
  suponga demérito. La retirada de accesos no utilizados es higiene de seguridad, no una sanción.
* La salida de un mantenedor exige rotación de las credenciales a las que tuviera acceso.

## 5. Continuidad

El proyecto debe mantener documentado, y actualizado:

* Quién tiene acceso a cada credencial crítica.
* Quién puede publicar una release.
* Qué ocurre si el mantenedor principal no está disponible.
* Dónde están los respaldos de la infraestructura del proyecto.

Sin esto, la promesa de durar diez años depende de la disponibilidad de una sola persona.
