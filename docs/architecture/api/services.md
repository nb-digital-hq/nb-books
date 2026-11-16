# Servicios IPC (Comandos Tauri)

> **Estado:** Implementado. Catálogo de los comandos registrados en `src-tauri/src/lib.rs`
> (`tauri::generate_handler!`), agrupados por el módulo que los define. La descripción es la primera frase de la
> documentación de cada función; la firma completa y los tipos están en el código y en `src/lib/bindings/`.

Las reglas de seguridad y diseño que todo comando debe cumplir están en
[ipc-contracts.md](../ipc-contracts.md). Este documento es el registro de qué comandos
existen; aquel documento es el registro de cómo deben construirse.

## Convención de invocación

El frontend nunca invoca `invoke` directamente desde un componente. Toda llamada pasa por la capa
`src/lib/core/ipc/index.ts` (importada como `$core/ipc`), que centraliza el tipado, la traducción de códigos de error
y el registro. La regla la hace cumplir el lint `noRestrictedImports`, que alcanza también a los archivos
`.svelte`.

Los tipos que cruzan el puente **se generan desde Rust**, no se escriben a mano. El CI falla si los bindings
versionados difieren de los generados. Ver [testing/contract.md](../../developer/testing/contract.md).

## Catálogo

`scripts/check-puente.mjs` cruza los comandos registrados con el puente del frontend y con las llamadas de la interfaz: un
comando sin interfaz no pasa la verificación.

### `ipc.rs`

| Comando | Qué hace |
|---|---|
| `runtime_ready` | Si el runtime terminó de prepararse. |
| `health_check` | Comprobación de vida del runtime. |
| `update_check` | La última versión publicada, si es más nueva que la instalada. |
| `legal_notices` | La licencia, el aviso y los avisos de terceros que viajan en el paquete, para Acerca de. |
| `update_managed_by_launcher` | Si las actualizaciones de esta copia las hace NB Launcher. |
| `update_install` | Descarga, comprueba e instala la versión más nueva, y reinicia la aplicación. |
| `retired_audio_notice` | Cuántas preparaciones de audio se borraron por usar una voz que ya no existe, para decirlo una vez. |
| `browse_letter_anchors` | Las iniciales de la biblioteca ordenada por título o autor, con dónde empieza cada una. |
| `edit_edition_field` | Corrige a mano un campo de una edición. |
| `delete_document` | Quita un documento de la biblioteca, con todo lo que colgaba de él. |
| `extensions_list` | Lo que hay instalado. |
| `extensions_api` | Qué versión de la API de extensiones habla esta aplicación. |
| `extensions_context_commands` | Las órdenes que las extensiones ponen en un sitio de la interfaz. |
| `extensions_study_views` | Las vistas que las extensiones aportan a Estudio. |
| `extensions_themes` | Los temas que aportan las extensiones instaladas y activas. |
| `extensions_reading_modes` | Los modos de lectura que aportan las extensiones instaladas y activas. |
| `extensions_soundscapes` | Los fondos sonoros que aportan las extensiones instaladas y activas. |
| `boards_list` | Los tableros que hay. |
| `boards_create` | Crea un tablero con sus primeras columnas, que nombra la interfaz en su idioma. |
| `boards_delete` | Borra un tablero. |
| `collections_list` | Las colecciones, con cuántos miembros tiene cada una. |
| `collections_create` | Crea una colección manual del usuario. |
| `collections_rename` | Cambia el nombre de una colección del usuario. |
| `collections_delete` | Borra una colección del usuario. |
| `collections_add` | Añade ediciones a una colección manual, todas o ninguna, y dice cuántas entraron nuevas. |
| `collections_remove` | Quita una edición de una colección manual. |
| `collections_containing` | Las colecciones manuales en las que está una edición, para su ficha. |
| `find_note` | Una nota por su identificador, o nada si ya no existe. |
| `board_limits` | Cuánto cabe en un tablero. |
| `board_columns` | Las columnas de un tablero con sus tarjetas, en orden. |
| `board_add_column` | Añade una columna al final del tablero. |
| `board_rename_column` | Cambia el nombre de una columna. |
| `board_delete_column` | Borra una columna y sus tarjetas. |
| `board_move_column` | Lleva una columna a otro lugar del tablero. |
| `board_add_note` | Pone una nota al final de una columna; si ya estaba en el tablero, la mueve. |
| `board_add_link` | Pone al final de una columna un enlace a otro tablero. |
| `board_create_note` | Crea una nota y la pone al final de una columna, en una sola escritura. |
| `board_move_card` | Lleva una tarjeta a un lugar de una columna, la misma u otra. |
| `board_remove_card` | Quita una tarjeta del tablero. |
| `annotation_counts` | Cuántas anotaciones hay de cada clase, para no ofrecer un filtro vacío. |
| `study_counts` | Cuántas tarjetas y conceptos hay guardados. |
| `extensions_citation_formats` | Los formatos de cita que aportan las extensiones instaladas y activas. |
| `review_metadata_in_bulk` | Pide a una extensión que revise los metadatos de un lote de ediciones. |
| `save_citations` | Guarda las citas de una edición en el formato que aporte una extensión. |
| `extensions_render_view` | Compone una vista de una extensión. |
| `extensions_run_command` | Ejecuta una orden de una extensión. |
| `extensions_install` | Instala una extensión desde una carpeta del disco. |
| `extensions_grant` | Concede a una extensión salida a un anfitrión que su manifiesto declara. |
| `extensions_revoke` | Revoca un anfitrión concreto a una extensión instalada. |
| `extensions_set_enabled` | Enciende o apaga una extensión instalada. |
| `extensions_uninstall` | Desinstala una extensión **y sus permisos**. |
| `extensions_generations` | Las generaciones a las que se puede volver, de la más antigua a la más nueva. |
| `extensions_withdrawals` | Los avisos de retirada que ya se conocen, sin salir a la red. |
| `extensions_check_withdrawals` | Le pregunta al registro qué ha retirado, y aplica lo que diga. |
| `extensions_rollback` | Vuelve a la generación anterior. |
| `component_states` | Estado de salud de todos los componentes del runtime. |
| `settings_snapshot` | Todas las preferencias con su valor efectivo. |
| `update_setting` | Escribe una preferencia y devuelve cómo quedó almacenada. |
| `open_document` | Abre un documento para leerlo y devuelve lo que el formato sabe hacer. |
| `close_document` | Cierra una sesión de lectura y suelta lo que retenía. |
| `save_progress` | Apunta dónde está leyendo el usuario ahora. |
| `flush_progress` | Escribe ya la última posición apuntada del documento abierto, sin cerrarlo. |
| `synchronized_audio` | La narración que trae el propio libro para un capítulo, si la trae. |
| `book_entities` | Los nombres del libro abierto, con dónde aparece cada uno. |
| `reading_pace` | A qué ritmo se lee una edición, sacado de sus sesiones de lectura. |
| `entity_places` | Dónde aparece un nombre del libro abierto. |
| `rescan_roots` | Mira si algo cambió en las carpetas referenciadas y lo refleja. |
| `render_document` | Pinta una posición del documento abierto. |
| `page_size` | Tamaño intrínseco de la página que se pinta en una posición. |
| `text_layer` | Capa de texto de la posición, para superponerla sobre la imagen. |
| `locator_for_selection` | Ancla una cita que el usuario acaba de seleccionar. |
| `create_annotation` | Crea una anotación anclada a un Locator. |
| `edit_annotation` | Edita una anotación: da de baja la anterior y devuelve la nueva. |
| `library_annotations` | Anotaciones de **toda** la biblioteca, de la más reciente a la más antigua. |
| `diagnostics_preview` | Paquete de diagnóstico, para verlo **antes** de guardarlo. |
| `save_diagnostics` | Guarda el paquete de diagnóstico donde el usuario haya elegido. |
| `recovery_report` | Qué pasó al abrir la biblioteca, cuando hubo que recuperarla. |
| `voice_status` | Qué puede hacer la voz del runtime en esta máquina. |
| `voice_speak` | Manda decir un tramo. |
| `voice_stop` | Corta lo que la voz esté diciendo. |
| `synthesize_speech` | Sintetiza un tramo con el servicio remoto y devuelve la ruta del audio. |
| `neural_voice` | La voz neuronal local: qué es, si está instalada y si esta máquina la mueve. |
| `synthesize_neural_speech` | Sintetiza un tramo con la voz neuronal local y devuelve la ruta del audio. |
| `estimate_audio` | Lo que costaría preparar el audio de un capítulo o de un libro. |
| `sample_audio` | Sintetiza una muestra corta **del propio libro**, para decidir antes de esperar. |
| `start_audio` | Empieza a preparar el audio, en segundo plano. |
| `start_audio_collection` | Encola el libro entero de cada obra de una colección. |
| `audio_queue` | Los que esperan turno detrás del que se está preparando. |
| `cancel_audio` | Corta la preparación en curso y vacía la cola. |
| `audio_job` | Lo que se está preparando ahora, si hay algo. |
| `prepared_audio` | El audio preparado de un alcance, con sus marcas para seguir el texto. |
| `forget_audio` | Quita el audio preparado de un alcance -un capítulo o el libro entero-: sus filas, sus marcas y sus archivos. |
| `allow_speech_host` | Autoriza al servicio de síntesis configurado a recibir el texto del libro. |
| `speech_endpoint` | La dirección exacta a la que se mandaría el texto, tal y como la entiende el runtime, o nada si no hay servicio escrito. |
| `speech_credential_status` | Si se puede guardar una credencial aquí, y si el servicio ya tiene una. |
| `set_speech_credential` | Guarda en el llavero del sistema la credencial del servicio configurado. |
| `clear_speech_credential` | Borra del llavero la credencial del servicio configurado. |
| `annotation_history` | Todas las versiones de una anotación, de la más antigua a la más reciente. |
| `delete_annotation` | Da de baja una anotación. |
| `annotation_outline` | Panel de anotaciones del documento abierto, en orden de lectura. |
| `create_bookmark` | Pone un marcador en la posición actual del documento abierto. |
| `indexing_progress` | Avance de la indexación de texto completo. |
| `set_indexing_paused` | Detiene o reanuda la indexación. |
| `import_path` | Importa un archivo o una carpeta entera. |
| `cancel_import` | Cancela la importación en curso. |
| `pending_import` | La importación que quedó a medias, si la hay. |
| `last_import` | La última importación, terminada o no, para poder enseñar su informe. |
| `import_rejections` | Lo que una importación dejó fuera, y por qué. |
| `import_duplicates` | Las copias exactas que encontró una importación, con el título y la ruta de lo que ya estaba. |
| `discard_pending_import` | Da por zanjada una importación a medias sin continuarla. |
| `reading_stats` | Lo que la pantalla de Inicio muestra sobre la lectura. |
| `home_rows` | Las filas de Inicio para una visita. |
| `list_notes` | Notas propias del usuario, de la más reciente a la más antigua. |
| `create_note` | Crea una nota propia. |
| `update_note` | Edita una nota propia. |
| `delete_note` | Borra una nota propia y sus enlaces. |
| `note_backlinks` | Notas que apuntan a esta. |
| `cite_annotation` | Cita una anotación desde una nota. |
| `uncite_annotation` | Deshace la cita. |
| `cited_annotations` | Los pasajes que una nota cita, con el libro del que salió cada uno. |
| `notes_citing` | Notas que citan una anotación. |
| `browse_library` | Devuelve una ventana de la biblioteca, ya ordenada y filtrada. |
| `library_facets` | Cuántas obras hay detrás de cada filtro fijo de la biblioteca. |
| `library_shelves` | Estanterías de una clase: autores, series o materias, con su tamaño. |
| `search_status` | Si un documento se puede buscar, y por qué no. |
| `search` | Busca en el alcance que se le diga. |
| `reindex` | Rehace el índice de búsqueda desde cero. |
| `lookup_selection` | Consulta la selección en los proveedores instalados. |
| `list_dictionaries` | Diccionarios instalados en esta máquina. |
| `install_dictionary` | Instala un diccionario a partir de su archivo `.ifo`. |
| `uninstall_dictionary` | Desinstala un diccionario. |
| `list_vocabulary` | El vocabulario consultado, de lo más reciente a lo más antiguo. |
| `delete_vocabulary` | Borra una palabra del vocabulario. |
| `clear_vocabulary` | Borra el vocabulario entero. |
| `save_annotations` | Guarda las anotaciones de una edición donde el usuario haya dicho: en el JSON de W3C, o en Markdown o texto plano con su cita y su localizador. |
| `save_vocabulary` | Guarda el vocabulario consultado donde el usuario haya dicho, en CSV. |
| `vocabulary_to_cards` | Hace tarjetas de estudio con el vocabulario consultado, con la definición del diccionario de aquí dentro en el reverso y sin salir a la red. |
| `save_library_export` | Exporta la biblioteca entera donde el usuario haya dicho. |
| `clear_reading_history` | Borra el historial de lectura y por dónde iba cada documento. |
| `clear_annotations_and_notes` | Borra todas las anotaciones, las notas, sus enlaces y los tableros. |
| `clear_library` | Vacía la biblioteca entera en la base de datos. |
| `library_roots` | Las raíces de la biblioteca y el estado de sus archivos. |
| `relocate_library_root` | Vuelve a apuntar una raíz a donde estén ahora sus archivos. |
| `refresh_availability` | Revisa dónde está cada archivo de la biblioteca. |
| `network_report` | Estado de la salida de red: interruptor, permisos y registro. |
| `revoke_network_consent` | Revoca un permiso de salida. |
| `allow_lookup_host` | Autoriza a un proveedor de consulta a salir a un destino. |
| `allow_catalog_host` | Autoriza a un catálogo a salir a un destino. |
| `ai_models` | Los modelos de IA del catálogo, como los enseña Ajustes. |
| `install_ai_model` | Empieza a instalar un modelo. |
| `cancel_ai_model_install` | Pide cortar la instalación en curso. |
| `uninstall_ai_model` | Quita un modelo instalado. |
| `allow_model_host` | Autoriza la descarga de modelos desde un anfitrión. |
| `ai_provider_status` | El proveedor de IA externo: cuál es, a dónde sale, si está autorizado y si tiene credencial. |
| `probe_ai_provider` | Prueba el proveedor externo. |
| `allow_ai_provider` | Autoriza el envío al proveedor externo **configurado**. |
| `set_ai_credential` | Guarda en el llavero la credencial del proveedor externo que hay escrito. |
| `clear_ai_credential` | Borra la credencial del proveedor externo que hay escrito. |
| `ai_sends` | Lo que se ha enviado a proveedores externos, de lo más reciente a lo más antiguo: el registro que `ai/privacy.md` pide poder auditar. |
| `clear_ai_sends` | Borra el registro de envíos. |
| `report_ai_output` | Marca una salida generada como incorrecta. |
| `ai_reports` | Lo marcado como incorrecto, de lo más reciente a lo más antiguo. |
| `forget_ai_report` | Quita un reporte. |
| `clear_ai_reports` | Borra los reportes. |
| `save_ai_reports` | Guarda los reportes donde el usuario diga, para poder mirarlos antes de decidir si los manda a alguna parte. |
| `estimate_vectors` | Lo que costaría vectorizar un alcance: documentos, fragmentos, disco y tiempo. |
| `start_vectors` | Empieza a vectorizar un alcance en segundo plano. |
| `pause_vectors` | Pausa o reanuda la generación de vectores. |
| `cancel_vectors` | Corta la generación de vectores. |
| `vector_job` | Cómo va la generación de vectores. |
| `reader_activity` | El lector está pintando: la generación de fondo se aparta un momento. |
| `vector_exclusion` | Si una edición o una colección está excluida de la búsqueda por significado. |
| `set_vector_exclusion` | Excluye o vuelve a incluir una edición o una colección. |
| `semantic_search` | Busca por significado en lo preparado de un alcance. |
| `study_availability` | Quién generaría las funciones de estudio ahora, y si puede. |
| `estimate_study_scope` | Estima el texto que saldría para una parte de un documento, en caracteres y tokens aproximados. |
| `generate_study` | Hace una función de estudio sobre una parte de un documento, con quien eligió la persona en Ajustes. |
| `cancel_study` | Corta la generación en curso, si la hay. |
| `save_study_cards` | Guarda tarjetas y preguntas aceptadas. |
| `study_cards` | Las tarjetas y preguntas guardadas, de un libro o de todos. |
| `review_study_card` | Registra la respuesta de repaso de una tarjeta o pregunta, recalculando su intervalo, facilidad y próxima fecha según SM-2 determinista. |
| `study_card_counts` | Cantidad total y pendientes de repasar hoy para las tarjetas y preguntas de estudio. |
| `edit_study_card` | Edita una tarjeta o una pregunta. |
| `delete_study_card` | Descarta una tarjeta o una pregunta guardada. |
| `save_concepts` | Guarda conceptos aceptados. |
| `concepts` | Los conceptos guardados, de un libro o de todos. |
| `edit_concept` | Edita un concepto. |
| `delete_concept` | Borra un concepto guardado. |
| `what_is_sent` | Qué se envía al salir con cada propósito, en lenguaje llano. |
| `search_online_metadata` | Busca en el catálogo abierto candidatos para completar una edición. |
| `apply_online_metadata` | Aplica un candidato a una edición, campo por campo. |
| `fetch_online_cover` | Descarga la portada de un candidato y la deja registrada en la edición. |
| `pending_suggestions` | Propuestas vivas de una edición. |
| `edition_details` | La ficha de una edición: editorial, fecha, idioma, materias y copias. |
| `allow_metadata_host` | Autoriza el catálogo abierto de metadatos desde la propia ficha. |
| `accept_suggestion` | Acepta una propuesta: la decisión pasa a ser del usuario. |
| `reject_suggestion` | Descarta una propuesta. |
| `list_catalogs` | Catálogos que el usuario ha añadido. |
| `add_catalog` | Añade un catálogo por su dirección. |
| `remove_catalog` | Quita un catálogo. |
| `browse_catalog` | Navega un catálogo: pide un feed y lo devuelve analizado. |
| `download_from_catalog` | Descarga un documento de un catálogo y lo importa. |
