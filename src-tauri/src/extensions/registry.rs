//! Ciclo de vida y activación perezosa.
//!
//! # Qué se paga al arrancar, y qué no
//!
//! Al arrancar se lee el perfil activo y el manifiesto de cada extensión
//! instalada. Eso es todo: unos cuantos kilobytes de JSON. **No se compila ni
//! un módulo.**
//!
//! Y ahí está la diferencia que importa. Leer un manifiesto cuesta microsegundos;
//! compilar un módulo de WebAssembly con Cranelift cuesta milisegundos, y veinte
//! extensiones compiladas al arrancar se comerían el presupuesto de 200 ms que
//! `performance-budgets.md` le da al runtime entero. Por eso el manifiesto es
//! declarativo: dice qué aporta una extensión sin que su código llegue a
//! existir, y el código solo se compila cuando alguien invoca una contribución.
//!
//! Es el modelo de VS Code, y el motivo por el que el manifiesto
//! lleva `contributes` dentro.
//!
//! # Desactivar no es desinstalar
//!
//! Una extensión desactivada sigue instalada, conserva sus permisos y conserva
//! su sitio en el perfil. Si desactivar borrara los permisos, probar cuál de
//! cinco extensiones molesta obligaría a volver a conceder cinco veces.

use std::collections::BTreeMap;

use crate::error::{NbError, NbResult};
use crate::security;

use super::manifest::{Command, Manifest};
use super::store::{Installed, Store};
use super::{Ejecucion, Limites, Motor};

/// Un comando aportado por una extensión, con quién lo aporta.
///
/// Es lo que puebla la paleta y los menús **antes** de que exista una sola
/// instrucción compilada de esa extensión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aportado {
    pub extension: String,
    pub comando: Command,
}

/// Lo que hace falta para ejecutar código de una extensión.
///
/// Se agrupa en un tipo y no se pasa suelto porque son las cuatro piezas del
/// mismo hecho —dónde corre, de dónde sale su código, y por dónde habla con la
/// base y con la red—, y porque sueltas hacían que la función pasara del número
/// de parámetros que el proyecto admite.
pub struct Entorno<'a> {
    pub motor: &'a Motor,
    pub almacen: &'a Store,
    pub db: &'a crate::db::Database,
    pub red: &'a std::sync::Arc<crate::net::Network>,
}

/// Lo que hay instalado y qué aporta, sin nada compilado.
pub struct Registro {
    instaladas: Vec<Installed>,
    manifiestos: BTreeMap<String, Manifest>,
}

impl Registro {
    /// Lee el perfil y los manifiestos. **No compila ni un módulo.**
    ///
    /// Una extensión cuyo manifiesto ya no se puede leer no tumba el arranque:
    /// se omite y se anota. Que una extensión rota impida abrir la aplicación
    /// sería exactamente el fallo de un componente tumbando a otro.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si no se puede leer el perfil.
    pub fn cargar(almacen: &Store) -> NbResult<Self> {
        let instaladas = almacen.perfil()?.extensions;
        let mut manifiestos = BTreeMap::new();
        for instalada in &instaladas {
            match almacen.manifiesto_de(instalada) {
                Ok(manifiesto) => {
                    manifiestos.insert(instalada.id.clone(), manifiesto);
                }
                Err(_) => {
                    tracing::warn!("una extensión instalada no tiene manifiesto legible");
                }
            }
        }
        Ok(Self {
            instaladas,
            manifiestos,
        })
    }

    /// Todo lo instalado, activado o no.
    #[must_use]
    pub fn instaladas(&self) -> &[Installed] {
        &self.instaladas
    }

    /// El manifiesto de una extensión instalada.
    #[must_use]
    pub fn manifiesto(&self, id: &str) -> Option<&Manifest> {
        self.manifiestos.get(id)
    }

    /// Si está instalada y activada.
    #[must_use]
    pub fn esta_activa(&self, id: &str) -> bool {
        self.instaladas.iter().any(|e| e.id == id && e.enabled)
    }

    /// Los comandos que pueblan la interfaz, solo de las extensiones activas.
    ///
    /// Sale del manifiesto, así que está disponible sin haber cargado código.
    #[must_use]
    pub fn comandos(&self) -> Vec<Aportado> {
        let mut salida: Vec<Aportado> = self
            .instaladas
            .iter()
            .filter(|e| e.enabled)
            .filter_map(|e| Some((e, self.manifiestos.get(&e.id)?)))
            .flat_map(|(e, manifiesto)| {
                manifiesto
                    .contributes
                    .commands
                    .iter()
                    .map(move |comando| Aportado {
                        extension: e.id.clone(),
                        comando: comando.clone(),
                    })
            })
            .collect();
        salida.sort_by(|a, b| a.comando.id.cmp(&b.comando.id));
        salida
    }

    /// Los comandos que aparecen en un sitio concreto de la interfaz.
    ///
    /// El sitio sale de `contributes.contextMenus` del manifiesto, así que un
    /// comando declarado y no colocado en ningún sitio no aparece en ninguna
    /// parte: declararlo no es ponerlo, y eso permite que una extensión tenga
    /// órdenes que solo invoca ella.
    #[must_use]
    pub fn comandos_en(&self, sitio: &str) -> Vec<Aportado> {
        self.comandos()
            .into_iter()
            .filter(|aportado| {
                self.manifiestos
                    .get(&aportado.extension)
                    .and_then(|m| m.contributes.context_menus.get(sitio))
                    .is_some_and(|ids| ids.contains(&aportado.comando.id))
            })
            .collect()
    }

    /// La clave con la que un manifiesto declara el módulo que ejecuta sus
    /// órdenes.
    pub const CLAVE_DE_ORDENES: &'static str = "command";

    /// Ejecuta una orden de una extensión. **Aquí se compila su código.**
    ///
    /// Hasta esta llamada, la orden estaba en la interfaz sin que existiera una
    /// sola instrucción compilada de esa extensión. Es el criterio y
    /// de lo que vive la activación perezosa.
    ///
    /// # Errors
    ///
    /// - [`NbError::ComponentUnavailable`] si la extensión no está, está
    ///   desactivada, no declara módulo de órdenes o no declara esa orden.
    /// - [`NbError::PathNotAllowed`] si su módulo queda fuera de su carpeta.
    pub fn ejecutar_orden(
        &self,
        entorno: &Entorno<'_>,
        extension: &str,
        orden: &str,
        limites: Limites,
    ) -> NbResult<super::world::Activacion> {
        if !self.esta_activa(extension) {
            return Err(NbError::ComponentUnavailable);
        }
        let manifiesto = self
            .manifiestos
            .get(extension)
            .ok_or(NbError::ComponentUnavailable)?;

        // Una orden que el manifiesto no declara no se ejecuta, ni siquiera si
        // el módulo la implementa: lo que el usuario aprobó al instalar es lo
        // que el manifiesto decía.
        if !manifiesto
            .contributes
            .commands
            .iter()
            .any(|c| c.id == orden)
        {
            tracing::info!("se pidió una orden que la extensión no declara");
            return Err(NbError::ComponentUnavailable);
        }

        let bytes = self.modulo_de(entorno.almacen, extension, Self::CLAVE_DE_ORDENES)?;
        let anfitrion = super::world::Anfitrion::para(
            entorno.db.clone(),
            std::sync::Arc::clone(entorno.red),
            extension,
            limites,
        );
        super::world::activar(entorno.motor, &bytes, limites, anfitrion, orden)
    }

    /// Las vistas que las extensiones aportan a Estudio, con quién las aporta.
    ///
    /// Salen del manifiesto, así que las pestañas existen **antes** de que se
    /// compile una sola instrucción de la extensión: componerlas es lo que la
    /// carga.
    #[must_use]
    pub fn vistas(&self) -> Vec<(String, super::manifest::ViewContribution)> {
        let mut salida: Vec<_> = self
            .instaladas
            .iter()
            .filter(|e| e.enabled)
            .filter_map(|e| Some((e, self.manifiestos.get(&e.id)?)))
            .flat_map(|(e, manifiesto)| {
                manifiesto
                    .contributes
                    .views
                    .iter()
                    .map(move |vista| (e.id.clone(), vista.clone()))
            })
            .collect();
        salida.sort_by(|a, b| a.1.id.cmp(&b.1.id));
        salida
    }

    /// Compone una vista de una extensión. **Aquí se compila su código.**
    ///
    /// # Errors
    ///
    /// [`NbError::ComponentUnavailable`] si la extensión no está, está apagada
    /// o no declara esa vista; [`NbError::PathNotAllowed`] si su módulo queda
    /// fuera de su carpeta.
    pub fn componer_vista(
        &self,
        entorno: &Entorno<'_>,
        extension: &str,
        vista: &str,
        limites: Limites,
    ) -> NbResult<super::view::View> {
        if !self.esta_activa(extension) {
            return Err(NbError::ComponentUnavailable);
        }
        let manifiesto = self
            .manifiestos
            .get(extension)
            .ok_or(NbError::ComponentUnavailable)?;

        // Una vista que el manifiesto no declara no se compone, por lo mismo
        // que una orden que no declara no se ejecuta: lo que el usuario aprobó
        // al instalar es lo que el manifiesto decía.
        if !manifiesto.contributes.views.iter().any(|v| v.id == vista) {
            tracing::info!("se pidió una vista que la extensión no declara");
            return Err(NbError::ComponentUnavailable);
        }

        let bytes = self.modulo_de(entorno.almacen, extension, super::view::CLAVE)?;
        let anfitrion = super::view::anfitrion_de(entorno.db, entorno.red, extension, limites);
        super::view::renderizar(entorno.motor, &bytes, limites, anfitrion, vista)
    }

    /// Los bytes del módulo que una extensión declara para una clase de
    /// proveedor, con su ruta acotada a su propia carpeta.
    /// # Errors
    ///
    /// - [`NbError::ComponentUnavailable`] si la extensión no está instalada o
    ///   no declara esa clase de proveedor.
    /// - [`NbError::PathNotAllowed`] si el manifiesto apunta fuera de su
    ///   carpeta del almacén.
    pub(crate) fn modulo_de(
        &self,
        almacen: &Store,
        extension: &str,
        clase: &str,
    ) -> NbResult<Vec<u8>> {
        let instalada = self
            .instaladas
            .iter()
            .find(|e| e.id == extension)
            .ok_or(NbError::ComponentUnavailable)?;
        let declarado = self
            .manifiestos
            .get(extension)
            .and_then(|m| m.contributes.providers.get(clase))
            .ok_or(NbError::ComponentUnavailable)?;

        // La ruta la escribe quien publica la extensión: se acota antes de leer.
        let carpeta = almacen.ruta_de(instalada);
        let modulo = crate::security::resolve_within(&carpeta, std::path::Path::new(declarado))?;
        std::fs::read(&modulo).map_err(|_| NbError::FileNotFound)
    }

    /// Invoca un proveedor de una extensión. **Aquí, y solo aquí, se compila.**
    ///
    /// # Errors
    ///
    /// - [`NbError::ComponentUnavailable`] si la extensión no está instalada,
    ///   está desactivada o no declara ese proveedor.
    /// - [`NbError::PathNotAllowed`] si el manifiesto apunta a un archivo fuera
    ///   de su propia carpeta del almacén.
    /// - [`NbError::FileNotFound`] si el módulo declarado no está.
    pub fn invocar(
        &self,
        motor: &Motor,
        almacen: &Store,
        id: &str,
        proveedor: &str,
        limites: Limites,
    ) -> NbResult<Ejecucion> {
        if !self.esta_activa(id) {
            return Err(NbError::ComponentUnavailable);
        }
        let instalada = self
            .instaladas
            .iter()
            .find(|e| e.id == id)
            .ok_or(NbError::ComponentUnavailable)?;
        let manifiesto = self
            .manifiestos
            .get(id)
            .ok_or(NbError::ComponentUnavailable)?;
        let declarado = manifiesto
            .contributes
            .providers
            .get(proveedor)
            .ok_or(NbError::ComponentUnavailable)?;

        // La ruta viene del manifiesto, que lo escribe quien publica la
        // extensión: es una ruta de fuera. `"../../../etc/passwd"` es un
        // manifiesto perfectamente válido, y sin acotarla el almacén serviría
        // como lector de archivos del sistema.
        let carpeta = almacen.ruta_de(instalada);
        let modulo = security::resolve_within(&carpeta, std::path::Path::new(declarado))?;
        let bytes = std::fs::read(&modulo).map_err(|_| NbError::FileNotFound)?;

        motor.ejecutar(&bytes, limites)
    }
}
