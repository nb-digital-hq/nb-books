# Contrato con NB Books Mobile

> **Estado:** Implementado. Protocolo `1`. Módulo: `src-tauri/src/movil/`. Decisión: [ADR-016](../adr/016-mobile-companion.md).
> Respuestas grabadas: `src-tauri/tests/fixtures/contrato-movil/`, comparadas por `tests/servidor_movil.rs`.

Este documento es el contrato entre NB Books y NB Books Mobile: qué escucha el PC, qué acepta y qué devuelve. Lo
que no está aquí no existe para el teléfono. Solo se admiten cambios **aditivos**: un campo nuevo, una ruta nueva o
un valor nuevo de una lista abierta. Quitar o cambiar el significado de algo es un ADR y el protocolo `2`.

## 1. Transporte

| Aspecto | Valor |
|---|---|
| Protocolo | HTTPS y WSS sobre HTTP/1.1, en la red local |
| Puertos | El primero libre entre `8831` y `8899`; NB Sound usa otro rango |
| Descubrimiento | mDNS `_nbbooks._tcp.local.`, con `version` y `servicio` en el registro TXT |
| Certificado | Autofirmado, ECDSA P-256, creado una vez en `data/movil/` con permisos `0600`; el teléfono fija su huella SHA-256 |
| Autenticación | `Authorization: Bearer <device_token>` en todo salvo `ping` y `pair` |
| Cuerpo máximo | 8 MiB por petición |
| Errores | `{"error": "<código>"}` con `400 peticion_invalida`, `401 token_invalido_o_expirado` o `no_autorizado`, `404 no_encontrado` o `no_existe`, `409 limite_alcanzado`, `416` en un tramo imposible y `500 fallo_interno` |

Con el servidor apagado, NB Books no escucha en ningún puerto. Se enciende en Ajustes, Teléfonos, o al emparejar.

## 2. Emparejamiento

El PC enseña un QR con este contenido, y debajo un código de 8 caracteres para teclearlo:

```json
{"host": "192.168.1.20", "puerto": 8831, "token": "<64 hex>", "version": 1,
 "tls": true, "tls_fingerprint": "<sha256 hex>", "servicio": "NB Books"}
```

El token vale **una sola vez** y cinco minutos; pedir otro invalida el anterior. El teléfono lo canjea:

| Petición | Respuesta |
|---|---|
| `POST /api/v1/pair` con `{"token", "nombre_dispositivo", "plataforma"}`; `token` admite también el código | `{"ok": true, "device_token", "dispositivo_id", "nombre"}` |

Del `device_token` el PC guarda solo su SHA-256. Caben 16 teléfonos; el decimoséptimo recibe `409`. Revocar uno en
Ajustes invalida su token y cierra sus conexiones en ese momento.

## 3. Rutas

| Ruta | Qué devuelve |
|---|---|
| `GET /api/v1/ping` | `{"ok", "servicio", "version_protocolo"}`. Sin autenticar |
| `GET /api/v1/seleccion` | La selección del teléfono: `todo`, `colecciones`, `autores`, `series`, `materias`, `ediciones` |
| `POST /api/v1/seleccion` | Sustituye la selección. `{"ok", "reiniciar"}`: con `reiniciar` cierto la selección se amplió y el teléfono pide el manifiesto desde `0` |
| `GET /api/v1/estanterias` | Lo que se puede elegir: colecciones, autores, series y materias con cuántas obras tienen, y `total` |
| `GET /api/v1/manifest?since=N&limit=M` | Una página de cambios de lo seleccionado (sección 4) |
| `GET /api/v1/catalogo?q=&desde=&limit=` | El catálogo entero, paginado y sin copiarlo, para la vista "Todo el catálogo"; cada ficha dice si está `seleccionada`. Hasta 200 por página |
| `GET /api/v1/buscar?q=&desde=&limit=` | La búsqueda de texto completo del PC sobre toda la biblioteca: `edicion`, `titulo`, `locator`, `pasaje`, `seleccionada`. Hasta 50 por página. Solo en directo: el teléfono no tiene el índice |
| `GET /api/v1/archivo/{huella}` | El archivo de una edición seleccionada, con `Range` de un tramo (`206`, `Content-Range`) para reanudar |
| `GET /api/v1/portada/{edicion}` | La miniatura de la portada de una edición seleccionada |
| `POST /api/v1/cambios` | Lo que sube el teléfono (sección 5) |
| `GET /api/v1/control` | El WebSocket de Leer en el PC (sección 6) |

Una edición que el teléfono no eligió no se sirve, aunque adivine su huella: el manifiesto, los archivos y las
portadas salen de la misma subconsulta de selección.

## 4. Manifiesto

Cada escritura en el PC deja una entrada en `registro_de_cambios` (migración V46) con una versión creciente. El
teléfono guarda la última versión que aplicó y pide desde ahí:

```json
{"cambios": [{"version": 41, "entidad": "edicion", "clave": "<id>", "borrado": false, "datos": {}}],
 "version_actual": 57, "siguiente": 41, "hay_mas": true}
```

| Entidad | Datos |
|---|---|
| `edicion` | Ficha: título, autoría, serie, idioma, editorial, fecha, sinopsis, materias, `portada` y `archivo` (`huella`, `bytes`, `tipo`, `disponible`) |
| `coleccion` | `id`, `nombre`, `tipo` y sus `ediciones` |
| `anotacion` | `id`, `edicion`, `tipo`, `texto`, `locator`, `trazo`, `creada_en`, `baja_en`, `sustituye_a` |
| `progreso` | La posición del PC: `edicion`, `locator`, `actualizado_en` |
| `sesion` | `id`, `edicion`, `inicio`, `fin`, `desde`, `hasta` |
| `vocabulario` | `id`, `edicion`, `lema`, `termino`, `proveedor`, `locator`, `creada_en` |
| `tarjeta` | `id`, `edicion`, `tipo`, `anverso`, `reverso`, `opciones` y su `base` de repaso espaciado |
| `repaso` | `id`, `tarjeta`, `calificacion`, `repasado_en` |
| `nota` | `id`, `titulo`, `cuerpo`, `revision`, `actualizada_en` |

Una lápida (`borrado: true`) no lleva `datos`. Las lápidas viajan aunque el teléfono nunca tuviera esa cosa:
ignorarla no le cuesta nada.

## 5. Lo que sube el teléfono

`POST /api/v1/cambios` con hasta 2.000 elementos por lista. Cada tipo tiene **un dueño y un sentido**
(ADR-016, punto 3), así que no existe el conflicto:

| Lista | Regla |
|---|---|
| `anotaciones` | Se suman por `id`; llegar dos veces no duplica. Una anotación es inmutable: editar es una nueva con `sustituye_a` |
| `posiciones` | Van a `progreso_de_dispositivos`, una por teléfono y edición, y nunca retroceden por `actualizado_en`. No tocan la del PC |
| `sesiones` | Se suman por `id` |
| `vocabulario` | Se suma por `id` |
| `repasos` | Se suman por `id` en `registro_de_repasos`, y la tarjeta se recalcula desde su base con SM-2: el mismo resultado en los dos lados |
| `notas` | Llevan la `revision_vista`. Si la nota cambió en el PC después, el cambio entra como nota nueva y se devuelve en `notas_copiadas` |

La respuesta es `{"ok": true, "resultado": {"rechazados": [...], "notas_copiadas": [[vieja, nueva]]}}`. Un
elemento mal formado se rechaza por su `id` sin tumbar el resto.

## 6. Leer en el PC

`GET /api/v1/control` abre un WebSocket. Al conectar, el PC envía dónde está su lector, y lo repite cada vez que
cambia:

```json
{"tipo": "estado", "estado": {"edicion", "titulo", "locator", "progresion", "capitulo",
 "capituloNumero", "capitulosTotales", "paginado", "avance", "voz", "fondoSonoro", "modo", "inmersivo"}}
```

`voz` es `parada`, `hablando` o `pausada`; `fondoSonoro` dice si suena uno; `modo` es el modo de lectura y
`inmersivo`, si la carcasa está retirada. La frase donde va el PC es la cita (`text.highlight`) de `locator`.

El teléfono envía órdenes, y cada una recibe `{"tipo": "ack", "aceptada": true|false}`:

| Orden | Efecto en el PC |
|---|---|
| `{"tipo": "comando", "id"}` | Un comando del registro del lector, por el mismo camino que su tecla. Admitidos: `pagina-siguiente`, `pagina-anterior`, `bloque-siguiente`, `bloque-anterior`, `marcador`, `voz`, `modo-de-lectura`, `fondo-sonoro`, `inmersion`, `pantalla-completa`, `acercar`, `alejar`, `letra-mayor`, `letra-menor` (cambian el cuerpo de la letra en un texto que fluye y el zoom en una página que se pinta como imagen), `capitulo-siguiente`, `capitulo-anterior`, `avance-automatico`, `salir` |
| `{"tipo": "abrir", "edicion"}` | Abre la edición en el lector |
| `{"tipo": "irA", "progresion"}` | Va al capítulo que contiene esa progresión, de 0 a 1 |
| `{"tipo": "irAlLocator", "locator"}` | Va a un Locator exacto |
| `{"tipo": "desplazar", "dy"}` | Desplaza en continuo, como mucho 4.000 píxeles; en paginado pasa página |
| `{"tipo": "avance", "velocidad"}` | Avance automático de 0 a 10; `0` lo detiene |

Tres comandos no hacen desde el teléfono lo mismo que su tecla, porque la tecla abre un panel que el teléfono no
ve: `voz` empieza, pausa o reanuda la lectura en voz alta desde lo visible; `fondo-sonoro` para el fondo que suena
o vuelve a poner el último; `modo-de-lectura` pasa al siguiente modo.

Con dos teléfonos, manda el último que dio una orden. El PC enseña "Controlado desde" con el nombre del teléfono y
un botón para cortar; cortar o revocar cierra las conexiones de control. Las órdenes nunca van en el otro sentido:
el PC no maneja el teléfono.

## 7. Cómo se comprueba

| Qué | Dónde |
|---|---|
| Emparejamiento de un solo uso, selección, tramos, subida, revocación, búsqueda y control | `src-tauri/tests/servidor_movil.rs`, contra el servidor real por TLS |
| Que las respuestas sean las grabadas | `las_respuestas_del_contrato_son_las_grabadas`; se regraban con `NB_GRABAR=1` y el mismo cambio se lleva a NB Books Mobile |
| El registro de cambios | `src-tauri/tests/registro_de_cambios.rs` |
| Que nada escuche fuera de `movil/` | `scripts/check-network.mjs` |
