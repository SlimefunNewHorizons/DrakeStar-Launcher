//! Importador 1-click de instancias de Prism Launcher (y forks compatibles).
//!
//! Lee la configuración (instance.cfg, mmc-pack.json) y migra mods, configs
//! y resourcepacks hacia un perfil nativo gestionado por DrakeStar.

use crate::{store::ProfileStore, AppError, AppResult, Layout};
use drakestar_core::{
    profile::{JavaSettings, Loader},
    Channel, Mode, Profile,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrismInstanceSummary {
    pub dir_name: String,
    pub path: String,
    pub name: String,
    pub icon: String,
    pub minecraft_version: String,
    pub loader_version: Option<String>,
    pub mod_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrismImportResult {
    pub profile_id: String,
    pub profile_name: String,
    pub mods_imported: usize,
}

/// Rutas canónicas donde Prism Launcher almacena instancias (Linux, macOS, Windows).
pub fn prism_instances_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    // 1. Linux / POSIX / macOS
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let xdg_data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"));

        roots.push(xdg_data.join("PrismLauncher/instances"));
        roots.push(xdg_data.join("prismlauncher/instances"));

        // Flatpak Prism Launcher
        roots.push(home.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/instances"));

        // macOS Application Support
        roots.push(home.join("Library/Application Support/PrismLauncher/instances"));
    }

    // 2. Windows (%APPDATA%, %LOCALAPPDATA%)
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        roots.push(appdata.join("PrismLauncher/instances"));
    }
    if let Some(localappdata) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        roots.push(localappdata.join("PrismLauncher/instances"));
    }

    roots
}

/// Lista todas las instancias detectadas en Prism Launcher
pub fn list_instances() -> Vec<PrismInstanceSummary> {
    let mut out = Vec::new();
    for root in prism_instances_roots() {
        if !root.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(summary) = inspect_instance(&path) {
                out.push(summary);
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Inspecciona el directorio de una instancia de Prism y extrae metadatos
pub fn inspect_instance(path: &Path) -> Option<PrismInstanceSummary> {
    let dir_name = path.file_name()?.to_string_lossy().to_string();
    let cfg_path = path.join("instance.cfg");
    let pack_path = path.join("mmc-pack.json");

    let mut name = dir_name.clone();
    let mut icon = "default".to_string();

    if let Ok(cfg) = std::fs::read_to_string(&cfg_path) {
        for line in cfg.lines() {
            let line = line.trim();
            if let Some(n) = line.strip_prefix("name=") {
                if !n.is_empty() {
                    name = n.to_string();
                }
            } else if let Some(ic) = line.strip_prefix("iconKey=") {
                if !ic.is_empty() {
                    icon = ic.to_string();
                }
            }
        }
    }

    let mut mc_version = "1.21.1".to_string();
    let mut loader_version = None;

    if let Ok(pack_str) = std::fs::read_to_string(&pack_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&pack_str) {
            if let Some(comps) = val.get("components").and_then(|c| c.as_array()) {
                for comp in comps {
                    let uid = comp.get("uid").and_then(|u| u.as_str()).unwrap_or_default();
                    let ver = comp.get("version").or_else(|| comp.get("cachedVersion")).and_then(|v| v.as_str());
                    if uid == "net.minecraft" {
                        if let Some(v) = ver {
                            mc_version = v.to_string();
                        }
                    } else if uid == "net.fabricmc.fabric-loader" {
                        loader_version = ver.map(str::to_string);
                    }
                }
            }
        }
    }

    // Contar mods
    let mc_dir = if path.join("minecraft").is_dir() {
        path.join("minecraft")
    } else {
        path.join(".minecraft")
    };
    let mods_dir = mc_dir.join("mods");
    let mod_count = if mods_dir.is_dir() {
        std::fs::read_dir(&mods_dir)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        name.ends_with(".jar") || name.ends_with(".jar.disabled")
                    })
                    .count()
            })
            .unwrap_or(0)
    } else {
        0
    };

    Some(PrismInstanceSummary {
        dir_name,
        path: path.to_string_lossy().to_string(),
        name,
        icon,
        minecraft_version: mc_version,
        loader_version,
        mod_count,
    })
}

/// Importa una instancia seleccionada hacia un perfil nativo de DrakeStar
pub fn import_instance(
    instance_dir_path: &Path,
    layout: &Layout,
    store: &ProfileStore,
) -> AppResult<PrismImportResult> {
    let summary = inspect_instance(instance_dir_path)
        .ok_or_else(|| AppError::Invalid("No es una instancia válida de Prism Launcher".into()))?;

    // Generar un ID limpio para el perfil
    let safe_slug: String = summary
        .dir_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let profile_id = format!("prism-{}", safe_slug.trim_matches('-'));

    let profile = Profile {
        schema: 1,
        id: profile_id.clone(),
        name: summary.name.clone(),
        mode: Mode::DrakesJava,
        channel: Channel::Stable,
        minecraft: summary.minecraft_version.clone(),
        loader: Loader::Fabric,
        java: JavaSettings::default(),
        server: None,
        preset: None,
        modpack: None,
        graphics: drakestar_core::profile::GraphicsSettings::default(),
        notes: format!("Importado desde Prism Launcher ({})", summary.dir_name),
    };

    // Guardar el perfil en el store de DrakeStar
    let _ = store.create(&profile);
    crate::instances::ensure_dirs(layout, &profile)?;

    // Directorio de destino de la instancia en DrakeStar
    let dst_inst = crate::fabric::instance_dir(layout, &profile.id);
    let dst_mods = dst_inst.join("mods");
    let dst_config = dst_inst.join("config");
    let dst_resourcepacks = dst_inst.join("resourcepacks");
    let dst_shaderpacks = dst_inst.join("shaderpacks");

    std::fs::create_dir_all(&dst_mods)?;

    // Origen dentro de Prism
    let src_mc = if instance_dir_path.join("minecraft").is_dir() {
        instance_dir_path.join("minecraft")
    } else {
        instance_dir_path.join(".minecraft")
    };

    let mut mods_imported = 0;
    let src_mods = src_mc.join("mods");
    if src_mods.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&src_mods) {
            for entry in entries.flatten() {
                let p = entry.path();
                let fname = entry.file_name();
                let sname = fname.to_string_lossy();
                if sname.ends_with(".jar") || sname.ends_with(".jar.disabled") {
                    let dst_file = dst_mods.join(&fname);
                    // Copiar o sobreescribir
                    if std::fs::copy(&p, &dst_file).is_ok() {
                        mods_imported += 1;
                    }
                }
            }
        }
    }

    // Copiar configs y packs si existen
    copy_dir_recursive_tolerant(&src_mc.join("config"), &dst_config);
    copy_dir_recursive_tolerant(&src_mc.join("resourcepacks"), &dst_resourcepacks);
    copy_dir_recursive_tolerant(&src_mc.join("shaderpacks"), &dst_shaderpacks);

    Ok(PrismImportResult {
        profile_id,
        profile_name: summary.name,
        mods_imported,
    })
}

fn copy_dir_recursive_tolerant(src: &Path, dst: &Path) {
    if !src.is_dir() {
        return;
    }
    let _ = std::fs::create_dir_all(dst);
    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let p = entry.path();
            let d = dst.join(entry.file_name());
            if p.is_dir() {
                copy_dir_recursive_tolerant(&p, &d);
            } else if p.is_file() {
                let _ = std::fs::copy(&p, &d);
            }
        }
    }
}
