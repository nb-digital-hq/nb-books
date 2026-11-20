//! Criterio de aceptación de la.
//!
//! "Importar un archivo corrupto, uno con extensión falsificada, uno inexistente
//! y uno con el disco lleno no deja registros huérfanos ni copias parciales."
//!
//! La importación es la puerta de entrada al sistema, y por tanto la superficie
//! por la que llega todo lo hostil. El corpus de `tests/fixtures/` existe
//! justamente para que estas defensas se prueben contra archivos concretos y no
//! contra la idea de un archivo hostil.
//!
//! Contrato: `docs/architecture/data-flows.md`, sección 1.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nb_books_lib::db::catalog::{Availability, StorageMode};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::events::{DomainEvent, EventBus};
use nb_books_lib::library::{
    import_directory, import_document, sniff, CancelToken, Format, ImportContext, ImportOutcome,
    ImportRequest,
};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Escenario {
    _dir: PathBuf,
    scope: PathBuf,
    library: PathBuf,
    db: Database,
    bus: EventBus,
    eventos: Arc<Mutex<Vec<String>>>,
}

impl Escenario {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-import-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let scope = dir.join("origen");
        let library = dir.join("biblioteca");
        std::fs::create_dir_all(&scope).expect("ámbito");
        std::fs::create_dir_all(&library).expect("biblioteca");

        let bus = EventBus::new();
        let eventos = Arc::new(Mutex::new(Vec::new()));
        let destino = Arc::clone(&eventos);
        bus.subscribe("espia", move |evento| {
            if let DomainEvent::DocumentImported { edition_id, .. } = evento {
                destino
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(edition_id.clone());
            }
        });

        Self {
            db: Database::open_in_memory().expect("base"),
            _dir: dir,
            scope,
            library,
            bus,
            eventos,
        }
    }

    /// Copia un archivo del corpus al ámbito autorizado, opcionalmente con otro
    /// nombre.
    fn traer(&self, fixture: &str, como: &str) -> PathBuf {
        let destino = self.scope.join(como);
        std::fs::copy(fixtures().join(fixture), &destino).expect("copia del corpus");
        destino
    }

    fn importar(&self, ruta: &Path, modo: StorageMode) -> Result<ImportOutcome, NbError> {
        let mut conn = self.db.connection().expect("conexión");
        import_document(
            &mut conn,
            &ImportContext {
                scope: &self.scope,
                library: &self.library,
                bus: &self.bus,
            },
            &ImportRequest {
                source: ruta.to_path_buf(),
                storage_mode: modo,
                root: None,
            },
        )
    }

    fn filas(&self, tabla: &str) -> i64 {
        self.db
            .connection()
            .expect("conexión")
            .query_row(&format!("SELECT count(*) FROM {tabla}"), [], |r| r.get(0))
            .expect("consulta")
    }

    /// Nada registrado, y nada a medias en la carpeta de biblioteca.
    fn nada_quedo_a_medias(&self) {
        for tabla in ["works", "editions", "files", "annotations"] {
            assert_eq!(self.filas(tabla), 0, "quedaron filas huérfanas en {tabla}");
        }

        let restos: Vec<_> = std::fs::read_dir(&self.library)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            restos.is_empty(),
            "quedaron copias en la biblioteca: {restos:?}"
        );
    }

    fn eventos(&self) -> Vec<String> {
        for _ in 0..200 {
            let vistos = self
                .eventos
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone();
            if !vistos.is_empty() {
                return vistos;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Vec::new()
    }
}

impl Drop for Escenario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self._dir);
    }
}

// --- Reconocimiento por contenido ------------------------------------------

/// El formato se decide por lo que el archivo **es**, no por cómo se llama.
#[test]
fn el_formato_se_reconoce_por_contenido() {
    assert_eq!(
        sniff(&fixtures().join("minimo-valido.pdf")),
        Ok(Format::Pdf)
    );
    assert_eq!(
        sniff(&fixtures().join("minimo-valido.epub")),
        Ok(Format::Epub)
    );
    assert_eq!(
        sniff(&fixtures().join("recorrido-directorios.cbz")),
        Ok(Format::ComicArchive)
    );

    // Un PDF que se anuncia como EPUB sigue siendo un PDF.
    assert_eq!(
        sniff(&fixtures().join("extension-falsificada.epub")),
        Ok(Format::Pdf)
    );
}

#[test]
fn un_archivo_truncado_se_reconoce_como_danado_y_no_como_valido() {
    assert_eq!(
        sniff(&fixtures().join("truncado.pdf")),
        Err(NbError::DocumentCorrupted)
    );
}

#[test]
fn un_formato_desconocido_se_rechaza_sin_confundirlo_con_uno_dañado() {
    assert_eq!(
        sniff(&fixtures().join("adversario-prompt.txt")),
        Err(NbError::FormatUnsupported)
    );
}

#[test]
fn un_archivo_inexistente_se_distingue_de_uno_ilegible() {
    let ruta = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-no-existe-{}", std::process::id()));
    assert_eq!(sniff(&ruta), Err(NbError::FileNotFound));
}

// --- El criterio de aceptación ---------------------------------------------

/// Camino feliz, para que los caminos de error signifiquen algo.
#[test]
fn un_documento_valido_entra_con_su_obra_su_edicion_y_su_archivo() {
    let esc = Escenario::nuevo("feliz");
    let ruta = esc.traer("minimo-valido.epub", "Un libro cualquiera.epub");

    let resultado = esc
        .importar(&ruta, StorageMode::Managed)
        .expect("importado");

    let ImportOutcome::Imported(importado) = resultado else {
        panic!("debía importarse: {resultado:?}");
    };
    assert_eq!(importado.work.canonical_title, "Un libro cualquiera");
    assert_eq!(importado.edition.title, "Un libro cualquiera");
    assert_eq!(importado.file.media_type, "application/epub+zip");
    assert_eq!(importado.file.storage_mode, StorageMode::Managed);
    assert_eq!(importado.file.availability, Availability::Available);
    assert!(!importado.extension_mismatch);

    assert_eq!(esc.filas("works"), 1);
    assert_eq!(esc.filas("editions"), 1);
    assert_eq!(esc.filas("files"), 1);

    // La copia administrada existe y no quedó ningún parcial.
    let conn = esc.db.connection().expect("conexión");
    let ruta_copia = nb_books_lib::library::resolve_path(&conn, &importado.file)
        .expect("consulta de la raíz")
        .expect("raíz localizada");
    assert!(ruta_copia.exists());
    let parciales: Vec<_> = std::fs::read_dir(&esc.library)
        .expect("biblioteca")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "parcial"))
        .collect();
    assert!(parciales.is_empty(), "quedó una copia parcial");

    // Y el resto del sistema se entera por el bus, no por una llamada directa.
    assert_eq!(esc.eventos(), vec![importado.edition.id.clone()]);
}

/// **Criterio, caso 1:** un archivo corrupto.
#[test]
fn un_archivo_corrupto_no_deja_registros_huerfanos_ni_copias_parciales() {
    let esc = Escenario::nuevo("corrupto");
    let ruta = esc.traer("truncado.pdf", "roto.pdf");

    assert_eq!(
        esc.importar(&ruta, StorageMode::Managed),
        Err(NbError::DocumentCorrupted)
    );
    esc.nada_quedo_a_medias();
}

/// **Criterio, caso 2:** un archivo con la extensión falsificada.
///
/// No se rechaza: manda el contenido, así que un PDF que se llama `.epub` entra
/// **como PDF**. Lo que no puede pasar es que entre como EPUB, porque entonces
/// el lector intentaría abrirlo con el motor equivocado.
#[test]
fn un_archivo_con_la_extension_falsificada_entra_como_lo_que_es() {
    let esc = Escenario::nuevo("falsificada");
    let ruta = esc.traer("extension-falsificada.epub", "disfrazado.epub");

    let resultado = esc
        .importar(&ruta, StorageMode::Managed)
        .expect("importado");

    let ImportOutcome::Imported(importado) = resultado else {
        panic!("debía importarse como PDF: {resultado:?}");
    };
    assert_eq!(
        importado.file.media_type, "application/pdf",
        "la extensión no puede decidir el formato"
    );
    assert!(
        importado.extension_mismatch,
        "la discrepancia tiene que informarse"
    );
    // Y la copia administrada lleva la extensión de lo que realmente es.
    assert!(
        importado.file.relative_path.ends_with(".pdf"),
        "{}",
        importado.file.relative_path
    );
}

/// **Criterio, caso 3:** un archivo inexistente.
#[test]
fn un_archivo_inexistente_no_deja_nada() {
    let esc = Escenario::nuevo("inexistente");
    let ruta = esc.scope.join("no-existe.epub");

    assert_eq!(
        esc.importar(&ruta, StorageMode::Managed),
        Err(NbError::FileNotFound)
    );
    esc.nada_quedo_a_medias();
}

/// **Criterio, caso 4:** no se puede escribir la copia.
///
/// El disco lleno no se puede simular de forma portátil, pero produce
/// exactamente el mismo camino que una biblioteca sin permiso de escritura: la
/// copia falla. Lo que se comprueba es lo que importa, que no quede ni copia
/// parcial ni registro.
#[cfg(unix)]
#[test]
fn si_la_copia_no_se_puede_escribir_no_queda_nada() {
    use std::os::unix::fs::PermissionsExt;

    let esc = Escenario::nuevo("sin-espacio");
    let ruta = esc.traer("minimo-valido.epub", "libro.epub");
    std::fs::set_permissions(&esc.library, std::fs::Permissions::from_mode(0o500))
        .expect("solo lectura");

    let resultado = esc.importar(&ruta, StorageMode::Managed);

    // Restaurar permisos antes de aseverar, para que la limpieza funcione
    // aunque la prueba falle.
    let _ = std::fs::set_permissions(&esc.library, std::fs::Permissions::from_mode(0o700));

    assert_eq!(resultado, Err(NbError::StorageFailure));
    esc.nada_quedo_a_medias();
}

/// Un formato que no reconocemos no entra, y tampoco deja rastro.
#[test]
fn un_formato_no_soportado_no_deja_nada() {
    let esc = Escenario::nuevo("no-soportado");
    let ruta = esc.traer("adversario-prompt.txt", "texto.txt");

    assert_eq!(
        esc.importar(&ruta, StorageMode::Managed),
        Err(NbError::FormatUnsupported)
    );
    esc.nada_quedo_a_medias();
}

/// El webview es zona no confiable: una ruta fuera del ámbito autorizado no se
/// importa aunque exista y sea un documento perfectamente válido.
#[test]
fn una_ruta_fuera_del_ambito_autorizado_se_rechaza() {
    let esc = Escenario::nuevo("fuera-de-ambito");
    let fuera = esc._dir.join("fuera.epub");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &fuera).expect("copia");

    assert_eq!(
        esc.importar(&fuera, StorageMode::Managed),
        Err(NbError::PathNotAllowed)
    );
    esc.nada_quedo_a_medias();
}

/// Importar dos veces el mismo contenido no crea un segundo documento. Es la
/// política que queda decidida aquí.
#[test]
fn importar_dos_veces_el_mismo_contenido_no_duplica() {
    let esc = Escenario::nuevo("duplicado");
    let ruta = esc.traer("minimo-valido.epub", "libro.epub");

    let primera = esc
        .importar(&ruta, StorageMode::Referenced)
        .expect("primera");
    let ImportOutcome::Imported(importado) = primera else {
        panic!("la primera debía importar");
    };

    // El mismo archivo otra vez.
    let segunda = esc
        .importar(&ruta, StorageMode::Referenced)
        .expect("segunda");
    assert!(matches!(segunda, ImportOutcome::AlreadyInLibrary(_)));

    // Y el mismo contenido con otro nombre, que es como llega de verdad un
    // duplicado. Ahora se dice como tal, y no como "ya estaba": no es el
    // mismo archivo visto otra vez, es el mismo libro dos veces en el disco.
    let copia = esc.traer("minimo-valido.epub", "el-mismo-libro-otra-vez.epub");
    let tercera = esc
        .importar(&copia, StorageMode::Referenced)
        .expect("tercera");
    let ImportOutcome::Duplicate(ya) = tercera else {
        panic!("una copia con otro nombre es una copia exacta de lo que ya estaba");
    };
    assert_eq!(ya.id, importado.file.id);

    assert_eq!(esc.filas("works"), 1);
    assert_eq!(esc.filas("files"), 1);
}

/// En modo referenciado el archivo se queda donde estaba: no se copia nada.
#[test]
fn el_modo_referenciado_no_copia_el_archivo() {
    let esc = Escenario::nuevo("referenciado");
    let ruta = esc.traer("minimo-valido.epub", "libro.epub");

    let resultado = esc
        .importar(&ruta, StorageMode::Referenced)
        .expect("importado");

    let ImportOutcome::Imported(importado) = resultado else {
        panic!("debía importarse");
    };
    assert_eq!(importado.file.storage_mode, StorageMode::Referenced);
    let conn = esc.db.connection().expect("conexión");
    assert_eq!(
        nb_books_lib::library::resolve_path(&conn, &importado.file)
            .expect("consulta de la raíz")
            .expect("raíz localizada")
            .canonicalize()
            .expect("canónica"),
        ruta.canonicalize().expect("canónica")
    );
    assert_eq!(
        std::fs::read_dir(&esc.library).expect("biblioteca").count(),
        0,
        "el modo referenciado no copia"
    );
}

/// **Regresión, medida sobre el corpus real.** Un libro cuyo título lleva puntos
/// suspensivos entra.
///
/// El esquema rechazaba toda ruta relativa que contuviera `..` en cualquier
/// posición, para impedir travesías de ruta. Sobre los 152.080 archivos del
/// corpus real, esa comprobación rechazaba **667** -el 0,44 %- y de esos 667,
/// **cero** eran travesías: eran elipsis dentro de títulos. Seiscientos sesenta
/// y siete libros que no podían entrar en la biblioteca de su dueño, y ni un
/// ataque detenido. Lo arregla V28, que mira componentes de ruta y no
/// subcadenas.
#[test]
fn un_titulo_con_puntos_suspensivos_entra_como_cualquier_otro() {
    let esc = Escenario::nuevo("elipsis");
    let ruta = esc.traer("minimo-valido.epub", "Y entonces.. el final.epub");

    let resultado = esc
        .importar(&ruta, StorageMode::Referenced)
        .expect("importado");

    let ImportOutcome::Imported(importado) = resultado else {
        panic!("un título con elipsis no es una travesía de ruta");
    };
    assert!(importado.file.relative_path.contains(".."));
}

/// Y lo que la comprobación existe para impedir se sigue impidiendo.
///
/// Sin esta mitad, la regresión de arriba se "arreglaría" quitando la
/// comprobación, que es la forma de hacer pasar la prueba rompiendo lo que
/// protegía.
#[test]
fn una_ruta_que_se_sale_de_su_raiz_se_sigue_rechazando() {
    use nb_books_lib::db::catalog::{
        ContentHash, Edition, File, NewEdition, NewFile, NewWork, Work,
    };

    let esc = Escenario::nuevo("travesia");
    let conn = esc.db.connection().expect("conexión");
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Obra".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: "Edición".to_owned(),
            ..NewEdition::default()
        },
    )
    .expect("edición");

    for travesia in [
        "../fuera.epub",
        "..",
        "autor/../../fuera.epub",
        "autor/..",
        "..\\fuera.epub",
        "autor\\..\\..\\fuera.epub",
        "/absoluta.epub",
        "\\absoluta.epub",
    ] {
        let intento = File::create(
            &conn,
            &NewFile {
                edition_id: edicion.id.clone(),
                root_id: nb_books_lib::db::catalog::MANAGED_ROOT_ID.to_owned(),
                relative_path: travesia.to_owned(),
                content_hash: ContentHash::from_sha256(&[0x33; 32]),
                byte_size: 1,
                media_type: "application/epub+zip".to_owned(),
                storage_mode: StorageMode::Managed,
            },
        );
        assert!(
            intento.is_err(),
            "{travesia:?} sale de la raíz y tiene que rechazarse"
        );
    }
}

/// El título con el que entra es provisional y lleva su procedencia, de modo que
/// la extracción de metadatos podrá mejorarlo y una corrección del usuario
/// seguirá ganando sobre ambas.
#[test]
fn el_titulo_provisional_entra_con_su_procedencia() {
    use nb_books_lib::metadata::{self, Field, Outcome, Source};

    let esc = Escenario::nuevo("titulo");
    let ruta = esc.traer("minimo-valido.epub", "nombre-del-archivo.epub");
    let ImportOutcome::Imported(importado) = esc
        .importar(&ruta, StorageMode::Referenced)
        .expect("importado")
    else {
        panic!("debía importarse");
    };

    let conn = esc.db.connection().expect("conexión");
    let historial =
        metadata::history(&conn, &importado.edition.id, Field::Title).expect("historial");
    assert_eq!(historial.len(), 1);
    assert_eq!(historial[0].source, Source::File);

    // La extracción de metadatos podrá mejorarlo sin preguntar.
    let mejorado = metadata::propose(
        &conn,
        &importado.edition.id,
        Field::Title,
        Source::File,
        Some("El título de verdad, sacado del EPUB"),
    )
    .expect("mejora");
    assert_eq!(mejorado, Outcome::Applied);

    // Pero una corrección del usuario gana sobre lo que venga después.
    metadata::set_manual(
        &conn,
        &importado.edition.id,
        Field::Title,
        Some("Como yo lo llamo"),
    )
    .expect("corrección");
    let posterior = metadata::propose(
        &conn,
        &importado.edition.id,
        Field::Title,
        Source::Online,
        Some("Lo que diga el proveedor"),
    )
    .expect("propuesta");
    assert!(matches!(posterior, Outcome::Suggested(_)));
}

// --- La puerta por la que entra todo -------------------------------

/// Importar una **carpeta** trae lo que hay dentro y no se cae con lo que no
/// sabe abrir.
///
/// Es el camino que recorre el usuario la primera vez que usa la aplicación, y
/// el que decide si la biblioteca existe o se queda vacía. Un archivo roto entre
/// mil no puede impedir que entren los otros novecientos noventa y nueve.
#[test]
fn importar_una_carpeta_trae_lo_que_sabe_abrir_y_dice_lo_que_no() {
    let escenario = Escenario::nuevo("carpeta-mixta");

    for fixture in ["minimo-valido.epub", "minimo-valido.cbz"] {
        std::fs::copy(fixtures().join(fixture), escenario.scope.join(fixture)).expect("copia");
    }
    std::fs::write(escenario.scope.join("no-es-un-libro.txt"), b"solo texto").expect("archivo");
    std::fs::copy(
        fixtures().join("truncado.pdf"),
        escenario.scope.join("truncado.pdf"),
    )
    .expect("copia");

    let mut conn = escenario.db.connection().expect("conexión");
    let informe = import_directory(
        &mut conn,
        &ImportContext {
            scope: &escenario.scope,
            library: &escenario.library,
            bus: &escenario.bus,
        },
        &escenario.scope,
        StorageMode::Referenced,
        &CancelToken::default(),
    )
    .expect("importación");

    assert_eq!(informe.imported, 2, "el EPUB y el cómic entran");
    // Lo que no es un documento se deja pasar sin contarlo como rechazo: en
    // una carpeta de apuntes eran 888 "rechazados" que escondían los de verdad.
    assert_eq!(informe.ignored, 1, "el que no es un libro se deja pasar");
    assert_eq!(
        informe.rejected.len(),
        1,
        "el documento dañado sí se rechaza"
    );
    assert!(!informe.cancelled);

    // Y la ruta del rechazo es **relativa**: la parte que revela dónde vive el
    // usuario no aporta nada para saber qué archivo falló.
    assert_eq!(informe.rejected[0].path, "truncado.pdf");
    assert!(!informe.rejected[0].path.contains('/'));
}

/// Volver a importar la misma carpeta no duplica nada.
///
/// Reanudar es, literalmente, volver a ejecutar lo mismo: la identidad por
/// contenido lo hace gratis, y por eso no hay cursor de progreso que pueda
/// quedar mal.
#[test]
fn volver_a_importar_la_misma_carpeta_no_duplica_nada() {
    let escenario = Escenario::nuevo("reanudar");
    std::fs::copy(
        fixtures().join("minimo-valido.epub"),
        escenario.scope.join("libro.epub"),
    )
    .expect("copia");

    let mut conn = escenario.db.connection().expect("conexión");
    let ctx = ImportContext {
        scope: &escenario.scope,
        library: &escenario.library,
        bus: &escenario.bus,
    };

    let primera = import_directory(
        &mut conn,
        &ctx,
        &escenario.scope,
        StorageMode::Referenced,
        &CancelToken::default(),
    )
    .expect("primera");
    let segunda = import_directory(
        &mut conn,
        &ctx,
        &escenario.scope,
        StorageMode::Referenced,
        &CancelToken::default(),
    )
    .expect("segunda");

    assert_eq!(primera.imported, 1);
    assert_eq!(segunda.imported, 0);
    assert_eq!(segunda.already_in_library, 1);
}

/// Una importación masiva **anuncia su final**, incluso cuando no había nada.
///
/// Sin este evento, la interfaz solo sabría que la importación acabó al dejar de
/// recibir avances, que es indistinguible de una que va lenta: se quedaría
/// esperando un avance que no va a llegar.
#[test]
fn una_importacion_masiva_anuncia_su_final_y_dice_si_la_pararon() {
    let escenario = Escenario::nuevo("final");
    std::fs::copy(
        fixtures().join("minimo-valido.epub"),
        escenario.scope.join("libro.epub"),
    )
    .expect("copia");

    // El espía se pone **antes** de la primera importación: el bus entrega los
    // eventos por su cuenta, así que suscribirse después no garantiza haberse
    // perdido los anteriores ni haberlos recibido.
    let finales = Arc::new(Mutex::new(Vec::new()));
    let destino = Arc::clone(&finales);
    escenario.bus.subscribe("final", move |evento| {
        if let DomainEvent::BulkImportFinished { imported, .. } = evento {
            destino
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(*imported);
        }
    });

    let mut conn = escenario.db.connection().expect("conexión");
    let ctx = ImportContext {
        scope: &escenario.scope,
        library: &escenario.library,
        bus: &escenario.bus,
    };

    for _ in 0..2 {
        import_directory(
            &mut conn,
            &ctx,
            &escenario.scope,
            StorageMode::Referenced,
            &CancelToken::default(),
        )
        .expect("importación");
    }

    // El bus entrega sin bloquear al emisor, así que se espera a que despache.
    for _ in 0..400 {
        if finales.lock().expect("eventos").len() >= 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    // Dos importaciones, dos finales: uno con el documento que entró y otro con
    // ninguno, porque ya estaba. El segundo es el que importa: sin él, la
    // interfaz no distinguiría "terminó sin traer nada" de "sigue trabajando".
    assert_eq!(
        finales.lock().expect("eventos").as_slice(),
        &[1, 0],
        "el final se anuncia siempre, incluso cuando no entra nada"
    );
}

/// Cancelar para entre archivo y archivo, y el final lo dice.
///
/// **Nunca a mitad de uno**: interrumpir una copia dejaría exactamente la copia
/// parcial que el producto existe para evitar.
#[test]
fn cancelar_para_la_importacion_y_el_final_lo_declara() {
    let escenario = Escenario::nuevo("cancelada");
    for i in 0..3 {
        std::fs::copy(
            fixtures().join("minimo-valido.epub"),
            escenario.scope.join(format!("libro-{i}.epub")),
        )
        .expect("copia");
    }

    let mut conn = escenario.db.connection().expect("conexión");
    let ctx = ImportContext {
        scope: &escenario.scope,
        library: &escenario.library,
        bus: &escenario.bus,
    };

    // Cancelado antes de empezar: no llega a mirar ningún archivo.
    let testigo = CancelToken::default();
    testigo.cancel();

    let informe = import_directory(
        &mut conn,
        &ctx,
        &escenario.scope,
        StorageMode::Referenced,
        &testigo,
    )
    .expect("importación");

    assert!(informe.cancelled);
    assert_eq!(informe.imported, 0);

    // Y el mismo testigo, reiniciado, vuelve a servir: sin esto, una
    // cancelación pararía para siempre las importaciones siguientes.
    testigo.reset();
    let segunda = import_directory(
        &mut conn,
        &ctx,
        &escenario.scope,
        StorageMode::Referenced,
        &testigo,
    )
    .expect("segunda");

    assert!(!segunda.cancelled);
    // Uno entra y dos se reconocen como el mismo contenido: los tres archivos
    // son copias, y la identidad por contenido lo sabe sin abrirlos.
    // Ahora se cuentan como copias exactas y no como "ya estaban".
    assert_eq!(segunda.imported, 1);
    assert_eq!(segunda.duplicates, 2);
    assert_eq!(segunda.already_in_library, 0);
    assert_eq!(segunda.processed, 3);
}
