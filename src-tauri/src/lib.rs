//! DrakeStar: launcher de Minecraft. Esta capa solo conecta la interfaz con `drakestar-mc` (que no depende de Tauri).
//!
//! Eventos hacia la UI: `progress` (etapa y fraccion), `game-line` (salida del juego), `game-state`, `msa` (login Microsoft),
//! `report` (reporte de error enviado).

use drakestar_mc::accounts::{Account, AccountList, Accounts};
use drakestar_mc::instances::{Instance, Instances};
use drakestar_mc::launch::{self, GameHandle};
use drakestar_mc::msa::{self, DeviceCode, Poll};
use drakestar_mc::progress::CancelFlag;
use drakestar_mc::settings::Settings;
use drakestar_mc::{manifest, play, server, support, Dirs, ProgressSink};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

type Res<T> = Result<T, String>;

/// Ids de instancia: solo minusculas, numeros, `-` y `_` (igual que `Instance::validate`); evita salirse de `instances/`.
fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}
fn s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

struct AppState {
    dirs: Dirs,
    cancel: Mutex<CancelFlag>,
    game: Mutex<Option<GameHandle>>,
    /// Servidor local gestionado (instancias `local`); se cierra con el juego.
    server: Mutex<Option<GameHandle>>,
    login_cancel: Mutex<Option<Arc<AtomicBool>>>,
    busy: AtomicBool,
    /// El jugador cerro el juego desde el launcher: no cuenta como fallo.
    closing: AtomicBool,
}

#[derive(Serialize, Clone)]
struct Progress {
    stage: String,
    text: String,
    fraction: f32,
}

struct Sink {
    app: AppHandle,
    dirs: Dirs,
    cancel: CancelFlag,
    cur: Mutex<(String, String)>,
}

impl ProgressSink for Sink {
    fn stage(&self, id: &str, text: &str) {
        *self.cur.lock().unwrap() = (id.into(), text.into());
        support::app_log(&self.dirs, "INFO", &format!("[{id}] {text}"));
        let _ = self.app.emit("progress", Progress { stage: id.into(), text: text.into(), fraction: 0.0 });
    }
    fn fraction(&self, f: f32) {
        let (st, tx) = self.cur.lock().unwrap().clone();
        let _ = self.app.emit("progress", Progress { stage: st, text: tx, fraction: f });
    }
    fn log(&self, line: &str) {
        support::app_log(&self.dirs, "INFO", line);
    }
    fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
    data_dir: String,
    os: &'static str,
    arch: &'static str,
    min_mc: &'static str,
    max_mc: &'static str,
    client_id_temporary: bool,
}

#[tauri::command]
fn app_info(st: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: st.dirs.root().to_string_lossy().to_string(),
        os: drakestar_mc::os_name(),
        arch: drakestar_mc::arch_name(),
        min_mc: manifest::MIN_SUPPORTED,
        max_mc: manifest::MAX_SUPPORTED,
        client_id_temporary: Settings::load(&st.dirs).client_id_is_temporary(),
    }
}

// ---------- ajustes ----------
#[tauri::command]
fn settings_get(st: State<'_, AppState>) -> Settings {
    Settings::load(&st.dirs)
}
#[tauri::command]
fn settings_set(st: State<'_, AppState>, settings: Settings) -> Res<Settings> {
    settings.save(&st.dirs).map_err(s)?;
    Ok(settings)
}

// ---------- cuentas ----------
#[tauri::command]
fn accounts_list(st: State<'_, AppState>) -> AccountList {
    Accounts::new(&st.dirs).list()
}
#[tauri::command]
fn account_add_offline(st: State<'_, AppState>, name: String) -> Res<Account> {
    Accounts::new(&st.dirs).add_offline(name.trim()).map_err(s)
}
#[tauri::command]
fn account_remove(st: State<'_, AppState>, id: String) -> Res<()> {
    Accounts::new(&st.dirs).remove(&id).map_err(s)
}
#[tauri::command]
fn account_set_default(st: State<'_, AppState>, id: String) -> Res<()> {
    Accounts::new(&st.dirs).set_default(&id).map_err(s)
}

#[derive(Serialize, Clone)]
struct MsaEvent {
    phase: &'static str,
    message: String,
    account: Option<Account>,
}

/// Inicia el login de Microsoft: devuelve el codigo, abre el navegador solo y sondea en segundo plano (evento `msa`).
#[tauri::command]
async fn msa_start(app: AppHandle, st: State<'_, AppState>) -> Res<DeviceCode> {
    let dirs = st.dirs.clone();
    let client_id = Settings::load(&dirs).client_id();
    let cid = client_id.clone();
    let dc = tauri::async_runtime::spawn_blocking(move || msa::start(&cid)).await.map_err(s)?.map_err(s)?;
    let url = dc.verification_uri_complete.clone().unwrap_or_else(|| dc.verification_uri.clone());
    let _ = app.opener().open_url(&url, None::<&str>);

    let cancel = Arc::new(AtomicBool::new(false));
    *st.login_cancel.lock().unwrap() = Some(cancel.clone());
    let (code, interval, expires) = (dc.device_code.clone(), dc.interval, dc.expires_in);
    let app2 = app.clone();
    std::thread::spawn(move || {
        let emit = |phase: &'static str, message: String, account: Option<Account>| {
            let _ = app2.emit("msa", MsaEvent { phase, message, account });
        };
        let started = std::time::Instant::now();
        let mut wait = interval;
        loop {
            if cancel.load(Ordering::SeqCst) {
                return emit("cancelled", "Login cancelado".into(), None);
            }
            if started.elapsed().as_secs() > expires {
                return emit("error", "El codigo caduco: vuelve a intentarlo.".into(), None);
            }
            std::thread::sleep(std::time::Duration::from_secs(wait));
            match msa::poll(&client_id, &code) {
                Ok(Poll::Pending(n)) => wait = n,
                Ok(Poll::Done(t)) => {
                    emit("working", "Verificando tu cuenta de Minecraft...".into(), None);
                    return match msa::minecraft_profile(&t.access_token).and_then(|p| Accounts::new(&dirs).add_microsoft(&p, &t.refresh_token)) {
                        Ok(acc) => {
                            support::app_log(&dirs, "INFO", &format!("cuenta Microsoft agregada: {}", acc.name));
                            emit("success", format!("Sesion iniciada como {}", acc.name), Some(acc))
                        }
                        Err(e) => {
                            support::app_log(&dirs, "ERROR", &format!("login Microsoft: {e}"));
                            emit("error", e.to_string(), None)
                        }
                    };
                }
                Err(e) => return emit("error", e.to_string(), None),
            }
        }
    });
    Ok(dc)
}

#[tauri::command]
fn msa_cancel(st: State<'_, AppState>) {
    if let Some(c) = st.login_cancel.lock().unwrap().take() {
        c.store(true, Ordering::SeqCst);
    }
}

// ---------- instancias ----------
#[tauri::command]
fn instances_list(st: State<'_, AppState>) -> Vec<Instance> {
    Instances::new(&st.dirs).list()
}
#[tauri::command]
fn instance_save(st: State<'_, AppState>, instance: Instance) -> Res<Instance> {
    Instances::new(&st.dirs).upsert(instance).map_err(s)
}
#[tauri::command]
fn instance_delete(st: State<'_, AppState>, id: String) -> Res<()> {
    Instances::new(&st.dirs).remove(&id).map_err(s)
}
#[tauri::command]
async fn versions_supported(st: State<'_, AppState>) -> Res<Vec<String>> {
    let dirs = st.dirs.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let m = manifest::load(&dirs).map_err(s)?;
        Ok(manifest::supported_releases(&m).into_iter().map(|e| e.id.clone()).collect())
    })
    .await
    .map_err(s)?
}

// ---------- jugar ----------
#[derive(Serialize, Clone)]
struct GameState {
    state: &'static str,
    message: String,
    code: Option<i32>,
    warnings: Vec<String>,
}

#[tauri::command]
fn play(app: AppHandle, st: State<'_, AppState>, instance_id: String, account_id: Option<String>) -> Res<()> {
    if st.busy.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una partida o una instalacion en curso.".into());
    }
    let dirs = st.dirs.clone();
    let accounts = Accounts::new(&dirs);
    let account = match account_id.and_then(|id| accounts.get(&id)).or_else(|| accounts.default_account()) {
        Some(a) => a,
        None => {
            st.busy.store(false, Ordering::SeqCst);
            return Err("Agrega una cuenta (Microsoft o sin premium) para jugar.".into());
        }
    };
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
    let app2 = app.clone();
    std::thread::spawn(move || {
        let st = app2.state::<AppState>();
        let emit = |state: &'static str, message: String, code: Option<i32>, warnings: Vec<String>| {
            let _ = app2.emit("game-state", GameState { state, message, code, warnings });
        };
        emit("preparing", format!("Preparando {}", inst.name), None, vec![]);
        let sink = Sink { app: app2.clone(), dirs: dirs.clone(), cancel: cancel.clone(), cur: Mutex::new((String::new(), String::new())) };
        let prepared = match play::prepare(&dirs, &inst, &account.id, &settings.client_id(), &sink) {
            Ok(p) => p,
            Err(e) => {
                support::app_log(&dirs, "ERROR", &format!("preparar {}: {e}", inst.id));
                emit(if cancel.is_cancelled() { "cancelled" } else { "error" }, e.to_string(), None, vec![]);
                st.busy.store(false, Ordering::SeqCst);
                return;
            }
        };
        if let Some(ls) = &prepared.server {
            emit("preparing", "Arrancando tu servidor local (la primera vez tarda)".into(), None, vec![]);
            support::app_log(&dirs, "INFO", &format!("servidor local de {} en el puerto {}", inst.id, ls.port));
            let dead = Arc::new(AtomicBool::new(false));
            let dead2 = dead.clone();
            let app_l = app2.clone();
            let sh = launch::spawn(
                &ls.plan,
                vec![],
                Some(ls.log_file.clone()),
                move |stream, line| {
                    let _ = app_l.emit("game-line", serde_json::json!({ "stream": stream, "line": format!("[servidor] {}", support::redact(line)) }));
                },
                move |_| dead2.store(true, Ordering::SeqCst),
            );
            let sh = match sh {
                Ok(h) => h,
                Err(e) => {
                    st.busy.store(false, Ordering::SeqCst);
                    emit("error", e.to_string(), None, vec![]);
                    return;
                }
            };
            *st.server.lock().unwrap() = Some(sh.clone());
            if let Err(e) = server::wait_ready(ls.port, std::time::Duration::from_secs(300), || !dead.load(Ordering::SeqCst), || cancel.is_cancelled()) {
                sh.kill();
                *st.server.lock().unwrap() = None;
                support::app_log(&dirs, "ERROR", &format!("servidor local: {e}"));
                st.busy.store(false, Ordering::SeqCst);
                emit(if cancel.is_cancelled() { "cancelled" } else { "error" }, e.to_string(), None, vec![]);
                return;
            }
        }
        support::app_log(&dirs, "INFO", &format!("lanzando {} ({}) con Java {}", inst.id, prepared.version_id, prepared.java_major));
        let started = std::time::SystemTime::now();
        let (app3, d3, inst3, game_dir) = (app2.clone(), dirs.clone(), inst.clone(), prepared.game_dir.clone());
        let secrets = vec![prepared.session.access_token.clone()];
        let log_file = prepared.game_dir.join("logs").join("launcher-session.log");
        let handle = launch::spawn(
            &prepared.plan,
            secrets,
            Some(log_file),
            {
                let app = app2.clone();
                move |stream, line| {
                    let _ = app.emit("game-line", serde_json::json!({ "stream": stream, "line": support::redact(line) }));
                }
            },
            move |code| {
                let st = app3.state::<AppState>();
                let crashed = !st.closing.swap(false, Ordering::SeqCst) && code != Some(0);
                *st.game.lock().unwrap() = None;
                if let Some(sv) = st.server.lock().unwrap().take() {
                    sv.kill();
                }
                st.busy.store(false, Ordering::SeqCst);
                support::app_log(&d3, "INFO", &format!("el juego termino ({code:?})"));
                let _ = app3.emit("game-state", GameState { state: if crashed { "crashed" } else { "exited" }, message: String::new(), code, warnings: vec![] });
                if crashed {
                    let _ = started;
                    let ctx = support::Context { app_version: env!("CARGO_PKG_VERSION").into(), reason: "crash".into(), instance: Some(inst3.id.clone()), minecraft: Some(inst3.mc.clone()), exit_code: code, game_dir: Some(game_dir) };
                    if let Ok(zip) = support::bundle(&d3, &ctx) {
                        let settings = Settings::load(&d3);
                        let sent = if settings.send_reports {
                            support::upload(&zip, &settings.report_endpoint(), "", &settings.device_id, env!("CARGO_PKG_VERSION")).is_ok()
                        } else {
                            false
                        };
                        let _ = app3.emit("report", serde_json::json!({ "path": zip.to_string_lossy(), "sent": sent }));
                    }
                }
            },
        );
        match handle {
            Ok(h) => {
                *st.game.lock().unwrap() = Some(h);
                emit("running", String::new(), None, prepared.warnings);
            }
            Err(e) => {
                support::app_log(&dirs, "ERROR", &format!("spawn: {e}"));
                st.busy.store(false, Ordering::SeqCst);
                emit("error", e.to_string(), None, vec![]);
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn play_cancel(st: State<'_, AppState>) {
    st.cancel.lock().unwrap().cancel();
}
#[tauri::command]
fn game_kill(st: State<'_, AppState>) {
    if let Some(g) = st.game.lock().unwrap().as_ref() {
        st.closing.store(true, Ordering::SeqCst);
        g.kill();
    }
}

// ---------- soporte y logs ----------
#[tauri::command]
fn logs_read(st: State<'_, AppState>, source: String, instance: Option<String>, max_lines: usize) -> Vec<String> {
    let path = match (source.as_str(), instance) {
        ("game", Some(id)) if safe_id(&id) => st.dirs.instance(&id).join("logs").join("latest.log"),
        _ => support::log_dir(&st.dirs).join("launcher.log"),
    };
    let path = if source == "launcher" {
        // el log del dia mas reciente
        std::fs::read_dir(support::log_dir(&st.dirs)).ok().and_then(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|x| x.to_str()) == Some("log")).max()).unwrap_or(path)
    } else {
        path
    };
    let text = std::fs::read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines.min(2000));
    lines[start..].iter().map(|l| support::redact(l)).collect()
}

/// Crea un reporte de soporte; si `send` y hay recolector, lo envia.
#[tauri::command]
async fn support_report(st: State<'_, AppState>, instance: Option<String>, send: bool) -> Res<serde_json::Value> {
    let dirs = st.dirs.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let inst = instance.as_deref().and_then(|id| Instances::new(&dirs).get(id).ok());
        let ctx = support::Context {
            app_version: env!("CARGO_PKG_VERSION").into(),
            reason: "manual".into(),
            instance: inst.as_ref().map(|i| i.id.clone()),
            minecraft: inst.as_ref().map(|i| i.mc.clone()),
            exit_code: None,
            game_dir: inst.as_ref().map(|i| dirs.instance(&i.id)),
        };
        let zip = support::bundle(&dirs, &ctx).map_err(s)?;
        let mut sent = false;
        if send {
            let st = Settings::load(&dirs);
            support::upload(&zip, &st.report_endpoint(), "", &st.device_id, env!("CARGO_PKG_VERSION")).map_err(s)?;
            sent = true;
        }
        Ok(serde_json::json!({ "path": zip.to_string_lossy(), "sent": sent }))
    })
    .await
    .map_err(s)?
}

#[tauri::command]
fn open_folder(app: AppHandle, st: State<'_, AppState>, kind: String, id: Option<String>) -> Res<()> {
    let path = match (kind.as_str(), id) {
        ("instance", Some(id)) if safe_id(&id) => st.dirs.instance(&id),
        ("logs", _) => support::log_dir(&st.dirs),
        ("support", _) => st.dirs.root().join("support"),
        _ => st.dirs.root().to_path_buf(),
    };
    std::fs::create_dir_all(&path).map_err(s)?;
    app.opener().open_path(path.to_string_lossy().to_string(), None::<&str>).map_err(s)
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Res<()> {
    if !url.starts_with("https://") {
        return Err("solo https".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(s)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let dir = match std::env::var_os("DRAKESTAR_DATA_DIR") {
                Some(d) => std::path::PathBuf::from(d),
                None => app.path().app_data_dir()?,
            };
            std::fs::create_dir_all(&dir)?;
            let dirs = Dirs::new(dir);
            support::install_panic_hook(dirs.clone());
            support::rotate(&dirs, 14);
            support::app_log(&dirs, "INFO", &format!("DrakeStar {} arranca ({} {})", env!("CARGO_PKG_VERSION"), drakestar_mc::os_name(), drakestar_mc::arch_name()));
            app.manage(AppState { dirs, cancel: Mutex::new(CancelFlag::default()), game: Mutex::new(None), server: Mutex::new(None), login_cancel: Mutex::new(None), busy: AtomicBool::new(false), closing: AtomicBool::new(false) });
            Ok(())
        })
        .on_window_event(|w, ev| {
            // al cerrar el launcher no se deja un servidor local huerfano
            if let tauri::WindowEvent::Destroyed = ev {
                if let Some(sv) = w.app_handle().state::<AppState>().server.lock().unwrap().take() {
                    sv.kill();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info, settings_get, settings_set, accounts_list, account_add_offline, account_remove, account_set_default,
            msa_start, msa_cancel, instances_list, instance_save, instance_delete, versions_supported,
            play, play_cancel, game_kill, logs_read, support_report, open_folder, open_url
        ])
        .run(tauri::generate_context!())
        .expect("error al ejecutar DrakeStar");
}
