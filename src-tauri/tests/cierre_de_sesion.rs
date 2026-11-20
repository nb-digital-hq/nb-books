//! Cómo terminó la sesión anterior se lee **antes** de marcar la nueva.
//!
//! Antes `bootstrap` ponía la marca de "sesión en curso" y el arranque
//! diferido leía después si la anterior había cerrado bien, así que siempre leía
//! lo que acababa de escribir `bootstrap`: en los 54 arranques que guardaba la
//! instalación real, 53 dijeron "la sesión anterior no cerró bien" y ninguno lo
//! contrario, y el índice se comprobaba en cada arranque. La prueba
//! pasaba porque llamaba a las funciones en el orden correcto; la aplicación no.
//! Por eso aquí se arranca de verdad, dos veces.
//!
//! Contrato: `docs/architecture/search.md`.

use nb_books_lib::search::CierreAnterior;

struct Temporal(std::path::PathBuf);

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn la_sesion_siguiente_sabe_como_termino_la_anterior() {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-books-cierre-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio");
    let _limpieza = Temporal(dir.clone());
    // SAFETY: esta prueba es la única de su binario.
    unsafe { std::env::set_var(nb_books_lib::paths::TEST_DIR_ENV, &dir) };

    // Un perfil recién creado: no hubo sesión anterior, y no se dice que muriera.
    let (estado, telemetria) = nb_books_lib::bootstrap().expect("primer arranque");
    assert_eq!(estado.cierre_anterior, CierreAnterior::PrimeraVez);

    // Se sale como se debe.
    nb_books_lib::cerrar_bien(&estado);
    drop(estado);
    drop(telemetria);

    let (estado, telemetria) = nb_books_lib::bootstrap().expect("segundo arranque");
    assert_eq!(
        estado.cierre_anterior,
        CierreAnterior::Limpio,
        "la sesión anterior firmó su salida; si esto dice otra cosa, se leyó después de marcar la nueva"
    );

    // Y esta muere sin firmar.
    drop(estado);
    drop(telemetria);

    let (estado, _telemetria) = nb_books_lib::bootstrap().expect("tercer arranque");
    assert_eq!(
        estado.cierre_anterior,
        CierreAnterior::Sucio,
        "una sesión que no firmó su salida obliga a comprobar el índice"
    );
}
