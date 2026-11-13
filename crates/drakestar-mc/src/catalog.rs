//! Catálogo Modrinth por instancia. Descargas verificadas, dependencias requeridas y propiedad de archivos.
use crate::instances::{Instance, Loader};
use crate::{mods::get_json, write_atomic, Dirs, McError, McResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

const API: &str = "https://api.modrinth.com/v2";
const MAX_FILE: u64 = 256 * 1024 * 1024;
const INDEX: &str = ".drakestar-catalog.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Mod,
    Resourcepack,
    Shader,
}
impl Kind {
    fn project_type(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Resourcepack => "resourcepack",
            Self::Shader => "shader",
        }
    }
    fn folder(self) -> &'static str {
        match self {
            Self::Mod => "mods",
            Self::Resourcepack => "resourcepacks",
            Self::Shader => "shaderpacks",
        }
    }
    fn extension(self) -> &'static str {
        if self == Self::Mod {
            ".jar"
        } else {
            ".zip"
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
}
#[derive(Debug, Deserialize)]
struct Search {
    hits: Vec<Hit>,
}
#[derive(Debug, Deserialize)]
struct Project {
    id: String,
    title: String,
    project_type: Kind,
    #[serde(default)]
    client_side: String,
}
#[derive(Debug, Deserialize)]
struct Version {
    id: String,
    project_id: String,
    version_type: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    files: Vec<File>,
    #[serde(default)]
    dependencies: Vec<Dependency>,
}
#[derive(Debug, Deserialize)]
struct File {
    url: String,
    filename: String,
    primary: bool,
    hashes: Hashes,
}
#[derive(Debug, Deserialize)]
struct Hashes {
    sha512: String,
}
#[derive(Debug, Deserialize)]
struct Dependency {
    project_id: Option<String>,
    version_id: Option<String>,
    dependency_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub kind: Kind,
    pub filename: String,
    pub sha512: String,
    pub minecraft: String,
    pub dependencies: Vec<String>,
    /// false si el mismo archivo ya existía antes de usar el catálogo: nunca se borra.
    pub owned: bool,
}
struct Planned {
    entry: Entry,
    url: String,
}
fn invalid(s: impl Into<String>) -> McError {
    McError::Invalid(s.into())
}
fn identifier(s: &str) -> McResult<()> {
    if s.is_empty()
        || s.len() > 128
        || !s
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(invalid("Identificador Modrinth no válido"));
    }
    Ok(())
}
fn encode(s: &str) -> String {
    s.bytes().map(|b| format!("%{b:02X}")).collect()
}
fn supported(inst: &Instance, kind: Kind) -> McResult<()> {
    inst.validate()?;
    if kind != Kind::Resourcepack && inst.loader != Loader::Fabric {
        return Err(invalid(
            "Los mods y shaders del catálogo necesitan una instancia Fabric.",
        ));
    }
    Ok(())
}
fn regular(path: &Path) -> McResult<()> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => {
            Err(invalid("El catálogo no admite enlaces simbólicos"))
        }
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
fn root(d: &Dirs, inst: &Instance) -> McResult<PathBuf> {
    inst.validate()?;
    regular(&d.instances())?;
    let root = d.instance(&inst.id);
    regular(&root)?;
    std::fs::create_dir_all(&root)?;
    Ok(root)
}
fn file_path(root: &Path, kind: Kind, name: &str) -> McResult<PathBuf> {
    if name.len() > 240
        || name.starts_with('.')
        || !name.ends_with(kind.extension())
        || name
            .chars()
            .any(|c| c.is_control() || "/\\:<>\"|?*".contains(c))
        || name.ends_with([' ', '.'])
    {
        return Err(invalid("Nombre de archivo no seguro en Modrinth"));
    }
    let folder = root.join(kind.folder());
    regular(&folder)?;
    let dest = folder.join(name);
    regular(&dest)?;
    Ok(dest)
}
fn hash(bytes: &[u8]) -> String {
    Sha512::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn download_url(url: &str) -> McResult<()> {
    if !url.starts_with("https://cdn.modrinth.com/") || url.contains(['\\', '\r', '\n']) {
        return Err(invalid(
            "La descarga debe proceder del CDN HTTPS de Modrinth",
        ));
    }
    Ok(())
}
fn fetch(url: &str, expected: &str) -> McResult<Vec<u8>> {
    download_url(url)?;
    let response = ureq::get(url)
        .set("User-Agent", "SlimefunNewHorizons/DrakeStar-Launcher")
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|e| McError::Net(e.to_string()))?;
    download_url(response.get_url())?;
    let mut bytes = vec![];
    response
        .into_reader()
        .take(MAX_FILE + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(invalid("Archivo demasiado grande (máximo 256 MiB)"));
    }
    if expected.len() != 128 || !hash(&bytes).eq_ignore_ascii_case(expected) {
        return Err(invalid("SHA-512 del archivo no coincide"));
    }
    Ok(bytes)
}
fn load(root: &Path) -> McResult<Vec<Entry>> {
    let path = root.join(INDEX);
    regular(&path)?;
    match std::fs::read(path) {
        Ok(b) => Ok(serde_json::from_slice(&b)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.into()),
    }
}
fn save(root: &Path, entries: &[Entry]) -> McResult<()> {
    regular(&root.join(INDEX))?;
    write_atomic(&root.join(INDEX), &serde_json::to_vec_pretty(entries)?)?;
    Ok(())
}

pub fn search(inst: &Instance, kind: Kind, query: &str) -> McResult<Vec<Hit>> {
    supported(inst, kind)?;
    if query.len() > 200 {
        return Err(invalid("Búsqueda demasiado larga"));
    }
    let mut facets = vec![
        vec![format!("project_type:{}", kind.project_type())],
        vec![format!("versions:{}", inst.mc)],
    ];
    if kind == Kind::Mod {
        facets.push(vec!["categories:fabric".into()]);
        facets.push(vec![
            "client_side:optional".into(),
            "client_side:required".into(),
        ]);
    }
    let search: Search = get_json(&format!(
        "{API}/search?limit=20&query={}&facets={}",
        encode(query),
        encode(&serde_json::to_string(&facets)?)
    ))?;
    Ok(search.hits)
}
pub fn list(d: &Dirs, inst: &Instance) -> McResult<Vec<Entry>> {
    load(&root(d, inst)?)
}

fn compatible(v: &Version, inst: &Instance, kind: Kind) -> bool {
    v.game_versions.contains(&inst.mc)
        && (kind != Kind::Mod || v.loaders.iter().any(|s| s == "fabric"))
}
fn resolve(
    inst: &Instance,
    project: &str,
    pinned: Option<&str>,
    existing: &[Entry],
    pending: &mut Vec<Planned>,
    visiting: &mut BTreeSet<String>,
) -> McResult<String> {
    identifier(project)?;
    let p: Project = get_json(&format!("{API}/project/{project}"))?;
    supported(inst, p.project_type)?;
    if p.project_type == Kind::Mod && p.client_side == "unsupported" {
        return Err(invalid("Este mod es solo para servidores"));
    }
    if let Some(e) = existing.iter().find(|e| e.project_id == p.id).or_else(|| {
        pending
            .iter()
            .map(|p| &p.entry)
            .find(|e| e.project_id == p.id)
    }) {
        if e.minecraft != inst.mc {
            return Err(invalid(format!("{} fue instalado para Minecraft {}. Quítalo antes de cambiar el contenido de esta instancia.", e.title, e.minecraft)));
        }
        if pinned.is_some_and(|id| id != e.version_id) {
            return Err(invalid(format!("{} requiere otra versión de una dependencia; quita primero los proyectos que la usan.", p.title)));
        }
        return Ok(p.id);
    }
    if pending.len() + visiting.len() >= 64 || !visiting.insert(p.id.clone()) {
        return Err(invalid("Demasiadas dependencias o ciclo de dependencias"));
    }
    let v: Version = if let Some(id) = pinned {
        identifier(id)?;
        get_json(&format!("{API}/version/{id}"))?
    } else {
        let loaders = if p.project_type == Kind::Mod {
            "&loaders=%5B%22fabric%22%5D"
        } else {
            ""
        };
        let versions: Vec<Version> = get_json(&format!(
            "{API}/project/{}/version?game_versions={}{loaders}",
            p.id,
            encode(&serde_json::to_string(&[&inst.mc])?)
        ))?;
        // Solo releases: no instala versiones experimentales automáticamente.
        versions
            .into_iter()
            .find(|v| v.version_type == "release" && compatible(v, inst, p.project_type))
            .ok_or_else(|| {
                invalid(format!(
                    "{}: no hay release compatible con {}",
                    p.title, inst.mc
                ))
            })?
    };
    if v.project_id != p.id || !compatible(&v, inst, p.project_type) {
        return Err(invalid("Dependencia incompatible con la instancia"));
    }
    let f = v
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| v.files.first())
        .ok_or_else(|| invalid("Versión sin archivos"))?;
    let mut dependencies = vec![];
    for dep in v
        .dependencies
        .iter()
        .filter(|d| d.dependency_type == "required")
    {
        let dep_project =
            match (&dep.project_id, &dep.version_id) {
                (Some(id), _) => id.clone(),
                (None, Some(id)) => {
                    identifier(id)?;
                    let dv: Version = get_json(&format!("{API}/version/{id}"))?;
                    dv.project_id
                }
                _ => return Err(invalid(
                    "Dependencia externa: debe instalarse manualmente antes de usar este proyecto",
                )),
            };
        dependencies.push(resolve(
            inst,
            &dep_project,
            dep.version_id.as_deref(),
            existing,
            pending,
            visiting,
        )?);
    }
    if p.project_type == Kind::Shader {
        dependencies.push(resolve(inst, "iris", None, existing, pending, visiting)?);
    }
    download_url(&f.url)?;
    pending.push(Planned {
        entry: Entry {
            project_id: p.id.clone(),
            version_id: v.id,
            title: p.title,
            kind: p.project_type,
            filename: f.filename.clone(),
            sha512: f.hashes.sha512.clone(),
            minecraft: inst.mc.clone(),
            dependencies,
            owned: true,
        },
        url: f.url.clone(),
    });
    visiting.remove(&p.id);
    Ok(p.id)
}

pub fn install(d: &Dirs, inst: &Instance, kind: Kind, project: &str) -> McResult<Vec<Entry>> {
    supported(inst, kind)?;
    let root = root(d, inst)?;
    let mut entries = load(&root)?;
    for e in &entries {
        let path = file_path(&root, e.kind, &e.filename)?;
        if !path.exists() || hash(&std::fs::read(&path)?) != e.sha512 {
            return Err(invalid(format!("{} fue eliminado o modificado fuera del catálogo. Revisa su archivo y quítalo de la lista antes de instalar más contenido.", e.title)));
        }
    }
    // No se adivinan archivos por el slug ni se borran versiones por prefijos.
    let mut pending = vec![];
    let id = resolve(
        inst,
        project,
        None,
        &entries,
        &mut pending,
        &mut BTreeSet::new(),
    )?;
    let actual = entries
        .iter()
        .find(|e| e.project_id == id)
        .or_else(|| {
            pending
                .iter()
                .map(|p| &p.entry)
                .find(|e| e.project_id == id)
        })
        .unwrap();
    if actual.kind != kind {
        return Err(invalid("El tipo del proyecto no coincide con el catálogo"));
    }
    // Validar todas las rutas y colisiones antes de descargar/escribir.
    let mut paths = BTreeSet::new();
    for p in &mut pending {
        let path = file_path(&root, p.entry.kind, &p.entry.filename)?;
        if !paths.insert(path.clone())
            || entries
                .iter()
                .any(|e| e.kind == p.entry.kind && e.filename == p.entry.filename)
        {
            return Err(invalid("Dos proyectos usan el mismo archivo"));
        }
        if path.exists() {
            if hash(&std::fs::read(&path)?) != p.entry.sha512 {
                return Err(invalid(format!(
                    "Ya existe un archivo diferente: {}. Revísalo en la carpeta de la instancia.",
                    p.entry.filename
                )));
            }
            p.entry.owned = false;
        }
    }
    for p in pending {
        let path = file_path(&root, p.entry.kind, &p.entry.filename)?;
        if p.entry.owned {
            let bytes = fetch(&p.url, &p.entry.sha512)?;
            std::fs::create_dir_all(path.parent().unwrap())?;
            write_atomic(&path, &bytes)?;
        }
        entries.push(p.entry.clone());
        // Si falla una descarga posterior, las dependencias ya instaladas quedan visibles y se pueden quitar.
        if let Err(e) = save(&root, &entries) {
            if p.entry.owned {
                let _ = std::fs::remove_file(path);
            }
            return Err(e);
        }
    }
    Ok(entries)
}

pub fn remove(d: &Dirs, inst: &Instance, project: &str) -> McResult<Vec<Entry>> {
    identifier(project)?;
    let root = root(d, inst)?;
    let mut entries = load(&root)?;
    let Some(entry) = entries.iter().find(|e| e.project_id == project).cloned() else {
        return Err(invalid("Proyecto no instalado desde el catálogo"));
    };
    if let Some(user) = entries
        .iter()
        .find(|e| e.dependencies.iter().any(|d| d == project))
    {
        return Err(invalid(format!(
            "Quita primero {}: necesita este proyecto.",
            user.title
        )));
    }
    let path = file_path(&root, entry.kind, &entry.filename)?;
    if entry.owned && path.exists() {
        if hash(&std::fs::read(&path)?) != entry.sha512 {
            return Err(invalid("El archivo fue modificado fuera del catálogo; revísalo en la carpeta de la instancia."));
        }
        std::fs::remove_file(&path)?;
    }
    entries.retain(|e| e.project_id != project);
    save(&root, &entries)?;
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn instance() -> Instance {
        serde_json::from_value(serde_json::json!({"id":"catalog-test", "name":"Test", "mc":"26.2", "loader":"fabric", "target":{"kind":"menu"}})).unwrap()
    }
    #[test]
    fn rejects_paths_urls_and_wrong_loader() {
        let root = std::env::temp_dir();
        for n in [
            "../bad.jar",
            "C:bad.jar",
            "a\\bad.jar",
            ".jar",
            "x.zip",
            "a/b.jar",
            "x\n.jar",
        ] {
            assert!(file_path(&root, Kind::Mod, n).is_err(), "{n}");
        }
        for url in [
            "http://cdn.modrinth.com/x",
            "https://cdn.modrinth.com.evil/x",
            "https://example.org/x",
        ] {
            assert!(download_url(url).is_err());
        }
        assert!(download_url("https://cdn.modrinth.com/data/x/x.jar").is_ok());
        let mut i = instance();
        i.loader = Loader::Vanilla;
        assert!(supported(&i, Kind::Mod).is_err());
        assert!(supported(&i, Kind::Shader).is_err());
        assert!(supported(&i, Kind::Resourcepack).is_ok());
    }
    #[test]
    fn removal_preserves_dependencies_and_modified_or_external_files() {
        let d = Dirs::new(std::env::temp_dir().join(format!("ds-catalog-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(d.root());
        let i = instance();
        let r = root(&d, &i).unwrap();
        std::fs::create_dir_all(r.join("mods")).unwrap();
        let dep = Entry {
            project_id: "dep".into(),
            version_id: "v1".into(),
            title: "Dependency".into(),
            kind: Kind::Mod,
            filename: "dep.jar".into(),
            sha512: hash(b"dep"),
            minecraft: i.mc.clone(),
            dependencies: vec![],
            owned: true,
        };
        let main = Entry {
            project_id: "main".into(),
            filename: "main.jar".into(),
            dependencies: vec!["dep".into()],
            owned: false,
            ..dep.clone()
        };
        std::fs::write(r.join("mods/dep.jar"), b"changed").unwrap();
        std::fs::write(r.join("mods/main.jar"), b"external").unwrap();
        save(&r, &[dep, main]).unwrap();
        assert!(remove(&d, &i, "dep").is_err());
        remove(&d, &i, "main").unwrap();
        assert!(r.join("mods/main.jar").exists());
        assert!(remove(&d, &i, "dep").is_err());
        std::fs::write(r.join("mods/dep.jar"), b"dep").unwrap();
        assert!(remove(&d, &i, "dep").unwrap().is_empty());
        assert!(!r.join("mods/dep.jar").exists());
        let _ = std::fs::remove_dir_all(d.root());
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlink_folder() {
        let r = std::env::temp_dir().join(format!("ds-catalog-link-{}", std::process::id()));
        std::fs::create_dir_all(&r).unwrap();
        std::os::unix::fs::symlink(std::env::temp_dir(), r.join("mods")).unwrap();
        assert!(file_path(&r, Kind::Mod, "safe.jar").is_err());
        std::fs::remove_dir_all(r).unwrap();
    }
}
