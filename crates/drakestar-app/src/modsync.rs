//! Sincronizacion diferencial (Delta-Sync) de mods Servidor <-> Cliente y
//! analizador pre-arranque de conflictos Fabric / Mixin en Rust.
//!
//! Cumple el flujo disenado para Project Purple (el equipo de DrakeStar):
//! 1. Al pulsar Play, el launcher compara los hashes SHA-256 de `mods/` contra el
//!    manifiesto del servidor: descarga SOLO los `.jar` nuevos o editados y
//!    elimina los `.jar` retirados del servidor sin re-descargar todo el modpack.
//! 2. Antes de arrancar la JVM, analiza dependencias (`depends`), incompatibilidades
//!    declaradas (`conflicts`/`breaks`), IDs duplicados y colisiones de clases objetivo
//!    de Mixin (`mixin_targets`) para avisar antes de que Fabric crashee en caliente.

use crate::{layout::safe_file_name, AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModEntry {
    pub file: String,
    pub id: String,
    pub version: String,
    pub sha256: String,
    pub size: u64,
    pub url: String,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub depends: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub mixin_targets: Vec<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerModManifest {
    pub schema: u32,
    pub server_name: String,
    pub minecraft: String,
    pub loader: String,
    pub mods: Vec<ModEntry>,
}

impl ServerModManifest {
    pub fn from_json(json: &str) -> AppResult<Self> {
        let m: Self = serde_json::from_str(json)?;
        if m.schema != 1 {
            return Err(AppError::Invalid(format!(
                "schema de ServerModManifest {} no soportado (esperado 1)",
                m.schema
            )));
        }
        for entry in &m.mods {
            if !safe_file_name(&entry.file) || !entry.file.to_lowercase().ends_with(".jar") {
                return Err(AppError::Invalid(format!(
                    "nombre de archivo de mod invalido en manifiesto: '{}'",
                    entry.file
                )));
            }
            if !drakestar_net::sha256_wellformed(&entry.sha256) {
                return Err(AppError::Invalid(format!(
                    "SHA-256 malformado para mod '{}'",
                    entry.file
                )));
            }
        }
        Ok(m)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DeltaSyncPlan {
    pub unchanged: Vec<String>,
    pub to_download: Vec<ModEntry>,
    pub to_remove: Vec<String>,
    pub total_download_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DeltaSyncResult {
    pub downloaded: Vec<String>,
    pub removed: Vec<String>,
    pub unchanged: Vec<String>,
    pub bytes_downloaded: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConflictReport {
    pub compatible: bool,
    pub mods_analyzed: usize,
    pub missing_dependencies: Vec<String>,
    pub explicit_conflicts: Vec<String>,
    pub duplicate_ids: Vec<String>,
    pub mixin_collisions: Vec<String>,
}

/// Calcula el plan diferencial entre la carpeta local `mods_dir` y el manifiesto del servidor.
pub fn compute_delta_sync(mods_dir: &Path, manifest: &ServerModManifest) -> AppResult<DeltaSyncPlan> {
    let mut expected_by_file: BTreeMap<&str, &ModEntry> = BTreeMap::new();
    for m in &manifest.mods {
        expected_by_file.insert(m.file.as_str(), m);
    }

    let mut local_hashes: BTreeMap<String, String> = BTreeMap::new();
    if mods_dir.is_dir() {
        for entry in std::fs::read_dir(mods_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let fname = entry.file_name().to_string_lossy().to_string();
            if !fname.to_lowercase().ends_with(".jar") {
                continue;
            }
            if let Ok(sha) = drakestar_net::sha256_file(&path) {
                local_hashes.insert(fname, sha);
            }
        }
    }

    let mut unchanged = Vec::new();
    let mut to_download = Vec::new();
    let mut total_download_bytes = 0u64;

    for m in &manifest.mods {
        match local_hashes.get(&m.file) {
            Some(local_sha) if local_sha.eq_ignore_ascii_case(&m.sha256) => {
                unchanged.push(m.file.clone());
            }
            _ => {
                total_download_bytes += m.size;
                to_download.push(m.clone());
            }
        }
    }

    let mut to_remove: Vec<String> = local_hashes
        .keys()
        .filter(|f| !expected_by_file.contains_key(f.as_str()))
        .cloned()
        .collect();
    to_remove.sort();

    Ok(DeltaSyncPlan {
        unchanged,
        to_download,
        to_remove,
        total_download_bytes,
    })
}

/// Ejecuta la sincronizacion diferencial sobre `mods_dir`: elimina `.jar` obsoletos
/// y descarga con verificacion SHA-256 solo los `.jar` nuevos o modificados.
pub fn apply_delta_sync(mods_dir: &Path, manifest: &ServerModManifest) -> AppResult<DeltaSyncResult> {
    std::fs::create_dir_all(mods_dir)?;
    let plan = compute_delta_sync(mods_dir, manifest)?;

    let mut removed = Vec::new();
    for obsolete in &plan.to_remove {
        let p = mods_dir.join(obsolete);
        if p.is_file() {
            std::fs::remove_file(&p)?;
            removed.push(obsolete.clone());
        }
    }

    let mut downloaded = Vec::new();
    let mut bytes_downloaded = 0u64;
    for item in &plan.to_download {
        let dest = mods_dir.join(&item.file);
        let written = drakestar_net::download_verified(
            &item.url,
            &dest,
            &item.sha256,
            Some(item.size),
            |_| {},
        )?;
        bytes_downloaded += written;
        downloaded.push(item.file.clone());
    }

    Ok(DeltaSyncResult {
        downloaded,
        removed,
        unchanged: plan.unchanged,
        bytes_downloaded,
    })
}

/// Analizador pre-arranque en Rust de conflictos entre mods de Fabric y sus Mixins.
pub fn analyze_mod_conflicts(entries: &[ModEntry]) -> ConflictReport {
    let mut present_ids: BTreeSet<&str> = BTreeSet::new();
    let mut duplicate_ids_set: BTreeSet<String> = BTreeSet::new();
    for e in entries {
        if !present_ids.insert(e.id.as_str()) {
            duplicate_ids_set.insert(e.id.clone());
        }
    }

    // Dependencias base provistas por el runtime de Fabric / Minecraft
    let builtin: BTreeSet<&str> = ["minecraft", "java", "fabricloader", "mixinextras"]
        .into_iter()
        .collect();

    let mut missing_dependencies = Vec::new();
    let mut explicit_conflicts = Vec::new();
    let mut mixin_map: BTreeMap<&str, Vec<&str>> = BTreeMap::new();

    for e in entries {
        for dep in &e.depends {
            if !builtin.contains(dep.as_str()) && !present_ids.contains(dep.as_str()) {
                missing_dependencies.push(format!("{} requiere '{}'", e.id, dep));
            }
        }
        for conf in &e.conflicts {
            if present_ids.contains(conf.as_str()) {
                explicit_conflicts.push(format!("{} es incompatible con '{}'", e.id, conf));
            }
        }
        for target in &e.mixin_targets {
            mixin_map.entry(target.as_str()).or_default().push(e.id.as_str());
        }
    }

    let mut mixin_collisions = Vec::new();
    for (target, mods) in mixin_map {
        if mods.len() > 1 {
            mixin_collisions.push(format!(
                "{} modificado simultaneamente por: {}",
                target,
                mods.join(", ")
            ));
        }
    }

    let duplicate_ids: Vec<String> = duplicate_ids_set.into_iter().collect();
    let compatible = missing_dependencies.is_empty()
        && explicit_conflicts.is_empty()
        && duplicate_ids.is_empty();

    ConflictReport {
        compatible,
        mods_analyzed: entries.len(),
        missing_dependencies,
        explicit_conflicts,
        duplicate_ids,
        mixin_collisions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const HELLO_SHA: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
    const WORLD_SHA: &str = "486ea46224d1bb4fb680f34f7c9ad96a8f24ec88be73ea8e5a6c65260e9cb8a7";

    #[test]
    fn delta_sync_downloads_only_changed_and_removes_obsolete() {
        let dir = crate::testutil::tmpdir("modsync");
        // Mod ya existente con hash identico (no se debe volver a descargar)
        std::fs::write(dir.join("sodium.jar"), b"hello").unwrap();
        // Mod obsoleto que ya se quito del servidor (se debe borrar)
        std::fs::write(dir.join("old-removed-mod.jar"), b"legacy").unwrap();

        let lst = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = lst.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = lst.accept() {
                let mut buf = [0u8; 512];
                let _ = s.read(&mut buf);
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nworld",
                );
            }
        });

        let manifest = ServerModManifest {
            schema: 1,
            server_name: "DrakesCraft T2".into(),
            minecraft: "26.2".into(),
            loader: "fabric".into(),
            mods: vec![
                ModEntry {
                    file: "sodium.jar".into(),
                    id: "sodium".into(),
                    version: "0.6.0".into(),
                    sha256: HELLO_SHA.into(),
                    size: 5,
                    url: format!("http://127.0.0.1:{port}/sodium.jar"),
                    required: true,
                    depends: vec![],
                    conflicts: vec![],
                    mixin_targets: vec![],
                },
                ModEntry {
                    file: "iris.jar".into(),
                    id: "iris".into(),
                    version: "1.8.0".into(),
                    sha256: WORLD_SHA.into(),
                    size: 5,
                    url: format!("http://127.0.0.1:{port}/iris.jar"),
                    required: true,
                    depends: vec!["sodium".into()],
                    conflicts: vec![],
                    mixin_targets: vec![],
                },
            ],
        };

        let plan = compute_delta_sync(&dir, &manifest).unwrap();
        assert_eq!(plan.unchanged, vec!["sodium.jar"]);
        assert_eq!(plan.to_download.len(), 1);
        assert_eq!(plan.to_download[0].file, "iris.jar");
        assert_eq!(plan.to_remove, vec!["old-removed-mod.jar"]);

        let res = apply_delta_sync(&dir, &manifest).unwrap();
        assert_eq!(res.unchanged, vec!["sodium.jar"]);
        assert_eq!(res.downloaded, vec!["iris.jar"]);
        assert_eq!(res.removed, vec!["old-removed-mod.jar"]);
        assert_eq!(res.bytes_downloaded, 5);
        assert!(dir.join("iris.jar").is_file());
        assert!(!dir.join("old-removed-mod.jar").exists());
    }

    #[test]
    fn fabric_conflict_analyzer_catches_missing_deps_conflicts_and_mixin_collisions() {
        let entries = vec![
            ModEntry {
                file: "sodium.jar".into(),
                id: "sodium".into(),
                version: "0.6.0".into(),
                sha256: HELLO_SHA.into(),
                size: 5,
                url: "https://example.com/sodium.jar".into(),
                required: true,
                depends: vec!["fabric-api".into()],
                conflicts: vec!["optifabric".into()],
                mixin_targets: vec!["net.minecraft.client.renderer.LevelRenderer".into()],
            },
            ModEntry {
                file: "optifabric.jar".into(),
                id: "optifabric".into(),
                version: "1.0.0".into(),
                sha256: WORLD_SHA.into(),
                size: 5,
                url: "https://example.com/optifabric.jar".into(),
                required: true,
                depends: vec![],
                conflicts: vec![],
                mixin_targets: vec!["net.minecraft.client.renderer.LevelRenderer".into()],
            },
        ];

        let report = analyze_mod_conflicts(&entries);
        assert!(!report.compatible);
        assert_eq!(report.mods_analyzed, 2);
        assert_eq!(
            report.missing_dependencies,
            vec!["sodium requiere 'fabric-api'"]
        );
        assert_eq!(
            report.explicit_conflicts,
            vec!["sodium es incompatible con 'optifabric'"]
        );
        assert_eq!(
            report.mixin_collisions,
            vec!["net.minecraft.client.renderer.LevelRenderer modificado simultaneamente por: sodium, optifabric"]
        );
    }
}
