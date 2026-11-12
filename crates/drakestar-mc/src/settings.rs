//! Ajustes del launcher (un solo `settings.json`). Pensados para que la app funcione sin tocar nada.

use crate::{write_atomic, Dirs, McResult};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Recolector de reportes de error por defecto (lo sirve la web de DrakesCraft).
pub const DEFAULT_REPORT_ENDPOINT: &str = "https://web.drakescraft.cl/api/launcher/report";

/// Client ID heredado (de otro launcher) que se usa solo mientras no haya uno propio: el login se marca como "temporal" en la interfaz.
pub const LEGACY_CLIENT_ID: &str = "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb";

/// Client ID de Azure por defecto: el de la app propia aprobada por Mojang (https://aka.ms/mce-reviewappid), que se fija al compilar
/// con la variable de entorno `DRAKESTAR_MSFT_CLIENT_ID` (en CI: variable del repositorio). Sin ella cae al heredado.
pub fn default_client_id() -> &'static str {
    option_env!("DRAKESTAR_MSFT_CLIENT_ID").filter(|s| !s.trim().is_empty()).unwrap_or(LEGACY_CLIENT_ID)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: String,
    pub azure_client_id: Option<String>,
    /// Enviar automaticamente un reporte cuando el juego se cierra con error (solo para depurar; datos personales tachados).
    pub send_reports: bool,
    pub report_endpoint: Option<String>,
    pub device_id: String,
    pub last_instance: Option<String>,
    pub auto_update: bool,
    pub first_run_done: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { language: "es".into(), azure_client_id: None, send_reports: true, report_endpoint: None, device_id: String::new(), last_instance: None, auto_update: true, first_run_done: false }
    }
}

fn path(d: &Dirs) -> PathBuf {
    d.root().join("settings.json")
}

impl Settings {
    pub fn load(d: &Dirs) -> Settings {
        let mut s: Settings = std::fs::read_to_string(path(d)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        if s.device_id.is_empty() {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|x| x.as_nanos()).unwrap_or(0);
            let mut h = Md5::new();
            h.update(format!("{nanos}-{}-{}", std::process::id(), d.root().display()).as_bytes());
            s.device_id = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
            let _ = s.save(d);
        }
        s
    }

    pub fn save(&self, d: &Dirs) -> McResult<()> {
        write_atomic(&path(d), serde_json::to_string_pretty(self)?.as_bytes())?;
        Ok(())
    }

    pub fn client_id(&self) -> String {
        std::env::var("DRAKESTAR_MSFT_CLIENT_ID")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| self.azure_client_id.clone().filter(|s| !s.trim().is_empty()))
            .unwrap_or_else(|| default_client_id().to_string())
    }

    /// `true` mientras se use el client ID heredado (de otro launcher): hay que reemplazarlo por uno propio.
    pub fn client_id_is_temporary(&self) -> bool {
        self.client_id() == LEGACY_CLIENT_ID
    }

    pub fn report_endpoint(&self) -> String {
        self.report_endpoint.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| DEFAULT_REPORT_ENDPOINT.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_generate_a_stable_device_id() {
        let d = Dirs::new(std::env::temp_dir().join(format!("dsmc-set-{}", std::process::id())));
        let a = Settings::load(&d);
        assert_eq!(a.device_id.len(), 32);
        assert!(a.send_reports && a.auto_update);
        assert_eq!(Settings::load(&d).device_id, a.device_id);
        assert!(a.client_id_is_temporary());
        let _ = std::fs::remove_dir_all(d.root());
    }
}
