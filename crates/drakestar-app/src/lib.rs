//! Logica de aplicacion de DrakeStar, independiente de Tauri:
//! almacen de perfiles, instancias, gestor de procesos con logs, gestion del
//! directorio del servidor local, resolucion/descarga de releases y delta-sync de mods.
//! El crate `src-tauri` solo expone estas funciones como comandos IPC.

pub mod assets;
pub mod compat_report;
pub mod engine;
pub mod error;
pub mod fabric;
pub mod instances;
pub mod java;
pub mod layout;
pub mod modsync;
pub mod modrinth;
pub mod mods;
pub mod native;
pub mod ping;
pub mod prism_import;
pub mod process;
pub mod release;
pub mod rustclient;
pub mod server;
pub mod service;
pub mod settings;
pub mod store;

pub use error::{AppError, AppResult};
pub use layout::Layout;

/// Milisegundos desde epoch (para marcas de tiempo de log).
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
pub(crate) mod testutil {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    pub fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "drakestar-app-{}-{}-{tag}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
