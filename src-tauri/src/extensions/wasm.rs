//! Lector mínimo del formato WebAssembly.
//!
//! Se escribe aquí en lugar de traer una dependencia porque cabe en menos de
//! cien líneas, que es el listón de `dependency-policy.md` sección 1. Solo
//! necesita dos cosas del binario: qué importa y qué secciones personalizadas
//! lleva.
//!
//! Nació en las pruebas del banco y se mudó aquí, cuando el
//! anfitrión necesitó leer `nb:api-version` antes de compilar nada. Las
//! pruebas lo siguen usando desde aquí, así que no hay dos copias.

/// Entero LEB128 sin signo. Es como WebAssembly codifica todas sus
/// longitudes e índices.
fn leb128(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    let (mut valor, mut desplazamiento) = (0_u64, 0_u32);
    loop {
        let byte = *bytes.get(*cursor)?;
        *cursor += 1;
        valor |= u64::from(byte & 0x7f) << desplazamiento;
        if byte & 0x80 == 0 {
            return Some(valor);
        }
        desplazamiento += 7;
    }
}

fn nombre(bytes: &[u8], cursor: &mut usize) -> Option<String> {
    let largo = leb128(bytes, cursor)? as usize;
    let trozo = bytes.get(*cursor..*cursor + largo)?;
    *cursor += largo;
    Some(String::from_utf8_lossy(trozo).into_owned())
}

/// Cabecera de un módulo: la firma y la versión 1 del formato.
pub fn es_modulo(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\0asm\x01\0\0\0")
}

/// Cabecera de un componente.
///
/// Comparte la firma con un módulo y se distingue en los cuatro bytes
/// siguientes: un módulo declara la versión 1 del formato y un componente
/// declara su capa. Confundirlos es lo que hace que un módulo suelto no pueda
/// instanciarse como componente, y al revés.
pub fn es_componente(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\0asm") && bytes.get(4..8) == Some(&[0x0d, 0x00, 0x01, 0x00])
}

/// Recorre las secciones. Devuelve pares de identificador y contenido.
fn secciones(bytes: &[u8]) -> Vec<(u8, &[u8])> {
    let mut salida = Vec::new();
    let mut cursor = 8;
    while cursor < bytes.len() {
        let identificador = bytes[cursor];
        cursor += 1;
        let Some(largo) = leb128(bytes, &mut cursor) else {
            break;
        };
        let largo = largo as usize;
        let Some(contenido) = bytes.get(cursor..cursor + largo) else {
            break;
        };
        salida.push((identificador, contenido));
        cursor += largo;
    }
    salida
}

/// Nombres de las funciones que el módulo pide al anfitrión, sin el módulo
/// de procedencia: `path_open`, no `wasi_snapshot_preview1::path_open`.
pub fn importaciones(bytes: &[u8]) -> Vec<String> {
    let mut salida = Vec::new();
    for (identificador, contenido) in secciones(bytes) {
        if identificador != 2 {
            continue;
        }
        let mut cursor = 0;
        let Some(cuantas) = leb128(contenido, &mut cursor) else {
            continue;
        };
        for _ in 0..cuantas {
            if nombre(contenido, &mut cursor).is_none() {
                break;
            }
            let Some(campo) = nombre(contenido, &mut cursor) else {
                break;
            };
            let Some(&clase) = contenido.get(cursor) else {
                break;
            };
            cursor += 1;
            // El descriptor se salta: aquí solo interesan los nombres.
            match clase {
                0 => {
                    leb128(contenido, &mut cursor);
                }
                1 => {
                    cursor += 1;
                    saltar_limites(contenido, &mut cursor);
                }
                2 => saltar_limites(contenido, &mut cursor),
                _ => cursor += 2,
            }
            salida.push(campo);
        }
    }
    salida
}

fn saltar_limites(bytes: &[u8], cursor: &mut usize) {
    let banderas = leb128(bytes, cursor).unwrap_or(0);
    leb128(bytes, cursor);
    if banderas & 1 == 1 {
        leb128(bytes, cursor);
    }
}

/// Los módulos de núcleo que lleva un componente, también los de sus componentes
/// anidados.
///
/// El formato de componente enmarca sus secciones igual que un módulo -un byte de
/// identificador y un largo en LEB128-, y la sección 1 es **un módulo entero**, con
/// su cabecera. La 4 es otro componente, que puede llevar los suyos. Es todo lo que
/// hace falta para mirar dentro de un componente con las mismas reglas que un
/// módulo suelto (R4 de la política del registro).
pub fn modulos_de_componente(bytes: &[u8]) -> Vec<&[u8]> {
    let mut salida = Vec::new();
    for (identificador, contenido) in secciones(bytes) {
        match identificador {
            1 => salida.push(contenido),
            4 => salida.extend(modulos_de_componente(contenido)),
            _ => {}
        }
    }
    salida
}

/// El contenido de todas las secciones personalizadas, con su nombre delante.
pub fn secciones_personalizadas(bytes: &[u8]) -> Vec<&[u8]> {
    secciones(bytes)
        .into_iter()
        .filter(|(identificador, _)| *identificador == 0)
        .map(|(_, contenido)| contenido)
        .collect()
}

/// Contenido de una sección personalizada por su nombre.
///
/// En un módulo busca entre sus secciones personalizadas. En un componente
/// busca primero en las secciones del componente, y si no está ahí, en las
/// de los módulos que lleva dentro (R11 de la política del registro).
pub fn seccion_personalizada(bytes: &[u8], buscada: &str) -> Option<Vec<u8>> {
    for (identificador, contenido) in secciones(bytes) {
        if identificador != 0 {
            continue;
        }
        let mut cursor = 0;
        let leido = nombre(contenido, &mut cursor)?;
        if leido == buscada {
            return Some(contenido[cursor..].to_vec());
        }
    }
    if es_componente(bytes) {
        for modulo in modulos_de_componente(bytes) {
            if let Some(encontrado) = seccion_personalizada(modulo, buscada) {
                return Some(encontrado);
            }
        }
    }
    None
}
