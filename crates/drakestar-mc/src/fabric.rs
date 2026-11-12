//! Fabric: pide a meta.fabricmc.net el perfil ya armado para una version de Minecraft y lo deja en `versions/`.

use crate::{write_atomic, Dirs, McError, McResult};
use serde::Deserialize;

const META: &str = "https://meta.fabricmc.net/v2/versions/loader";

#[derive(Debug, Deserialize)]
struct Entry {
    loader: Loader,
}
#[derive(Debug, Deserialize)]
struct Loader {
    version: String,
    stable: bool,
}

/// Version de loader estable mas nueva para ese Minecraft.
pub fn latest_loader(mc: &str) -> McResult<String> {
    let text = drakestar_net::fetch_text(&format!("{META}/{mc}"))?;
    let entries: Vec<Entry> = serde_json::from_str(&text)?;
    entries
        .into_iter()
        .find(|e| e.loader.stable)
        .map(|e| e.loader.version)
        .ok_or_else(|| McError::NotFound(format!("Fabric no soporta Minecraft {mc} (aun)")))
}

/// Baja el perfil de Fabric y devuelve su id (`fabric-loader-<loader>-<mc>`). Idempotente.
pub fn ensure_profile(d: &Dirs, mc: &str, loader: Option<&str>) -> McResult<String> {
    let loader = match loader {
        Some(l) => l.to_string(),
        None => latest_loader(mc)?,
    };
    let id = format!("fabric-loader-{loader}-{mc}");
    let path = d.version_json(&id);
    if !path.exists() {
        let text = drakestar_net::fetch_text(&format!("{META}/{mc}/{loader}/profile/json"))?;
        // validar que es un JSON de version antes de guardarlo
        crate::version::VersionJson::parse(&text)?;
        write_atomic(&path, text.as_bytes())?;
    }
    Ok(id)
}
