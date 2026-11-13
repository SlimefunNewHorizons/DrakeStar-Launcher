//! Mods desde Modrinth (verificados con SHA-512) y desde los releases de GitHub (DrakeStar Fused).

use crate::{McError, McResult};
use serde::Deserialize;
use sha2::{Digest, Sha512};
use std::io::Read;
use std::path::{Path, PathBuf};

const MODRINTH: &str = "https://api.modrinth.com/v2";

/// Mods de rendimiento que se instalan solos en el modo "Rendimiento" (los que no existan para esa version se omiten).
pub const PERFORMANCE_SET: &[&str] = &["sodium", "lithium", "ferrite-core", "immediatelyfast", "entityculling"];

#[derive(Debug, Deserialize)]
struct MrVersion {
    version_type: String,
    files: Vec<MrFile>,
    #[serde(default)]
    dependencies: Vec<MrDep>,
}
#[derive(Debug, Deserialize)]
struct MrFile {
    url: String,
    filename: String,
    primary: bool,
    hashes: MrHashes,
}
#[derive(Debug, Deserialize)]
struct MrHashes {
    sha512: String,
}
#[derive(Debug, Deserialize)]
struct MrDep {
    project_id: Option<String>,
    dependency_type: String,
}

#[derive(Debug, Clone)]
pub struct Installed {
    pub slug: String,
    pub file: PathBuf,
    pub filename: String,
}

pub(crate) fn get_json<T: for<'de> Deserialize<'de>>(url: &str) -> McResult<T> {
    let r = ureq::get(url)
        .set("User-Agent", "SlimefunNewHorizons/DrakeStar-Launcher (launcher de Minecraft)")
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| McError::Net(format!("{url}: {e}")))?;
    r.into_json().map_err(|e| McError::Json(e.to_string()))
}

fn download_sha512(url: &str, dest: &Path, expected: &str) -> McResult<()> {
    let r = ureq::get(url).set("User-Agent", "SlimefunNewHorizons/DrakeStar-Launcher").timeout(std::time::Duration::from_secs(120)).call().map_err(|e| McError::Net(format!("{url}: {e}")))?;
    let mut buf = vec![];
    r.into_reader().read_to_end(&mut buf)?;
    let got: String = Sha512::digest(&buf).iter().map(|b| format!("{b:02x}")).collect();
    if !got.eq_ignore_ascii_case(expected) {
        return Err(McError::Invalid(format!("SHA-512 no coincide para {url}")));
    }
    crate::write_atomic(dest, &buf)?;
    Ok(())
}

/// Instala la version estable mas nueva de `slug` compatible con `mc` y `loader`. `Ok(None)` si no hay (aun) para esa version.
pub fn ensure_modrinth(mods_dir: &Path, slug: &str, mc: &str, loader: &str) -> McResult<Option<Installed>> {
    let url = format!("{MODRINTH}/project/{slug}/version?game_versions=%5B%22{mc}%22%5D&loaders=%5B%22{loader}%22%5D");
    let versions: Vec<MrVersion> = match get_json(&url) {
        Ok(v) => v,
        Err(McError::Net(e)) if e.contains("404") => return Ok(None),
        Err(e) => return Err(e),
    };
    let Some(v) = versions.iter().find(|v| v.version_type == "release").or_else(|| versions.first()) else { return Ok(None) };
    let f = v.files.iter().find(|f| f.primary).or_else(|| v.files.first()).ok_or_else(|| McError::Invalid(format!("{slug}: version sin archivos")))?;
    let dest = mods_dir.join(&f.filename);
    if !dest.exists() {
        std::fs::create_dir_all(mods_dir)?;
        // si habia otra version del mismo mod, se reemplaza
        for e in std::fs::read_dir(mods_dir)?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with(&format!("{slug}-")) && n.ends_with(".jar") && n != f.filename {
                let _ = std::fs::remove_file(e.path());
            }
        }
        download_sha512(&f.url, &dest, &f.hashes.sha512)?;
    }
    let _ = &v.dependencies.iter().filter(|d| d.dependency_type == "required" && d.project_id.is_some()).count();
    Ok(Some(Installed { slug: slug.into(), file: dest, filename: f.filename.clone() }))
}

/// Fabric API es obligatorio para casi todos los mods de Fabric.
pub fn ensure_fabric_api(mods_dir: &Path, mc: &str) -> McResult<Installed> {
    ensure_modrinth(mods_dir, "fabric-api", mc, "fabric")?.ok_or_else(|| McError::NotFound(format!("Fabric API aun no existe para Minecraft {mc}")))
}

/// Instala el set de rendimiento (best effort) y devuelve los que si se instalaron.
pub fn ensure_performance(mods_dir: &Path, mc: &str, log: &dyn Fn(&str)) -> Vec<Installed> {
    let mut out = vec![];
    for slug in PERFORMANCE_SET {
        match ensure_modrinth(mods_dir, slug, mc, "fabric") {
            Ok(Some(i)) => out.push(i),
            Ok(None) => log(&format!("mod {slug}: no hay version para {mc}, se omite")),
            Err(e) => log(&format!("mod {slug}: {e} (se omite)")),
        }
    }
    out
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}
#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// Releases de Fused que no cargan fuera del entorno de desarrollo (usaban mixins sin remapear): no se instalan nunca.
const FUSED_BROKEN_TAGS: [&str; 2] = ["v1.0.0-alpha.1", "v1.0.0-alpha.2"];

fn remove_matching(dir: &Path, f: impl Fn(&str) -> bool) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if f(&e.file_name().to_string_lossy()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Borra Fused de mods/ (cuando la instancia lo desactiva).
pub fn remove_fused(mods_dir: &Path) {
    remove_matching(mods_dir, |n| n.starts_with("drakestar-fused-") && n.ends_with(".jar"));
}

/// DrakeStar Fused: busca en los releases de GitHub el jar del loader para esa version de Minecraft.
/// Nombre esperado: `drakestar-fused-<loader>-<mc>-<version>.jar`, o `drakestar-fused-<loader>-universal-<version>.jar` si sirve para todas
/// (desde la alpha.3 el de Fabric es universal: Minecraft 1.20 a 26.2).
pub fn ensure_fused(mods_dir: &Path, mc: &str, loader: &str) -> McResult<Option<PathBuf>> {
    // jars defectuosos que pudieron quedar de instalaciones anteriores
    remove_matching(mods_dir, |n| n.starts_with("drakestar-fused-") && FUSED_BROKEN_TAGS.iter().any(|t| n.ends_with(&format!("-{}.jar", t.trim_start_matches('v')))));
    let rels: Vec<GhRelease> = get_json("https://api.github.com/repos/SlimefunNewHorizons/drakestar-fused/releases?per_page=10")?;
    let exact = format!("drakestar-fused-{loader}-{mc}-");
    let uni = format!("drakestar-fused-{loader}-universal-");
    for r in rels.iter().filter(|r| !FUSED_BROKEN_TAGS.contains(&r.tag_name.as_str())) {
        let pick = r.assets.iter().find(|a| a.name.starts_with(&exact) && a.name.ends_with(".jar")).or_else(|| r.assets.iter().find(|a| a.name.starts_with(&uni) && a.name.ends_with(".jar")));
        if let Some(a) = pick {
            let dest = mods_dir.join(&a.name);
            std::fs::create_dir_all(mods_dir)?;
            if !dest.exists() || std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0) != a.size {
                drakestar_net::download_direct(&a.browser_download_url, &dest)?;
            }
            // un solo Fused en mods/: se borran las versiones anteriores (Fabric no admite el mismo mod dos veces)
            remove_matching(mods_dir, |n| n.starts_with("drakestar-fused-") && n.ends_with(".jar") && n != a.name);
            return Ok(Some(dest));
        }
    }
    Ok(None)
}

/// Slimefun4-Drake para el servidor local: el jar `Slimefun-*-<mc>-SNAPSHOT.jar` del release mas reciente que lo tenga.
/// `None` si no hay build para esa version de Minecraft.
pub fn ensure_slimefun(plugins_dir: &Path, mc: &str) -> McResult<Option<PathBuf>> {
    let rels: Vec<GhRelease> = get_json("https://api.github.com/repos/SlimefunNewHorizons/Slimefun4-Drake/releases?per_page=15")?;
    let tail = format!("-{mc}-SNAPSHOT.jar");
    for r in &rels {
        if let Some(a) = r.assets.iter().find(|a| a.name.starts_with("Slimefun-") && a.name.ends_with(&tail)) {
            let dest = plugins_dir.join(&a.name);
            std::fs::create_dir_all(plugins_dir)?;
            if !dest.exists() || std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0) != a.size {
                drakestar_net::download_direct(&a.browser_download_url, &dest)?;
            }
            remove_matching(plugins_dir, |n| n.starts_with("Slimefun-") && n.ends_with(".jar") && n != a.name);
            return Ok(Some(dest));
        }
    }
    Ok(None)
}
