//! Arma la linea de comandos de Minecraft (JVM + juego) desde el JSON de version fusionado y lanza el proceso,
//! capturando stdout/stderr linea a linea para los logs y la consola.

use crate::install::Installed;
use crate::version::{Arg, RuleCtx};
use crate::{Dirs, McError, McResult};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

pub const LAUNCHER_NAME: &str = "DrakeStar";

/// Cuenta con la que se juega (premium o sin premium).
#[derive(Debug, Clone)]
pub struct Session {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    /// `msa` (premium) o `legacy` (sin premium).
    pub user_type: String,
    pub xuid: Option<String>,
    pub client_id: Option<String>,
}

impl Session {
    pub fn offline(name: &str) -> Self {
        Session { name: name.into(), uuid: crate::offline::offline_uuid(name), access_token: "0".into(), user_type: "legacy".into(), xuid: None, client_id: None }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub game_dir: PathBuf,
    pub java_bin: PathBuf,
    pub xmx_mb: u32,
    pub xms_mb: u32,
    pub extra_jvm: Vec<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// `host[:port]` al que entrar directo (quickPlay multijugador).
    pub join_server: Option<String>,
    /// Nombre de una carpeta de `saves/` para abrir directo (quickPlay un jugador).
    pub open_world: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

fn subst(s: &str, vars: &BTreeMap<&str, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

/// Construye el plan de lanzamiento.
pub fn plan(d: &Dirs, inst: &Installed, s: &Session, o: &Options) -> McResult<Plan> {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let classpath = inst.classpath.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join(sep);
    let mut features = vec!["has_quick_plays_support"];
    if o.width.is_some() && o.height.is_some() {
        features.push("has_custom_resolution");
    }
    if o.join_server.is_some() {
        features.push("is_quick_play_multiplayer");
    }
    if o.open_world.is_some() {
        features.push("is_quick_play_singleplayer");
    }
    let ctx = RuleCtx::current(&features);
    let mut vars: BTreeMap<&str, String> = BTreeMap::new();
    vars.insert("auth_player_name", s.name.clone());
    vars.insert("version_name", inst.id.clone());
    vars.insert("game_directory", o.game_dir.to_string_lossy().to_string());
    vars.insert("assets_root", d.assets().to_string_lossy().to_string());
    vars.insert("game_assets", d.assets().to_string_lossy().to_string());
    vars.insert("assets_index_name", inst.asset_index.clone());
    vars.insert("auth_uuid", s.uuid.clone());
    vars.insert("auth_access_token", s.access_token.clone());
    vars.insert("auth_session", s.access_token.clone());
    vars.insert("clientid", s.client_id.clone().unwrap_or_default());
    vars.insert("auth_xuid", s.xuid.clone().unwrap_or_else(|| "0".into()));
    vars.insert("user_type", s.user_type.clone());
    vars.insert("user_properties", "{}".into());
    vars.insert("version_type", inst.merged.kind.clone().unwrap_or_else(|| "release".into()));
    vars.insert("resolution_width", o.width.unwrap_or(854).to_string());
    vars.insert("resolution_height", o.height.unwrap_or(480).to_string());
    vars.insert("natives_directory", inst.natives_dir.to_string_lossy().to_string());
    vars.insert("launcher_name", LAUNCHER_NAME.into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
    vars.insert("classpath", classpath);
    vars.insert("classpath_separator", sep.into());
    vars.insert("library_directory", d.libraries().to_string_lossy().to_string());
    vars.insert("quickPlayPath", o.game_dir.join("logs").join("quickPlay.json").to_string_lossy().to_string());
    vars.insert("quickPlaySingleplayer", o.open_world.clone().unwrap_or_default());
    vars.insert("quickPlayMultiplayer", o.join_server.clone().unwrap_or_default());
    vars.insert("quickPlayRealms", String::new());

    let main_class = inst.merged.main_class.clone().ok_or_else(|| McError::Invalid(format!("{}: sin mainClass", inst.id)))?;

    // JVM
    let mut jvm: Vec<String> = vec![format!("-Xmx{}M", o.xmx_mb.max(512)), format!("-Xms{}M", o.xms_mb.max(256).min(o.xmx_mb.max(512)))];
    match inst.merged.arguments.as_ref() {
        Some(a) if !a.jvm.is_empty() => {
            for arg in &a.jvm {
                for v in arg.values(&ctx) {
                    jvm.push(subst(&v, &vars));
                }
            }
        }
        _ => {
            jvm.push(format!("-Djava.library.path={}", inst.natives_dir.display()));
            jvm.push("-cp".into());
            jvm.push(vars["classpath"].clone());
        }
    }
    if let Some(l) = &inst.logging_arg {
        jvm.push(l.clone());
    }
    jvm.extend(o.extra_jvm.iter().cloned());
    // modernas (1.13+) traen "-cp ${classpath}" dentro de arguments.jvm; si no, lo anadimos
    if !jvm.iter().any(|a| a == "-cp" || a == "-classpath") {
        jvm.push("-cp".into());
        jvm.push(vars["classpath"].clone());
    }

    // juego
    let mut game: Vec<String> = vec![];
    if let Some(a) = inst.merged.arguments.as_ref().filter(|a| !a.game.is_empty()) {
        for arg in &a.game {
            for v in arg.values(&ctx) {
                game.push(subst(&v, &vars));
            }
        }
    } else if let Some(legacy) = &inst.merged.minecraft_arguments {
        for part in legacy.split_whitespace() {
            game.push(subst(part, &vars));
        }
    }
    // pantalla completa / resolucion en versiones que no las traen por regla
    let _ = Arg::Plain(String::new());

    let mut args = jvm;
    args.push(main_class);
    args.extend(game);
    Ok(Plan { program: o.java_bin.clone(), args, cwd: o.game_dir.clone() })
}

/// Proceso del juego en ejecucion.
#[derive(Clone)]
pub struct GameHandle {
    child: Arc<Mutex<Option<Child>>>,
}

impl GameHandle {
    pub fn kill(&self) {
        if let Some(c) = self.child.lock().unwrap().as_mut() {
            let _ = c.kill();
        }
    }
    pub fn pid(&self) -> Option<u32> {
        self.child.lock().unwrap().as_ref().map(|c| c.id())
    }
}

/// Lanza el plan. `on_line(stream, line)` recibe cada linea ("out"/"err"); `on_exit(code)` se llama al terminar.
/// Si `log_file` existe, copia ahi toda la salida (sin el token de acceso).
pub fn spawn(
    plan: &Plan,
    secrets: Vec<String>,
    log_file: Option<PathBuf>,
    on_line: impl Fn(&str, &str) + Send + Sync + 'static,
    on_exit: impl FnOnce(Option<i32>) + Send + 'static,
) -> McResult<GameHandle> {
    std::fs::create_dir_all(&plan.cwd)?;
    let mut cmd = Command::new(&plan.program);
    cmd.args(&plan.args).current_dir(&plan.cwd).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: sin consola negra
    }
    let mut child = cmd.spawn().map_err(|e| McError::Invalid(format!("no se pudo ejecutar {}: {e}", plan.program.display())))?;
    let out = child.stdout.take();
    let err = child.stderr.take();
    let handle = GameHandle { child: Arc::new(Mutex::new(Some(child))) };
    let on_line = Arc::new(on_line);
    let file = Arc::new(Mutex::new(log_file.and_then(|p| {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::File::create(p).ok()
    })));
    let secrets = Arc::new(secrets.into_iter().filter(|s| s.len() > 6).collect::<Vec<_>>());
    let mut threads = vec![];
    for (stream, reader) in [("out", out.map(|r| Box::new(r) as Box<dyn std::io::Read + Send>)), ("err", err.map(|r| Box::new(r) as Box<dyn std::io::Read + Send>))] {
        let Some(reader) = reader else { continue };
        let (cb, file, secrets) = (on_line.clone(), file.clone(), secrets.clone());
        threads.push(std::thread::spawn(move || {
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                let mut clean = line;
                for s in secrets.iter() {
                    clean = clean.replace(s.as_str(), "***");
                }
                if let Some(f) = file.lock().unwrap().as_mut() {
                    let _ = writeln!(f, "[{stream}] {clean}");
                }
                cb(stream, &clean);
            }
        }));
    }
    let child_arc = handle.child.clone();
    std::thread::spawn(move || {
        // espera activa corta para no retener el lock mientras el juego corre
        let code = loop {
            {
                let mut g = child_arc.lock().unwrap();
                match g.as_mut() {
                    Some(c) => match c.try_wait() {
                        Ok(Some(st)) => break st.code(),
                        Ok(None) => {}
                        Err(_) => break None,
                    },
                    None => break None,
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        };
        for t in threads {
            let _ = t.join();
        }
        on_exit(code);
    });
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::VersionJson;

    fn inst(json: &str, cp: Vec<&str>) -> Installed {
        let merged = VersionJson::parse(json).unwrap();
        Installed {
            id: merged.id.clone(),
            merged,
            classpath: cp.into_iter().map(PathBuf::from).collect(),
            natives_dir: PathBuf::from("/n"),
            java_major: 21,
            asset_index: "17".into(),
            logging_arg: Some("-Dlog4j.configurationFile=/l.xml".into()),
        }
    }

    #[test]
    fn modern_arguments_with_quickplay_and_resolution() {
        let j = r#"{"id":"1.21.1","type":"release","mainClass":"net.minecraft.client.main.Main",
          "arguments":{"game":["--username","${auth_player_name}","--uuid","${auth_uuid}","--accessToken","${auth_access_token}","--userType","${user_type}",
            {"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--width","${resolution_width}","--height","${resolution_height}"]},
            {"rules":[{"action":"allow","features":{"is_quick_play_multiplayer":true}}],"value":["--quickPlayMultiplayer","${quickPlayMultiplayer}"]}],
           "jvm":["-Djava.library.path=${natives_directory}","-cp","${classpath}"]}}"#;
        let i = inst(j, vec!["/a.jar", "/b.jar"]);
        let d = Dirs::new("/root");
        let s = Session::offline("Alex");
        let o = Options { game_dir: "/g".into(), java_bin: "/j/java".into(), xmx_mb: 2048, xms_mb: 512, width: Some(1280), height: Some(720), join_server: Some("mc.drakescraft.cl:25565".into()), ..Default::default() };
        let p = plan(&d, &i, &s, &o).unwrap();
        let joined = p.args.join(" ");
        assert!(joined.contains("-Xmx2048M"));
        assert!(joined.contains("--username Alex"));
        assert!(joined.contains("--userType legacy"));
        assert!(joined.contains("--width 1280 --height 720"));
        assert!(joined.contains("--quickPlayMultiplayer mc.drakescraft.cl:25565"));
        assert!(joined.contains("-Dlog4j.configurationFile=/l.xml"));
        let cp = p.args.iter().position(|a| a == "-cp").unwrap();
        let sep = if cfg!(windows) { ";" } else { ":" };
        assert_eq!(p.args[cp + 1], format!("/a.jar{sep}/b.jar"));
        let main = p.args.iter().position(|a| a == "net.minecraft.client.main.Main").unwrap();
        assert!(main > cp);
    }

    #[test]
    fn no_quickplay_when_not_requested() {
        let j = r#"{"id":"1.20.1","mainClass":"M","arguments":{"game":[{"rules":[{"action":"allow","features":{"is_quick_play_multiplayer":true}}],"value":["--quickPlayMultiplayer","${quickPlayMultiplayer}"]}],"jvm":["-cp","${classpath}"]}}"#;
        let p = plan(&Dirs::new("/r"), &inst(j, vec!["/a"]), &Session::offline("Zed"), &Options { game_dir: "/g".into(), java_bin: "j".into(), xmx_mb: 1024, xms_mb: 256, ..Default::default() }).unwrap();
        assert!(!p.args.iter().any(|a| a == "--quickPlayMultiplayer"));
    }

    #[test]
    fn legacy_arguments_string() {
        let j = r#"{"id":"1.12.2","mainClass":"M","minecraftArguments":"--username ${auth_player_name} --version ${version_name}"}"#;
        let p = plan(&Dirs::new("/r"), &inst(j, vec!["/a"]), &Session::offline("Zed"), &Options { game_dir: "/g".into(), java_bin: "j".into(), xmx_mb: 1024, xms_mb: 256, ..Default::default() }).unwrap();
        assert!(p.args.join(" ").contains("--username Zed --version 1.12.2"));
        assert!(p.args.iter().any(|a| a == "-cp"));
    }
}
