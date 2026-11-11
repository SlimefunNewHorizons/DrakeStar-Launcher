//! Glue de Tauri: cada comando delega en `drakestar-app`. La logica (y sus tests)
//! vive en `crates/drakestar-app`, que no depende de Tauri.

use drakestar_app::{
    compat_report::{self, ModeCompat},
    instances::{self, InstanceInfo, LogFile},
    java::{self, JavaInfo},
    fabric::ClientStatus,
    mods,
    modsync::{self, ConflictReport, DeltaSyncPlan, DeltaSyncResult, ServerModManifest},
    ping::{self, ServerStatusPing},
    process::{EventSink, LineRec, ProcEvent, ProcStatus},
    release::{self, Resolution},
    rustclient::RustClientStatus,
    server::{PluginInfo, Prop, ServerStatus, WorldInfo},
    service::Service,
    settings::Settings,
    store::ProfileList,
    Layout,
};
use drakestar_core::{Mode, Profile};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

type Res<T> = Result<T, String>;
fn s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

struct TauriSink(AppHandle);
impl EventSink for TauriSink {
    fn emit(&self, ev: ProcEvent) {
        let _ = self.0.emit("proc-event", ev);
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(s)
}

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    data_dir: String,
    platform: String,
    /// Siempre true aqui: distingue el backend real del mock del modo navegador.
    backend: &'static str,
}

#[tauri::command]
fn app_info(svc: State<'_, Service>) -> AppInfo {
    AppInfo {
        name: "DrakeStar",
        version: env!("CARGO_PKG_VERSION"),
        data_dir: svc.layout.root().to_string_lossy().to_string(),
        platform: release::platform(),
        backend: "tauri",
    }
}

#[tauri::command]
fn compat_report() -> Vec<ModeCompat> {
    compat_report::report()
}

// ---------- perfiles ----------
#[tauri::command]
fn list_profiles(svc: State<'_, Service>) -> ProfileList {
    svc.store.list()
}
#[tauri::command]
fn validate_profile(profile: Profile) -> Option<String> {
    profile.validate().err().map(|e| e.to_string())
}
#[tauri::command]
fn create_profile(svc: State<'_, Service>, profile: Profile) -> Res<Profile> {
    svc.create_profile(&profile).map_err(s)?;
    Ok(profile)
}
#[tauri::command]
fn update_profile(svc: State<'_, Service>, profile: Profile) -> Res<Profile> {
    svc.store.update(&profile).map_err(s)?;
    drakestar_app::instances::ensure_dirs(&svc.layout, &profile).map_err(s)?;
    Ok(profile)
}
#[tauri::command]
fn delete_profile(svc: State<'_, Service>, id: String, remove_data: bool) -> Res<()> {
    svc.delete_profile(&id, remove_data).map_err(s)
}
#[tauri::command]
fn duplicate_profile(svc: State<'_, Service>, id: String, new_id: String, new_name: String) -> Res<Profile> {
    let p = svc.store.duplicate(&id, &new_id, &new_name).map_err(s)?;
    instances::ensure_dirs(&svc.layout, &p).map_err(s)?;
    Ok(p)
}
#[tauri::command]
fn import_profile(svc: State<'_, Service>, json: String, overwrite: bool) -> Res<Profile> {
    let p = svc.store.import_json(&json, overwrite).map_err(s)?;
    instances::ensure_dirs(&svc.layout, &p).map_err(s)?;
    Ok(p)
}
#[tauri::command]
fn export_profile(svc: State<'_, Service>, id: String) -> Res<String> {
    svc.store.export_json(&id).map_err(s)
}
#[tauri::command]
fn export_profile_file(svc: State<'_, Service>, id: String) -> Res<String> {
    Ok(svc.store.export_to_file(&id).map_err(s)?.to_string_lossy().to_string())
}

// ---------- ajustes ----------
#[tauri::command]
fn get_settings(svc: State<'_, Service>) -> Settings {
    svc.settings()
}
#[tauri::command]
fn set_settings(svc: State<'_, Service>, settings: Settings) -> Res<Settings> {
    settings.save(&svc.layout).map_err(s)?;
    Ok(settings)
}

// ---------- autenticación Microsoft (OAuth 2.0 Device Code) ----------
#[tauri::command]
async fn msft_login_start() -> Res<drakestar_rust_client::auth::DeviceCodeAuth> {
    blocking(|| drakestar_rust_client::auth::request_device_code().map_err(s)).await?
}

#[tauri::command]
async fn msft_login_poll(
    svc: State<'_, Service>,
    device_code: String,
) -> Res<drakestar_rust_client::auth::PollResult> {
    let svc = svc.inner().clone();
    blocking(move || {
        let res = drakestar_rust_client::auth::poll_device_code(&device_code).map_err(s)?;
        if let drakestar_rust_client::auth::PollResult::Success(ref session) = res {
            let mut st = svc.settings();
            st.session_uuid = Some(session.uuid.clone());
            st.session_token = Some(session.access_token.clone());
            st.session_name = Some(session.name.clone());
            st.save(&svc.layout).map_err(s)?;
        }
        Ok(res)
    })
    .await?
}

#[tauri::command]
fn msft_logout(svc: State<'_, Service>) -> Res<Settings> {
    let mut st = svc.settings();
    st.session_uuid = None;
    st.session_token = None;
    st.session_name = None;
    st.save(&svc.layout).map_err(s)?;
    Ok(st)
}
#[tauri::command]
async fn detect_java(svc: State<'_, Service>) -> Res<JavaInfo> {
    let bin = svc.settings().java_bin();
    blocking(move || java::detect(&bin)).await
}
#[derive(Serialize)]
struct JavaCheck {
    info: JavaInfo,
    warning: Option<String>,
}
/// Detecta Java y avisa si no alcanza lo que pide el perfil (local-server/clientes).
#[tauri::command]
async fn java_check(svc: State<'_, Service>, id: String) -> Res<JavaCheck> {
    let p = svc.store.get(&id).map_err(s)?;
    let bin = svc.settings().java_bin();
    blocking(move || {
        let info = java::detect(&bin);
        let warning = java::compatibility_warning(&info, &p.minecraft, p.java.major);
        JavaCheck { info, warning }
    })
    .await
}

// ---------- instancias ----------
#[derive(Serialize)]
struct InstanceRow {
    profile: Profile,
    info: InstanceInfo,
    status: ProcStatus,
}
#[tauri::command]
async fn list_instances(svc: State<'_, Service>) -> Res<Vec<InstanceRow>> {
    let svc = svc.inner().clone();
    blocking(move || {
        svc.store
            .list()
            .profiles
            .into_iter()
            .map(|p| InstanceRow { info: instances::info(&svc.layout, &p), status: svc.procs.status(&p.id), profile: p })
            .collect()
    })
    .await
}
#[tauri::command]
fn ensure_instance(svc: State<'_, Service>, id: String) -> Res<()> {
    let p = svc.store.get(&id).map_err(s)?;
    instances::ensure_dirs(&svc.layout, &p).map_err(s)
}
#[tauri::command]
fn list_logs(svc: State<'_, Service>, id: String) -> Res<Vec<LogFile>> {
    let p = svc.store.get(&id).map_err(s)?;
    Ok(instances::list_logs(&svc.layout, &p))
}
#[tauri::command]
fn read_log(svc: State<'_, Service>, id: String, source: String, name: String, max_lines: usize) -> Res<Vec<String>> {
    let p = svc.store.get(&id).map_err(s)?;
    instances::read_log_tail(&svc.layout, &p, &source, &name, max_lines.min(5000)).map_err(s)
}
#[tauri::command]
fn clear_logs(svc: State<'_, Service>, id: String) -> Res<usize> {
    let p = svc.store.get(&id).map_err(s)?;
    instances::clear_launcher_logs(&svc.layout, &p).map_err(s)
}

// ---------- procesos / consola ----------
#[tauri::command]
fn server_start(svc: State<'_, Service>, id: String) -> Res<ProcStatus> {
    svc.server_start(&id).map_err(s)
}
#[tauri::command]
fn server_stop(svc: State<'_, Service>, id: String) -> Res<()> {
    svc.server_stop(&id).map_err(s)
}
#[tauri::command]
fn server_kill(svc: State<'_, Service>, id: String) -> Res<()> {
    svc.procs.kill(&id).map_err(s)
}
#[tauri::command]
fn server_command(svc: State<'_, Service>, id: String, line: String) -> Res<()> {
    svc.server_command(&id, &line).map_err(s)
}
#[tauri::command]
fn proc_status(svc: State<'_, Service>, id: String) -> ProcStatus {
    svc.procs.status(&id)
}
#[tauri::command]
fn console_history(svc: State<'_, Service>, id: String) -> Vec<LineRec> {
    svc.procs.history(&id)
}
#[tauri::command]
fn console_clear(svc: State<'_, Service>, id: String) {
    svc.procs.clear_history(&id)
}
#[tauri::command]
fn server_plan(svc: State<'_, Service>, id: String) -> Res<String> {
    let plan = svc.server_plan(&id).map_err(s)?;
    Ok(format!("{} {}\n(cwd: {})", plan.program, plan.args.join(" "), plan.cwd))
}

// ---------- servidor local ----------
#[tauri::command]
fn server_status(svc: State<'_, Service>, id: String) -> Res<ServerStatus> {
    Ok(svc.server_dir(&id).map_err(s)?.1.status())
}
#[tauri::command]
fn server_accept_eula(svc: State<'_, Service>, id: String) -> Res<()> {
    svc.server_dir(&id).map_err(s)?.1.accept_eula().map_err(s)
}
#[tauri::command]
fn server_properties(svc: State<'_, Service>, id: String) -> Res<Vec<Prop>> {
    Ok(svc.server_dir(&id).map_err(s)?.1.read_properties())
}
#[tauri::command]
fn server_set_properties(svc: State<'_, Service>, id: String, updates: BTreeMap<String, String>) -> Res<Vec<Prop>> {
    let sd = svc.server_dir(&id).map_err(s)?.1;
    sd.ensure().map_err(s)?;
    sd.write_properties(&updates).map_err(s)?;
    Ok(sd.read_properties())
}
#[tauri::command]
fn server_plugins(svc: State<'_, Service>, id: String) -> Res<Vec<PluginInfo>> {
    Ok(svc.server_dir(&id).map_err(s)?.1.plugins())
}
#[tauri::command]
fn server_toggle_plugin(svc: State<'_, Service>, id: String, file: String) -> Res<String> {
    if svc.procs.is_active(&id) {
        return Err("detén el servidor antes de activar/desactivar plugins".into());
    }
    svc.server_dir(&id).map_err(s)?.1.toggle_plugin(&file).map_err(s)
}
#[tauri::command]
fn server_worlds(svc: State<'_, Service>, id: String) -> Res<Vec<WorldInfo>> {
    Ok(svc.server_dir(&id).map_err(s)?.1.worlds())
}
#[tauri::command]
async fn server_backup_world(svc: State<'_, Service>, id: String, world_name: String) -> Res<String> {
    let svc = svc.inner().clone();
    blocking(move || {
        let (_, sd) = svc.server_dir(&id).map_err(s)?;
        let backups_dir = sd.dir.join("backups");
        let dst = sd.backup_world(&world_name, &backups_dir).map_err(s)?;
        Ok(dst.to_string_lossy().to_string())
    })
    .await?
}

// ---------- releases ----------
#[tauri::command]
fn manifest_import(svc: State<'_, Service>, json: String) -> Res<usize> {
    release::import_manifest(&svc.layout, &json).map(|m| m.releases.len()).map_err(s)
}
#[tauri::command]
fn manifest_summary(svc: State<'_, Service>) -> Option<usize> {
    release::load_manifest(&svc.layout).map(|m| m.releases.len())
}
#[tauri::command]
fn resolve_release(svc: State<'_, Service>, id: String) -> Res<Resolution> {
    let p = svc.store.get(&id).map_err(s)?;
    Ok(release::resolve(release::load_manifest(&svc.layout).as_ref(), &p))
}
#[tauri::command]
async fn install_server_jar(svc: State<'_, Service>, id: String) -> Res<u64> {
    let svc = svc.inner().clone();
    blocking(move || svc.install_server_jar(&id).map_err(s)).await?
}

// ---------- delta-sync de mods y analizador Fabric ----------
#[tauri::command]
async fn modpack_delta_plan(svc: State<'_, Service>, id: String, manifest_json: String) -> Res<DeltaSyncPlan> {
    let svc = svc.inner().clone();
    blocking(move || {
        let p = svc.store.get(&id).map_err(s)?;
        let m = ServerModManifest::from_json(&manifest_json).map_err(s)?;
        let mods_dir = instances::data_dir(&svc.layout, &p).join("mods");
        modsync::compute_delta_sync(&mods_dir, &m).map_err(s)
    })
    .await?
}

#[tauri::command]
async fn modpack_delta_apply(svc: State<'_, Service>, id: String, manifest_json: String) -> Res<DeltaSyncResult> {
    let svc = svc.inner().clone();
    blocking(move || {
        let p = svc.store.get(&id).map_err(s)?;
        let m = ServerModManifest::from_json(&manifest_json).map_err(s)?;
        let mods_dir = instances::data_dir(&svc.layout, &p).join("mods");
        modsync::apply_delta_sync(&mods_dir, &m).map_err(s)
    })
    .await?
}

#[tauri::command]
fn analyze_fabric_conflicts(manifest_json: String) -> Res<ConflictReport> {
    let m = ServerModManifest::from_json(&manifest_json).map_err(s)?;
    Ok(modsync::analyze_mod_conflicts(&m.mods))
}

// ---------- online ----------
#[tauri::command]
async fn ping_server(address: String) -> Res<u128> {
    blocking(move || ping::tcp_ping(&address, Duration::from_secs(4)).map_err(s)).await?
}
#[tauri::command]
async fn ping_mc_status(address: String) -> Res<ServerStatusPing> {
    blocking(move || ping::mc_status_ping(&address, Duration::from_secs(4)).map_err(s)).await?
}

// ---------- cliente Java gestionado (Fabric) ----------
#[tauri::command]
fn client_status(svc: State<'_, Service>, id: String) -> Res<ClientStatus> {
    svc.client_status(&id).map_err(s)
}
#[tauri::command]
async fn client_install(svc: State<'_, Service>, id: String) -> Res<ClientStatus> {
    let svc = svc.inner().clone();
    blocking(move || svc.client_install(&id).map_err(s)).await?
}
#[tauri::command]
fn client_plan(svc: State<'_, Service>, id: String) -> Res<String> {
    svc.client_plan(&id).map_err(s)
}
#[tauri::command]
fn client_start(svc: State<'_, Service>, id: String) -> Res<ProcStatus> {
    svc.client_start(&id).map_err(s)
}
#[tauri::command]
fn client_stop(svc: State<'_, Service>, id: String) -> Res<()> {
    svc.client_stop(&id).map_err(s)
}

// ---------- cliente Rust gestionado (DrakesRust) ----------
#[tauri::command]
fn rust_client_status(svc: State<'_, Service>, id: String) -> Res<RustClientStatus> {
    svc.rust_client_status(&id).map_err(s)
}
#[tauri::command]
async fn rust_client_install(svc: State<'_, Service>, id: String) -> Res<RustClientStatus> {
    let svc = svc.inner().clone();
    blocking(move || svc.rust_client_install(&id).map_err(s)).await?
}
#[tauri::command]
fn rust_client_plan(svc: State<'_, Service>, id: String) -> Res<String> {
    let plan = svc.rust_client_plan(&id).map_err(s)?;
    Ok(format!("{} {}\n(cwd: {})", plan.program, plan.args.join(" "), plan.cwd))
}
#[tauri::command]
fn rust_client_start(svc: State<'_, Service>, id: String) -> Res<ProcStatus> {
    svc.rust_client_start(&id).map_err(s)
}
#[tauri::command]
fn rust_client_stop(svc: State<'_, Service>, id: String) -> Res<()> {
    svc.rust_client_stop(&id).map_err(s)
}
#[tauri::command]
fn rust_client_command(svc: State<'_, Service>, id: String, line: String) -> Res<()> {
    svc.rust_client_command(&id, &line).map_err(s)
}

// ---------- mods (cargador) ----------
#[tauri::command]
fn mods_report(svc: State<'_, Service>, id: String) -> Res<mods::LoadReport> {
    let p = svc.store.get(&id).map_err(s)?;
    mods::load(&svc.layout, &p, drakestar_core::mods::Environment::Client).map_err(s)
}
#[tauri::command]
fn mods_decide(svc: State<'_, Service>, id: String) -> Res<mods::LoadReport> {
    svc.mods_decide(&id).map_err(s)
}

// ---------- Blackwall Guard & Modrinth & Prism Import ----------
#[tauri::command]
fn mods_guard_override(svc: State<'_, Service>, id: String, item: String, allow: bool) -> Res<()> {
    let p = svc.store.get(&id).map_err(s)?;
    let instance = drakestar_app::fabric::instance_dir(&svc.layout, &p.id);
    let mods_dir = instance.join("mods");
    mods::set_override(&mods_dir, &item, allow).map_err(s)
}

#[tauri::command]
async fn modrinth_search(query: String, limit: Option<usize>) -> Res<Vec<drakestar_app::modrinth::ModrinthSearchHit>> {
    blocking(move || drakestar_app::modrinth::search_mods(&query, limit.unwrap_or(20)).map_err(s)).await?
}

#[tauri::command]
async fn modrinth_versions(project_id: String, mc_version: Option<String>) -> Res<Vec<drakestar_app::modrinth::ModrinthVersion>> {
    blocking(move || drakestar_app::modrinth::get_project_versions(&project_id, mc_version.as_deref()).map_err(s)).await?
}

#[tauri::command]
async fn modrinth_install(svc: State<'_, Service>, id: String, file_url: String, file_name: String) -> Res<u64> {
    let svc = svc.inner().clone();
    blocking(move || {
        let p = svc.store.get(&id).map_err(s)?;
        let instance = drakestar_app::fabric::instance_dir(&svc.layout, &p.id);
        let mods_dir = instance.join("mods");
        drakestar_app::modrinth::install_mod(&mods_dir, &file_url, &file_name).map_err(s)
    }).await?
}

#[tauri::command]
fn prism_list_instances() -> Vec<drakestar_app::prism_import::PrismInstanceSummary> {
    drakestar_app::prism_import::list_instances()
}

#[tauri::command]
async fn prism_import_instance(svc: State<'_, Service>, path: String) -> Res<drakestar_app::prism_import::PrismImportResult> {
    let svc = svc.inner().clone();
    blocking(move || {
        drakestar_app::prism_import::import_instance(std::path::Path::new(&path), &svc.layout, &svc.store).map_err(s)
    }).await?
}

#[tauri::command]
async fn diagnostics(svc: State<'_, Service>) -> Res<String> {
    let svc = svc.inner().clone();
    blocking(move || {
        let st = svc.settings();
        let list = svc.store.list();
        let v = serde_json::json!({
            "app": "DrakeStar", "version": env!("CARGO_PKG_VERSION"),
            "backend": "tauri", "platform": release::platform(),
            "data_dir": svc.layout.root(),
            "java": java::detect(&st.java_bin()),
            "profiles": list.profiles.iter().map(|p| serde_json::json!({
                "id": p.id, "mode": p.mode, "channel": p.channel, "minecraft": p.minecraft })).collect::<Vec<_>>(),
            "invalid_profiles": list.invalid,
            "manifest_releases": release::load_manifest(&svc.layout).map(|m| m.releases.len()),
            "launch_implemented": [Mode::LocalServer, Mode::DrakesRust],
        });
        serde_json::to_string_pretty(&v).unwrap_or_default()
    })
    .await
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = match std::env::var_os("DRAKESTAR_DATA_DIR") {
                Some(d) => std::path::PathBuf::from(d),
                None => app.path().app_data_dir()?,
            };
            std::fs::create_dir_all(&dir)?;
            let svc = Service::new(Layout::new(dir), Arc::new(TauriSink(app.handle().clone())));
            app.manage(svc);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info, compat_report, list_profiles, validate_profile, create_profile, update_profile,
            delete_profile, duplicate_profile, import_profile, export_profile, export_profile_file,
            get_settings, set_settings, detect_java, java_check, list_instances, ensure_instance,
            list_logs, read_log, clear_logs, server_start, server_stop, server_kill, server_command,
            proc_status, console_history, console_clear, server_plan, server_status, server_accept_eula,
            server_properties, server_set_properties, server_plugins, server_toggle_plugin, server_worlds,
            server_backup_world, manifest_import, manifest_summary, resolve_release, install_server_jar,
            modpack_delta_plan, modpack_delta_apply, analyze_fabric_conflicts,
            ping_server, ping_mc_status, diagnostics,
            client_status, client_install, client_plan, client_start, client_stop,
            rust_client_status, rust_client_install, rust_client_plan, rust_client_start,
            rust_client_stop, rust_client_command, mods_report, mods_decide,
            mods_guard_override, modrinth_search, modrinth_versions, modrinth_install,
            prism_list_instances, prism_import_instance,
            msft_login_start, msft_login_poll, msft_logout
        ])
        .run(tauri::generate_context!())
        .expect("error al ejecutar DrakeStar");
}
