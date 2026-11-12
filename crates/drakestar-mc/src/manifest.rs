//! Manifiesto de versiones de Mojang (v2) con cache en disco para poder abrir el launcher sin red.

use crate::{write_atomic, Dirs, McError, McResult};
use serde::{Deserialize, Serialize};

pub const MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

/// Rango que soporta el launcher: de 1.20 a 26.2 (ambos incluidos, solo releases).
pub const MIN_SUPPORTED: &str = "1.20";
pub const MAX_SUPPORTED: &str = "26.2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub sha1: String,
    #[serde(rename = "releaseTime", default)]
    pub release_time: String,
}

/// `"1.21.1"` -> `[1,21,1]`; `None` si no es una release numerica (snapshots, pre-releases, `26.1-rc-1`).
pub fn parse_release(id: &str) -> Option<Vec<u32>> {
    if id.contains(|c: char| c.is_ascii_alphabetic() || c == '-') {
        return None;
    }
    let v: Option<Vec<u32>> = id.split('.').map(|p| p.parse().ok()).collect();
    v.filter(|v| !v.is_empty())
}

fn cmp(a: &[u32], b: &[u32]) -> std::cmp::Ordering {
    let n = a.len().max(b.len());
    for i in 0..n {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x.cmp(&y);
        }
    }
    std::cmp::Ordering::Equal
}

/// Dentro del rango soportado (1.20 <= v <= 26.2.x).
pub fn supported(id: &str) -> bool {
    let Some(v) = parse_release(id) else { return false };
    let lo = parse_release(MIN_SUPPORTED).unwrap();
    let mut hi = parse_release(MAX_SUPPORTED).unwrap();
    hi.push(u32::MAX); // 26.2.x entra
    cmp(&v, &lo) != std::cmp::Ordering::Less && cmp(&v, &hi) != std::cmp::Ordering::Greater
}

fn cache_path(d: &Dirs) -> std::path::PathBuf {
    d.root().join("cache").join("version_manifest_v2.json")
}

/// Descarga el manifiesto; si no hay red usa el de disco.
pub fn load(d: &Dirs) -> McResult<Manifest> {
    match drakestar_net::fetch_text(MANIFEST_URL) {
        Ok(text) => {
            let m: Manifest = serde_json::from_str(&text)?;
            let _ = write_atomic(&cache_path(d), text.as_bytes());
            Ok(m)
        }
        Err(net) => match std::fs::read_to_string(cache_path(d)) {
            Ok(t) => Ok(serde_json::from_str(&t)?),
            Err(_) => Err(McError::Net(format!("sin conexion y sin cache del manifiesto de versiones ({net})"))),
        },
    }
}

/// Releases soportadas, de la mas nueva a la mas vieja.
pub fn supported_releases(m: &Manifest) -> Vec<&Entry> {
    m.versions.iter().filter(|e| e.kind == "release" && supported(&e.id)).collect()
}

pub fn find<'a>(m: &'a Manifest, id: &str) -> McResult<&'a Entry> {
    m.versions.iter().find(|e| e.id == id).ok_or_else(|| McError::NotFound(format!("Minecraft {id} no esta en el manifiesto de Mojang")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_is_1_20_to_26_2() {
        for ok in ["1.20", "1.20.1", "1.20.6", "1.21", "1.21.1", "1.21.11", "26.1", "26.2"] {
            assert!(supported(ok), "{ok}");
        }
        for no in ["1.19.4", "1.12.2", "26.3", "27.0", "24w14a", "1.21-pre1", "26.1-rc-1"] {
            assert!(!supported(no), "{no}");
        }
    }

    #[test]
    fn point_releases_of_26_2_pass() {
        assert!(supported("26.2.1"));
        assert!(!supported("26.3.1"));
    }
}
