//! Nucleo de lanzador de Minecraft (sin Tauri): version JSON de Mojang, librerias, assets, Java
//! administrado (Adoptium), Fabric, argumentos de arranque y proceso con captura de logs.
//!
//! Disposicion en disco (igual que el launcher oficial, asi las versiones se comparten entre instancias):
//! ```text
//! <root>/versions/<id>/<id>.json, <id>.jar, natives/
//! <root>/libraries/<maven path>
//! <root>/assets/indexes/<id>.json, objects/xx/<hash>, log_configs/<file>
//! <root>/runtimes/java-<major>/
//! <root>/instances/<id>/  (directorio de juego: mods, saves, logs...)
//! ```

pub mod accounts;
pub mod catalog;
pub mod assets;
pub mod error;
pub mod ely;
pub mod fabric;
pub mod install;
pub mod instances;
pub mod importing;
pub mod memory;
pub mod java;
pub mod launch;
pub mod manifest;
pub mod mods;
pub mod msa;
pub mod offline;
pub mod play;
pub mod progress;
pub mod server;
pub mod settings;
pub mod support;
pub mod version;

use std::path::{Path, PathBuf};

pub use error::{McError, McResult};
pub use progress::{NullSink, ProgressSink};

/// Directorios del launcher.
#[derive(Debug, Clone)]
pub struct Dirs {
    root: PathBuf,
}

impl Dirs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions().join(id)
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }
    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }
    pub fn natives(&self, id: &str) -> PathBuf {
        self.version_dir(id).join("natives")
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn runtimes(&self) -> PathBuf {
        self.root.join("runtimes")
    }
    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn instance(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }
}

/// Sistema operativo en el vocabulario de Mojang (`windows`, `linux`, `osx`).
pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// Arquitectura en el vocabulario de Mojang (`x86`, `x86_64`, `arm64`).
pub fn arch_name() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x86_64"
    }
}

/// Escritura atomica (temporal + rename).
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}
