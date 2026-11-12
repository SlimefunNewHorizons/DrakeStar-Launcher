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

/// Perfil de Fabric ya instalado para ese Minecraft (el de loader mas nuevo).
fn installed_profile(d: &Dirs, mc: &str) -> Option<String> {
    let suffix = format!("-{mc}");
    let key = |id: &str| -> Vec<u32> { id.trim_start_matches("fabric-loader-").trim_end_matches(suffix.as_str()).split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    std::fs::read_dir(d.versions())
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("fabric-loader-") && n.ends_with(suffix.as_str()) && d.version_json(n).exists())
        .max_by_key(|n| key(n))
}

/// Baja el perfil de Fabric y devuelve su id (`fabric-loader-<loader>-<mc>`). Idempotente.
pub fn ensure_profile(d: &Dirs, mc: &str, loader: Option<&str>) -> McResult<String> {
    let loader = match loader {
        Some(l) => l.to_string(),
        // sin conexion se usa el perfil que ya estaba instalado para ese Minecraft
        None => match latest_loader(mc) {
            Ok(l) => l,
            Err(e) => return installed_profile(d, mc).ok_or(e),
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_newest_installed_profile_for_a_version() {
        let d = Dirs::new(std::env::temp_dir().join(format!("dsmc-fab-{}", std::process::id())));
        for id in ["fabric-loader-0.9.3-1.21.1", "fabric-loader-0.16.10-1.21.1", "fabric-loader-0.20.1-1.21", "fabric-loader-0.99.0-1.20.1"] {
            std::fs::create_dir_all(d.version_dir(id)).unwrap();
            std::fs::write(d.version_json(id), "{}").unwrap();
        }
        assert_eq!(installed_profile(&d, "1.21.1").as_deref(), Some("fabric-loader-0.16.10-1.21.1"));
        assert_eq!(installed_profile(&d, "1.21").as_deref(), Some("fabric-loader-0.20.1-1.21"));
        assert_eq!(installed_profile(&d, "26.1"), None);
        let _ = std::fs::remove_dir_all(d.root());
    }
}
