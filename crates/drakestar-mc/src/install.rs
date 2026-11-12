//! Instalacion de una version de Minecraft (vanilla o con loader): JSON, jar, librerias, natives, assets y config de logging.

use crate::assets::{download_all, ensure_assets, Job};
use crate::manifest;
use crate::progress::ProgressSink;
use crate::version::{allowed, maven_path, Library, RuleCtx, VersionJson};
use crate::{write_atomic, Dirs, McError, McResult};
use std::path::PathBuf;

/// Todo lo necesario para lanzar una version ya instalada.
#[derive(Debug, Clone)]
pub struct Installed {
    pub id: String,
    pub merged: VersionJson,
    pub classpath: Vec<PathBuf>,
    pub natives_dir: PathBuf,
    pub java_major: u32,
    pub asset_index: String,
    pub logging_arg: Option<String>,
}

fn lib_jobs(d: &Dirs, libs: &[Library], ctx: &RuleCtx) -> (Vec<Job>, Vec<PathBuf>, Vec<(PathBuf, Library)>) {
    let mut jobs = vec![];
    let mut classpath = vec![];
    let mut legacy_natives = vec![];
    for lib in libs {
        if !allowed(&lib.rules, ctx) {
            continue;
        }
        // natives de formato antiguo: un jar por sistema que se extrae, no va al classpath
        if let Some(map) = &lib.natives {
            if let Some(classifier) = map.get(&ctx.os) {
                let classifier = classifier.replace("${arch}", if ctx.arch == "x86" { "32" } else { "64" });
                if let Some(a) = lib.downloads.as_ref().and_then(|dl| dl.classifiers.as_ref()).and_then(|c| c.get(&classifier)) {
                    let dest = d.libraries().join(&a.path);
                    jobs.push(Job { url: a.url.clone(), dest: dest.clone(), sha1: a.sha1.clone(), size: a.size });
                    legacy_natives.push((dest, lib.clone()));
                }
            }
        }
        if let Some(a) = lib.downloads.as_ref().and_then(|dl| dl.artifact.as_ref()) {
            if a.url.is_empty() {
                continue; // librerias "locales" sin URL (algunas de Forge): no hay nada que bajar
            }
            let dest = d.libraries().join(&a.path);
            jobs.push(Job { url: a.url.clone(), dest: dest.clone(), sha1: a.sha1.clone(), size: a.size });
            classpath.push(dest);
        } else if lib.downloads.is_none() {
            // Fabric y compania: solo nombre maven + URL base, sin hash
            if let (Some(base), Some(path)) = (&lib.url, maven_path(&lib.name)) {
                let dest = d.libraries().join(&path);
                jobs.push(Job { url: format!("{}{}", base.trim_end_matches('/').to_owned() + "/", path), dest: dest.clone(), sha1: String::new(), size: 0 });
                classpath.push(dest);
            }
        }
    }
    (jobs, classpath, legacy_natives)
}

fn extract_natives(natives: &[(PathBuf, Library)], out: &std::path::Path) -> McResult<()> {
    std::fs::create_dir_all(out)?;
    for (jar, lib) in natives {
        let excludes: Vec<String> = lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default();
        let mut z = zip::ZipArchive::new(std::fs::File::open(jar)?)?;
        for i in 0..z.len() {
            let mut f = z.by_index(i)?;
            let name = f.name().to_string();
            if f.is_dir() || excludes.iter().any(|e| name.starts_with(e)) || name.contains("..") {
                continue;
            }
            let leaf = std::path::Path::new(&name).file_name().map(|s| s.to_owned());
            let Some(leaf) = leaf else { continue };
            let mut o = std::fs::File::create(out.join(leaf))?;
            std::io::copy(&mut f, &mut o)?;
        }
    }
    Ok(())
}

/// Descarga el JSON de una version de Mojang (verificado con el SHA-1 del manifiesto) y lo guarda.
fn ensure_vanilla_json(d: &Dirs, id: &str) -> McResult<VersionJson> {
    let path = d.version_json(id);
    let m = manifest::load(d)?;
    if let Ok(entry) = manifest::find(&m, id) {
        download_all(vec![Job { url: entry.url.clone(), dest: path.clone(), sha1: entry.sha1.clone(), size: 0 }], 1, &crate::NullSink)?;
    } else if !path.exists() {
        return Err(McError::NotFound(format!("Minecraft {id} no esta en el manifiesto de Mojang")));
    }
    VersionJson::parse(&std::fs::read_to_string(&path)?)
}

/// Instala una version ya conocida: vanilla (`id` en el manifiesto) o con loader (`versions/<id>/<id>.json` con `inheritsFrom`).
pub fn install(d: &Dirs, id: &str, sink: &dyn ProgressSink) -> McResult<Installed> {
    let local = d.version_json(id);
    let ctx = RuleCtx::current(&[]);

    sink.stage("minecraft", &format!("Preparando Minecraft {id}"));
    let child = if local.exists() {
        VersionJson::parse(&std::fs::read_to_string(&local)?)?
    } else {
        ensure_vanilla_json(d, id)?
    };
    let (parent, parent_id) = match &child.inherits_from {
        Some(p) => (Some(ensure_vanilla_json(d, p)?), Some(p.clone())),
        None => (None, None),
    };
    let merged = match &parent {
        Some(p) => VersionJson::merged(p, &child),
        None => child.clone(),
    };

    // jar del cliente: el del padre cuando hay loader
    let jar_owner = parent_id.clone().unwrap_or_else(|| id.to_string());
    let client = merged
        .downloads
        .as_ref()
        .and_then(|dl| dl.client.clone())
        .ok_or_else(|| McError::Invalid(format!("{id}: el JSON no trae descarga del cliente")))?;
    let jar_path = d.version_jar(&jar_owner);
    let mut jobs = vec![Job { url: client.url, dest: jar_path.clone(), sha1: client.sha1, size: client.size }];
    let (ljobs, mut classpath, legacy) = lib_jobs(d, &merged.libraries, &ctx);
    jobs.extend(ljobs);
    sink.log(&format!("{id}: {} archivos (jar + {} librerias)", jobs.len(), classpath.len()));
    download_all(jobs, 8, sink)?;
    classpath.push(jar_path);

    let natives_dir = d.natives(id);
    if !legacy.is_empty() {
        extract_natives(&legacy, &natives_dir)?;
    } else {
        std::fs::create_dir_all(&natives_dir)?;
    }

    // assets
    sink.stage("assets", "Descargando sonidos y recursos");
    let ai = merged.asset_index.clone().ok_or_else(|| McError::Invalid(format!("{id}: sin asset index")))?;
    ensure_assets(d, &ai, sink)?;

    // No se usa la config de logging de Mojang: emite XML por consola. El juego escribe su propio logs/latest.log legible.
    let logging_arg: Option<String> = None;

    let java_major = merged.java_version.as_ref().map(|j| j.major_version).filter(|m| *m > 0).unwrap_or(8);
    // el JSON fusionado se guarda para poder lanzar sin red
    let _ = write_atomic(&d.root().join("cache").join(format!("merged-{id}.json")), serde_json::to_string(&merged)?.as_bytes());
    Ok(Installed { id: id.to_string(), merged: merged.clone(), classpath, natives_dir, java_major, asset_index: ai.id, logging_arg })
}
