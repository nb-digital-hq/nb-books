# ADR-016: NB Books Mobile y el servidor local del PC

**Fecha:** 2026-11-19
**Estado:** Aceptado

## Contexto

Se lee también en el teléfono. NB Books Mobile, el lector de la suite para Android e iOS, necesita los libros de
la biblioteca y, sobre todo, lo que la persona hace al leer: posición, anotaciones, notas, vocabulario y repasos.
La Constitución dice que la biblioteca y las anotaciones son de la persona y no salen del equipo, y la única
salida de red es `net.rs`. Además, una posición EPUB es un desplazamiento sobre el texto que extrae el propio
motor de lectura ([ADR-004](004-reader-locator-model.md)): otro motor en el teléfono señalaría otro sitio.

## Alternativas consideradas

1. **Una nube o un servidor de NB Digital entre los dos.** Contradice la regla 2 de la Constitución: dependería de
   una cuenta y de un servidor ajeno a la persona.
2. **Copiar la biblioteca por un medio externo (carpeta compartida, USB).** Sin sincronización de lo que se hace al
   leer, que es lo que importa.
3. **Sincronización bidireccional con "gana el más reciente".** Depende de los relojes y pierde en silencio la
   versión que no gana.
4. **Un servidor local en el PC, emparejado explícitamente, con un dueño y un sentido por cada tipo de dato, y el
   mismo motor de lectura en los dos dispositivos.**

## Decisión

Alternativa 4:

1. **Servidor local y emparejamiento.** El PC atiende en la red local por HTTPS y WSS con un certificado autofirmado
   que no cambia (ECDSA P-256). El teléfono se empareja escaneando un QR con un token de un solo uso y cinco minutos
   de vigencia, o con la dirección y un código de 8 caracteres; recibe un `device_token` que guarda en su llavero y
   fija la huella del certificado. Se anuncia por mDNS como `_nbbooks._tcp.local.`. Los teléfonos se ven y se
   revocan en Ajustes, Teléfonos, y revocar cierra sus conexiones al momento. Con el servidor apagado, NB Books
   no escucha en ningún puerto.
2. **Lo entrante vive en un solo módulo.** Todo lo que escucha está en `src-tauri/src/movil/`, igual que todo lo que
   sale está en `net.rs`; `scripts/check-network.mjs` lo hace cumplir. El servidor no abre conexiones hacia fuera.
3. **Un dueño por tipo de dato.** El catálogo, los archivos, las tarjetas generadas y las notas son del PC. La
   posición se guarda por dispositivo y ninguna pisa a la otra. Anotaciones, marcadores, trazos, sesiones y
   vocabulario son registros que se suman: las anotaciones ya son inmutables. El repaso espaciado viaja como
   registro de repasos y cada lado recalcula SM-2 con el mismo resultado. Un cambio de nota del teléfono lleva la
   revisión que vio; si la nota cambió en el PC, entra como nota nueva. Nada se sobrescribe desde el otro lado.
4. **Un solo motor.** El motor de lectura es el crate `src-tauri/lector/` (`nb-books-lector`), que NB Books Mobile
   recibe como exportación sellada. Un corpus de conformidad generado fija el texto y los locators que los dos
   tienen que producir.
5. **Leer en el PC.** Un teléfono emparejado puede manejar el lector del PC en vivo. Cada orden es un comando del
   registro del lector y se ejecuta por el mismo camino que el teclado; el PC enseña desde qué teléfono se le
   controla y se puede cortar en el momento.
6. **Contrato estable.** El protocolo es la versión `1`, documentado en
   [contrato móvil](../architecture/mobile-contract.md); sus respuestas están grabadas y `cargo test` las compara.
   Solo se admiten cambios aditivos; uno incompatible es un ADR y una versión `2`.

## Consecuencias

* **Positivas:** se lee en el teléfono sin cuenta ni nube; la anotación hecha en un dispositivo cae en el mismo
  sitio en el otro; no existe la resolución de conflictos porque no existe el conflicto.
* **Negativas:** NB Books escucha en la red local mientras hay teléfonos emparejados, lo que amplía la superficie
  de ataque ([modelo de amenazas](../audit/threat-model.md)); el motor de lectura pasa a ser un contrato compartido
  que no se puede cambiar sin pensar en el teléfono.
* **Riesgos y cómo se vigilan:** un teléfono perdido sigue emparejado hasta que se revoca, y la revocación es
  inmediata; un cambio del motor que mueva posiciones lo detecta el corpus de conformidad; un cambio del contrato
  que no sea aditivo lo detectan las respuestas grabadas.
