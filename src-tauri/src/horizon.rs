//! Comandos Tauri del motor Horizon (modo DrakesRust con cliente nativo).
//!
//! Misma mecánica de estados y progreso que `play`: `preparing` → `running` →
//! `exited`/`crashed`, con los eventos que la UI ya escucha.

use crate::{s, AppState, GameState, Sink};
use drakestar_mc::accounts::Accounts;
use drakestar_mc::horizon::{self, Hz};
use drakestar_mc::instances::Instances;
use drakestar_mc::launch;
use drakestar_mc::settings::Settings;
use drakestar_mc::support;
use drakestar_mc::progress::CancelFlag;
use drakestar_mc::ProgressSink;
use drakestar_mc::Dirs;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Serialize)]
pub struct HzStatus {
    pub binaries: bool,
    pub resources: bool,
    pub java: bool,
    pub ready: bool,
    pub reason: Option<String>,
}

/// Estado del motor sin elegir instancia (todo compartido salvo los saves).
#[tauri::command]
pub fn horizon_status(st: State<'_, AppState>) -> Result<HzStatus, String> {
    let dirs = st.dirs.clone();
    // `game` solo lo usa el lanzamiento: aqui da igual (el estado es compartido)
    let hz = Hz { shared: horizon::shared_dir(&dirs), home: dirs.root().join("horizon-home"), game: dirs.root().join("horizon-home") };
    let binaries = hz.installed();
    let resources = hz.resources().is_dir();
    let java = drakestar_mc::java::installed(&dirs, horizon::PREPARE_JAVA).is_some();
    let (ready, reason) = if !binaries {
        (false, Some("faltan los binarios de Horizon".into()))
    } else if !resources {
        (false, Some("faltan preparar los recursos".into()))
    } else if !java {
        (false, Some(format!("falta el JDK {} para preparar", horizon::PREPARE_JAVA)))
    } else {
        (true, None)
    };
    Ok(HzStatus { binaries, resources, java, ready, reason })
}

/// Archivo de sesión para `--session-file` (formato que valida Horizon:
/// name, uuid, access_token y user_type de cuenta Microsoft).
fn write_session(dirs: &Dirs, id: &str, account_id: &str, client_id: &str) -> Result<std::path::PathBuf, String> {
    let session = Accounts::new(dirs).session(account_id, client_id).map_err(s)?;
    let path = dirs.instance(id).join("horizon-session.json");
    std::fs::create_dir_all(dirs.instance(id)).map_err(s)?;
    let json = serde_json::json!({
        "name": session.name,
        "uuid": session.uuid,
        "access_token": session.access_token,
        "user_type": "msa",
        "xuid": session.xuid,
    });
    std::fs::write(&path, json.to_string()).map_err(s)?;
    // el token da acceso a la cuenta: que no lo lean otros usuarios
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(path)
}

/// Baja los binarios del cliente nativo (una vez).
#[tauri::command]
pub fn horizon_install(app: AppHandle, st: State<'_, AppState>) -> Result<(), String> {
    if st.busy.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una partida o una instalacion en curso.".into());
    }
    let dirs = st.dirs.clone();
    let sink = Sink { app: app.clone(), dirs: dirs.clone(), cancel: CancelFlag::default(), cur: Mutex::new((String::new(), String::new())) };
    sink.stage("horizon", "Descargando el cliente nativo");
    let hz = Hz { shared: horizon::shared_dir(&dirs), home: dirs.root().join("horizon-home"), game: dirs.root().join("horizon-home") };
    let r = horizon::install(&hz).map_err(s);
    st.busy.store(false, Ordering::SeqCst);
    let _ = app.emit("progress", serde_json::json!({ "stage": "horizon", "text": "", "fraction": 1.0 }));
    r
}

/// Juega con el motor nativo. `server` (host[:puerto]) enciende el adaptador
/// experimental para versiones viejas y pide cuenta de Microsoft.
#[tauri::command]
pub fn horizon_play(app: AppHandle, st: State<'_, AppState>, instance_id: String, server: Option<String>) -> Result<(), String> {
    if st.busy.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una partida o una instalacion en curso.".into());
    }
    let dirs = st.dirs.clone();
    let inst = Instances::new(&dirs).get(&instance_id).map_err(|e| {
        st.busy.store(false, Ordering::SeqCst);
        s(e)
    })?;
    let mut settings = Settings::load(&dirs);
    settings.last_instance = Some(inst.id.clone());
    let _ = settings.save(&dirs);

    let cancel = CancelFlag::default();
    *st.cancel.lock().unwrap() = cancel.clone();
    st.closing.store(false, Ordering::SeqCst);
    std::thread::spawn(move || {
        let st = app.state::<AppState>();
        let fail_app = app.clone();
        let fail = move |message: String| {
            let st = fail_app.state::<AppState>();
            st.busy.store(false, Ordering::SeqCst);
            let _ = fail_app.emit("game-state", GameState { state: "error", message, code: None, warnings: vec![] });
        };
        let emit_app = app.clone();
        let emit = move |state: &'static str, message: String| {
            let _ = emit_app.emit("game-state", GameState { state, message, code: None, warnings: vec![] });
        };
        emit("preparing", "Preparando el motor nativo (la primera vez tarda)".into());
        let sink = Sink { app: app.clone(), dirs: dirs.clone(), cancel: cancel.clone(), cur: Mutex::new((String::new(), String::new())) };
        let hz = horizon::paths(&dirs, &inst.id);

        if !hz.installed() {
            if let Err(e) = horizon::install(&hz) {
                support::app_log(&dirs, "ERROR", &format!("instalar Horizon: {e}"));
                fail(e.to_string());
                return;
            }
        }
        let inputs = match horizon::ensure_inputs(&hz, &dirs, horizon::MC_NATIVE, &sink) {
            Ok(i) => i,
            Err(e) => {
                support::app_log(&dirs, "ERROR", &format!("insumos de Horizon: {e}"));
                fail(e.to_string());
                return;
            }
        };
        let jdk = match horizon::prepare_java(&dirs, &sink) {
            Ok(j) => j,
            Err(e) => {
                support::app_log(&dirs, "ERROR", &format!("JDK para preparar: {e}"));
                fail(e.to_string());
                return;
            }
        };
        if let Err(e) = horizon::prepare(&hz, &inputs, &jdk) {
            support::app_log(&dirs, "ERROR", &format!("preparar recursos: {e}"));
            fail(e.to_string());
            return;
        }

        // cuenta: con servidor hace falta sesión real; sin servidor, da igual
        let session_file = match server.as_deref() {
            Some(_) => {
                let accounts = Accounts::new(&dirs);
                let Some(acc) = accounts.default_account() else {
                    fail("Agrega una cuenta (Microsoft o sin premium) para jugar en linea.".into());
                    return;
                };
                match write_session(&dirs, &inst.id, &acc.id, &settings.client_id()) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        fail(e);
                        return;
                    }
                }
            }
            None => None,
        };

        emit("preparing", "Abriendo el cliente nativo".into());
        let plan = horizon::launch_plan(&hz, server.as_deref(), session_file.as_deref(), horizon::DEFAULT_RENDER_DISTANCE);
        let _ = std::fs::create_dir_all(hz.home.join("logs"));
        let log_file = hz.home.join("logs").join("launcher-session.log");
        let app2 = app.clone();
        let d3 = dirs.clone();
        let home3 = hz.home.clone();
        let inst3 = inst.clone();
        let handle = launch::spawn(
            &plan,
            vec![],
            Some(log_file),
            {
                let app = app2.clone();
                move |stream, line| {
                    let _ = app.emit("game-line", serde_json::json!({ "stream": stream, "line": support::redact(line) }));
                }
            },
            move |code| {
                let st = app2.state::<AppState>();
                let crashed = !st.closing.swap(false, Ordering::SeqCst) && code != Some(0);
                *st.game.lock().unwrap() = None;
                st.busy.store(false, Ordering::SeqCst);
                support::app_log(&d3, "INFO", &format!("el cliente nativo termino ({code:?})"));
                let _ = app2.emit("game-state", GameState { state: if crashed { "crashed" } else { "exited" }, message: String::new(), code, warnings: vec![] });
                if crashed {
                    let ctx = support::Context {
                        app_version: env!("CARGO_PKG_VERSION").into(),
                        reason: "crash".into(),
                        instance: Some(inst3.id.clone()),
                        minecraft: Some(inst3.mc.clone()),
                        exit_code: code,
                        game_dir: Some(home3.clone()),
                    };
                    if let Ok(zip) = support::bundle(&d3, &ctx) {
                        let settings = Settings::load(&d3);
                        let sent = if settings.send_reports {
                            support::upload(&zip, &settings.report_endpoint(), "", &settings.device_id, env!("CARGO_PKG_VERSION")).is_ok()
                        } else {
                            false
                        };
                        let _ = app2.emit("report", serde_json::json!({ "path": zip.to_string_lossy(), "sent": sent }));
                    }
                }
            },
        );
        match handle {
            Ok(h) => {
                *st.game.lock().unwrap() = Some(h.clone());
                emit("running", String::new());
                // la ventana arranca como "Horizon": se renombra en cuanto
                // aparece (solo X11; en Windows/macOS se deja como está)
                if let Some(pid) = h.pid() {
                    horizon::spawn_retitle(pid);
                }
            }
            Err(e) => {
                support::app_log(&dirs, "ERROR", &format!("lanzar Horizon: {e}"));
                fail(e.to_string());
            }
        }
    });
    Ok(())
}

/// Cierra el cliente nativo.
#[tauri::command]
pub fn horizon_stop(app: AppHandle, st: State<'_, AppState>) -> Result<(), String> {
    let game = st.game.lock().unwrap().take();
    if let Some(h) = game {
        h.kill();
    }
    st.busy.store(false, Ordering::SeqCst);
    let _ = app.emit("game-state", GameState { state: "exited", message: String::new(), code: None, warnings: vec![] });
    Ok(())
}