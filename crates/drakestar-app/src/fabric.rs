//! Instalacion y lanzamiento de clientes Java gestionados (Fabric).
//!
//! Resuelve la version de Mojang y el loader de Fabric, descarga todo lo que
//! publican verificado por hash (SHA-1 de Mojang, SHA-256/1 de Fabric) a
//! `profiles/<id>/instance/` y arma el plan con `drakestar-core::fabric`.
//! La ejecucion la hace el `ProcessManager`, que ya es generico. La capa Tauri
//! envuelve estas funciones en `spawn_blocking` (hacen red y disco).
//!
//! shortcut: modo offline (sin cuenta Microsoft): singleplayer y servidores
//! `online-mode=false`. Y no se descargan los assets (sonidos/texturas por
//! ahora): el juego arranca igual, pero sin sonido hasta que se baje el indice.

use crate::{layout::write_atomic, AppError, AppResult, Layout};
use drakestar_core::fabric::{self, ClientLaunch};
use drakestar_core::{launch, profile::Loader, Mode, Profile};
use drakestar_net::{self, Hash};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MOJANG_MANIFEST: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2/versions/loader/";
const CLIENT_JAR: &str = "client.jar";

// ---------- JSON de Fabric (meta.fabricmc.net) ----------

#[derive(Debug, Deserialize, Serialize)]
pub struct FabricEntry {
    pub loader: FabricLoaderVer,
    #[serde(rename = "launcherMeta")]
    pub launcher_meta: LauncherMeta,
    #[serde(default)]
    pub intermediary: Option<FabricMaven>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FabricLoaderVer {
    pub version: String,
    pub stable: bool,
    pub maven: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FabricMaven {
    pub maven: String,
}

/// `mainClass` viene como objeto `{client, server}` en los loader nuevos y como
/// string simple en los viejos.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MainClass {
    Split(FabricMainClass),
    One(String),
}

impl MainClass {
    pub fn client(&self) -> &str {
        match self {
            MainClass::Split(m) => &m.client,
            MainClass::One(s) => s,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LauncherMeta {
    pub libraries: LauncherLibraries,
    #[serde(rename = "mainClass")]
    pub main_class: MainClass,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LauncherLibraries {
    pub common: Vec<FabricLib>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FabricMainClass {
    pub client: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FabricLib {
    pub name: String,
    /// Entradas viejas del meta solo traen el nombre: se ignoran.
    #[serde(default)]
    pub url: Option<String>,
    pub sha256: Option<String>,
    pub sha1: Option<String>,
    /// Algunas entradas no lo traen: 0 = no comprobar tamano.
    #[serde(default)]
    pub size: u64,
}

// ---------- estado para la UI ----------

#[derive(Debug, Clone, Serialize)]
pub struct ClientStatus {
    pub profile: String,
    pub minecraft: String,
    /// Java que pide el manifiesto (o el perfil, si lo fija).
    pub java_required: u32,
    pub loader_version: Option<String>,
    pub main_class: Option<String>,
    /// manifiesto resuelto y guardado en disco: se puede instalar y planear.
    pub resolved: bool,
    pub artifacts: usize,
    pub present: usize,
    pub bytes: u64,
    pub ready: bool,
    pub reason: Option<String>,
}

/// Un artefacto pendiente o ya presente.
#[derive(Debug, Clone)]
pub struct Planned {
    pub name: String,
    pub url: String,
    pub hash: String,
    pub size: u64,
    pub dest: PathBuf,
}

// ---------- resolucion (red) ----------

/// Lo resuelto: el JSON de Mojang tal cual vino, y el cache de Fabric (entrada
/// del meta + sidecars `.sha1` del loader y el intermediary).
#[derive(Debug)]
pub struct Resolved {
    pub version_text: String,
    fabric: FabricCache,
}

/// Lo que se guarda en disco de Fabric: la entrada del meta mas los hashes
/// resueltos de los jars que no traen hash.
#[derive(Debug, Serialize, Deserialize)]
pub struct FabricCache {
    pub entry: FabricEntry,
    pub sidecars: BTreeMap<String, String>,
}

fn parse_version(text: &str) -> AppResult<fabric::VersionInfo> {
    serde_json::from_str(text).map_err(|e| AppError::Invalid(format!("JSON de version invalido: {e}")))
}

/// Version de Mojang + loader estable mas nuevo de Fabric para ese Minecraft.
pub fn resolve(p: &Profile) -> AppResult<Resolved> {
    if p.mode != Mode::DrakesJava || p.loader != Loader::Fabric {
        return Err(AppError::Invalid(
            "solo los perfiles drakes-java con loader fabric tienen cliente gestionado".into(),
        ));
    }
    let manifest_text = drakestar_net::fetch_text(MOJANG_MANIFEST).map_err(AppError::from)?;
    let manifest: fabric::VersionManifest = serde_json::from_str(&manifest_text)
        .map_err(|e| AppError::Invalid(format!("JSON invalido en {MOJANG_MANIFEST}: {e}")))?;
    let entry = fabric::pick_version(&manifest.versions, &p.minecraft).ok_or_else(|| {
        AppError::NotFound(format!("Minecraft '{}' no esta en el manifiesto de versiones", p.minecraft))
    })?;
    let version_text = drakestar_net::fetch_text(&entry.url).map_err(AppError::from)?;
    let fabric_url = format!("{FABRIC_META}{}", p.minecraft);
    let fabric_text = drakestar_net::fetch_text(&fabric_url).map_err(AppError::from)?;
    let entries: Vec<FabricEntry> = serde_json::from_str(&fabric_text)
        .map_err(|e| AppError::Invalid(format!("JSON invalido en {fabric_url}: {e}")))?;
    // la lista viene de mas nuevo a mas viejo: el primer estable es el mas nuevo
    let loader = entries
        .into_iter()
        .find(|e| e.loader.stable)
        .ok_or_else(|| AppError::NotFound(format!("no hay loader estable de Fabric para Minecraft {}", p.minecraft)))?;
    // loader + intermediary: sin hash en el meta, se saca del sidecar del repo
    let mut sidecars = BTreeMap::new();
    let mut wanted = vec![loader.loader.maven.clone()];
    if let Some(i) = &loader.intermediary {
        wanted.push(i.maven.clone());
    }
    for name in wanted {
        let Some(url) = maven_url("https://maven.fabricmc.net/", &name) else { continue };
        let Some(sha) = sidecar_sha1(&url) else { continue };
        sidecars.insert(name, sha);
    }
    Ok(Resolved { version_text, fabric: FabricCache { entry: loader, sidecars } })
}

// ---------- disco ----------

pub fn instance_dir(l: &Layout, id: &str) -> PathBuf {
    l.instance_dir(id)
}

fn cache_version(l: &Layout, id: &str) -> PathBuf {
    instance_dir(l, id).join("version.json")
}

fn cache_fabric(l: &Layout, id: &str) -> PathBuf {
    instance_dir(l, id).join("fabric.json")
}

fn write_cache(path: &Path, json: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_atomic(path, json.as_bytes())?;
    Ok(())
}

/// Lo resuelto y cacheado, sin red. `None` si nunca se instalo.
pub fn cached(l: &Layout, id: &str) -> Option<(fabric::VersionInfo, FabricCache)> {
    let text = std::fs::read_to_string(cache_version(l, id)).ok()?;
    let v = parse_version(&text).ok()?;
    let f: FabricCache = serde_json::from_str(&std::fs::read_to_string(cache_fabric(l, id)).ok()?).ok()?;
    Some((v, f))
}

/// `group:artifact:version[:classifier]` -> ruta maven.
pub fn maven_path(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let (group, artifact, version) = (parts[0], parts[1], parts[2]);
    let classifier = parts.get(3);
    if group.is_empty() || artifact.is_empty() || version.is_empty() {
        return None;
    }
    let file = match classifier {
        Some(c) if !c.is_empty() => format!("{artifact}-{version}-{c}.jar"),
        _ => format!("{artifact}-{version}.jar"),
    };
    Some(format!("{}/{}/{}/{}", group.replace('.', "/"), artifact, version, file))
}

/// URL maven: base + ruta maven. Las bases en `http://` del meta (Maven Central
/// antiguo) se suben a `https://`: el repo lo sirve igual y no bajamos nada en
/// claro, que es la politica de `drakestar-net`.
pub fn maven_url(base: &str, name: &str) -> Option<String> {
    let path = maven_path(name)?;
    let base = base.trim_end_matches('/');
    let base = if let Some(rest) = base.strip_prefix("http://") { format!("https://{rest}") } else { base.to_string() };
    Some(format!("{base}/{path}"))
}

/// `.sha1` que publica el repo maven junto al jar: es el hash que verificamos
/// para el loader y el intermediary, que el meta no trae.
pub fn sidecar_sha1(jar_url: &str) -> Option<String> {
    let text = drakestar_net::fetch_text(&format!("{jar_url}.sha1")).ok()?;
    let s = text.trim();
    Hash::parse(s)?;
    let Hash::Sha1(_) = Hash::parse(s)? else { return None };
    Some(s.to_string())
}

/// Artefactos a tener en disco, en orden de classpath (librerias; cliente al
/// final). El loader y el intermediary van justo antes del cliente.
pub fn artifacts(l: &Layout, id: &str, v: &fabric::VersionInfo, f: &FabricCache) -> Vec<Planned> {
    let mut out = vec![];
    for lib in &v.libraries {
        if let Some((Some(art), _)) = fabric::select(lib) {
            out.push(Planned {
                name: lib.name.clone(),
                url: art.url.clone(),
                hash: art.sha1.clone(),
                size: art.size,
                dest: instance_dir(l, id).join("libraries").join(&art.path),
            });
        }
    }
    for lib in &f.entry.launcher_meta.libraries.common {
        // entradas viejas del meta sin url o sin hash: no se pueden verificar
        let (Some(url), Some(hash)) = (lib.url.as_ref(), lib.sha256.clone().or_else(|| lib.sha1.clone())) else {
            continue;
        };
        if let Some(path) = maven_path(&lib.name) {
            out.push(Planned {
                name: lib.name.clone(),
                url: maven_url(url, &lib.name).unwrap_or_default(),
                hash,
                size: lib.size,
                dest: instance_dir(l, id).join("libraries").join(path),
            });
        }
    }
    // loader + intermediary: el hash sale del sidecar del repo maven
    let mut wanted = vec![f.entry.loader.maven.clone()];
    if let Some(i) = &f.entry.intermediary {
        wanted.push(i.maven.clone());
    }
    for name in wanted {
        let (Some(path), Some(hash)) = (maven_path(&name), f.sidecars.get(&name)) else { continue };
        out.push(Planned {
            name: name.clone(),
            url: maven_url("https://maven.fabricmc.net/", &name).unwrap_or_default(),
            hash: hash.clone(),
            size: 0,
            dest: instance_dir(l, id).join("libraries").join(path),
        });
    }
    // el cliente va ultimo: las librerias (y sus versiones) tienen prioridad
    out.push(Planned {
        name: "minecraft".into(),
        url: v.downloads.client.url.clone(),
        hash: v.downloads.client.sha1.clone(),
        size: v.downloads.client.size,
        dest: instance_dir(l, id).join(CLIENT_JAR),
    });
    out
}

fn hash_of(path: &Path, hash: &Hash) -> Result<String, drakestar_net::NetError> {
    match hash {
        Hash::Sha256(_) => drakestar_net::sha256_file(path),
        Hash::Sha1(_) => drakestar_net::sha1_file(path),
    }
}

/// El archivo esta y su hash coincide con el que manda el manifiesto.
fn installed_ok(a: &Planned, hash: &Hash) -> bool {
    a.dest.is_file() && hash_of(&a.dest, hash).map(|h| h.eq_ignore_ascii_case(&a.hash)).unwrap_or(false)
}

/// Descarga lo que falta (lo que ya esta con hash valido se salta) y guarda el
/// manifiesto resuelto. Devuelve el estado final.
pub fn install(l: &Layout, p: &Profile) -> AppResult<ClientStatus> {
    let r = resolve(p)?;
    let version = parse_version(&r.version_text)?;
    let dir = instance_dir(l, &p.id);
    std::fs::create_dir_all(dir.join("libraries"))?;
    std::fs::create_dir_all(dir.join("natives"))?;
    write_cache(&cache_version(l, &p.id), &r.version_text)?;
    write_cache(&cache_fabric(l, &p.id), &serde_json::to_string_pretty(&r.fabric)?)?;
    let planned = artifacts(l, &p.id, &version, &r.fabric);
    let mut failed = vec![];
    for a in &planned {
        let Some(hash) = Hash::parse(&a.hash) else {
            failed.push(format!("{} (hash invalido)", a.name));
            continue;
        };
        if installed_ok(a, &hash) {
            continue;
        }
        match drakestar_net::download_verified(&a.url, &a.dest, &a.hash, Some(a.size), |_| {}) {
            Ok(_) => {}
            Err(e) => failed.push(format!("{} ({e})", a.name)),
        }
    }
    if !failed.is_empty() {
        return Err(AppError::Net(format!("no se pudieron verificar {} artefactos: {}", failed.len(), failed.join("; "))));
    }
    // Asegura el índice de assets oficial (sonidos/idiomas) para evitar cliente mudo o fallo de index
    if let Some(ai) = &version.asset_index {
        let assets_dir = resolve_assets_dir_for_version(&dir, Some(&ai.id));
        if let Ok(idx_path) = crate::assets::ensure_asset_index(&assets_dir, ai) {
            let _ = crate::assets::sync_essential_assets(&assets_dir, &idx_path);
        }
    }
    let mut st = status_from(&version, &r.fabric, &planned);
    st.profile = p.id.clone();
    st.reason = Some(format!(
        "Minecraft {} con Fabric {} listo",
        version.id, r.fabric.entry.loader.version
    ));
    Ok(st)
}

/// Estado sin red: lo que dice el cache y lo que hay en disco.
pub fn status(l: &Layout, p: &Profile) -> ClientStatus {
    let java_required = p.java.major.unwrap_or_else(|| launch::required_java(&p.minecraft));
    let Some((version, cache)) = cached(l, &p.id) else {
        return ClientStatus {
            profile: p.id.clone(),
            minecraft: p.minecraft.clone(),
            java_required,
            loader_version: None,
            main_class: None,
            resolved: false,
            artifacts: 0,
            present: 0,
            bytes: 0,
            ready: false,
            reason: Some("el cliente no esta instalado todavia".into()),
        };
    };
    let planned = artifacts(l, &p.id, &version, &cache);
    let mut st = status_from(&version, &cache, &planned);
    st.profile = p.id.clone();
    st.java_required = java_required;
    st.reason = Some(if st.ready {
        format!("Minecraft {} con Fabric {}", version.id, cache.entry.loader.version)
    } else {
        format!("faltan {} de {} artefactos", st.artifacts - st.present, st.artifacts)
    });
    st
}

fn status_from(v: &fabric::VersionInfo, f: &FabricCache, planned: &[Planned]) -> ClientStatus {
    let present = planned
        .iter()
        .filter(|a| Hash::parse(&a.hash).map(|h| installed_ok(a, &h)).unwrap_or(false))
        .count();
    let bytes = planned
        .iter()
        .filter_map(|a| std::fs::metadata(&a.dest).ok())
        .map(|m| m.len())
        .sum();
    ClientStatus {
        profile: String::new(),
        minecraft: v.id.clone(),
        java_required: v.java_version.as_ref().map(|j| j.major_version).unwrap_or(0),
        loader_version: Some(f.entry.loader.version.clone()),
        main_class: Some(f.entry.launcher_meta.main_class.client().to_string()),
        resolved: true,
        artifacts: planned.len(),
        present,
        bytes,
        ready: present == planned.len(),
        reason: None,
    }
}

/// Classpath de lo instalado, en el orden de `artifacts`.
pub fn classpath(l: &Layout, p: &Profile) -> AppResult<String> {
    let (version, loader) = cached(l, &p.id).ok_or_else(|| AppError::NotFound("cliente no instalado".into()))?;
    let planned = artifacts(l, &p.id, &version, &loader);
    let miss: Vec<String> = planned.iter().filter(|a| !a.dest.is_file()).map(|a| a.name.clone()).collect();
    if !miss.is_empty() {
        return Err(AppError::NotFound(format!(
            "faltan {} artefactos (instala el cliente): {}",
            miss.len(),
            miss.join(", ")
        )));
    }
    // separador de classpath por sistema
    let sep = if cfg!(target_os = "windows") { ";" } else { ":" };
    Ok(planned.iter().map(|a| a.dest.to_string_lossy().to_string()).collect::<Vec<_>>().join(sep))
}

/// Resuelve el directorio de assets usando el módulo soberano multiplataforma.
/// Se delega a `crate::assets::resolve_assets_dir` garantizando verificación de índices
/// y compatibilidad con Windows, macOS, Linux XDG y Flatpak.
pub fn resolve_assets_dir(inst: &Path) -> PathBuf {
    crate::assets::resolve_assets_dir(inst, None)
}

pub fn resolve_assets_dir_for_version(inst: &Path, index_id: Option<&str>) -> PathBuf {
    crate::assets::resolve_assets_dir(inst, index_id)
}

/// Plan de lanzamiento del cliente (sin ejecutar).
pub fn plan(l: &Layout, p: &Profile, java_bin: &str, username: &str) -> AppResult<fabric::ClientPlan> {
    let (version, cache) = cached(l, &p.id).ok_or_else(|| AppError::NotFound("cliente no instalado".into()))?;
    let inst = instance_dir(l, &p.id);
    let cp = classpath(l, p)?;
    let index_id = version.asset_index.as_ref().map(|a| a.id.as_str());
    let assets_dir = resolve_assets_dir_for_version(&inst, index_id);
    fabric::client_plan(
        p,
        &version,
        &ClientLaunch {
            java_bin,
            username,
            classpath: &cp,
            game_dir: &inst.to_string_lossy(),
            assets_dir: &assets_dir.to_string_lossy(),
            natives_dir: &inst.join("natives").to_string_lossy(),
            version_label: &format!("fabric-loader-{}-{}", cache.entry.loader.version, p.minecraft),
            launcher_name: "DrakeStar",
            launcher_version: env!("CARGO_PKG_VERSION"),
            uuid: None,
            access_token: None,
            user_type: None,
        },
    )
    .ok_or_else(|| AppError::Invalid("este perfil no lanza un cliente Fabric".into()))
}

/// Nombre de usuario offline: de los ajustes, o el id del perfil.
pub fn username(s: &crate::settings::Settings, p: &Profile) -> String {
    s.username
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .unwrap_or(&p.id)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::sample;
    use drakestar_core::Channel;

    fn profile() -> Profile {
        let mut p = sample("cli", Mode::DrakesJava);
        p.loader = Loader::Fabric;
        p.channel = Channel::Stable;
        p
    }

    #[test]
    fn maven_paths_and_urls() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.20.1").unwrap(),
            "net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar"
        );
        assert_eq!(maven_path("org.ow2.asm:asm:9.10.1").unwrap(), "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar");
        assert_eq!(
            maven_path("com.mojang:jtracy:1.0.37:natives-linux").unwrap(),
            "com/mojang/jtracy/1.0.37/jtracy-1.0.37-natives-linux.jar"
        );
        assert_eq!(
            maven_url("https://maven.fabricmc.net/", "net.fabricmc:fabric-loader:0.20.1").unwrap(),
            "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.20.1/fabric-loader-0.20.1.jar"
        );
        assert!(maven_path("roto").is_none());
        assert!(maven_path("a:b:").is_none());
    }

    #[test]
    fn artifacts_end_with_the_client_and_filter_by_os() {
        let l = Layout::new(crate::testutil::tmpdir("fab-art"));
        let s = "a".repeat(40);
        let v: fabric::VersionInfo = serde_json::from_str(&format!(
            r#"{{ "id": "26.2", "mainClass": "m", "javaVersion": {{ "majorVersion": 25 }},
                 "downloads": {{ "client": {{ "sha1": "{s}", "size": 3, "url": "https://a/c.jar" }} }},
                 "assetIndex": {{ "id": "32", "sha1": "b", "size": 1, "url": "https://a/i.json" }},
                 "libraries": [
                   {{ "name": "com.mojang:logging:1.0.0", "downloads": {{ "artifact": {{ "path": "com/mojang/logging/1.0.0/logging-1.0.0.jar", "sha1": "{s}", "size": 1, "url": "https://a/l.jar" }} }} }},
                   {{ "name": "org.x:only-win:1", "rules": [ {{ "action": "allow", "os": {{ "name": "windows" }} }}, {{ "action": "disallow" }} ],
                     "downloads": {{ "artifact": {{ "path": "w.jar", "sha1": "{s}", "size": 1, "url": "https://a/w.jar" }} }} }}
                 ] }}"#
        ))
        .unwrap();
        let f = FabricCache {
            entry: FabricEntry {
                loader: FabricLoaderVer {
                    version: "0.20.1".into(),
                    stable: true,
                    maven: "net.fabricmc:fabric-loader:0.20.1".into(),
                },
                launcher_meta: serde_json::from_str::<LauncherMeta>(
                    r#"{ "libraries": { "common": [ { "name": "org.ow2.asm:asm:9.10.1", "url": "http://repo.maven.apache.org/maven2/", "sha256": "ed825d10ab1399c8c0cb669e688cf0c8c82629b4c8399b58352b68e92ca10fcb", "size": 1 },
                                                    { "name": "net.minecraft:launchwrapper:1.12" } ] },
                                  "mainClass": { "client": "net.fabricmc.loader.impl.launch.knot.KnotClient" } }"#,
                )
                .unwrap(),
                intermediary: Some(FabricMaven { maven: "net.fabricmc:intermediary:0.0.0".into() }),
            },
            sidecars: [
                ("net.fabricmc:fabric-loader:0.20.1".to_string(), s.clone()),
                ("net.fabricmc:intermediary:0.0.0".to_string(), s.clone()),
            ]
            .into_iter()
            .collect(),
        };
        let a = artifacts(&l, "cli", &v, &f);
        // logging + asm(meta) + loader(sidecar) + intermediary(sidecar) + cliente
        assert_eq!(a.len(), 5, "nombres: {:?}", a.iter().map(|x| x.name.clone()).collect::<Vec<_>>());
        assert_eq!(a.last().unwrap().name, "minecraft");
        assert_eq!(a.last().unwrap().dest, l.instance_dir("cli").join("client.jar"));
        assert!(a[..a.len() - 1].iter().all(|x| x.dest.starts_with(l.instance_dir("cli").join("libraries"))));
        // la base http del meta se sube a https (no se baja nada en claro)
        let asm = a.iter().find(|x| x.name == "org.ow2.asm:asm:9.10.1").unwrap();
        assert!(asm.url.starts_with("https://repo.maven.apache.org/maven2/"), "{}", asm.url);
        // loader e intermediary entran con el hash del sidecar del repo de Fabric
        for n in ["net.fabricmc:fabric-loader:0.20.1", "net.fabricmc:intermediary:0.0.0"] {
            let x = a.iter().find(|x| x.name == n).unwrap();
            assert_eq!(x.hash, s);
            assert!(x.url.starts_with("https://maven.fabricmc.net/"), "{}", x.url);
        }
        // la libreria solo-Windows no entra en Linux; la entrada sin url/hash se ignora
        assert!(!a.iter().any(|x| x.name.contains("only-win")));
        assert!(!a.iter().any(|x| x.name.contains("launchwrapper")));
    }

    #[test]
    fn status_without_cache_is_not_ready() {
        let l = Layout::new(crate::testutil::tmpdir("fab-status"));
        let s = status(&l, &profile());
        assert!(!s.resolved && !s.ready && s.artifacts == 0);
        assert!(s.reason.is_some());
        assert_eq!(s.java_required, 25);
    }

    #[test]
    fn classpath_needs_everything_installed() {
        let l = Layout::new(crate::testutil::tmpdir("fab-cp"));
        assert!(classpath(&l, &profile()).is_err());
    }

    #[test]
    fn plan_needs_the_client_installed() {
        let l = Layout::new(crate::testutil::tmpdir("fab-plan"));
        assert!(plan(&l, &profile(), "java", "nix").is_err());
    }

    #[test]
    fn username_falls_back_to_the_profile_id() {
        let p = profile();
        let s = crate::settings::Settings::default();
        assert_eq!(username(&s, &p), "cli");
        let mut s2 = s.clone();
        s2.username = Some("  ".into());
        assert_eq!(username(&s2, &p), "cli");
        s2.username = Some("nix".into());
        assert_eq!(username(&s2, &p), "nix");
    }

    /// Smoke test REAL contra los servidores de Mojang y Fabric: resuelve,
    /// descarga y verifica todo, y arranca el JVM con el plan. No corre en CI
    /// (necesita red y descarga ~100 MB); ejecutar a mano:
    /// `cargo test -p drakestar-app fabric::tests::real_install_and_launch -- --ignored --nocapture`
    /// Con pantalla (X11/Wayland) el cliente llega a abrir la ventana; sin ella
    /// igualmente debe cargar Fabric y fallar recien al pedir el display.
    #[test]
    #[ignore]
    fn real_install_and_launch() {
        use crate::process::{EventSink, ProcEvent};
        use std::sync::Mutex;
        use std::time::{Duration, Instant};

        #[derive(Default)]
        struct Collect(Mutex<Vec<ProcEvent>>);
        impl EventSink for Collect {
            fn emit(&self, ev: ProcEvent) {
                self.0.lock().unwrap().push(ev);
            }
        }

        let l = Layout::new(crate::testutil::tmpdir("fab-real"));
        let p = profile();
        let st = install(&l, &p).expect("instalacion real");
        eprintln!("estado: {st:?}");
        assert!(st.ready, "el cliente no quedo listo: {}", st.reason.unwrap_or_default());
        assert_eq!(st.java_required, 25);

        let plan = plan(&l, &p, "java", "tester").expect("plan");
        assert!(plan.args.contains(&"-cp".to_string()));
        let cp = plan.args[plan.args.iter().position(|a| a == "-cp").unwrap() + 1].clone();
        assert!(cp.split(if cfg!(target_os = "windows") { ';' } else { ':' }).all(|j| std::path::Path::new(j).is_file()));

        let svc = crate::service::Service::new(l.clone(), std::sync::Arc::new(Collect::default()));
        svc.create_profile(&p).unwrap();
        let started = svc.client_start("cli");
        match started {
            Ok(st2) => eprintln!("arrancado: {st2:?}"),
            Err(e) => panic!("no arranco: {e}"),
        }
        // esperar a que el proceso termine (sin display muere solo) o 120 s
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(120) && svc.procs.is_active("cli") {
            std::thread::sleep(Duration::from_millis(200));
        }
        let hist = svc.procs.history("cli");
        let text: String = hist.iter().map(|h| h.line.clone()).collect::<Vec<_>>().join("\n");
        eprintln!("---- salida ({} lineas) ----\n{text}", hist.len());
        assert!(text.contains("Fabric") || text.to_lowercase().contains("fabric"), "no se ve Fabric cargando");
        // limpieza: si seguia vivo, se detiene
        let _ = svc.procs.stop("cli", Some("stop"), Duration::from_secs(5));
        let _ = svc.procs.kill("cli");
    }
}
