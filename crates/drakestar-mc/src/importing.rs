//! Copia instancias instaladas (Prism/CurseForge) o exportadas (Prism ZIP/Modrinth mrpack).
//! No importa cuentas, comandos de inicio ni argumentos JVM del otro launcher.
use crate::instances::{Instance, Instances, Loader, Target};
use crate::{memory::MemoryProfile, Dirs, McError, McResult};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

const MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const DATA: &[&str] = &[
    "mods",
    "config",
    "defaultconfigs",
    "saves",
    "resourcepacks",
    "shaderpacks",
    "screenshots",
    "schematics",
    "kubejs",
    "scripts",
    "patchouli_books",
    "options.txt",
    "optionsof.txt",
    "servers.dat",
    "icon.png",
];

fn invalid(message: impl Into<String>) -> McError {
    McError::Invalid(message.into())
}
fn json(path: &Path) -> McResult<Value> {
    if fs::metadata(path)?.len() > 8 * 1024 * 1024 {
        return Err(invalid("metadatos demasiado grandes"));
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn text<'a>(v: &'a Value, key: &str) -> McResult<&'a str> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or_else(|| invalid(format!("falta {key}")))
}
fn loader(name: &str) -> McResult<Loader> {
    match name {
        "vanilla" => Ok(Loader::Vanilla),
        "fabric" => Ok(Loader::Fabric),
        _ => Err(invalid(format!("Loader {name} no soportado: solo Vanilla y Fabric. La instancia original se conserva."))),
    }
}

#[derive(Debug, Serialize)]
pub struct Preview {
    pub format: String,
    pub instance: Instance,
}

fn inspect(root: &Path) -> McResult<Preview> {
    let fallback = root.file_name().and_then(|s| s.to_str()).unwrap_or("Importada").to_string();
    let (format, name, mc, kind, version) = if root.join("mmc-pack.json").is_file() {
        let pack = json(&root.join("mmc-pack.json"))?;
        let components = pack["components"].as_array().ok_or_else(|| invalid("Prism: faltan components"))?;
        let mut mc = None;
        let mut kind = Loader::Vanilla;
        let mut version = None;
        for c in components {
            if c["disabled"] == true {
                continue;
            }
            match text(c, "uid")? {
                "net.minecraft" => mc = Some(text(c, "version")?.to_string()),
                "net.fabricmc.fabric-loader" => {
                    kind = Loader::Fabric;
                    version = Some(text(c, "version")?.to_string());
                }
                "net.fabricmc.intermediary" | "org.lwjgl3" | "org.lwjgl" => {}
                other => return Err(invalid(format!("Componente Prism no soportado: {other}"))),
            }
        }
        let cfg = fs::read_to_string(root.join("instance.cfg")).unwrap_or_default();
        let name = cfg.lines().find_map(|l| l.trim().strip_prefix("name=")).unwrap_or(&fallback).to_string();
        ("Prism", name, mc.ok_or_else(|| invalid("Prism: falta net.minecraft"))?, kind, version)
    } else if root.join("minecraftinstance.json").is_file() {
        let v = json(&root.join("minecraftinstance.json"))?;
        let spec = v["baseModLoader"]["name"].as_str().unwrap_or("vanilla");
        let mut parts = spec.split('-');
        let kind = loader(parts.next().unwrap_or("vanilla"))?;
        let version = if kind == Loader::Fabric {
            Some(parts.next().filter(|s| !s.is_empty()).ok_or_else(|| invalid("CurseForge: falta version Fabric"))?.to_string())
        } else {
            None
        };
        ("CurseForge", v["name"].as_str().unwrap_or(&fallback).to_string(), text(&v, "gameVersion")?.to_string(), kind, version)
    } else if root.join("modrinth.index.json").is_file() {
        let v = json(&root.join("modrinth.index.json"))?;
        if v["formatVersion"] != 1 || v["game"] != "minecraft" {
            return Err(invalid("Formato Modrinth no soportado"));
        }
        let deps = v["dependencies"].as_object().ok_or_else(|| invalid("Modrinth: faltan dependencies"))?;
        for key in deps.keys() {
            if key != "minecraft" && key != "fabric-loader" {
                return Err(invalid(format!("Loader Modrinth no soportado: {key}")));
            }
        }
        let version = deps.get("fabric-loader").map(|x| x.as_str().ok_or_else(|| invalid("version Fabric no valida"))).transpose()?.map(str::to_string);
        (
            "Modrinth",
            text(&v, "name")?.to_string(),
            text(&v["dependencies"], "minecraft")?.to_string(),
            if version.is_some() { Loader::Fabric } else { Loader::Vanilla },
            version,
        )
    } else {
        return Err(invalid("Selecciona una instancia Prism/CurseForge instalada, un ZIP de Prism o un .mrpack de Modrinth"));
    };
    let instance = Instance {
        id: "importada".into(),
        name,
        mc,
        loader: kind,
        loader_version: version,
        target: Target::Menu,
        performance: false,
        fused: false,
        xmx_mb: MemoryProfile::detect().recommended_mb,
        extra_jvm: vec![],
        note: format!("Importada de {format}"),
    };
    instance.validate()?;
    Ok(Preview { format: format.into(), instance })
}

/// Reserva un directorio exclusivo; se retira en caso de error o al terminar una vista previa.
struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.0.as_os_str().is_empty() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
fn reserve(parent: &Path, prefix: &str) -> McResult<TempDir> {
    fs::create_dir_all(parent)?;
    for n in 1..10000 {
        let path = parent.join(format!("{prefix}-{n}"));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(TempDir(path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(invalid("demasiadas importaciones"))
}
fn relative(raw: &str) -> McResult<PathBuf> {
    // Windows tambien interpreta barras inversas, prefijos y ADS: rechazarlos en todos los SO.
    if raw.contains(['\\', ':']) || Path::new(raw).components().any(|c| !matches!(c, Component::Normal(_))) || raw.is_empty() {
        return Err(invalid("ruta insegura en el paquete"));
    }
    Ok(PathBuf::from(raw))
}
fn unpack(source: &Path, dest: &Path) -> McResult<()> {
    let mut zip = zip::ZipArchive::new(fs::File::open(source)?)?;
    let mut total = 0u64;
    if zip.len() > 100000 {
        return Err(invalid("demasiados archivos"));
    }
    for n in 0..zip.len() {
        let mut entry = zip.by_index(n)?;
        let rel = relative(entry.name().trim_end_matches('/'))?;
        if entry.unix_mode().map(|m| m & 0o170000 == 0o120000).unwrap_or(false) {
            return Err(invalid("no se importan enlaces simbolicos"));
        }
        total = total.checked_add(entry.size()).ok_or_else(|| invalid("paquete demasiado grande"))?;
        if total > MAX_BYTES {
            return Err(invalid("el paquete supera 4 GiB"));
        }
        let path = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(path)?;
        } else {
            fs::create_dir_all(path.parent().unwrap())?;
            let mut out = fs::OpenOptions::new().write(true).create_new(true).open(path)?;
            let limit = entry.size();
            let copied = std::io::copy(&mut (&mut entry).take(limit + 1), &mut out)?;
            if copied != limit {
                return Err(invalid("tamano ZIP no coincide"));
            }
            out.flush()?;
        }
    }
    Ok(())
}
fn with_source<T>(d: &Dirs, source: &Path, action: impl FnOnce(&Path) -> McResult<T>) -> McResult<T> {
    let source = fs::canonicalize(source)?;
    if source.is_dir() {
        return action(&source);
    }
    let tmp = reserve(&d.root().join("imports"), "extract")?;
    unpack(&source, &tmp.0)?;
    if ["mmc-pack.json", "modrinth.index.json"].iter().any(|f| tmp.0.join(f).exists()) {
        return action(&tmp.0);
    }
    let entries = fs::read_dir(&tmp.0)?.collect::<Result<Vec<_>, _>>()?;
    if entries.len() == 1 && entries[0].file_type()?.is_dir() {
        return action(&entries[0].path());
    }
    Err(invalid("no hay metadatos de instancia en el paquete"))
}
pub fn preview(d: &Dirs, source: &Path) -> McResult<Preview> {
    with_source(d, source, inspect)
}

fn copy_tree(source: &Path, dest: &Path, total: &mut u64) -> McResult<()> {
    let meta = fs::symlink_metadata(source)?;
    if meta.is_dir() {
        fs::create_dir_all(dest)?;
        for e in fs::read_dir(source)? {
            let e = e?;
            copy_tree(&e.path(), &dest.join(e.file_name()), total)?;
        }
    } else if meta.is_file() {
        *total = total.checked_add(meta.len()).ok_or_else(|| invalid("instancia demasiado grande"))?;
        if *total > MAX_BYTES {
            return Err(invalid("la instancia supera 4 GiB"));
        }
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::copy(source, dest)?;
    } else {
        return Err(invalid("no se importan enlaces ni archivos especiales"));
    }
    Ok(())
}
fn copy_data(source: &Path, dest: &Path, total: &mut u64) -> McResult<()> {
    if fs::symlink_metadata(source)?.file_type().is_symlink() {
        return Err(invalid("directorio de juego enlazado"));
    }
    for name in DATA {
        let src = source.join(name);
        if src.symlink_metadata().is_ok() {
            copy_tree(&src, &dest.join(name), total)?;
        }
    }
    Ok(())
}
fn download_modrinth(root: &Path, dest: &Path, total: &mut u64) -> McResult<()> {
    let index = json(&root.join("modrinth.index.json"))?;
    for file in index["files"].as_array().ok_or_else(|| invalid("Modrinth: falta files"))? {
        if file["env"]["client"] == "unsupported" {
            continue;
        }
        let rel = relative(text(file, "path")?)?;
        if !DATA.iter().any(|p| rel.starts_with(p)) {
            return Err(invalid("ruta Modrinth fuera de los datos de juego"));
        }
        let size = file["fileSize"].as_u64().ok_or_else(|| invalid("Modrinth: falta fileSize"))?;
        *total = total.checked_add(size).ok_or_else(|| invalid("paquete demasiado grande"))?;
        if *total > MAX_BYTES {
            return Err(invalid("el paquete supera 4 GiB"));
        }
        let hash = text(&file["hashes"], "sha1")?;
        let urls = file["downloads"].as_array().ok_or_else(|| invalid("Modrinth: faltan downloads"))?;
        let url = urls
            .iter()
            .filter_map(Value::as_str)
            .find(|s| s.starts_with("https://cdn.modrinth.com/"))
            .ok_or_else(|| invalid("Modrinth: solo se admiten descargas HTTPS del CDN oficial"))?;
        drakestar_net::download_verified(url, &dest.join(rel), hash, Some(size), |_| {})?;
    }
    Ok(())
}

pub fn import(d: &Dirs, source: &Path) -> McResult<Instance> {
    with_source(d, source, |root| {
        let mut p = inspect(root)?;
        let store = Instances::new(d);
        let known = store.list();
        // Los ids reservados en instances.json (incluso sin carpeta) tampoco se sobrescriben.
        fs::create_dir_all(d.instances())?;
        let mut reserved = None;
        for n in 1..10000 {
            let id = format!("importada-{n}");
            if known.iter().any(|i| i.id == id) {
                continue;
            }
            let path = d.instance(&id);
            match fs::create_dir(&path) {
                Ok(()) => {
                    p.instance.id = id;
                    reserved = Some(TempDir(path));
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        let mut slot = reserved.ok_or_else(|| invalid("demasiadas importaciones"))?;
        if slot.0.starts_with(root) || root.starts_with(&slot.0) {
            return Err(invalid("el origen contiene al destino"));
        }
        let mut total = 0;
        if p.format == "Modrinth" {
            download_modrinth(root, &slot.0, &mut total)?;
            for dir in ["overrides", "client-overrides"] {
                if root.join(dir).exists() {
                    copy_data(&root.join(dir), &slot.0, &mut total)?;
                }
            }
        } else {
            let game = [".minecraft", "minecraft"].iter().map(|s| root.join(s)).find(|p| p.is_dir()).unwrap_or_else(|| root.to_path_buf());
            copy_data(&game, &slot.0, &mut total)?;
        }
        let instance = store.upsert(p.instance)?;
        // La carpeta deja de ser temporal solo tras guardar el catalogo correctamente.
        slot.0 = PathBuf::new();
        Ok(instance)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    fn fixture() -> TempDir {
        reserve(&std::env::temp_dir(), &format!("dsmc-import-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))).unwrap()
    }
    fn write(root: &Path, path: &str, content: &str) {
        let p = root.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }
    fn prism(root: &Path) {
        write(
            root,
            "mmc-pack.json",
            r#"{"formatVersion":1,"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.10"}]}"#,
        );
        write(root, "instance.cfg", "[General]\nname=Mi mundo\nPreLaunchCommand=malicioso\n");
        write(root, ".minecraft/saves/mundo/level.dat", "mundo");
        write(root, ".minecraft/mods/example.jar", "mod");
        write(root, ".minecraft/accounts.json", "no copiar");
    }
    fn archive(root: &Path, files: &[(&str, &str)]) -> PathBuf {
        let path = root.join("pack.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, data) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(data.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }
    #[test]
    fn imports_prism_without_overwriting_or_copying_accounts() {
        let f = fixture();
        let src = f.0.join("prism");
        prism(&src);
        let d = Dirs::new(f.0.join("launcher"));
        let first = import(&d, &src).unwrap();
        let second = import(&d, &src).unwrap();
        assert_ne!(first.id, second.id);
        assert_eq!(first.name, "Mi mundo");
        assert_eq!(first.loader_version.as_deref(), Some("0.16.10"));
        assert_eq!(fs::read(d.instance(&first.id).join("saves/mundo/level.dat")).unwrap(), b"mundo");
        assert!(src.join(".minecraft/mods/example.jar").exists());
        assert!(!d.instance(&first.id).join("accounts.json").exists());
        assert!(first.extra_jvm.is_empty());
        assert_eq!(Instances::new(&d).get(&first.id).unwrap().loader_version, first.loader_version);
    }
    #[test]
    fn imports_installed_curseforge_and_rejects_forge() {
        let f = fixture();
        let src = f.0.join("cf");
        write(&src, "minecraftinstance.json", r#"{"name":"CF","gameVersion":"1.20.1","baseModLoader":{"name":"fabric-0.16.10-1.20.1"}}"#);
        write(&src, "config/example.json", "{}");
        let d = Dirs::new(f.0.join("launcher"));
        let i = import(&d, &src).unwrap();
        assert_eq!(i.loader, Loader::Fabric);
        assert!(d.instance(&i.id).join("config/example.json").exists());
        write(&src, "minecraftinstance.json", r#"{"gameVersion":"1.20.1","baseModLoader":{"name":"forge-47.0"}}"#);
        assert!(import(&d, &src).unwrap_err().to_string().contains("forge"));
        assert_eq!(Instances::new(&d).list().len(), 4);
    }
    #[test]
    fn imports_modrinth_client_overrides_and_skips_server_files() {
        let f = fixture();
        let path = archive(
            &f.0,
            &[
                (
                    "modrinth.index.json",
                    r#"{"formatVersion":1,"game":"minecraft","name":"MR","dependencies":{"minecraft":"1.21.1","fabric-loader":"0.16.10"},"files":[{"env":{"client":"unsupported"}}]}"#,
                ),
                ("overrides/config/test.txt", "base"),
                ("client-overrides/config/test.txt", "client"),
                ("server-overrides/config/test.txt", "server"),
            ],
        );
        let d = Dirs::new(f.0.join("launcher"));
        assert_eq!(preview(&d, &path).unwrap().format, "Modrinth");
        let i = import(&d, &path).unwrap();
        assert_eq!(fs::read_to_string(d.instance(&i.id).join("config/test.txt")).unwrap(), "client");
        assert_eq!(fs::read_dir(d.root().join("imports")).unwrap().count(), 0);
    }
    #[test]
    fn rejects_zip_traversal_and_cleans_extraction() {
        let f = fixture();
        let path = archive(&f.0, &[("../outside.txt", "bad")]);
        let d = Dirs::new(f.0.join("launcher"));
        assert!(import(&d, &path).is_err());
        assert!(!f.0.join("outside.txt").exists());
        assert_eq!(fs::read_dir(d.root().join("imports")).unwrap().count(), 0);
        for path in ["C:/secret", "mods/../../secret", "mods\\secret", "/absolute"] {
            assert!(relative(path).is_err());
        }
    }
    #[test]
    fn rejects_bad_versions_without_registering() {
        let f = fixture();
        let src = f.0.join("cf");
        write(&src, "minecraftinstance.json", r#"{"gameVersion":"1.19.4"}"#);
        let d = Dirs::new(f.0.join("launcher"));
        assert!(import(&d, &src).is_err());
        assert!(!d.root().join("instances.json").exists());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_failure_rolls_back_instance_directory_and_catalog() {
        let f = fixture();
        let src = f.0.join("prism");
        prism(&src);
        std::os::unix::fs::symlink("/etc/hosts", src.join(".minecraft/mods/link.jar")).unwrap();
        let d = Dirs::new(f.0.join("launcher"));
        assert!(import(&d, &src).is_err());
        assert_eq!(Instances::new(&d).list().len(), 3);
        assert_eq!(fs::read_dir(d.instances()).unwrap().count(), 0);
    }
    #[test]
    fn imports_prism_zip_with_wrapper_directory() {
        let f = fixture();
        let path = archive(
            &f.0,
            &[
                ("Pack/mmc-pack.json", r#"{"components":[{"uid":"net.minecraft","version":"1.21.1"}]}"#),
                ("Pack/instance.cfg", "name=Zip pack"),
                ("Pack/minecraft/saves/level.dat", "save"),
            ],
        );
        let d = Dirs::new(f.0.join("launcher"));
        let i = import(&d, &path).unwrap();
        assert_eq!(i.name, "Zip pack");
        assert_eq!(i.loader, Loader::Vanilla);
        assert!(d.instance(&i.id).join("saves/level.dat").exists());
    }
    #[test]
    fn does_not_replace_catalog_only_instances_or_orphan_directories() {
        let f = fixture();
        let src = f.0.join("prism");
        prism(&src);
        let d = Dirs::new(f.0.join("launcher"));
        let store = Instances::new(&d);
        let mut existing = store.get("drakescraft").unwrap();
        existing.id = "importada-1".into();
        store.upsert(existing).unwrap();
        write(&d.instance("importada-2"), "keep.txt", "keep");
        let i = import(&d, &src).unwrap();
        assert_eq!(i.id, "importada-3");
        assert_eq!(store.get("importada-1").unwrap().name, "DrakesCraft (online)");
        assert_eq!(fs::read_to_string(d.instance("importada-2").join("keep.txt")).unwrap(), "keep");
    }
    #[test]
    fn failed_modrinth_download_is_not_registered() {
        let f = fixture();
        let path = archive(
            &f.0,
            &[(
                "modrinth.index.json",
                r#"{"formatVersion":1,"game":"minecraft","name":"Invalid","dependencies":{"minecraft":"1.21.1"},"files":[{"path":"mods/test.jar","fileSize":1,"hashes":{"sha1":"0000000000000000000000000000000000000000"},"downloads":["http://127.0.0.1/secret"]}]}"#,
            )],
        );
        let d = Dirs::new(f.0.join("launcher"));
        assert!(import(&d, &path).unwrap_err().to_string().contains("HTTPS"));
        assert_eq!(Instances::new(&d).list().len(), 3);
        assert_eq!(fs::read_dir(d.instances()).unwrap().count(), 0);
    }
}
