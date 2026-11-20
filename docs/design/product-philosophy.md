# Filosofía de Producto (Product Philosophy)

> **Estado:** Vigente. Criterio de referencia para toda decisión de producto.

> **"NB Books no pretende añadir más piezas al ecosistema digital de lectura y aprendizaje. Pretende eliminar las costuras entre las piezas que ya deberían haber funcionado juntas."**

Este documento establece los cimientos ideológicos y de diseño de NB Books. Ninguna decisión técnica, arquitectónica o de interfaz debe contradecir los principios aquí expuestos. Si una funcionalidad rompe la filosofía del producto, debe replantearse.

---

## 1. El Problema a Resolver

La lectura digital, la investigación y el aprendizaje sufren de una severa **fragmentación de herramientas**. Hoy en día, un usuario necesita de un gestor de bibliotecas, un lector de EPUB, un lector de PDF, un navegador para diccionarios y traductores, aplicaciones para tomar notas o crear flashcards, y clientes para interactuar con IA o bases de datos académicas.

* Cada aplicación puede resolver correctamente su función individual, pero **producen una experiencia deficiente cuando forman parte de un mismo flujo de trabajo**.
* Las instituciones (universidades, CRAIs) invierten enormemente en recursos digitales, pero gran parte del valor se pierde por la **fricción entre el recurso y el momento real de uso**.

**Conclusión:** El problema no es la falta de funcionalidades; es que las funcionalidades no están integradas alrededor del objetivo real del usuario: adquirir y gestionar conocimiento.

---

## 2. La Visión de NB Books

NB Books no es "otro lector de EPUB", "un Calibre moderno", o "un NotebookLM de escritorio". Su objetivo es transversal:

* **Visión Inicial:** Construir el sistema operativo para bibliotecas digitales.
* **Visión a Largo Plazo:** Construir el entorno operativo para trabajar con conocimiento digital.

---

## 3. El Cambio de Perspectiva

NB Books no se pregunta: *"¿Qué funcionalidad debemos agregar?"*
Se pregunta: *"¿Qué intenta lograr realmente el usuario?"*

* Un usuario no quiere un traductor; **quiere comprender un texto** en otro idioma sin perder contexto.
* Un usuario no quiere un lector EPUB; **quiere leer, comprender, recordar e investigar**.
* Un estudiante no quiere una plataforma adicional; **quiere encontrar los materiales de su curso** y estudiarlos sin saltar entre sistemas.
* Un profesor no quiere otro gestor documental; **quiere proporcionar materiales, orientar la lectura y contextualizar información**.

---

## 4. Principios de Experiencia de Usuario (UX) y Diseño

Para garantizar la coherencia de NB Books, todo el equipo de producto e ingeniería debe adherirse a los siguientes postulados:

1. **No construir software. Resolver problemas:** La meta es el conocimiento, no la tecnología.
2. **Competir por experiencia, no por funcionalidades:** Las funcionalidades son herramientas; la experiencia es el producto real.
3. **Innovar no es inventar, es replantear:** Cuestionamos las decisiones heredadas del software de lectura tradicional y reconstruimos la experiencia.
4. **Copiar principios, no funcionalidades:** Entendemos qué problemas resolvieron herramientas como Calibre, Foliate o COSMIC, pero los rediseñamos para nuestro ecosistema.
5. **Todo debe sentirse parte del mismo producto:** Rechazamos las interfaces que parecen un "frankenstein" de módulos inconexos.
6. **Eliminar fricción vale más que agregar funciones:** Cada clic, ventana y configuración debe justificar implacablemente su existencia.
7. **Lo complejo es interno, lo sencillo es externo:** La arquitectura puede y debe ser sofisticada, pero la interfaz debe sentirse completamente natural.
8. **Diseñar para el objetivo, no para el formato:** Archivos (`.epub`, `.pdf`, `.cbz`) son el medio. Extraer su conocimiento es el fin.
9. **La simplicidad exige ingeniería superior:** Lograr que una interfaz sea limpia requiere una arquitectura subyacente considerablemente más compleja.
10. **Evolucionar sin romper:** Las actualizaciones deben aumentar capacidades sin destruir los hábitos del usuario ni su confianza.
11. **Recomendar no es obligar:** Cuando la aplicación mide el equipo y sabe qué funcionará mejor, **ofrece todas las opciones**: una marcada como recomendada con su motivo, y en las demás lo que es probable que pase según los datos medidos. Nada que se pueda ejecutar se esconde por ser lento, de peor calidad o arriesgado; lo arriesgado se confirma antes. A veces el usuario no quiere lo usable. Ver [component-model.md](../architecture/component-model.md), sección 2.

---

## 5. Principios de Continuidad y Conocimiento

* **El conocimiento debe fluir:** Si el usuario debe abandonar la aplicación para continuar un proceso de lectura, traducción o investigación, hemos fallado.
* **El usuario siempre mantiene el control:** La IA sugiere, no reemplaza. Las automatizaciones pueden ignorarse, la IA puede apagarse, y los datos siempre se pueden exportar de forma transparente.
* **El conocimiento tácito es deuda técnica:** Si una decisión importante solo vive en la cabeza de una persona, aún no forma parte del proyecto. La documentación *es* el producto.

## 6. Diferenciación de Audiencia

**Para quién es:** cualquier persona que lea en su computador: libros electrónicos, PDF, cómics y novelas
gráficas, sin pedir cuenta ni conexión. No hace falta ser estudiante ni investigador; las funciones de estudio están
para quien las quiera y no estorban a quien solo quiere leer.

NB Books respeta profundamente a su audiencia, entregando valor acorde a sus necesidades sin superponerlas:

* **Suite Comunidad:** La persona no debe sentirse obligada a pagar ni a pertenecer a una institución para tener una biblioteca personal de calidad superior. NB Books es completa sin ninguna edición de pago.
* **Ninguna institución** recibe nada de esta aplicación: no hay conectores, cuentas ni canales hacia terceros.

---

## 7. Principios de Estado Vivo y Autosuficiencia

> Salen de contrastar NB Books con el ecosistema existente -gestores y lectores de escritorio, lectores móviles,
> servidores autohospedados y rastreadores-. No son preferencias de implementación: son la diferencia entre una
> aplicación que hace muchas cosas y una que se siente **una sola cosa**.

1. **Reiniciar no es una forma de refrescar.** Ningún cambio -un ajuste, un metadato corregido, una extensión
   instalada, un libro importado, un tema aplicado- puede exigir cerrar y volver a abrir la aplicación para verse.
   La **única** excepción admisible es aplicar una actualización de la propia aplicación. Calibre exige el reinicio completo para que se reflejen cambios masivos
   de metadatos o de plugins; ese es exactamente el listón que aquí no se baja.

2. **Lo que se ve se actualiza solo, y en todas partes a la vez.** Si el usuario lee y avanza, el porcentaje cambia
   en el lector, en la biblioteca, en Inicio y en cualquier otro sitio donde ese número aparezca, **sin cerrar el
   lector y sin volver a entrar**. Un dato que se ve en dos pantallas es el mismo dato, no dos copias que alguien
   sincroniza. Kavita obliga a forzar un escaneo y recargar la página para ver lo que cambió; Komga tuvo que montar
   websockets para evitarlo. Aquí las dos mitades viven en el mismo proceso, así que no hay excusa arquitectónica:
   quien escribe publica el cambio en el bus y quien lo muestra se entera.

3. **Nada externo que instalar, salvo lo que es de la comunidad.** El usuario no instala Docker, ni un servidor, ni
   una máquina virtual de Java, ni un motor de terceros, ni Calibre para corregir un metadato que nuestra interfaz
   debería poder corregir. Lo único que legítimamente viene de fuera es **contenido de la comunidad**: extensiones,
   diccionarios y catálogos.

4. **Salir de la aplicación para terminar algo es un defecto nuestro.** Es la sección 5 dicha de forma que se pueda
   comprobar: si para completar una tarea de lectura, estudio o gestión hay que abrir un navegador, un explorador de
   archivos u otra aplicación, no falta una función en el sistema operativo del usuario; falta en NB Books, o está
   mal planteada.

5. **La instalación es inmediata, y el núcleo no depende de nada opcional.** Se instala y funciona: biblioteca,
   lectura, anotación, estudio y búsqueda sin configurar nada, sin cuenta, sin servidor y sin conexión. Las
   extensiones, los modelos de IA y los proveedores externos **enriquecen**, y ninguno es condición para que el
   núcleo haga su trabajo. Una aplicación que exige instalar un reproductor, un intérprete concreto y media docena
   de paquetes antes de arrancar no la usa nadie.

**Qué los hace exigibles y no un deseo.** Un principio sin consecuencia comprobable acaba siendo decoración, así
que aquí queda dicho qué lo sostiene:

| Principio | Qué lo hace cumplir |
|---|---|
| 5, plug and play | **El empaquetado**: los instaladores llevan todo dentro y no hay nada que instalar aparte. Ver [linux.md](../developer/deployment/linux.md) |
| 3, nada externo | El mismo empaquetado y [dependency-policy.md](../architecture/dependency-policy.md) |
| 1 y 2, estado vivo | Toda escritura que cambia algo que se pinta se anuncia en el bus, y la interfaz relee por esos anuncios en lugar de sincronizar copias. Lo comprueban dos mitades: `src-tauri/tests/estado_vivo.rs` exige el anuncio de cada comando que escribe, y los recorridos de estado vivo de `e2e/flujo.spec.ts` -en `pnpm verify`- comprueban sin recargar un ajuste, un tema, un metadato, una extensión instalada y quitada, un documento importado, una colección, el progreso en tres pantallas y un libro terminado. Y lo que cambia **fuera** de la aplicación, en una carpeta de la biblioteca, lo recoge el repaso al volver a la ventana, con su recorrido y sus pruebas en `src-tauri/tests/repaso_de_carpetas.rs` |
| 4, no salir | Se revisa al diseñar cada destino, con el criterio de [ADR-013](../adr/013-information-architecture.md). No es automatizable |

---

## 8. Definición operativa de "sin fricción"

> **Estado del listón:** Aplicable.

"Sin fricción" es el criterio central de este documento, pero sin medidas comprobables sería un adjetivo subjetivo. Se
define operativamente mediante **cinco condiciones verificables** sobre las tareas reales del usuario:

### 8.1 Cero cambios de aplicación durante el flujo de conocimiento

Completar el ciclo `importar → catalogar → leer → consultar significado o traducción → resaltar y anotar → convertir en
tarjeta → repasar → escuchar en voz alta` exige **cero** aperturas de aplicaciones externas (navegador web, diccionario
de escritorio, gestor de notas externo o terminal) y **cero** operaciones manuales de copiar y pegar entre programas. Si
el usuario tiene que salir de la ventana de NB Books para terminar cualquiera de esos pasos, hay fricción.

### 8.2 Tope de interacciones para las tareas cotidianas

Cada acción frecuente se mide por el número máximo de clics o pulsaciones de tecla necesarios desde el estado donde
surge la intención (descontando la escritura libre de texto cuando la acción consiste en escribir):

| Tarea comprobable | Punto de partida | Tope sin fricción | Verificación en la suite |
|---|---|---|---|
| **F1. Retomar la lectura en curso** | `Inicio` recién abierto | **1 acción** (clic o `Enter` sobre *Continuar*) | `e2e/flujo.spec.ts` |
| **F2. Consultar diccionario o traducir en contexto** | `Lector` con bloque o palabra elegida | **1 acción** para abrir consulta con la cita ya cargada, sin cambiar de vista ni perder el punto de lectura | `e2e/flujo.spec.ts` |
| **F3. Resaltar un pasaje o añadirle una nota** | `Lector` con bloque seleccionado | **1 tecla** (`H`) para resaltar; **2 acciones** (`N` y `Ctrl+Enter`) para abrir y guardar una nota | `e2e/flujo.spec.ts` |
| **F4. Convertir vocabulario o anotaciones en tarjetas** | `Lector` o `Estudio` | **≤ 2 acciones**, arrastrando contexto, cita y vínculo al libro sin volver a teclearlos | `src-tauri/tests/vocabulario.rs` y `e2e/flujo.spec.ts` |
| **F5. Ajustar tipografía, tema o modo de lectura** | `Lector` | **≤ 2 acciones**, viendo el cambio aplicado sobre la página abierta sin cerrar el panel ni recargar | `e2e/flujo.spec.ts` |
| **F6. Incorporar una biblioteca existente (carpeta o Calibre)** | `Importar` | **1 elección de carpeta**, conservando metadatos, etiquetas y archivos originales sin duplicarlos ni tocar su hora de acceso | `src-tauri/tests/calibre.rs` y `src-tauri/tests/hora_de_acceso.rs` |

### 8.3 Presupuesto temporal de respuesta y ausencia de esperas mudas

1. **Interacción en el fotograma siguiente:** el desplazamiento y las transiciones sostienen una mediana de **≤ 17 ms**
   por fotograma en el motor del sistema ([performance-budgets.md](../architecture/performance-budgets.md)
   y `scripts/recorrido-webkitgtk.py`).
2. **Hilo de ventana siempre libre:** el 100 % de los comandos del puente son asíncronos (`scripts/check-comandos.mjs`),
   por lo que ninguna operación de disco, búsqueda o IA congela el cursor ni los controles.
3. **Umbral de 300 ms para indicadores de espera:** lo que termina en menos de 300 ms no muestra ningún
   indicador intermedio (evitando parpadeos visuales); lo que supera 300 ms muestra estado explícito y permite seguir
   usando o cancelar donde corresponda ([ui-conventions.md](ui-conventions.md) sección 12.1).

### 8.4 Cero guardado manual y propagación instantánea

Ninguna preferencia, anotación, progreso de lectura, colección o cambio de metadatos exige pulsar un botón "Aplicar" o
"Refrescar" para surtir efecto en el resto de la aplicación (`src-tauri/tests/estado_vivo.rs`).

### 8.5 Salida limpia y errores accionables

Todo panel flotante, diálogo o modo inmersivo se abandona con una sola pulsación de `Escape` devolviendo el foco al
origen; y ningún fallo muestra códigos internos crudos (`scripts/check-error-contract.mjs` y `scripts/check-i18n.mjs`),
sino el motivo traducido y la acción disponible.

---
*Este documento es la brújula de NB Books. Toda PR (Pull Request), nueva feature o refactorización debe pasar la prueba ácida de los principios aquí listados.*

