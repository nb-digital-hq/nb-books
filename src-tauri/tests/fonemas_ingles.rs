//! El inglés por diccionario: el mecanismo, probado con una tabla
//! pequeña, porque la de verdad **no está en el repositorio**.
//!
//! La tabla viaja con la voz, como un archivo más del modelo: son megabytes y son
//! del modelo, así que entran por la puerta verificada del catálogo -licencia,
//! huella y descarga comprobada- y no por una carpeta que nadie revisa. Lo que se
//! comprueba aquí es lo que no depende de qué tabla sea: que se lee, que se usa, que
//! lo que falta **se puede saber antes de hablar** y que lo aproximado no se cuela
//! como si fuera sabido.

use nb_books_lib::voice::fonemas::ingles::{self, Diccionario};

const TABLA: &str = "# una tabla de prueba, con el formato de la de verdad\n\
                     the\tðə\n\
                     book\tbˈʊk\n\
                     reader\trˈiːdər\n\
                     \n\
                     rota sin fonemas\t\n";

/// Lo que está en la tabla se dice como dice la tabla, no como parezca.
#[test]
fn lo_que_esta_en_la_tabla_se_dice_como_ella_manda() {
    let diccionario = Diccionario::leer(TABLA);
    assert_eq!(
        diccionario.len(),
        3,
        "se leyeron entradas de más o de menos"
    );

    let dicho = ingles::de_texto("The book, the reader.", &diccionario);
    assert_eq!(dicho, "ðə bˈʊk, ðə rˈiːdər.");
}

/// Y lo que no está **se sabe antes de hablar**, que es lo que permite no ofrecer
/// una voz que iba a inventarse media frase.
#[test]
fn lo_que_falta_en_la_tabla_se_puede_saber_antes_de_hablar() {
    let diccionario = Diccionario::leer(TABLA);
    let faltan = ingles::sin_diccionario("The book of Faulkner", &diccionario);
    assert_eq!(faltan, vec!["of".to_owned(), "faulkner".to_owned()]);

    // Y lo aproximado suena a algo: una palabra desconocida **no desaparece**, que
    // es lo que hace un sintetizador que se come lo que no sabe.
    let dicho = ingles::de_texto("Faulkner", &diccionario);
    assert!(
        !dicho.is_empty(),
        "una palabra desconocida se perdió entera"
    );
    assert!(
        nb_books_lib::voice::fonemas::todos_admitidos(&dicho),
        "la aproximación produjo fonemas que el modelo no sabe decir: {dicho}"
    );
}

/// La tabla se carga de la carpeta del modelo, con un nombre fijo.
#[test]
fn la_tabla_se_carga_de_la_carpeta_del_modelo() {
    let carpeta = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-ingles-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&carpeta);
    std::fs::create_dir_all(&carpeta).expect("carpeta");

    assert!(
        Diccionario::cargar(&carpeta).is_none(),
        "dijo tener tabla sin haberla"
    );

    std::fs::write(carpeta.join(ingles::ARCHIVO), TABLA).expect("tabla");
    let cargado = Diccionario::cargar(&carpeta).expect("la tabla está ahí");
    assert_eq!(
        cargado.buscar("Book"),
        Some("bˈʊk"),
        "no busca sin mayúsculas"
    );

    let _ = std::fs::remove_dir_all(&carpeta);
}
