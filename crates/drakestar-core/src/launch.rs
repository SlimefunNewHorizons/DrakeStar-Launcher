//! Planificacion de lanzamiento: convierte un perfil en una linea de comandos,
//! sin ejecutarla (la ejecucion vive en el gestor de procesos del shell).

use crate::profile::{Mode, Profile};

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub envs: BTreeMap<String, String>,
}

impl LaunchPlan {
    pub fn new(program: impl Into<String>, args: Vec<String>, cwd: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args,
            cwd: cwd.into(),
            envs: BTreeMap::new(),
        }
    }
}

/// Major de Java exigido por una version de Minecraft. Los snapshots/versiones
/// "26.x" requieren Java 25 (el servidor DrakesPurpur 26.2 usa Java 25).
pub fn required_java(minecraft: &str) -> u32 {
    let major: u32 = minecraft
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if major >= 26 {
        25
    } else {
        21
    }
}

/// Plan del servidor local DrakesPurpur. El JAR lo aporta el gestor de releases.
pub fn local_server_plan(
    p: &Profile,
    java_bin: &str,
    server_jar: &str,
    root: &str,
) -> Option<LaunchPlan> {
    if p.mode != Mode::LocalServer {
        return None;
    }
    let mut args = vec![
        format!("-Xms{}M", p.java.min_mem_mb),
        format!("-Xmx{}M", p.java.max_mem_mb),
    ];
    args.extend(p.java.extra_jvm_args.iter().cloned());
    // Biblioteca nativa del motor hibrido: se extrae sola del JAR (DrakesPurpur Hybrid),
    // aqui solo habilitamos el acceso nativo que exige Java 25 para FFM.
    args.push("--enable-native-access=ALL-UNNAMED".into());
    args.extend(["-jar".into(), server_jar.into(), "--nogui".into()]);
    Some(LaunchPlan {
        program: java_bin.into(),
        args,
        cwd: format!("{root}/profiles/{}/server", p.id),
        envs: BTreeMap::new(),
    })
}

/// Plan del cliente Rust con credenciales de sesión opcionales.
pub fn rust_client_plan_with_auth(
    p: &Profile,
    client_bin: &str,
    root: &str,
    username: &str,
    uuid: Option<&str>,
    access_token: Option<&str>,
) -> Option<LaunchPlan> {
    if p.mode != Mode::DrakesRust {
        return None;
    }
    let server = p
        .server
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("mc.drakescraft.cl:25565");
    let mut args = vec!["--server".into(), server.to_string(), "--name".into(), username.to_string()];
    if let (Some(u), Some(tok)) = (uuid, access_token) {
        args.push("--uuid".into());
        args.push(u.to_string());
        args.push("--token".into());
        args.push(tok.to_string());
    }
    let mut envs = BTreeMap::new();
    if p.graphics.discrete_gpu {
        envs.insert("DRI_PRIME".into(), "1".into());
        envs.insert("__NV_PRIME_RENDER_OFFLOAD".into(), "1".into());
        envs.insert("__GLX_VENDOR_LIBRARY_NAME".into(), "nvidia".into());
        envs.insert("__VK_LAYER_NV_optimus".into(), "NVIDIA_only".into());
    }
    Some(LaunchPlan {
        program: client_bin.into(),
        args,
        cwd: format!("{root}/profiles/{}/instance", p.id),
        envs,
    })
}

/// Plan del cliente Rust (DrakesRust). El binario lo aporta el gestor de releases
/// (`drakestar-app::rustclient`); aqui solo se arma la linea de comandos.
/// `None` si el perfil no es drakes-rust o no declara servidor al que conectar.
pub fn rust_client_plan(p: &Profile, client_bin: &str, root: &str, username: &str) -> Option<LaunchPlan> {
    rust_client_plan_with_auth(p, client_bin, root, username, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{channel::Channel, profile::*};

    #[test]
    fn java_major_for_26x() {
        assert_eq!(required_java("26.2"), 25);
        assert_eq!(required_java("1.21.4"), 21);
    }

    #[test]
    fn local_server_plan_only_for_local_server() {
        let mut p = Profile {
            schema: 1,
            id: "dev".into(),
            name: "Dev".into(),
            mode: Mode::LocalServer,
            channel: Channel::Unstable,
            minecraft: "26.2".into(),
            loader: Loader::Vanilla,
            java: JavaSettings::default(),
            server: None,
            preset: Some("slimefun".into()),
            modpack: None,
            graphics: GraphicsSettings::default(),
            notes: String::new(),
        };
        let plan = local_server_plan(&p, "java", "drakespurpur.jar", "/d").unwrap();
        assert!(plan.args.contains(&"-Xmx4096M".to_string()));
        assert!(plan
            .args
            .contains(&"--enable-native-access=ALL-UNNAMED".to_string()));
        assert_eq!(plan.cwd, "/d/profiles/dev/server");
        p.mode = Mode::Online;
        assert!(local_server_plan(&p, "java", "x", "/d").is_none());
    }

    fn rust_profile(server: Option<&str>) -> Profile {
        Profile {
            schema: 1,
            id: "rust".into(),
            name: "Rust".into(),
            mode: Mode::DrakesRust,
            channel: Channel::Stable,
            minecraft: "26.2".into(),
            loader: Loader::Vanilla,
            java: JavaSettings::default(),
            server: server.map(str::to_string),
            preset: None,
            modpack: None,
            graphics: GraphicsSettings::default(),
            notes: String::new(),
        }
    }

    #[test]
    fn rust_client_plan_carries_server_and_user() {
        let plan = rust_client_plan(&rust_profile(Some("localhost:25565")), "/bin/drakestar-rust-client", "/d", "nix").unwrap();
        assert_eq!(plan.program, "/bin/drakestar-rust-client");
        assert_eq!(plan.args, vec!["--server", "localhost:25565", "--name", "nix"]);
        assert_eq!(plan.cwd, "/d/profiles/rust/instance");
        // sin servidor explicito cae por defecto a Dallas (mc.drakescraft.cl:25565)
        let def_plan = rust_client_plan(&rust_profile(None), "x", "/d", "nix").unwrap();
        assert_eq!(def_plan.args, vec!["--server", "mc.drakescraft.cl:25565", "--name", "nix"]);
        let def_plan_space = rust_client_plan(&rust_profile(Some(" ")), "x", "/d", "nix").unwrap();
        assert_eq!(def_plan_space.args, vec!["--server", "mc.drakescraft.cl:25565", "--name", "nix"]);
        // con sesion autenticada (uuid y token)
        let auth_plan = rust_client_plan_with_auth(
            &rust_profile(Some("mc.drakescraft.cl")),
            "/bin/drakestar-rust-client",
            "/d",
            "JackStar",
            Some("12345678-1234-1234-1234-123456789abc"),
            Some("token_xyz"),
        )
        .unwrap();
        assert_eq!(
            auth_plan.args,
            vec![
                "--server", "mc.drakescraft.cl",
                "--name", "JackStar",
                "--uuid", "12345678-1234-1234-1234-123456789abc",
                "--token", "token_xyz"
            ]
        );
        // ni para otros modos
        let mut p = rust_profile(Some("localhost"));
        p.mode = Mode::DrakesJava;
        assert!(rust_client_plan(&p, "x", "/d", "nix").is_none());
    }
}
