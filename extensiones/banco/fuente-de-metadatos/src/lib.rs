//! Una fuente de metadatos aportada por una extensión.
//!
//! Consulta un servicio que responde en un formato de líneas `clave=valor`. Es
//! deliberadamente el formato más simple que sigue siendo una respuesta de
//! verdad: lo que se demuestra aquí no es el protocolo, es que una fuente nueva
//! entra por la misma puerta de red y con el mismo trato que la del núcleo.
//!
//! Propone; no escribe. Lo que devuelva pasa por la procedencia en el
//! anfitrión, así que una edición manual gana siempre.

wit_bindgen::generate!({
    path: "../../../src-tauri/wit",
    world: "metadata-provider",
});

#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

use crate::nb::extension::host::{fetch, FetchError};

/// El anfitrión al que consulta. Se declara también en su manifiesto, y sin ese
/// permiso concedido la salida se deniega antes de llegar al transporte.
const SERVICIO: &str = "https://datos.ejemplo.org/api/buscar";

struct Fuente;

impl Guest for Fuente {
    fn lookup(q: Query) -> Result<Vec<Candidate>, LookupError> {
        if q.title.trim().is_empty() {
            // Sin título no hay nada que preguntar. Lista vacía no es un error:
            // es no saber.
            return Ok(Vec::new());
        }

        let respuesta = fetch(&format!("{SERVICIO}?t={}", q.title)).map_err(|motivo| {
            // Los tres motivos se conservan tal cual: quien los reciba tiene
            // que poder distinguir "pide permiso" de "no hay red".
            match motivo {
                FetchError::Denied => LookupError::Denied,
                FetchError::Offline => LookupError::Offline,
                FetchError::Failed => LookupError::Failed,
            }
        })?;

        let cuerpo = String::from_utf8(respuesta.body).map_err(|_| LookupError::Failed)?;
        let mut candidato = Candidate {
            title: String::new(),
            authors: None,
            published_date: None,
            publisher: None,
            language: None,
        };

        for linea in cuerpo.lines() {
            let Some((clave, valor)) = linea.split_once('=') else {
                continue;
            };
            let valor = valor.trim().to_owned();
            match clave.trim() {
                "titulo" => candidato.title = valor,
                "autores" => candidato.authors = Some(valor),
                "anio" => candidato.published_date = Some(valor),
                "editorial" => candidato.publisher = Some(valor),
                "idioma" => candidato.language = Some(valor),
                _ => {}
            }
        }

        // Una ficha sin título no es una ficha. Devolverla haría que la
        // interfaz enseñara una fila en blanco que no se puede elegir.
        if candidato.title.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![candidato])
    }
}

export!(Fuente);
