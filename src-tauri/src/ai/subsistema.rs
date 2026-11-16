//! El servicio de IA vivo en la aplicación: su proveedor local, su catálogo y
//! la instalación en curso.
//!
//! Contrato: `docs/architecture/ai/local-models.md`.
//!
//! # Por qué existe aparte de `AiService`
//!
//! `AiService` es el contrato y se construye en una prueba sin montar nada.
//! Esto es lo que la aplicación necesita **además** para ofrecerlo: saber dónde
//! viven los modelos, instalar uno en segundo plano sin bloquear a nadie, avisar
//! del avance, y volver a publicar el estado del componente cuando cambia. Meterlo
//! en el contrato obligaría a cualquier prueba del contrato a traerse una base, una
//! puerta de red y un bus.
//!
//! # Una instalación a la vez
//!
//! Por el mismo motivo que la importación: dos hilos descargando el mismo modelo
//! a la misma carpeta temporal se pisarían, y el avance saltaría hacia atrás.
//! Pedir instalar con una en marcha **no es un error**: la interfaz tiene que
//! hacer lo mismo en los dos casos, enseñar el avance de la que hay.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use serde::Serialize;
use ts_rs::TS;

use super::catalogo::{self, ANFITRION};
use super::externo::{self, Configuracion, ExternalProvider, Protocolo};
use super::hardware;
use super::instalacion::{self, Instalable, Instalacion};
use super::local::LocalProvider;
use super::modelo::{self, Residente};
use super::presupuesto::{Presupuesto, Suspension};
use super::{Absence, AiProvider, AiService, Availability, Capability, Choice, Request};
use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::events::{DomainEvent, EventBus};
use crate::monitor::{components, Monitor};
use crate::net::{Network, Purpose};

/// En qué estado está un modelo del catálogo en esta instalación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "AiModelStatus"
)]
pub enum EstadoDelModelo {
    /// Instalado y verificado.
    Installed,
    /// No está, y nadie lo ha instalado.
    Available,
    /// Se instaló y ya no está entero. Se ofrece reinstalar.
    Broken,
    /// Se está descargando ahora.
    Installing,
}

/// Un modelo del catálogo, como lo enseña Ajustes.
///
/// **La licencia viaja siempre**, esté instalado o no: el criterio es que
/// el usuario la vea antes de descargar, y un dato que solo llega al pulsar
/// "descargar" llegaría tarde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "AiModel")]
pub struct VistaDelModelo {
    pub id: String,
    pub name: String,
    pub author: String,
    /// Identificador SPDX.
    pub license: String,
    /// Etiquetas BCP-47.
    pub languages: Vec<String>,
    #[ts(type = "number")]
    pub disk_bytes: u64,
    #[ts(type = "number")]
    pub memory_bytes: u64,
    pub status: EstadoDelModelo,
    /// De dónde se descarga. Es el primer anfitrión que hay que autorizar.
    pub host: String,
    /// Qué se envía al descargar, en lenguaje llano. Sale del propósito de red.
    pub what_is_sent: String,
    /// Si cabe en el disco con su margen. `None` si no se pudo saber.
    pub fits: Option<bool>,
    /// Si el tope de memoria vigente lo deja sin cargar. Lo decide el
    /// runtime, que es quien lo hace cumplir, y no la interfaz comparando cifras.
    pub exceeds_memory_limit: bool,
    #[ts(type = "number | null")]
    pub free_bytes: Option<u64>,
    /// Si genera texto en lugar de vectores. Ajustes dice para qué sirve cada
    /// uno: descargar un gigabyte sin saber qué hace es firmar en blanco.
    pub generates: bool,
    /// Si es el que se recomienda **para este equipo**, leído de la máquina.
    pub recommended: bool,
    /// Si es el que se usa ahora para generar. Solo uno a la vez.
    pub in_use: bool,
    /// Qué clase de salida da, medido: `fiel`, `prosa` o `vectores`. La interfaz lo
    /// traduce, y con él dice qué gana y qué pierde cada uno.
    pub trait_measured: String,
    /// Memoria y núcleos desde los que se recomienda, para poder decir por qué no.
    #[ts(type = "number")]
    pub needs_memory_bytes: u64,
    pub needs_cores: u32,
    /// Si **esta compilación** lo ejecuta despacio aunque la máquina diera para más:
    /// un modelo que genera sin AVX2 compilado en x86-64. Ver
    /// `generativo::motor_rapido`.
    ///
    /// Antes se llamaba `unsupported_build` y **quitaba el botón de
    /// descargar**. Ese día se fijó que la aplicación recomienda y no obliga, así
    /// que ahora se ofrece igual y se dice cuánto tardará.
    pub slow_build: bool,
    /// Segundos por función de estudio **en este equipo**, estimados a partir de lo
    /// medido en la máquina de referencia. Nulo en un modelo que no genera. Enteros
    /// para que la vista siga siendo comparable por igualdad.
    #[ts(type = "number | null")]
    pub estimated_seconds: Option<u32>,
    /// Lo que hay que saber antes de elegirlo: `EXCEEDS_MACHINE_MEMORY`,
    /// `SLOW_BUILD`, `BELOW_RECOMMENDED`, `CITES_WORSE`. Ver `catalogo::prevision`.
    ///
    /// Cota: son cuatro códigos posibles, y cada uno aparece una vez como mucho.
    pub warnings: Vec<String>,
    /// El tope de memoria más pequeño de los admitidos con el que **sí** se podría
    /// cargar, si con el de ahora no. Es lo que permite ofrecer "subir el tope y
    /// usarlo" en un paso, en vez de decir que no cabe y dejar a la persona buscando
    /// dónde se cambia.
    #[ts(type = "number | null")]
    pub suggested_limit_mb: Option<u32>,
}

/// Lo que Ajustes necesita para la sección entera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "AiModels")]
pub struct VistaDeModelos {
    /// Si este equipo da para la inferencia local. Sin eso no se ofrece descargar
    /// nada: bajar doscientos megabytes que no se van a poder ejecutar es peor
    /// que no ofrecerlo.
    pub machine_supported: bool,
    #[ts(type = "number | null")]
    pub machine_memory_bytes: Option<u64>,
    /// Cota: el catálogo, compilado en el binario.
    pub models: Vec<VistaDelModelo>,
    /// La instalación en curso, si la hay: modelo, bytes hechos y total.
    pub installing: Option<InstalacionEnCurso>,
    /// Núcleos de este equipo. De ahí salen los hilos que Ajustes puede ofrecer:
    /// nunca todos.
    pub cores: u32,
    /// Cuántos hilos es "automático" en este equipo. Lo resuelve el runtime para
    /// que la interfaz no repita la regla: ver `hardware::hilos_para_inferencia`.
    pub automatic_threads: u32,
    /// El tope de memoria vigente, en bytes.
    #[ts(type = "number")]
    pub memory_limit_bytes: u64,
    /// Por qué el trabajo de fondo de la IA está esperando ahora, si lo está:
    /// `AI_SUSPENDED_BATTERY` o `AI_SUSPENDED_HEAT`.
    pub background_pause: Option<&'static str>,
    /// Dónde calcula la IA en este equipo con esta edición: `cpu`, `cuda` o
    /// `metal`. **Preguntado al motor**, no deducido de la compilación: ver
    /// `hardware::acelerador`. Ajustes lo dice, porque de ahí sale casi toda la
    /// diferencia de velocidad entre equipos.
    pub accelerator: String,
    /// Si esta edición trae la versión rápida de los núcleos del procesador (AVX2
    /// en x86-64). Con `false`, Ajustes señala la edición rápida: se recomienda, no
    /// se impone.
    pub fast_build: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "AiInstallProgress"
)]
pub struct InstalacionEnCurso {
    pub model_id: String,
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
}

/// El subsistema de IA de la aplicación.
pub struct Subsistema {
    servicio: RwLock<AiService>,
    /// El modelo del proveedor local, para cambiarle el presupuesto sin buscarlo
    /// dentro del servicio.
    residente: Arc<Residente>,
    modelos: PathBuf,
    monitor: Monitor,
    cancelado: AtomicBool,
    en_curso: Mutex<Option<InstalacionEnCurso>>,
    /// El último anfitrión que la instalación pidió autorizar.
    ///
    /// Es lo que impide que el webview autorice lo que quiera: solo se concede
    /// el anfitrión del catálogo o el que el propio runtime acaba de proponer al
    /// seguir una redirección. El mismo criterio que la consulta en línea.
    propuesto: Mutex<Option<String>>,
    /// El proveedor externo configurado, si lo hay. Está también
    /// registrado en el servicio; se guarda aquí para que Ajustes pueda enseñar qué
    /// enviaría antes de enviarlo.
    externo: RwLock<Option<Arc<ExternalProvider>>>,
    /// Si se pidió cortar la función de estudio en curso. Compartido con el
    /// modelo generativo local, que lo mira entre token y token.
    estudio_cancelado: Arc<AtomicBool>,
    /// El modelo que genera en local, para cambiarle el presupuesto.
    generativo: Arc<super::generativo::GenerativoLocal>,
}

/// El identificador de algo que se pueda instalar, venga del catálogo que venga.
///
/// Son dos catálogos y una sola disposición de instalación. El de la voz no es un
/// modelo de IA —no tiene capacidad, ni dimensión, ni agrupación— y necesita
/// exactamente la misma garantía: huella revisada, carpeta temporal y renombrado
/// atómico. Tener dos instaladores sería tener dos sitios donde equivocarse con
/// los bytes que después se ejecutan.
fn instalable(id: &str) -> Option<&'static (dyn Instalable + Sync)> {
    if let Some(modelo) = catalogo::modelo(id) {
        return Some(modelo);
    }
    (id == crate::voice::catalogo::KOKORO_82M.id)
        .then_some(&crate::voice::catalogo::KOKORO_82M as &'static (dyn Instalable + Sync))
}

impl Subsistema {
    /// Detecta la máquina, registra el proveedor local y publica el estado.
    ///
    /// `presupuesto` es el de las preferencias guardadas. Ver
    /// [`Presupuesto::desde`].
    #[must_use]
    pub fn arrancar(modelos: &Path, monitor: Monitor, presupuesto: Presupuesto) -> Self {
        let mut servicio = AiService::new().con_monitor(monitor.clone());
        let local = LocalProvider::detectar(modelos, presupuesto);
        let residente = local.residente();
        servicio.register(Box::new(local));
        let estudio_cancelado = Arc::new(AtomicBool::new(false));
        let generativo = Arc::new(super::generativo::GenerativoLocal::nuevo(
            modelos,
            presupuesto,
            Arc::clone(&estudio_cancelado),
        ));
        servicio.register(Box::new(super::generativo::Compartido(Arc::clone(
            &generativo,
        ))));

        let subsistema = Self {
            servicio: RwLock::new(servicio),
            residente,
            modelos: modelos.to_path_buf(),
            monitor,
            cancelado: AtomicBool::new(false),
            en_curso: Mutex::new(None),
            propuesto: Mutex::new(None),
            externo: RwLock::new(None),
            estudio_cancelado,
            generativo,
        };
        subsistema.publicar_estado();
        subsistema
    }

    /// Vuelve a calcular el estado del componente y lo publica.
    pub fn publicar_estado(&self) {
        let (estado, motivo) = self
            .servicio
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .component_state();
        self.monitor.set(components::AI, estado, motivo);
    }

    /// Cambia el presupuesto de la inferencia en caliente y publica el estado, que
    /// puede cambiar: un tope por debajo del modelo lo deja sin ofrecer.
    pub fn aplicar_presupuesto(&self, presupuesto: Presupuesto) {
        self.residente.aplicar(presupuesto);
        self.generativo.aplicar(presupuesto);
        self.publicar_estado();
    }

    /// Si el trabajo de fondo de los embeddings tiene que esperar ahora.
    #[must_use]
    pub fn pausa_de_fondo(&self) -> Option<Suspension> {
        self.servicio
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .pausa_de_fondo(super::Capability::Embedding, &super::Choice::Auto)
    }

    /// Si este equipo da para la inferencia local.
    fn maquina_capaz(&self) -> bool {
        !matches!(
            self.servicio
                .read()
                .unwrap_or_else(|p| p.into_inner())
                .availability(
                    super::Capability::Embedding,
                    &super::Choice::Provider(super::local::ID.to_owned())
                ),
            Availability::Absent(Absence::NotSupported)
        )
    }

    /// Lo que Ajustes enseña.
    #[must_use]
    pub fn vista(&self) -> VistaDeModelos {
        let en_curso = self
            .en_curso
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let libre = hardware::disco_libre(self.raiz_existente());
        let presupuesto = self.residente.presupuesto();
        let recursos = hardware::detectar();

        let (en_uso, recomendado) = self.modelos_generativos();
        let models = catalogo::CATALOGO
            .iter()
            .map(|modelo| {
                let status = if en_curso.as_ref().is_some_and(|c| c.model_id == modelo.id) {
                    EstadoDelModelo::Installing
                } else if instalacion::instalado(&self.modelos, modelo) {
                    EstadoDelModelo::Installed
                } else if instalacion::hay_rastro(&self.modelos, modelo) {
                    EstadoDelModelo::Broken
                } else {
                    EstadoDelModelo::Available
                };
                let prevision = self.generativo.prevision(modelo);
                VistaDelModelo {
                    id: modelo.id.to_owned(),
                    name: modelo.nombre.to_owned(),
                    author: modelo.autor.to_owned(),
                    license: modelo.licencia.to_owned(),
                    languages: modelo.idiomas.iter().map(|&i| i.to_owned()).collect(),
                    disk_bytes: modelo.disco(),
                    memory_bytes: modelo.memoria,
                    status,
                    host: ANFITRION.to_owned(),
                    what_is_sent: Purpose::Model.what_is_sent().to_owned(),
                    fits: libre.map(|l| instalacion::cabe(l, modelo)),
                    free_bytes: libre,
                    exceeds_memory_limit: !modelo::cabe(modelo, presupuesto),
                    generates: modelo.genera(),
                    trait_measured: modelo.rasgo.as_str().to_owned(),
                    recommended: recomendado == Some(modelo.id),
                    in_use: modelo.genera() && en_uso == modelo.id,
                    needs_memory_bytes: modelo.equipo_recomendado.memoria,
                    needs_cores: u32::try_from(modelo.equipo_recomendado.nucleos)
                        .unwrap_or(u32::MAX),
                    slow_build: modelo.genera() && !super::generativo::motor_rapido(),
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    estimated_seconds: prevision.minutos.map(|m| (m * 60.0).round() as u32),
                    warnings: prevision.avisos.iter().map(|a| (*a).to_owned()).collect(),
                    suggested_limit_mb: (!modelo::cabe(modelo, presupuesto))
                        .then(|| tope_que_cabe(modelo, presupuesto))
                        .flatten(),
                }
            })
            .collect();

        VistaDeModelos {
            machine_supported: self.maquina_capaz(),
            machine_memory_bytes: recursos.memoria_total,
            models,
            installing: en_curso,
            cores: u32::try_from(recursos.nucleos).unwrap_or(u32::MAX),
            automatic_threads: u32::try_from(hardware::hilos_para_inferencia(recursos.nucleos))
                .unwrap_or(u32::MAX),
            memory_limit_bytes: presupuesto.memoria,
            background_pause: self.pausa_de_fondo().map(Suspension::reason_code),
            accelerator: recursos.acelerador.to_string(),
            fast_build: super::generativo::motor_rapido(),
        }
    }

    /// La carpeta de modelos, o la primera antecesora que exista: el espacio libre
    /// se mide en el sistema de archivos donde irá, y en una instalación recién
    /// hecha la carpeta todavía no existe.
    fn raiz_existente(&self) -> &Path {
        let mut ruta = self.modelos.as_path();
        while !ruta.exists() {
            match ruta.parent() {
                Some(padre) => ruta = padre,
                None => break,
            }
        }
        ruta
    }

    /// Empieza a instalar un modelo en segundo plano.
    ///
    /// Devuelve `true` si ya había una instalación en marcha, que no es un error.
    /// El avance llega por `AiModelProgress` y el final por `AiModelFinished`.
    ///
    /// # Errors
    ///
    /// - [`NbError::DocumentNotFound`] si el identificador no es del catálogo.
    /// - [`NbError::ComponentUnavailable`] si el equipo no da para ejecutarlo:
    ///   descargar lo que no se va a poder usar es peor que no hacerlo.
    pub fn instalar(
        self: &Arc<Self>,
        id: &str,
        db: Database,
        red: Arc<Network>,
        bus: EventBus,
    ) -> NbResult<bool> {
        let modelo = instalable(id).ok_or(NbError::DocumentNotFound)?;
        if !self.maquina_capaz() {
            return Err(NbError::ComponentUnavailable);
        }

        {
            let mut en_curso = self.en_curso.lock().unwrap_or_else(|p| p.into_inner());
            if en_curso.is_some() {
                return Ok(true);
            }
            *en_curso = Some(InstalacionEnCurso {
                model_id: modelo.id().to_owned(),
                done: 0,
                total: modelo.disco(),
            });
        }
        self.cancelado.store(false, Ordering::SeqCst);

        let subsistema = Arc::clone(self);
        let hilo = std::thread::Builder::new()
            .name("nb-modelos".to_owned())
            .spawn(move || subsistema.trabajar(modelo, &db, &red, &bus));

        if hilo.is_err() {
            *self.en_curso.lock().unwrap_or_else(|p| p.into_inner()) = None;
            return Err(NbError::Internal);
        }
        Ok(false)
    }

    fn trabajar(
        &self,
        modelo: &'static (dyn Instalable + Sync),
        db: &Database,
        red: &Network,
        bus: &EventBus,
    ) {
        let mut ultimo_aviso = 0_u64;
        // Un aviso por cada megabyte y no por cada bloque: a 64 KiB por bloque
        // serían tres mil cuatrocientos eventos por el puente para una barra que
        // no se distingue a esa resolución.
        const CADA: u64 = 1024 * 1024;

        let resultado = instalacion::instalar(
            db,
            red,
            &self.modelos,
            modelo,
            &self.cancelado,
            &mut |hecho, total| {
                if let Some(en_curso) = self
                    .en_curso
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .as_mut()
                {
                    en_curso.done = hecho;
                }
                if hecho == total || hecho >= ultimo_aviso + CADA {
                    ultimo_aviso = hecho;
                    bus.emit(DomainEvent::AiModelProgress {
                        model_id: modelo.id().to_owned(),
                        done: hecho,
                        total,
                    });
                }
            },
        );

        *self.en_curso.lock().unwrap_or_else(|p| p.into_inner()) = None;

        let (outcome, host, reason_code) = match resultado {
            Ok(Instalacion::Instalado) => ("installed", None, None),
            Ok(Instalacion::FaltaPermiso { host }) => {
                *self.propuesto.lock().unwrap_or_else(|p| p.into_inner()) = Some(host.clone());
                ("needsConsent", Some(host), None)
            }
            Err(NbError::Cancelled) => ("cancelled", None, None),
            Err(error) => ("failed", None, Some(error.code())),
        };

        self.publicar_estado();
        bus.emit(DomainEvent::AiModelFinished {
            model_id: modelo.id().to_owned(),
            outcome,
            host,
            reason_code,
        });
    }

    /// Pide cortar la instalación en curso. Sin ninguna, no hace nada.
    pub fn cancelar(&self) {
        self.cancelado.store(true, Ordering::SeqCst);
    }

    /// Quita un modelo instalado y publica el estado nuevo.
    ///
    /// # Errors
    ///
    /// - [`NbError::DocumentNotFound`] si el identificador no es del catálogo.
    /// - [`NbError::StorageFailure`] si no se puede borrar, o si se está
    ///   instalando ese mismo modelo: borrar su carpeta mientras el hilo escribe
    ///   dejaría al hilo renombrando una carpeta que ya no existe.
    pub fn desinstalar(&self, id: &str) -> NbResult<()> {
        let modelo = instalable(id).ok_or(NbError::DocumentNotFound)?;
        if self
            .en_curso
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .is_some_and(|c| c.model_id == modelo.id())
        {
            return Err(NbError::StorageFailure);
        }
        instalacion::desinstalar(&self.modelos, modelo)?;
        self.publicar_estado();
        Ok(())
    }

    /// Autoriza la salida a un anfitrión **que el runtime propuso**.
    ///
    /// # Errors
    ///
    /// - [`NbError::PathNotAllowed`] si no es el anfitrión del catálogo ni el que
    ///   la última instalación pidió autorizar.
    /// - [`NbError::StorageFailure`] si la escritura falla.
    pub fn autorizar(&self, db: &Database, host: &str) -> NbResult<()> {
        let anfitrion = crate::net::host_de(&format!("https://{host}/"))?;
        let propuesto = self
            .propuesto
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let reconocido = anfitrion == ANFITRION || propuesto.as_deref() == Some(anfitrion.as_str());
        if !reconocido {
            tracing::info!("se pidió autorizar para modelos un anfitrión que nadie propuso");
            return Err(NbError::PathNotAllowed);
        }
        let conn = db.connection()?;
        crate::net::grant(&conn, &anfitrion, Purpose::Model)?;
        tracing::info!("el usuario autorizó un destino de descarga de modelos");
        Ok(())
    }
}

/// El proveedor externo, como lo enseña Ajustes.
///
/// **Nunca la credencial**: si la hay y si cabe guardarla, y nada más. Es la misma
/// regla que `CredentialStatus` de la voz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "AiProviderStatus"
)]
pub struct VistaDelProveedor {
    /// `none`, `openai` o `anthropic`.
    pub protocol: String,
    /// Si hay protocolo, dirección válida y modelo: si hay proveedor activo.
    pub configured: bool,
    /// El anfitrión al que se saldría, si la dirección escrita vale.
    pub host: Option<String>,
    /// La dirección exacta a la que se manda.
    pub endpoint: Option<String>,
    pub model: String,
    /// Si hay una dirección escrita que no vale: en claro hacia internet, o sin
    /// anfitrión. Se dice al escribirla y no al enviar.
    pub invalid_address: bool,
    /// Si ya se autorizó salir a ese anfitrión con este propósito.
    pub authorized: bool,
    pub keyring_available: bool,
    pub credential_stored: bool,
    /// Qué sale del equipo, en lenguaje llano. Sale del propósito de red.
    pub what_is_sent: String,
}

/// Lo que pasó al probar el proveedor externo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "outcome",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "AiProbe")]
pub enum Prueba {
    /// No hay proveedor externo configurado.
    NotConfigured,
    /// **Es el primer envío a ese anfitrión**: no se ha enviado nada, y esto es
    /// exactamente lo que se enviaría.
    NeedsConsent {
        host: String,
        what_is_sent: String,
        /// El mensaje entero, sin la credencial, que va en una cabecera.
        body: String,
    },
    /// Respondió, y esto es lo que dijo.
    Answered {
        host: String,
        model: String,
        text: String,
    },
    /// Salió y el servicio lo rechazó: una clave que no vale, un modelo que no
    /// existe, un límite de uso.
    Rejected { host: String, status: u16 },
}

/// La instrucción de la prueba. Corta, sin nada de ningún documento, y con una
/// respuesta que se puede comprobar a simple vista.
const INSTRUCCION_DE_PRUEBA: &str =
    "Esto es una prueba de conexión de NB Books. Responde únicamente con la palabra: listo.";

/// Cuánto de la respuesta de la prueba se enseña, en caracteres.
const MAXIMO_DE_RESPUESTA_DE_PRUEBA: usize = 400;

impl Subsistema {
    /// Vuelve a leer la configuración del proveedor externo y lo registra, lo
    /// sustituye o lo quita, **sin reiniciar**.
    ///
    /// Quitar la dirección en Ajustes tiene que dejar de permitir enviar en ese
    /// momento: un proveedor que siguiera registrado hasta el siguiente arranque
    /// seguiría mandando texto a un sitio que alguien acaba de borrar.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si las preferencias no se pueden leer. Una
    /// dirección que no vale **no es un error**: no hay proveedor, y Ajustes lo dice.
    pub fn configurar_externo(&self, db: &Database, red: &Arc<Network>) -> NbResult<()> {
        let conn = db.connection()?;
        let configuracion = Configuracion::guardada(&conn)?;
        drop(conn);

        let nuevo = configuracion.and_then(|c| {
            ExternalProvider::nuevo(c, db.clone(), Arc::clone(red))
                .map_err(|_| tracing::info!("la dirección del proveedor de IA externo no vale"))
                .ok()
                .map(Arc::new)
        });

        {
            let mut servicio = self.servicio.write().unwrap_or_else(|p| p.into_inner());
            servicio.quitar(externo::ID);
            if let Some(proveedor) = &nuevo {
                servicio.register(Box::new(Arc::clone(proveedor)));
            }
        }
        *self.externo.write().unwrap_or_else(|p| p.into_inner()) = nuevo;
        self.publicar_estado();
        Ok(())
    }

    fn externo(&self) -> Option<Arc<ExternalProvider>> {
        self.externo
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// A quién le pide una función que genera texto.
    ///
    /// Al externo solo si está configurado, y configurarlo es la elección
    /// consciente que exige la regla 2 de `ai/philosophy.md`. Sin él, lo local, que
    /// hoy no genera: la función no se ofrece. **Nunca al revés**: un fallo del
    /// externo no pasa la petición a otro sitio.
    #[must_use]
    pub fn eleccion(&self) -> Choice {
        if self.externo().is_some() {
            Choice::Provider(externo::ID.to_owned())
        } else {
            Choice::Auto
        }
    }

    /// Qué hay para una capacidad con una elección, preguntado al servicio.
    ///
    /// Es la pregunta que hará una función antes de ofrecerse, y la que comprueba
    /// que quitar el proveedor externo lo quita **del servicio** y no solo de
    /// Ajustes.
    #[must_use]
    pub fn disponibilidad(&self, capability: Capability, choice: &Choice) -> Availability {
        self.servicio
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .availability(capability, choice)
    }

    /// Lo que Ajustes enseña del proveedor externo.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la base no responde.
    pub fn vista_del_proveedor(&self, db: &Database) -> NbResult<VistaDelProveedor> {
        let conn = db.connection()?;
        let texto = |clave| match crate::settings::get(&conn, clave) {
            Ok(crate::settings::SettingValue::Text(valor)) => Ok(valor.trim().to_owned()),
            Ok(_) => Ok(String::new()),
            Err(error) => Err(error),
        };
        let protocolo = texto(crate::settings::SettingKey::AiProvider)?;
        let escrito = texto(crate::settings::SettingKey::AiHost)?;
        let model = texto(crate::settings::SettingKey::AiModel)?;

        let host = if escrito.is_empty() {
            None
        } else {
            crate::net::host_de(&crate::net::direccion_escrita(&escrito)).ok()
        };
        let endpoint = Protocolo::desde(&protocolo)
            .filter(|_| host.is_some())
            .map(|p| {
                Configuracion {
                    protocolo: p,
                    host: escrito.clone(),
                    modelo: model.clone(),
                }
                .punto()
            });
        let authorized = match &host {
            Some(h) => crate::net::has_consent(&conn, h, Purpose::Ai)?,
            None => false,
        };
        drop(conn);

        let keyring_available = crate::secrets::disponible();
        let credential_stored = match (&host, keyring_available) {
            (Some(h), true) => crate::secrets::leer(&crate::secrets::cuenta_de_ia(h))
                .map(|c| c.is_some())
                .unwrap_or(false),
            _ => false,
        };

        Ok(VistaDelProveedor {
            protocol: protocolo,
            configured: self.externo().is_some(),
            invalid_address: !escrito.is_empty() && host.is_none(),
            host,
            endpoint,
            model,
            authorized,
            keyring_available,
            credential_stored,
            what_is_sent: Purpose::Ai.what_is_sent().to_owned(),
        })
    }

    /// Prueba el proveedor externo con una petición mínima.
    ///
    /// **Si es el primer envío a ese anfitrión no envía nada**: devuelve el mensaje
    /// entero que enviaría, para que el usuario lo vea y decida. Es el criterio de
    /// aceptación, y no depende de que la interfaz se acuerde: sin
    /// consentimiento, la puerta de red no deja salir.
    ///
    /// # Errors
    ///
    /// - [`NbError::NetworkDisabled`] con el modo sin red puesto.
    /// - [`NbError::NetworkFailure`] si no se pudo hablar con el servicio.
    /// - [`NbError::StorageFailure`] si la base no responde.
    pub fn probar(&self, db: &Database) -> NbResult<Prueba> {
        let Some(proveedor) = self.externo() else {
            return Ok(Prueba::NotConfigured);
        };
        let peticion = Request {
            capability: Capability::Summary,
            instruction: INSTRUCCION_DE_PRUEBA.to_owned(),
            passages: Vec::new(),
            task: String::new(),
            language: None,
        };
        let carta = externo::componer(proveedor.configuracion(), &peticion)?;
        let host = proveedor.anfitrion().to_owned();

        let conn = db.connection()?;
        if crate::net::is_offline(&conn)? {
            return Err(NbError::NetworkDisabled);
        }
        if !crate::net::has_consent(&conn, &host, Purpose::Ai)? {
            // Legible y no compacto: es el mismo mensaje, con saltos de línea para
            // poder leerlo antes de decidir.
            let body = serde_json::from_str::<serde_json::Value>(&carta.cuerpo)
                .and_then(|v| serde_json::to_string_pretty(&v))
                .unwrap_or(carta.cuerpo);
            return Ok(Prueba::NeedsConsent {
                host,
                what_is_sent: Purpose::Ai.what_is_sent().to_owned(),
                body,
            });
        }
        drop(conn);

        let respuesta = proveedor.enviar(&peticion)?;
        Ok(match respuesta.texto {
            Some(texto) => Prueba::Answered {
                host,
                model: proveedor.configuracion().modelo.clone(),
                text: texto.chars().take(MAXIMO_DE_RESPUESTA_DE_PRUEBA).collect(),
            },
            None => Prueba::Rejected {
                host,
                status: respuesta.status,
            },
        })
    }

    /// Autoriza salir al proveedor externo **configurado**.
    ///
    /// No recibe a qué anfitrión, por lo mismo que la voz: si el webview pudiera
    /// decirlo, podría autorizar cualquier destino con el permiso de la IA.
    ///
    /// # Errors
    ///
    /// - [`NbError::PathNotAllowed`] si no hay proveedor externo configurado.
    /// - [`NbError::StorageFailure`] si la escritura falla.
    pub fn autorizar_externo(&self, db: &Database) -> NbResult<()> {
        let Some(proveedor) = self.externo() else {
            tracing::info!("se pidió autorizar un proveedor de IA externo sin configurar");
            return Err(NbError::PathNotAllowed);
        };
        let conn = db.connection()?;
        crate::net::grant(&conn, proveedor.anfitrion(), Purpose::Ai)?;
        tracing::info!("el usuario autorizó un proveedor de IA externo");
        Ok(())
    }

    /// El anfitrión de la dirección escrita en Ajustes, para guardar o borrar su
    /// credencial. Aparte del proveedor porque la clave se puede guardar antes de
    /// escribir el modelo.
    ///
    /// # Errors
    ///
    /// - [`NbError::PathNotAllowed`] si no hay dirección o no vale.
    /// - [`NbError::StorageFailure`] si la base no responde.
    pub fn anfitrion_escrito(db: &Database) -> NbResult<String> {
        let conn = db.connection()?;
        let escrito = match crate::settings::get(&conn, crate::settings::SettingKey::AiHost)? {
            crate::settings::SettingValue::Text(valor) => valor,
            _ => String::new(),
        };
        if escrito.trim().is_empty() {
            return Err(NbError::PathNotAllowed);
        }
        crate::net::host_de(&crate::net::direccion_escrita(&escrito))
    }
}

/// El subsistema calcula los vectores de la generación de fondo con el
/// proveedor local, y nunca con uno externo: `Choice::Auto` solo mira lo local.
impl super::generacion::Vectorizador for Subsistema {
    fn modelo(&self) -> Option<&'static str> {
        self.disponibilidad(Capability::Embedding, &Choice::Auto)
            .is_ready()
            .then(|| self.residente.modelo())
    }

    fn vectores(&self, textos: &[String]) -> NbResult<Vec<super::Embedding>> {
        self.servicio
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .embed(textos, &Choice::Auto)
    }

    fn pausa(&self) -> Option<Suspension> {
        self.pausa_de_fondo()
    }
}

/// Quién generaría las funciones de estudio, como lo enseña cada pantalla de IA.
///
/// Existe por la sección 6 de `ai/privacy.md`: **el proveedor activo se ve en toda
/// pantalla de IA**, y con él si el texto sale del equipo y a dónde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "StudyAvailability"
)]
pub struct DisponibilidadDeEstudio {
    /// `local` o `external`: lo que eligió la persona.
    pub generator: String,
    pub ready: bool,
    /// Por qué no, con el código de [`Absence::reason_code`].
    pub reason: Option<&'static str>,
    pub provider_label: Option<String>,
    pub left_device: bool,
    /// A dónde sale, si sale.
    pub host: Option<String>,
    /// Si ya se autorizó salir a ese anfitrión.
    pub authorized: bool,
    /// Qué se envía, en lenguaje llano.
    pub what_is_sent: String,
    /// **Qué funciones puede hacer quien genera ahora.**
    ///
    /// Con el modelo local son las cuatro ahora: la autoevaluación ya
    /// no se esconde, se desaconseja -ver `discouraged`-. La interfaz ofrece esto y no
    /// una lista fija.
    pub functions: Vec<super::estudio::Funcion>,
    /// Las que se ofrecen **sin recomendarlas**, con el código de por qué.
    ///
    /// Se decidió el 2026-09-22: "una cosa es recomendar y otra obligar". Lo
    /// medido deja de ser motivo para esconder una función y pasa a ser lo que se le
    /// dice a quien la elige. Cota: una por función como mucho.
    pub discouraged: Vec<FuncionDesaconsejada>,
    /// Cuántos segundos es probable que tarde una función **en este equipo**, si
    /// genera el modelo local. Estimado de lo medido: ver `catalogo::prevision`.
    pub estimated_seconds: Option<u32>,
    /// Lo que la previsión del modelo local avisa -`SLOW_BUILD`, `BELOW_RECOMMENDED`,
    /// `EXCEEDS_MACHINE_MEMORY`-, para decirlo **antes** de pulsar generar.
    /// Cota: cuatro códigos posibles, una vez cada uno.
    pub warnings: Vec<String>,
}

/// Una función que se ofrece y no se recomienda, con su motivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "DiscouragedFunction"
)]
pub struct FuncionDesaconsejada {
    pub function: super::estudio::Funcion,
    pub reason: &'static str,
}

impl Subsistema {
    /// Quién generaría ahora, y si puede.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la base no responde.
    pub fn disponibilidad_de_estudio(
        &self,
        db: &Database,
        _red: &Arc<Network>,
    ) -> NbResult<DisponibilidadDeEstudio> {
        let eleccion = eleccion_para_generar(db)?;
        let generator = if matches!(eleccion, Choice::Auto) {
            "local"
        } else {
            "external"
        };
        let disponible = self.disponibilidad(Capability::Summary, &eleccion);
        let (ready, reason, info) = match disponible {
            Availability::Ready(info) => (true, None, Some(info)),
            Availability::Absent(ausencia) => (false, Some(ausencia.reason_code()), None),
        };
        let host = info.as_ref().and_then(|i| i.host.clone());
        let authorized = match &host {
            Some(h) => {
                let conn = db.connection()?;
                crate::net::has_consent(&conn, h, Purpose::Ai)?
            }
            None => false,
        };
        let funciones = [
            super::estudio::Funcion::Summary,
            super::estudio::Funcion::Flashcards,
            super::estudio::Funcion::Questions,
            super::estudio::Funcion::Concepts,
        ];
        let functions = funciones
            .into_iter()
            .filter(|f| {
                super::estudio::plantilla_de(*f)
                    .is_some_and(|p| self.disponibilidad(p.capacidad, &eleccion).is_ready())
            })
            .collect();

        // Lo desaconsejado y la previsión solo tienen sentido con el modelo local:
        // de un proveedor externo no se ha medido nada aquí, y decir una cifra sería
        // inventarla.
        let local = generator == "local";
        let discouraged = if local {
            super::generativo::DESACONSEJADAS
                .iter()
                .filter_map(|(capacidad, motivo)| {
                    funciones
                        .iter()
                        .find(|f| {
                            super::estudio::plantilla_de(**f)
                                .is_some_and(|p| p.capacidad == *capacidad)
                        })
                        .map(|f| FuncionDesaconsejada {
                            function: *f,
                            reason: motivo,
                        })
                })
                .collect()
        } else {
            Vec::new()
        };
        let prevision = local.then(|| self.generativo.prevision(self.generativo.elegido()));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let estimated_seconds = prevision
            .as_ref()
            .and_then(|p| p.minutos)
            .map(|m| (m * 60.0).round() as u32);
        let warnings = prevision
            .map(|p| p.avisos.iter().map(|a| (*a).to_owned()).collect())
            .unwrap_or_default();

        Ok(DisponibilidadDeEstudio {
            generator: generator.to_owned(),
            functions,
            discouraged,
            estimated_seconds,
            warnings,
            ready,
            reason,
            left_device: info
                .as_ref()
                .is_some_and(|i| i.placement.sale_del_dispositivo()),
            provider_label: info.map(|i| i.label),
            host,
            authorized,
            what_is_sent: Purpose::Ai.what_is_sent().to_owned(),
        })
    }

    /// Hace una función de estudio con quien eligió la persona.
    ///
    /// # Errors
    ///
    /// Los de [`super::estudio::hacer`], y [`NbError::Cancelled`] si se canceló
    /// mientras generaba.
    pub fn estudiar(
        &self,
        db: &Database,
        documentos: &dyn super::generacion::Documentos,
        funcion: super::estudio::Funcion,
        alcance: &super::estudio::AlcanceDeEstudio,
    ) -> NbResult<super::estudio::Generacion> {
        let eleccion = eleccion_para_generar(db)?;
        let idioma = {
            let conn = db.connection()?;
            match crate::settings::get(&conn, crate::settings::SettingKey::Language)? {
                crate::settings::SettingValue::Text(t) => t,
                _ => String::new(),
            }
        };
        self.estudio_cancelado.store(false, Ordering::SeqCst);
        let generadora = GeneradoraElegida {
            subsistema: self,
            eleccion,
            capacidad: super::estudio::plantilla_de(funcion)
                .map_or(Capability::Summary, |p| p.capacidad),
        };
        let resultado = super::estudio::hacer(documentos, &generadora, funcion, alcance, &idioma);
        // Un proveedor externo no se puede cortar a mitad de respuesta: lo que
        // llegue después de cancelar se tira en lugar de enseñarse.
        if self.estudio_cancelado.swap(false, Ordering::SeqCst) {
            return Err(NbError::Cancelled);
        }
        resultado
    }

    /// Estima los caracteres y tokens aproximados que saldrían para un alcance.
    ///
    /// # Errors
    ///
    /// Los de [`super::estudio::estimar`].
    pub fn estimar_alcance(
        &self,
        db: &Database,
        documentos: &dyn super::generacion::Documentos,
        alcance: &super::estudio::AlcanceDeEstudio,
    ) -> NbResult<super::estudio::EstimacionDeAlcance> {
        let eleccion = eleccion_para_generar(db)?;
        let max_caracteres = self
            .servicio
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .input_chars(Capability::Summary, &eleccion)
            .unwrap_or(super::CARACTERES_DE_ENTRADA);
        super::estudio::estimar(documentos, alcance, max_caracteres)
    }

    /// Qué modelo generativo se usa, leído de las preferencias.
    ///
    /// Vacío significa "el que se recomienda para este equipo", que es lo que hace la
    /// aplicación recién instalada. Elegir otro es de quien tiene la máquina: es lo
    /// único que no se puede igualar entre usuarios, y por eso el catálogo ofrece
    /// varios (decisión de producto, 2026-09-17).
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si las preferencias no se pueden leer.
    pub fn aplicar_modelo_generativo(&self, db: &Database) -> NbResult<()> {
        let conn = db.connection()?;
        let elegido = match crate::settings::get(&conn, crate::settings::SettingKey::AiModelLocal)?
        {
            crate::settings::SettingValue::Text(valor) => valor,
            _ => String::new(),
        };
        drop(conn);
        let id = if elegido.is_empty() {
            self.generativo
                .recomendado()
                .map(|m| m.id.to_owned())
                .unwrap_or_default()
        } else {
            elegido
        };
        if !id.is_empty() {
            self.generativo.usar(&id);
        }
        self.publicar_estado();
        Ok(())
    }

    /// El modelo generativo que se usa y el que se recomienda para este equipo.
    #[must_use]
    pub fn modelos_generativos(&self) -> (&'static str, Option<&'static str>) {
        (
            self.generativo.elegido().id,
            self.generativo.recomendado().map(|m| m.id),
        )
    }

    /// Pide cortar la función de estudio en curso.
    pub fn cancelar_estudio(&self) {
        self.estudio_cancelado.store(true, Ordering::SeqCst);
    }

    /// La señal de cancelación, para el modelo generativo local.
    #[must_use]
    pub fn senal_de_cancelacion(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.estudio_cancelado)
    }
}

/// Quién genera, según eligió la persona en Ajustes.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si las preferencias no se pueden leer.
pub fn eleccion_para_generar(db: &Database) -> NbResult<Choice> {
    let conn = db.connection()?;
    let generador = match crate::settings::get(&conn, crate::settings::SettingKey::AiGenerator)? {
        crate::settings::SettingValue::Text(valor) => valor,
        _ => String::new(),
    };
    // Externo nombra al externo **esté o no configurado**: si no lo está, la
    // función dice que no hay quien la atienda. Caer al local sería el repliegue
    // silencioso que la preferencia existe para evitar.
    Ok(if generador == "external" {
        Choice::Provider(externo::ID.to_owned())
    } else {
        Choice::Auto
    })
}

/// El subsistema con la elección de quien genera ya hecha.
pub struct GeneradoraElegida<'a> {
    pub subsistema: &'a Subsistema,
    pub eleccion: Choice,
    pub capacidad: Capability,
}

/// A quién le toca generar, ya sacado de su cerrojo.
///
/// Existe para poder comprobar **a quién se le pidió**, que es lo que la regla 2 de
/// `philosophy.md` gobierna. Sin esto, la prueba de que nombrar un proveedor ausente
/// no cae en el otro pasaba igual con el repliegue puesto: los dos caminos devuelven
/// el mismo error cuando ninguno puede atender, y la diferencia solo se ve en cuál se
/// intentó. Lo destapó una mutación.
#[derive(Clone)]
pub enum AQuien {
    Externo(Arc<ExternalProvider>),
    Local(Arc<super::generativo::GenerativoLocal>),
}

impl GeneradoraElegida<'_> {
    /// Quién generaría, o `None` si la elección nombra a alguien que no está.
    ///
    /// **Nunca devuelve al otro.** Quien eligió un servicio no recibe sin saberlo lo
    /// que da un modelo pequeño, y quien no eligió ninguno no manda su texto fuera.
    #[must_use]
    pub fn a_quien(&self) -> Option<AQuien> {
        match &self.eleccion {
            Choice::Provider(_) => self.subsistema.externo().map(AQuien::Externo),
            Choice::Auto => Some(AQuien::Local(Arc::clone(&self.subsistema.generativo))),
        }
    }
}

impl super::estudio::Generadora for GeneradoraElegida<'_> {
    fn quien(&self) -> Option<super::ProviderInfo> {
        match self
            .subsistema
            .disponibilidad(self.capacidad, &self.eleccion)
        {
            Availability::Ready(info) => Some(info),
            Availability::Absent(_) => None,
        }
    }

    fn caracteres(&self) -> usize {
        self.subsistema
            .servicio
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .input_chars(self.capacidad, &self.eleccion)
            .unwrap_or(super::CARACTERES_DE_ENTRADA)
    }

    /// Genera **sin retener el cerrojo del servicio** mientras dura.
    ///
    /// Generar en local son minutos. `AiService::generate` presta el proveedor desde
    /// dentro del `RwLock`, así que pedirlo por ahí dejaría el cerrojo tomado todo
    /// ese rato, y cualquier escritura —cambiar la dirección del proveedor externo en
    /// Ajustes, que toma el cerrojo de escritura— se quedaría esperando con la
    /// ventana quieta. Aquí se saca primero el proveedor, que es un `Arc`, y se
    /// suelta el cerrojo antes de empezar.
    ///
    /// El pánico se atrapa igual que en el servicio, y por el mismo motivo: un motor
    /// que se cae no puede llevarse por delante al hilo que atendía la petición.
    fn generar(&self, request: &Request) -> NbResult<super::Completion> {
        let quien: Box<dyn Fn() -> NbResult<super::Completion>> =
            match self.a_quien().ok_or(NbError::ComponentUnavailable)? {
                AQuien::Externo(proveedor) => Box::new(move || proveedor.generate(request)),
                AQuien::Local(generativo) => Box::new(move || {
                    super::generativo::Compartido(Arc::clone(&generativo)).generate(request)
                }),
            };

        let resultado = std::panic::catch_unwind(std::panic::AssertUnwindSafe(quien));
        match resultado {
            Ok(salida) => salida,
            Err(_) => {
                tracing::error!("un proveedor de IA entró en pánico al generar");
                self.subsistema.monitor.set(
                    components::AI,
                    crate::monitor::ComponentState::Failed,
                    Some("AI_PROVIDER_CRASH"),
                );
                Err(NbError::ComponentUnavailable)
            }
        }
    }
}

/// El tope de memoria más pequeño de los admitidos con el que `modelo` cabe, con los
/// mismos hilos y el mismo lote que el presupuesto de ahora. `None` si ni con el
/// mayor.
fn tope_que_cabe(modelo: &catalogo::Modelo, presupuesto: Presupuesto) -> Option<u32> {
    crate::settings::MEMORIAS_DE_IA_ADMITIDAS
        .iter()
        .copied()
        .find(|mib| {
            let probado = Presupuesto {
                memoria: (*mib as u64) * 1024 * 1024,
                ..presupuesto
            };
            modelo::cabe(modelo, probado)
        })
        .map(|mib| mib as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::catalogo::GRANITE_107M_MULTILINGUAL;
    use crate::monitor::ComponentState;

    fn temporal(nombre: &str) -> PathBuf {
        let dir = crate::paths::temporal_de_pruebas()
            .join(format!("nb-subsistema-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        dir
    }

    /// Los archivos del modelo con el tamaño revisado, dispersos: la prueba no
    /// escribe doscientos megabytes para comprobar un estado.
    fn disponer(modelos: &Path) {
        let carpeta = instalacion::carpeta(modelos, &GRANITE_107M_MULTILINGUAL);
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        for archivo in GRANITE_107M_MULTILINGUAL.archivos {
            std::fs::File::create(carpeta.join(archivo.nombre))
                .and_then(|f| f.set_len(archivo.bytes))
                .expect("archivo");
        }
    }

    fn arrancado(modelos: &Path) -> (Subsistema, Monitor) {
        let monitor = Monitor::new(EventBus::new());
        (
            Subsistema::arrancar(modelos, monitor.clone(), Presupuesto::de_fabrica(8)),
            monitor,
        )
    }

    /// Solo se concede el anfitrión del catálogo o el que el runtime propuso. Sin
    /// esto, el webview decidiría a dónde se sale con un permiso de modelos.
    #[test]
    fn solo_se_autoriza_un_anfitrion_que_el_runtime_propuso() {
        let (ia, _) = arrancado(&temporal("autorizar"));
        let db = Database::open_in_memory().expect("base");

        assert_eq!(
            ia.autorizar(&db, "servidor.cualquiera"),
            Err(NbError::PathNotAllowed)
        );
        ia.autorizar(&db, ANFITRION).expect("el del catálogo sí");

        // La red de distribución, solo después de que la instalación la proponga.
        assert_eq!(
            ia.autorizar(&db, "cdn.ejemplo"),
            Err(NbError::PathNotAllowed)
        );
        *ia.propuesto.lock().expect("propuesto") = Some("cdn.ejemplo".to_owned());
        ia.autorizar(&db, "cdn.ejemplo").expect("ahora sí");

        let conn = db.connection().expect("conexión");
        assert!(crate::net::has_consent(&conn, ANFITRION, Purpose::Model).expect("consulta"));
        assert!(crate::net::has_consent(&conn, "cdn.ejemplo", Purpose::Model).expect("consulta"));
        // Y con el propósito de modelos, no con otro.
        assert!(!crate::net::has_consent(&conn, ANFITRION, Purpose::Catalog).expect("consulta"));
    }

    /// Sin nada instalado, el componente está detenido y **no pide atención**; con
    /// el modelo instalado, listo; quitado, otra vez detenido. Es el cálculo que
    /// hace que la tarjeta de estado no aparezca para quien no ha pedido la IA.
    #[test]
    fn el_estado_del_componente_sigue_a_lo_que_hay_instalado() {
        let modelos = temporal("estado");
        let (ia, monitor) = arrancado(&modelos);

        // En un equipo que no dé para la inferencia sería `NotSupported`, que
        // tampoco pide atención; la prueba acepta los dos y exige que no sea
        // `NotInstalled`, que es el que la pediría.
        let inicial = monitor.get(components::AI).expect("publicado");
        assert!(
            matches!(
                inicial,
                ComponentState::Stopped | ComponentState::NotSupported
            ),
            "sin nada instalado se publicó {inicial:?}"
        );
        if inicial == ComponentState::NotSupported {
            return;
        }

        disponer(&modelos);
        ia.publicar_estado();
        assert_eq!(monitor.get(components::AI), Some(ComponentState::Ready));

        ia.desinstalar(GRANITE_107M_MULTILINGUAL.id)
            .expect("desinstalar");
        assert_eq!(monitor.get(components::AI), Some(ComponentState::Stopped));
    }

    /// Bajar el tope de memoria por debajo del modelo instalado **se publica en el
    /// momento** como detenido con su motivo, sin pedir atención, y subirlo lo
    /// devuelve. Es lo que hace el comando de Ajustes al guardar la preferencia.
    #[test]
    fn cambiar_el_tope_de_memoria_se_publica_sin_reiniciar() {
        let modelos = temporal("tope");
        let (ia, monitor) = arrancado(&modelos);
        if monitor.get(components::AI) == Some(ComponentState::NotSupported) {
            return;
        }
        disponer(&modelos);
        ia.publicar_estado();
        assert_eq!(monitor.get(components::AI), Some(ComponentState::Ready));

        ia.aplicar_presupuesto(Presupuesto::desde(0.0, 512.0, 8));
        assert_eq!(monitor.get(components::AI), Some(ComponentState::Stopped));
        let vista = ia.vista();
        assert_eq!(vista.memory_limit_bytes, 512 * 1024 * 1024);
        assert!(vista.models.iter().all(|m| m.exceeds_memory_limit));

        ia.aplicar_presupuesto(Presupuesto::de_fabrica(8));
        assert_eq!(monitor.get(components::AI), Some(ComponentState::Ready));
        // El de embeddings vuelve a caber; el que genera necesita 2 GB, más que el
        // tope de fábrica, y lo sigue diciendo.
        let vista = ia.vista();
        let de = |id: &str| {
            vista
                .models
                .iter()
                .find(|m| m.id == id)
                .expect("en el catálogo")
        };
        assert!(!de(GRANITE_107M_MULTILINGUAL.id).exceeds_memory_limit);
        assert!(de(catalogo::QWEN25_15B_INSTRUCT.id).exceeds_memory_limit);
    }

    /// Ajustes distingue los cuatro casos, y la licencia viaja en todos: el
    /// criterio es verla **antes** de descargar.
    #[test]
    fn ajustes_ve_la_licencia_y_distingue_disponible_instalado_y_roto() {
        let modelos = temporal("vista");
        let (ia, _) = arrancado(&modelos);

        let estado = |ia: &Subsistema| {
            let vista = ia.vista();
            let modelo = vista
                .models
                .into_iter()
                .find(|m| m.id == GRANITE_107M_MULTILINGUAL.id)
                .expect("el modelo del catálogo");
            assert_eq!(modelo.license, "Apache-2.0");
            assert_eq!(modelo.host, ANFITRION);
            assert!(!modelo.what_is_sent.is_empty());
            modelo.status
        };

        assert_eq!(estado(&ia), EstadoDelModelo::Available);

        disponer(&modelos);
        assert_eq!(estado(&ia), EstadoDelModelo::Installed);

        std::fs::File::options()
            .write(true)
            .open(instalacion::carpeta(&modelos, &GRANITE_107M_MULTILINGUAL).join("config.json"))
            .and_then(|f| f.set_len(1))
            .expect("truncar");
        assert_eq!(estado(&ia), EstadoDelModelo::Broken);

        *ia.en_curso.lock().expect("en curso") = Some(InstalacionEnCurso {
            model_id: GRANITE_107M_MULTILINGUAL.id.to_owned(),
            done: 1,
            total: 2,
        });
        assert_eq!(estado(&ia), EstadoDelModelo::Installing);
    }

    /// No se puede quitar el modelo que se está instalando: borrar su carpeta
    /// mientras el hilo escribe dejaría al hilo renombrando algo que ya no existe.
    #[test]
    fn no_se_puede_quitar_un_modelo_mientras_se_instala() {
        let modelos = temporal("quitar-en-curso");
        let (ia, _) = arrancado(&modelos);
        *ia.en_curso.lock().expect("en curso") = Some(InstalacionEnCurso {
            model_id: GRANITE_107M_MULTILINGUAL.id.to_owned(),
            done: 0,
            total: 1,
        });

        assert_eq!(
            ia.desinstalar(GRANITE_107M_MULTILINGUAL.id),
            Err(NbError::StorageFailure)
        );
    }

    /// Pedir instalar con una instalación en marcha **no es un error** y no
    /// arranca otro hilo: la interfaz enseña el avance de la que hay.
    #[test]
    fn pedir_instalar_con_una_en_marcha_no_arranca_otra() {
        let modelos = temporal("dos-a-la-vez");
        let (ia, _) = arrancado(&modelos);
        let ia = Arc::new(ia);
        if !ia.maquina_capaz() {
            return;
        }
        *ia.en_curso.lock().expect("en curso") = Some(InstalacionEnCurso {
            model_id: GRANITE_107M_MULTILINGUAL.id.to_owned(),
            done: 0,
            total: 1,
        });

        let red = Arc::new(Network::new(Box::new(crate::net::UreqTransport::new())));
        let ya_habia = ia
            .instalar(
                GRANITE_107M_MULTILINGUAL.id,
                Database::open_in_memory().expect("base"),
                red,
                EventBus::new(),
            )
            .expect("sin error");

        assert!(ya_habia);
    }

    #[test]
    fn un_modelo_que_no_es_del_catalogo_no_se_instala_ni_se_quita() {
        let (ia, _) = arrancado(&temporal("desconocido"));
        let ia = Arc::new(ia);
        let red = Arc::new(Network::new(Box::new(crate::net::UreqTransport::new())));

        assert_eq!(
            ia.instalar(
                "no-existe",
                Database::open_in_memory().expect("base"),
                red,
                EventBus::new()
            ),
            Err(NbError::DocumentNotFound)
        );
        assert_eq!(ia.desinstalar("no-existe"), Err(NbError::DocumentNotFound));
    }

    /// **Elegir un proveedor que no está no cae en el otro** (regla 2 de
    /// `philosophy.md` en sus dos direcciones). Con "externo" elegido y ninguno
    /// configurado, la función dice que no hay quien la atienda; no la hace el
    /// modelo local a espaldas de quien eligió un servicio.
    #[test]
    fn elegir_un_generador_que_no_esta_no_cae_en_el_otro() {
        use crate::ai::estudio::Generadora;

        let (ia, _) = arrancado(&temporal("sin-repliegue"));
        let db = Database::open_in_memory().expect("base");
        {
            let conn = db.connection().expect("conexión");
            crate::settings::set(
                &conn,
                crate::settings::SettingKey::AiGenerator,
                &crate::settings::SettingValue::Text("external".to_owned()),
            )
            .expect("preferencia");
        }
        let eleccion = eleccion_para_generar(&db).expect("elección");
        assert_eq!(eleccion, Choice::Provider(externo::ID.to_owned()));

        let generadora = GeneradoraElegida {
            subsistema: &ia,
            eleccion,
            capacidad: Capability::Summary,
        };
        assert!(
            generadora.quien().is_none(),
            "no hay proveedor externo configurado"
        );
        assert!(
            generadora.a_quien().is_none(),
            "se eligió el externo y se iba a pedir a otro"
        );
        assert_eq!(
            generadora.generar(&Request {
                capability: Capability::Summary,
                instruction: "INSTRUCCION".to_owned(),
                passages: Vec::new(),
                task: "TAREA".to_owned(),
                language: None,
            }),
            Err(NbError::ComponentUnavailable)
        );

        // Y el local sigue siendo el de "auto", que es lo que elige quien no cambia
        // la preferencia.
        let conn = db.connection().expect("conexión");
        crate::settings::set(
            &conn,
            crate::settings::SettingKey::AiGenerator,
            &crate::settings::SettingValue::Text("local".to_owned()),
        )
        .expect("preferencia");
        drop(conn);
        assert_eq!(eleccion_para_generar(&db).expect("elección"), Choice::Auto);
        let local = GeneradoraElegida {
            subsistema: &ia,
            eleccion: Choice::Auto,
            capacidad: Capability::Summary,
        };
        assert!(matches!(local.a_quien(), Some(AQuien::Local(_))));
    }

    /// Lo que cuesta arrancar el subsistema, **medido y no supuesto**, porque
    /// corre dentro de `bootstrap`, antes de la ventana, con un presupuesto de
    /// 200 ms para todo. Detectar la máquina incluye una multiplicación con
    /// Candle, que la primera vez inicializa su motor. No se afirma un umbral —la
    /// prueba corre con la suite en paralelo y mediría la máquina—, se deja la
    /// cifra a la vista con `--nocapture`.
    #[test]
    fn arrancar_el_subsistema_se_mide() {
        let modelos = temporal("coste");
        let inicio = std::time::Instant::now();
        let _ = arrancado(&modelos);
        eprintln!("arrancar el subsistema de IA: {:?}", inicio.elapsed());
    }
}
