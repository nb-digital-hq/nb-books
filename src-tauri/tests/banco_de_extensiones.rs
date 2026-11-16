//! El banco de extensiones adversas.
//!
//! Comprueba que los módulos de `extensiones/banco/` **intentan de verdad lo
//! que dicen intentar**. No comprueba que el anfitrión los detenga: eso es
//! el banco adverso, y no se podía escribir sin el anfitrión.
//!
//! # Por qué existe esta prueba y no solo los módulos
//!
//! Un banco que no puede fallar convierte cualquier medición en una
//! confirmación. Ya ocurrió: el simulador de la biblioteca servía
//! una portada idéntica para las 152.080 obras, así que el coste de
//! descodificar portadas distintas no podía aparecer en la medida, y se
//! concluyó que no hacía falta caché. Ver `performance-budgets.md`.
//!
//! El equivalente aquí sería un cajón de arena probado con un módulo que
//! saluda. Por eso cada ficha de `banco.json` declara qué función del anfitrión
//! abusa, y esta prueba **abre el binario y comprueba que la importación está**.
//! Un módulo que dijera leer archivos sin importar `path_open` no intentaría
//! nada, y el día que el aislamiento se rompiera la suite seguiría en verde.
//!
//! Dos conductas no se pueden probar así, y sus fichas lo dicen en lugar de
//! fingir una evidencia: quemar procesador y agotar memoria no necesitan al
//! anfitrión para nada. Su prueba es ejecutarlas contra los límites.
//!
//! # Cómo se ejecutan
//!
//! Se omiten solas si el objetivo `wasm32-wasip1` no está instalado, porque
//! quien clone el repositorio puede no tenerlo. En esta máquina sí está desde
//! el 2026-09-07.

mod comun;

use comun::{banco_construido, bytes_de_ficha, directorio, fichas, raiz_banco, OBJETIVO};
use nb_books_lib::extensions::manifest::{self, DefectCode};
use nb_books_lib::extensions::wasm;

/// Un módulo sin ficha no se prueba, y una ficha sin módulo prueba algo que no
/// existe. Las dos mitades del mismo defecto: que el banco deje de cubrir lo
/// que dice cubrir sin que nadie se entere.
#[test]
fn el_banco_y_sus_fichas_se_corresponden_uno_a_uno() {
    let raiz = raiz_banco();
    let mut en_disco: Vec<String> = std::fs::read_dir(&raiz)
        .expect("el banco existe")
        .filter_map(Result::ok)
        .filter(|e| e.path().join("Cargo.toml").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    en_disco.sort();

    let mut declarados: Vec<String> = fichas().iter().map(directorio).collect();
    declarados.sort();

    assert_eq!(
        en_disco, declarados,
        "banco.json y los directorios del banco han dejado de corresponderse"
    );
}

#[test]
fn cada_modulo_del_banco_compila_a_webassembly_valido() {
    let Some(_) = banco_construido() else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    for ficha in fichas() {
        let nombre = directorio(&ficha);
        let bytes = bytes_de_ficha(&ficha).expect("el banco está construido");
        // Un componente y un módulo comparten firma y se distinguen en los
        // cuatro bytes siguientes. Que la prueba sepa cuál espera es lo que
        // impide que un objetivo mal puesto pase por bueno.
        let valido = if ficha["objetivo"].as_str() == Some(comun::OBJETIVO_COMPONENTE) {
            wasm::es_componente(&bytes)
        } else {
            wasm::es_modulo(&bytes)
        };
        assert!(
            valido,
            "{nombre} no empieza por la firma que le corresponde a su objetivo"
        );
    }
}

/// La comprobación que impide que este banco sea un banco que no puede fallar.
#[test]
fn cada_modulo_importa_de_verdad_la_funcion_que_dice_abusar() {
    let Some(_) = banco_construido() else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    for ficha in fichas() {
        let nombre = directorio(&ficha);
        let esperadas: Vec<&str> = ficha["importa"]
            .as_array()
            .expect("importa")
            .iter()
            .map(|v| v.as_str().expect("nombre de función"))
            .collect();
        if esperadas.is_empty() {
            continue;
        }
        let bytes = bytes_de_ficha(&ficha).expect("módulo");
        let importa = wasm::importaciones(&bytes);
        for funcion in esperadas {
            assert!(
                importa.iter().any(|i| i == funcion),
                "{nombre} dice abusar de {funcion} y no la importa: no intenta nada, \
                 así que probarlo contra el anfitrión no demostraría nada"
            );
        }
    }
}

#[test]
fn el_modulo_que_declara_una_api_futura_lleva_la_seccion_que_dice() {
    let Some(_) = banco_construido() else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let mut comprobadas = 0;
    for ficha in fichas() {
        let Some(seccion) = ficha.get("seccion") else {
            continue;
        };
        let nombre = directorio(&ficha);
        let etiqueta = seccion["nombre"].as_str().expect("nombre de sección");
        let esperado = seccion["valor"].as_str().expect("valor declarado");
        let bytes = bytes_de_ficha(&ficha).expect("módulo");
        let contenido = wasm::seccion_personalizada(&bytes, etiqueta)
            .unwrap_or_else(|| panic!("{nombre} no lleva la sección {etiqueta}"));
        assert_eq!(
            &contenido[..esperado.len()],
            esperado.as_bytes(),
            "{nombre} declara {esperado} en {etiqueta} y lleva otra cosa"
        );
        comprobadas += 1;
    }
    assert!(
        comprobadas > 0,
        "ninguna ficha declara sección: el banco ha dejado de cubrir la versión de API"
    );
}

/// El manifiesto de un módulo del banco, tal cual está en disco.
fn manifiesto_de(nombre: &str) -> String {
    std::fs::read_to_string(raiz_banco().join(nombre).join(manifest::ARCHIVO))
        .unwrap_or_else(|_| panic!("{nombre} tiene manifiesto"))
}

#[test]
fn solo_el_manifiesto_declarado_invalido_lo_es() {
    // Ahora esto lo decide el validador de verdad, el mismo del que sale
    // el esquema publicado. Antes había aquí una comprobación provisional, y el
    // banco es lo primero que ese esquema valida, como decía su ficha.
    for ficha in fichas() {
        let nombre = directorio(&ficha);
        let esperado = ficha["manifiesto"].as_str().expect("manifiesto");
        let resultado = manifest::parse(&manifiesto_de(&nombre));
        match esperado {
            "valido" => {
                resultado.unwrap_or_else(|defectos| {
                    panic!("{nombre} se declara válido y tiene defectos: {defectos:?}")
                });
            }
            "invalido" => assert!(
                resultado.is_err(),
                "{nombre} se declara inválido y pasa la validación: \
                 el caso que la validación tiene que rechazar ha dejado de existir"
            ),
            otro => {
                panic!("{nombre} declara un manifiesto {otro}, que no es ni válido ni inválido")
            }
        }
    }
}

#[test]
fn el_manifiesto_roto_del_banco_señala_sus_tres_campos_y_no_uno() {
    // Su ficha dice que tiene tres defectos a la vez, y el valor del caso está
    // justo en que se informen los tres: quien arregla uno y vuelve a fallar
    // por el siguiente hace tantas vueltas como defectos tenga.
    let defectos = manifest::parse(&manifiesto_de("manifiesto-invalido"))
        .expect_err("el manifiesto del banco está roto a propósito");
    let senalados: Vec<(DefectCode, &str)> = defectos
        .iter()
        .map(|d| (d.code, d.field.as_str()))
        .collect();
    assert_eq!(
        senalados,
        vec![
            (DefectCode::MissingField, "id"),
            (DefectCode::WrongType, "version"),
            (DefectCode::ValueNotAllowed, "type"),
        ]
    );
}

#[test]
fn cada_manifiesto_del_banco_declara_la_misma_version_que_su_modulo() {
    let Some(_) = banco_construido() else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    for ficha in fichas() {
        let nombre = directorio(&ficha);
        if ficha["manifiesto"].as_str() == Some("invalido") {
            continue;
        }
        let manifiesto = manifest::parse(&manifiesto_de(&nombre)).expect("válido");
        let bytes = bytes_de_ficha(&ficha).expect("módulo");
        // La versión se escribe en dos sitios y es fácil subir uno sin el otro.
        // Que el banco entero sea coherente es lo que hace creíble el caso de
        // version-futura, donde la incoherencia sería el defecto y no lo es.
        assert!(
            manifest::coherente_con_el_modulo(&manifiesto, &bytes).is_none(),
            "{nombre} declara {} en el manifiesto y otra cosa en el módulo",
            manifiesto.api_version
        );
    }
}

/// Dos módulos con la misma conducta cubren lo mismo dos veces y dejan un
/// hueco en otro sitio. Es la forma en que un banco se degrada sin que se note.
#[test]
fn ninguna_conducta_del_banco_esta_repetida() {
    let mut vistas: Vec<String> = fichas()
        .iter()
        .filter(|f| f["hostil"].as_bool().unwrap_or(false))
        .map(|f| f["conducta"].as_str().expect("conducta").to_string())
        .collect();
    let cuantas = vistas.len();
    vistas.sort();
    vistas.dedup();
    assert_eq!(
        cuantas,
        vistas.len(),
        "hay dos módulos con la misma conducta"
    );
    assert!(
        cuantas >= 7,
        "el banco declara {cuantas} conductas hostiles y la Fase 2 especifica siete"
    );
}

/// El par que existe: la primera versión es inofensiva y la
/// segunda no. Si las dos fueran hostiles, una revisión que rechazara siempre
/// pasaría la prueba sin distinguir nada.
#[test]
fn el_par_de_carga_remota_tiene_una_version_benigna_y_otra_no() {
    let por_directorio = |nombre: &str| -> serde_json::Value {
        fichas()
            .into_iter()
            .find(|f| directorio(f) == nombre)
            .unwrap_or_else(|| panic!("falta la ficha de {nombre}"))
    };
    assert_eq!(por_directorio("carga-remota-v1")["hostil"], false);
    assert_eq!(por_directorio("carga-remota-v2")["hostil"], true);
}
