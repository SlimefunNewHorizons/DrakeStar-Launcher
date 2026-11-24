//! Resolucion de releases (Manifest::resolve de drakestar-core) e instalacion
//! con verificacion SHA-256 (drakestar-net). El manifiesto lo importa el usuario;
//! todavia no hay un manifiesto oficial publicado ni firma de manifiestos.

use crate::{layout::write_atomic, server::DEFAULT_JAR, AppError, AppResult, Layout};
use drakestar_core::release::{sha256_wellformed, Manifest};
use drakestar_core::{Mode, Profile};
use serde::Serialize;

pub fn component_for(mode: Mode) -> Option<&'static str> {
    match mode {
        Mode::LocalServer => Some("drakespurpur"),
        Mode::DrakesRust => Some("drakes-rust-client"),
        _ => None,
    }
}

pub fn platform() -> String {
    let os = match std::env::consts::OS {
        "macos" => "macos",
        o => o,
    };
    format!("{os}-{}", std::env::consts::ARCH)
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedRelease {
    pub version: String,
    pub release_channel: drakestar_core::Channel,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub sha256_wellformed: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Resolution {
    pub profile: String,
    pub profile_channel: drakestar_core::Channel,
    pub component: Option<&'static str>,
    pub platform: String,
    pub minecraft: String,
    pub resolved: Option<ResolvedRelease>,
    pub reason: Option<String>,
}

pub const DEFAULT_MANIFEST: &str = include_str!("../../../manifest.json");

pub fn load_manifest(l: &Layout) -> Option<Manifest> {
    if let Ok(text) = std::fs::read_to_string(l.manifest_path()) {
        if let Ok(m) = Manifest::from_json(&text) {
            return Some(m);
        }
    }
    Manifest::from_json(DEFAULT_MANIFEST).ok()
}

pub fn import_manifest(l: &Layout, json: &str) -> AppResult<Manifest> {
    let m = Manifest::from_json(json)?;
    if m.schema != 1 {
        return Err(AppError::Invalid(format!("schema de manifiesto {} no soportado (esperado 1)", m.schema)));
    }
    write_atomic(&l.manifest_path(), serde_json::to_string_pretty(&m)?.as_bytes())?;
    Ok(m)
}

pub fn resolve(manifest: Option<&Manifest>, p: &Profile) -> Resolution {
    let component = component_for(p.mode);
    let plat = platform();
    let mut r = Resolution {
        profile: p.id.clone(),
        profile_channel: p.channel,
        component,
        platform: plat.clone(),
        minecraft: p.minecraft.clone(),
        resolved: None,
        reason: None,
    };
    let Some(comp) = component else {
        r.reason = Some("este modo no tiene un componente descargable gestionado todavia".into());
        return r;
    };
    let Some(m) = manifest else {
        r.reason = Some("no hay manifiesto de releases importado".into());
        return r;
    };
    match m.resolve(p.channel, comp, &plat, &p.minecraft) {
        Some((rel, art)) => {
            r.resolved = Some(ResolvedRelease {
                version: rel.version.clone(),
                release_channel: rel.channel,
                url: art.url.clone(),
                sha256: art.sha256.clone(),
                size: art.size,
                sha256_wellformed: sha256_wellformed(&art.sha256),
                notes: rel.notes.clone(),
            });
        }
        None => {
            r.reason = Some(format!(
                "ninguna release de '{comp}' para {plat} y Minecraft {} es aceptable en el canal {}",
                p.minecraft,
                p.channel.label().to_lowercase()
            ))
        }
    }
    r
}

/// Descarga el JAR resuelto a `server/drakespurpur.jar` verificando SHA-256.
/// Solo para `local-server`. Devuelve bytes escritos.
pub fn install_server_jar(l: &Layout, p: &Profile, manifest: Option<&Manifest>) -> AppResult<u64> {
    if p.mode != Mode::LocalServer {
        return Err(AppError::Invalid("solo los perfiles local-server instalan un JAR de servidor".into()));
    }
    let res = resolve(manifest, p);
    let rel = res.resolved.ok_or_else(|| AppError::NotFound(res.reason.unwrap_or_else(|| "release".into())))?;
    if !rel.sha256_wellformed {
        return Err(AppError::Invalid("el manifiesto no trae un SHA-256 valido: no se instala".into()));
    }
    let dest = l.server_dir(&p.id).join(DEFAULT_JAR);
    let n = drakestar_net::download_verified(&rel.url, &dest, &rel.sha256, Some(rel.size), |_| {})?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::sample;
    use drakestar_core::Channel;

    fn manifest(url: &str, sha: &str, size: u64) -> String {
        format!(
            r#"{{"schema":1,"releases":[
 {{"version":"1.0.0","channel":"stable","minecraft":["26.2"],"artifacts":[{{"component":"drakespurpur","platform":"any","url":"{url}","sha256":"{sha}","size":{size}}}]}},
 {{"version":"1.1.0-nightly","channel":"nightly","minecraft":["26.2"],"artifacts":[{{"component":"drakespurpur","platform":"any","url":"{url}","sha256":"{sha}","size":{size}}}]}}
]}}"#
        )
    }

    #[test]
    fn channels_respected_through_resolution() {
        let l = Layout::new(crate::testutil::tmpdir("rel"));
        let m = import_manifest(&l, &manifest("https://x/y.jar", &"a".repeat(64), 1)).unwrap();
        assert!(load_manifest(&l).is_some());
        let mut p = sample("s", Mode::LocalServer);
        let r = resolve(Some(&m), &p);
        assert_eq!(r.resolved.as_ref().unwrap().version, "1.0.0");
        p.channel = Channel::Nightly;
        assert_eq!(resolve(Some(&m), &p).resolved.unwrap().version, "1.1.0-nightly");
        p.minecraft = "9.9".into();
        let r = resolve(Some(&m), &p);
        assert!(r.resolved.is_none() && r.reason.is_some());
        assert!(resolve(None, &p).reason.unwrap().contains("manifiesto"));
        assert!(resolve(Some(&m), &sample("j", Mode::DrakesJava)).component.is_none());
        assert!(import_manifest(&l, r#"{"schema":2,"releases":[]}"#).is_err());
    }

    #[test]
    fn install_verifies_sha_from_local_http() {
        use std::io::{Read, Write};
        let l = Layout::new(crate::testutil::tmpdir("inst-rel"));
        let hello = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        let lst = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = lst.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for _ in 0..2 {
                if let Ok((mut s, _)) = lst.accept() {
                    let mut b = [0u8; 1024];
                    let _ = s.read(&mut b);
                    let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello");
                }
            }
        });
        let url = format!("http://127.0.0.1:{port}/s.jar");
        let p = sample("s", Mode::LocalServer);
        let m = Manifest::from_json(&manifest(&url, hello, 5)).unwrap();
        assert_eq!(install_server_jar(&l, &p, Some(&m)).unwrap(), 5);
        assert_eq!(std::fs::read(l.server_dir("s").join(DEFAULT_JAR)).unwrap(), b"hello");
        // hash incorrecto: error y nada nuevo escrito
        let _ = std::fs::remove_file(l.server_dir("s").join(DEFAULT_JAR));
        let bad = Manifest::from_json(&manifest(&url, &"b".repeat(64), 5)).unwrap();
        assert!(install_server_jar(&l, &p, Some(&bad)).is_err());
        assert!(!l.server_dir("s").join(DEFAULT_JAR).exists());
        assert!(install_server_jar(&l, &sample("j", Mode::DrakesJava), Some(&m)).is_err());
    }

    /// El manifest que se commitea en el repo tiene que ser utilizable: los tres
    /// artefactos (servidor, híbrido y cliente) con SHA bien formado, y el
    /// cliente resolviendo para linux-x86_64 desde un perfil nightly.
    #[test]
    fn committed_manifest_resolves_real_artifacts() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../manifest.json");
        let text = std::fs::read_to_string(path).expect("manifest.json en la raíz del repo");
        let m = Manifest::from_json(&text).expect("manifest.json parseable");
        assert_eq!(m.schema, 1);

        // todos los artefactos con hash válido (si no, no se instalan)
        for r in &m.releases {
            for a in &r.artifacts {
                assert!(
                    sha256_wellformed(&a.sha256),
                    "{}: SHA mal formado ('{}')",
                    a.component,
                    a.sha256
                );
                assert!(a.size > 0, "{}: size 0 (placeholder)", a.component);
            }
        }

        // el cliente resuelve en Linux desde un perfil nightly: el manifiesto solo
        // publica linux-x86_64. En Windows/macOS todavía no hay binario: no debe
        // resolver y tiene que decir por qué (misma prueba, las dos caras).
        let mut p = sample("rust", Mode::DrakesRust);
        p.channel = Channel::Nightly;
        p.minecraft = "26.2".into();
        let res = resolve(Some(&m), &p);
        #[cfg(target_os = "linux")]
        {
            let rel = res
                .resolved
                .unwrap_or_else(|| panic!("el cliente debería resolver: {}", res.reason.unwrap_or_default()));
            assert_eq!(rel.version, "0.1");
            assert!(
                rel.url.starts_with("https://"),
                "el binario tiene que venir por https: {}",
                rel.url
            );
            // no debe seguir apuntando al ZIP de Horizon: eso no lo puede ejecutar
            assert!(
                !rel.url.contains("horizon"),
                "el manifest sigue apuntando a Horizon: {}",
                rel.url
            );
        }
        #[cfg(not(target_os = "linux"))]
        {
            assert!(res.resolved.is_none(), "sin binario para esta plataforma no debe resolver");
            assert!(res.reason.unwrap_or_default().contains("drakes-rust-client"));
        }

        // un perfil stable NO resuelve un nightly (regla de canales, no un bug)
        let mut stable = p.clone();
        stable.channel = Channel::Stable;
        let r2 = resolve(Some(&m), &stable);
        assert!(r2.resolved.is_none(), "un perfil stable no debe resolver un nightly");
        assert!(r2.reason.is_some(), "y tiene que explicar por qué");
    }
}
