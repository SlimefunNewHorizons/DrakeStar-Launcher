//! Java administrado: el launcher baja un JRE de Eclipse Temurin (Adoptium) por version mayor y lo guarda en
//! `runtimes/java-<major>/`. Asi el juego arranca en un PC sin Java instalado (o con uno equivocado).

use crate::progress::ProgressSink;
use crate::{Dirs, McError, McResult};
use serde::Deserialize;
use std::path::{Path, PathBuf};

fn adoptium_os() -> &'static str {
    match crate::os_name() {
        "windows" => "windows",
        "osx" => "mac",
        _ => "linux",
    }
}

fn adoptium_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    }
}

#[derive(Debug, Deserialize)]
struct Asset {
    binary: Binary,
}
#[derive(Debug, Deserialize)]
struct Binary {
    package: Package,
}
#[derive(Debug, Deserialize)]
struct Package {
    link: String,
    checksum: String,
    size: u64,
    name: String,
}

pub fn runtime_dir(d: &Dirs, major: u32) -> PathBuf {
    d.runtimes().join(format!("java-{major}"))
}

/// Ejecutable de Java dentro de un directorio de runtime (busca un nivel hacia abajo: `jdk-25+9-jre/`, `Contents/Home/`).
pub fn find_java(home: &Path) -> Option<PathBuf> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    let direct = [home.join("bin").join(exe), home.join("Contents/Home/bin").join(exe)];
    for c in direct {
        if c.exists() {
            return Some(c);
        }
    }
    for e in std::fs::read_dir(home).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(j) = find_java(&p) {
                return Some(j);
            }
        }
    }
    None
}

/// Java ya instalado por el launcher (sin red).
pub fn installed(d: &Dirs, major: u32) -> Option<PathBuf> {
    find_java(&runtime_dir(d, major))
}

fn extract_zip(archive: &Path, out: &Path) -> McResult<()> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
    for i in 0..z.len() {
        let mut f = z.by_index(i)?;
        let Some(rel) = f.enclosed_name() else { continue };
        let dest = out.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else {
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut o = std::fs::File::create(&dest)?;
            std::io::copy(&mut f, &mut o)?;
        }
    }
    Ok(())
}

fn extract_targz(archive: &Path, out: &Path) -> McResult<()> {
    let gz = flate2::read::GzDecoder::new(std::fs::File::open(archive)?);
    let mut t = tar::Archive::new(gz);
    t.set_preserve_permissions(true);
    t.unpack(out)?;
    Ok(())
}

/// Garantiza un JRE de la version mayor pedida y devuelve el ejecutable de `java`.
pub fn ensure(d: &Dirs, major: u32, sink: &dyn ProgressSink) -> McResult<PathBuf> {
    if let Some(j) = installed(d, major) {
        return Ok(j);
    }
    sink.stage("java", &format!("Descargando Java {major} (solo la primera vez)"));
    let api = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={}&image_type=jre&os={}&vendor=eclipse",
        adoptium_arch(),
        adoptium_os()
    );
    let text = drakestar_net::fetch_text(&api)?;
    let assets: Vec<Asset> = serde_json::from_str(&text)?;
    let pkg = assets.into_iter().next().ok_or_else(|| McError::NotFound(format!("Adoptium no tiene un JRE {major} para {} {}", adoptium_os(), adoptium_arch())))?.binary.package;

    let tmp = d.runtimes().join(format!("_download-{major}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    let archive = tmp.join(&pkg.name);
    sink.log(&format!("java {major}: {} ({} MB)", pkg.name, pkg.size / 1_048_576));
    drakestar_net::download_verified(&pkg.link, &archive, &pkg.checksum, Some(pkg.size), |_| {})?;
    sink.fraction(0.7);

    let dest = runtime_dir(d, major);
    let stage = tmp.join("extract");
    std::fs::create_dir_all(&stage)?;
    if pkg.name.ends_with(".zip") {
        extract_zip(&archive, &stage)?;
    } else {
        extract_targz(&archive, &stage)?;
    }
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::create_dir_all(d.runtimes())?;
    std::fs::rename(&stage, &dest)?;
    let _ = std::fs::remove_dir_all(&tmp);
    sink.fraction(1.0);
    find_java(&dest).ok_or_else(|| McError::Invalid(format!("el JRE {major} se extrajo pero no se encontro java en {}", dest.display())))
}

/// Raiz del runtime (para `JAVA_HOME` / `--java-home`): el directorio que contiene `bin/`.
pub fn java_home_of(java_bin: &Path) -> Option<PathBuf> {
    java_bin.parent()?.parent().map(|p| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nested_java() {
        let t = std::env::temp_dir().join(format!("dsmc-java-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        let bin = t.join("jdk-25+9-jre").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(exe), b"x").unwrap();
        assert!(find_java(&t).is_some());
        assert_eq!(java_home_of(&find_java(&t).unwrap()).unwrap(), t.join("jdk-25+9-jre"));
        let _ = std::fs::remove_dir_all(&t);
    }
}
