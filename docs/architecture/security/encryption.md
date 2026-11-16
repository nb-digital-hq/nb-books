# Cifrado

> **Estado:** Implementado. La política de privacidad vive en [privacy.md](../../audit/privacy.md).

## 1. Datos en reposo

NB Books **no cifra a nivel de aplicación** los documentos ni la base SQLite: confía en el cifrado de disco del
sistema operativo (LUKS, FileVault, BitLocker). Se declara de forma explícita, en `SECURITY.md` y aquí, porque una
persona que asume que su biblioteca está cifrada cuando no lo está sufre un fallo de honestidad del producto.

Motivos:

* Los documentos son archivos del usuario en sus propias carpetas, referenciados y no copiados; cifrarlos exigiría
  apropiarse de ellos.
* Un cifrado propio de la base dejaría igualmente texto claro en la memoria del proceso mientras un documento está
  abierto, y la memoria de intercambio y la hibernación quedan fuera del control de la aplicación.
* El cifrado de disco protege a la vez la base, la caché, los registros y los documentos.

## 2. Datos en tránsito

Toda salida pasa por `src-tauri/src/net.rs`:

* **TLS** por defecto, con el almacén de confianza del sistema. Se respeta así la inspección TLS legítima de redes
  institucionales con una CA corporativa instalada.
* **No se fijan certificados** de terceros: su rotación está fuera de control y dejaría la aplicación inutilizable.
* Un catálogo OPDS que solo se sirve en claro exige **su propio consentimiento**, que dice exactamente lo que
  implica: quien está en la red ve qué se busca y qué se descarga. Tener permiso para un anfitrión por `https` no
  da permiso para hablarle en claro.

## 3. Firma

| Uso | Algoritmo | Dónde |
|---|---|---|
| Publicaciones de la aplicación | Ed25519 sobre `SHA256SUMS` | `src-tauri/src/artefactos.rs`, [releases](../../developer/deployment/releases.md) |
| Paquetes de extensión | Ed25519 sobre la huella del paquete | `src-tauri/src/extensions/signature.rs` |
| Huellas de modelos y voces | SHA-256 compilado en el binario | `src-tauri/src/ai/catalogo.rs`, `src-tauri/src/voice/catalogo.rs` |

La verificación usa `ring`. La clave pública de las publicaciones viaja compilada en el binario y publicada en
`docs/architecture/api/clave-de-publicacion.pem`; nunca al lado de lo que firma.

## 4. Gestión de claves

* Las credenciales de servicios viven en el almacén del sistema: [authentication.md](authentication.md).
* Las claves privadas de firma de publicaciones y de extensiones propias **nunca** entran en el repositorio.
