# Chaos Testing (Resiliencia Automatizada)

> **Estado:** Implementado. Suite: `src-tauri/tests/caos.rs`, `caos_pdf.rs` y las pruebas de navegador de `e2e/`.
> La Constitución (regla 1) manda la degradación elegante. El Chaos Testing es la disciplina de inyectar fallas
> intencionales en CI para validar que el sistema no colapsa, sino que se degrada correctamente.

Sin estas pruebas, "Graceful Degradation" es una aspiración escrita, no una propiedad verificada del sistema.

## 1. Escenarios de caos obligatorios

1. **Muerte del motor PDF:** un test mata abruptamente el hilo o simula un `panic!` en PDFium. El frontend debe
   mostrar un estado de error manejado ("El motor de renderizado falló") en lugar de quedarse en blanco.
2. **I/O lento (latencia de disco):** se inyectan retardos en las operaciones de lectura de SQLite. La UI debe
   mostrar estados de carga y no bloquearse, validando que el IPC es verdaderamente asíncrono.
3. **Base de datos corrupta:** se corrompen intencionalmente bytes del archivo SQLite antes del arranque. Rust
   debe detectar la corrupción, aislar el archivo como `.corrupted`, crear una base nueva y avisar al usuario
   sin perder el archivo original.
4. **Base de datos bloqueada:** dos procesos escriben simultáneamente. No debe producirse `database is locked`
   visible para el usuario.
5. **Descarga interrumpida:** se corta la red a mitad de la descarga de un modelo. No queda nada que parezca
   instalado y el siguiente intento empieza limpio.
6. **Biblioteca referenciada desconectada:** se desmonta el volumen donde viven los archivos referenciados. Los
   documentos deben aparecer en estado `NOT_FOUND` conservando metadatos y anotaciones.
7. **Ausencia de `libpdfium`:** el arranque debe degradar a "sin soporte PDF", no fallar.
8. **Servicio de IA que no puede ejecutarse:** el motor no arranca en esta máquina. El componente debe reportar
   `NOT_SUPPORTED` y la UI ocultar las funciones dependientes.
   Un equipo justo de memoria no esconde nada: ofrece el modelo con lo que es probable que pase y lo confirma
   antes si es probable que el sistema cierre la aplicación. `NOT_SUPPORTED` queda para lo que no se puede ejecutar
   de ninguna manera. `DEGRADED` corresponde a un fallo **durante** la inferencia, que se puede reintentar.
9. **Disco lleno durante importación:** error controlado y reversión de la transacción.
10. **Apagado abrupto durante una migración:** al reabrir, la base debe estar en el estado previo consistente.
11. **Extensión que no devuelve el control:** un módulo en bucle infinito. El anfitrión lo interrumpe en su plazo
    y sigue sirviendo; el subsistema **no** cambia de estado, porque contener a una extensión es el sistema
    funcionando y no una avería.
12. **Extensión que pide memoria sin freno:** se corta en su límite, y el mismo motor ejecuta otra cosa justo
    después.
13. **Perfil de extensiones ilegible al arrancar:** se recupera desde la última generación archivada, así que no
    se pierde lo instalado. Sin histórico del que tirar, se arranca vacío: es lo único que queda, y es mejor que
    no arrancar.

## 1.1 Dónde está cada uno

Esta tabla es la respuesta a "¿está cubierto el escenario N?", y existe para que esa pregunta no se conteste
leyendo código. Un escenario sin prueba se dice; no se deja adivinar.

| # | Escenario | Dónde | Estado |
|---|---|---|---|
| 1 | Muerte del motor de formato | `caos.rs::motor_que_se_cae` | Cubierto |
| 2 | I/O lento | `e2e/flujo.spec.ts`, las dos pruebas de "runtime lento" | Cubierto |
| 3 | Base de datos corrupta | `caos.rs::caos_base_de_datos_corrupta` | Cubierto |
| 4 | Base de datos bloqueada | `caos.rs::caos_base_de_datos_bloqueada` | Cubierto |
| 5 | Descarga interrumpida | `descarga_de_modelos.rs::una_descarga_a_medias_de_otra_sesion_no_cuenta_y_se_limpia_al_reintentar` | Cubierto |
| 6 | Biblioteca referenciada desconectada | `almacenamiento.rs::desconectar_el_volumen_deja_el_documento_visible_y_no_disponible` | Cubierto |
| 7 | Ausencia de `libpdfium` | `caos_pdf.rs`, su propio binario de pruebas | Cubierto |
| 8 | Servicio de IA que no puede ejecutarse | `caos.rs::caos_un_motor_de_ia_que_no_puede_ejecutarse_se_oculta_y_no_tumba_al_resto` | Cubierto |
| 8b | Servicio de IA apagado del todo | `caos.rs::caos_el_servicio_de_ia_apagado_no_degrada_ninguna_otra_parte` | Cubierto. Es el criterio de aceptación: con el subsistema apagado se recorre catálogo, lectura, indexación, búsqueda y anotación, y todo tiene que dar lo mismo |
| 8c | Proveedor de IA que entra en pánico | `caos.rs::caos_un_proveedor_de_ia_que_se_cae_no_se_lleva_al_resto` | Cubierto. Un motor de inferencia es código nativo de terceros por FFI: cuando se cae, se cae dentro de nuestro proceso. `ai::AiService::sin_tumbar` lo recoge, igual que `reader::session` |
| 9 | Disco lleno durante importación | `caos.rs::disco_lleno` | Cubierto |
| 10 | Apagado abrupto durante una migración | `caos.rs::apagado_durante_la_migracion` | Cubierto |

**Cómo se simula cada uno importa tanto como que exista la prueba.** Tres merecen decirse:

* El **motor que se cae** no se simula con un error devuelto: se provoca un pánico de verdad dentro del
  proveedor. El perfil de publicación evita `panic = "abort"` a propósito, así que desenrolla; pero desenrollar
  no basta, porque sin nadie que lo recoja el pánico sube por el comando del puente y quien lo pidió espera una
  respuesta que no llega. `ReadingSession` lo recoge, devuelve `COMPONENT_UNAVAILABLE` y deja el componente en
  `Failed` con motivo `ENGINE_CRASH`.
* El **disco lleno** no llena un disco —haría falta un sistema de archivos propio y permisos de
  administrador—: produce el mismo fallo por el mismo camino, quitando permiso de escritura a la biblioteca.
  Para `import_document` es indistinguible, porque lo que ve es un error de entrada y salida al copiar.
* El **apagado durante una migración** mata un proceso de verdad con `SIGKILL`, que no se puede atrapar, no
  corre destructores y no vacía búferes: es la luz yéndose. Se mata a treinta instantes distintos, del
  milisegundo cero al veintinueve, y la propiedad tiene que valer en los treinta. Elegir un solo momento sería
  elegir uno afortunado.

## 1.2 El banco de extensiones adversas

Vive en `extensiones/banco/` y lo comprueba `src-tauri/tests/banco_de_extensiones.rs`. Son módulos WebAssembly con
**una sola** conducta hostil declarada cada uno —leer un archivo, abrir una conexión, pedir una capacidad que el
anfitrión no ofrece, no devolver el control, agotar memoria, declarar una API que no existe, traer un manifiesto
roto, y un par cuya segunda versión introduce carga remota de código donde la primera no la tenía— y otros que no
intentan nada (la versión benigna del par, un componente y un formato) para comprobar que el camino honesto
funciona. Alimenta los escenarios 11 a 13.

La contención tiene dos mecanismos y el banco cubre los dos: `abre-una-conexion` importa `sock_accept`, que el
anfitrión implementa sin recurso, y `pide-lo-que-no-existe` importa algo que no ofrece en absoluto y se detiene
antes de instanciar.

**La prueba abre los binarios.** Cada ficha de `banco.json` declara qué función del anfitrión abusa el módulo, y la
prueba lee la sección de importaciones del `.wasm` para comprobar que esa función está de verdad. Un módulo que
dijera leer archivos sin importar `path_open` no intentaría nada, y la suite seguiría en verde aunque el aislamiento
se rompiera. Quemar procesador y agotar memoria no llaman al anfitrión: se prueban ejecutándolas contra los
límites.

## 2. Criterio de aprobación

Para cada escenario:

* La aplicación sigue ejecutándose.
* El componente afectado reporta un estado explícito de la máquina de estados definida en
  [component-model.md](../../architecture/component-model.md).
* No se pierde ningún dato del usuario.
* El mensaje mostrado es comprensible y accionable, sin jerga interna.

## 3. Frecuencia

La suite entera corre en cada `cargo test` y en el flujo de integración continua `.github/workflows/ci.yml`.
