# Credenciales

> **Estado:** Implementado en `src-tauri/src/secrets.rs`.

NB Books no tiene inicio de sesión: es local-first y ninguna función exige una cuenta. Lo que sí guarda son las
credenciales de los servicios opcionales que la persona configura, como un servidor de voz o un proveedor de IA
externo. Este documento cubre dónde viven. El control de **qué puede hacer** cada parte del sistema está en
[authorization.md](authorization.md).

## 1. Almacén de credenciales del sistema

**Nunca** se guardan claves de API ni tokens en texto plano en SQLite, en `localStorage` ni en archivos de
configuración. NB Books usa la biblioteca de Rust `keyring` para acceder al almacén del sistema:

| Sistema | Almacén |
|---|---|
| Linux | Secret Service API (GNOME Keyring o KWallet) |
| macOS | Keychain |
| Windows | Credential Locker |

## 2. Propiedades

* **Sin almacén no hay campo.** `secrets::disponible` lo consulta sin crear ninguna entrada. Si no hay Secret
  Service (sesiones mínimas, algunos contenedores), la interfaz **no ofrece el campo** donde se escribiría la
  clave, dice por qué y deja usable todo lo que no la necesita. No existe recurso a texto plano.
* **La cuenta lleva el destinatario dentro** (por ejemplo `voz@<anfitrión>`). Cambiar de servicio no entrega al
  nuevo la credencial del anterior, porque se busca bajo otro nombre. `src-tauri/tests/llavero.rs` lo verifica
  contra el llavero real y falla si se quita el anfitrión de la cuenta.
* **El secreto no cruza el puente.** `CredentialStatus` dice si cabe guardarlo y si existe; nunca cuál es.
* **Ningún secreto llega a los registros**, ni siquiera truncado, y el paquete de diagnóstico no incluye material
  criptográfico.
