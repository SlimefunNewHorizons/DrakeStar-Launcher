//! Gestión robusta y multiplataforma de assets de Minecraft (audio, lenguajes, índices).
//!
//! Políticas:
//! 1. Prioridad:
//!    - Variable de entorno `DRAKESTAR_ASSETS_DIR` (si está definida).
//!    - Instancia local (`<inst>/assets`) si el índice correspondiente ya existe.
//!    - Cachés del sistema (Windows `%APPDATA%`, macOS `Library/Application Support`, Linux XDG/Flatpak/Prism/Vanilla),
//!      ÚNICAMENTE si el archivo `assets/indexes/<id>.json` existe en dicha caché.
//!    - Fallback final: `<inst>/assets`.
//! 2. Descarga verificada desde Mojang CDN:
//!    - Si el índice no existe en ninguna caché, se descarga el JSON de índice validado por SHA-1.
//!    - Objetos esenciales (idiomas, eventos) se sincronizan bajo demanda con SHA-1 en `objects/<xx>/<hash>`.

use crate::{AppError, AppResult};
use drakestar_core::fabric::AssetIndex;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Representación del JSON de índice de assets de Mojang (`indexes/<id>.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct MojangAssetIndex {
    #[serde(default)]
    pub objects: HashMap<String, MojangAssetObject>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MojangAssetObject {
    pub hash: String,
    pub size: u64,
}

/// Devuelve la lista de directorios candidatos de assets en el sistema actual.
pub fn candidate_assets_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // 1. Variable de entorno personalizada
    if let Some(env_dir) = std::env::var_os("DRAKESTAR_ASSETS_DIR").map(PathBuf::from) {
        dirs.push(env_dir);
    }

    // 2. Rutas POSIX / Linux / XDG / Flatpak
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let xdg_data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"));

        dirs.push(xdg_data.join("PrismLauncher/assets"));
        dirs.push(xdg_data.join("prismlauncher/assets"));
        dirs.push(xdg_data.join("multimc/assets"));
        dirs.push(xdg_data.join("ModrinthApp/assets"));

        // Flatpak Prism y Minecraft
        dirs.push(home.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/assets"));
        dirs.push(home.join(".var/app/com.mojang.Minecraft/data/.minecraft/assets"));

        // Vanilla Linux & ATLauncher
        dirs.push(home.join(".minecraft/assets"));
        dirs.push(home.join(".atlauncher/assets"));

        // macOS Application Support
        dirs.push(home.join("Library/Application Support/minecraft/assets"));
        dirs.push(home.join("Library/Application Support/PrismLauncher/assets"));
        dirs.push(home.join("Library/Application Support/MultiMC/assets"));
    }

    // 3. Rutas Windows (%APPDATA%, %LOCALAPPDATA%)
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        dirs.push(appdata.join(".minecraft/assets"));
        dirs.push(appdata.join("PrismLauncher/assets"));
        dirs.push(appdata.join("MultiMC/assets"));
        dirs.push(appdata.join("ATLauncher/assets"));
        dirs.push(appdata.join("ModrinthApp/assets"));
        dirs.push(appdata.join("com.modrinth.theseus/meta/assets"));
    }
    if let Some(localappdata) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        dirs.push(localappdata.join("PrismLauncher/assets"));
    }

    dirs
}

/// Comprueba si un directorio de assets candidato es válido para un `index_id` específico.
/// Es ESTRICTAMENTE OBLIGATORIO que exista `indexes/<id>.json` para evitar que el cliente
/// falle al arrancar con un índice inexistente.
pub fn is_valid_assets_cache(dir: &Path, index_id: Option<&str>) -> bool {
    if !dir.join("objects").is_dir() {
        return false;
    }
    if let Some(id) = index_id {
        let index_file = dir.join("indexes").join(format!("{id}.json"));
        index_file.is_file()
    } else {
        dir.join("indexes").is_dir()
    }
}

/// Resuelve el directorio de assets con prioridad determinista:
/// 1. `DRAKESTAR_ASSETS_DIR` (si está definida en entorno y existe).
/// 2. `<inst>/assets` si ya cuenta con el índice requerido.
/// 3. Cachés del sistema (Prism, Vanilla, etc.) SI Y SÓLO SI contienen `indexes/<id>.json`.
/// 4. Fallback: `<inst>/assets`.
pub fn resolve_assets_dir(inst: &Path, index_id: Option<&str>) -> PathBuf {
    // 1. Override por variable de entorno
    if let Some(env_dir) = std::env::var_os("DRAKESTAR_ASSETS_DIR").map(PathBuf::from) {
        if env_dir.is_dir() {
            return env_dir;
        }
    }

    // 2. Si la propia instancia ya tiene el índice requerido, no buscar afuera
    let inst_assets = inst.join("assets");
    if is_valid_assets_cache(&inst_assets, index_id) {
        return inst_assets;
    }

    // 3. Buscar en cachés externas compatibles que tengan el índice verificado
    for cand in candidate_assets_dirs() {
        if is_valid_assets_cache(&cand, index_id) {
            return cand;
        }
    }

    // 4. Fallback final: la carpeta propia de la instancia
    inst_assets
}

/// Asegura la presencia del archivo `indexes/<id>.json` en el directorio de assets.
/// Si no está presente, lo descarga desde la URL oficial de Mojang con validación SHA-1.
pub fn ensure_asset_index(assets_dir: &Path, index: &AssetIndex) -> AppResult<PathBuf> {
    let index_file = assets_dir.join("indexes").join(format!("{}.json", index.id));
    if index_file.is_file() {
        if let Ok(hash) = drakestar_net::sha1_file(&index_file) {
            if hash.eq_ignore_ascii_case(&index.sha1) {
                return Ok(index_file);
            }
        }
    }

    if let Some(p) = index_file.parent() {
        std::fs::create_dir_all(p)?;
    }

    drakestar_net::download_verified(&index.url, &index_file, &index.sha1, Some(index.size), |_| {})
        .map_err(|e| AppError::Net(format!("no se pudo descargar el índice de assets {}: {e}", index.id)))?;

    Ok(index_file)
}

/// Sincroniza objetos esenciales del índice (idiomas y metadatos base).
/// Mojang CDN: `https://resources.download.minecraft.net/<hash[0..2]>/<hash>`.
pub fn sync_essential_assets(assets_dir: &Path, index_path: &Path) -> AppResult<usize> {
    let content = std::fs::read_to_string(index_path)?;
    let parsed: MojangAssetIndex = serde_json::from_str(&content)?;

    let objects_dir = assets_dir.join("objects");
    let mut downloaded = 0;

    for (name, obj) in parsed.objects {
        let is_essential = name.starts_with("minecraft/lang/") || name == "pack.mcmeta";
        if !is_essential || obj.hash.len() < 2 {
            continue;
        }

        let prefix = &obj.hash[0..2];
        let target_path = objects_dir.join(prefix).join(&obj.hash);

        if target_path.is_file() {
            if let Ok(meta) = std::fs::metadata(&target_path) {
                if meta.len() == obj.size {
                    continue;
                }
            }
        }

        let url = format!("https://resources.download.minecraft.net/{prefix}/{}", obj.hash);
        if let Some(p) = target_path.parent() {
            std::fs::create_dir_all(p)?;
        }

        if drakestar_net::download_verified(&url, &target_path, &obj.hash, Some(obj.size), |_| {}).is_ok() {
            downloaded += 1;
        }
    }

    Ok(downloaded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_assets_cache_verification() {
        let tmp = std::env::temp_dir().join(format!("ds_test_assets_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);

        let objects = tmp.join("objects");
        let indexes = tmp.join("indexes");
        std::fs::create_dir_all(&objects).unwrap();
        std::fs::create_dir_all(&indexes).unwrap();

        // Sin archivo de índice para "26.2", debe rechazar
        assert!(!is_valid_assets_cache(&tmp, Some("26.2")));

        // Creamos el archivo de índice requerido
        std::fs::write(indexes.join("26.2.json"), b"{}").unwrap();
        assert!(is_valid_assets_cache(&tmp, Some("26.2")));

        // Otro índice distinto no debe validar
        assert!(!is_valid_assets_cache(&tmp, Some("1.20")));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_instance_priority_when_index_exists() {
        let tmp = std::env::temp_dir().join(format!("ds_test_inst_assets_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);

        let inst_indexes = tmp.join("assets").join("indexes");
        let inst_objects = tmp.join("assets").join("objects");
        std::fs::create_dir_all(&inst_indexes).unwrap();
        std::fs::create_dir_all(&inst_objects).unwrap();
        std::fs::write(inst_indexes.join("test_index.json"), b"{}").unwrap();

        let resolved = resolve_assets_dir(&tmp, Some("test_index"));
        assert_eq!(resolved, tmp.join("assets"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_parse_asset_index_json() {
        let json = r#"{
            "objects": {
                "minecraft/lang/en_us.json": {
                    "hash": "e8d641113b28b49e6d2bc6ecf3ee2f534604e4c3",
                    "size": 12345
                }
            }
        }"#;
        let index: MojangAssetIndex = serde_json::from_str(json).unwrap();
        assert_eq!(index.objects.len(), 1);
        let obj = index.objects.get("minecraft/lang/en_us.json").unwrap();
        assert_eq!(obj.hash, "e8d641113b28b49e6d2bc6ecf3ee2f534604e4c3");
        assert_eq!(obj.size, 12345);
    }
}
