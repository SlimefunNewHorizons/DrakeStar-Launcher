//! Flujo completo del boton JUGAR: loader -> version -> Java -> mods -> cuenta -> plan de lanzamiento.

use crate::accounts::Accounts;
use crate::instances::{Instance, Loader, Target};
use crate::launch::{self, Options, Plan, Session};
use crate::progress::ProgressSink;
use crate::{fabric, install, java, mods, Dirs, McError, McResult};

/// Resultado de preparar una instancia: todo listo para `launch::spawn`.
pub struct Prepared {
    pub plan: Plan,
    pub session: Session,
    pub game_dir: std::path::PathBuf,
    /// Avisos para mostrar al usuario (mods omitidos, Fused no disponible para la version...).
    pub warnings: Vec<String>,
    pub java_major: u32,
    pub version_id: String,
}

/// Prepara la instancia sin lanzar. `client_id` es el de la app de Azure para renovar cuentas Microsoft.
pub fn prepare(d: &Dirs, inst: &Instance, account_id: &str, client_id: &str, sink: &dyn ProgressSink) -> McResult<Prepared> {
    inst.validate()?;
    let mut warnings = vec![];

    // 1) cuenta primero: si falla el login no vale la pena bajar 400 MB
    sink.stage("account", "Comprobando tu cuenta");
    let session = Accounts::new(d).session(account_id, client_id)?;

    // 2) version (+ loader)
    let version_id = match inst.loader {
        Loader::Vanilla => inst.mc.clone(),
        Loader::Fabric => fabric::ensure_profile(d, &inst.mc, None)?,
    };
    let installed = install::install(d, &version_id, sink)?;
    if sink.cancelled() {
        return Err(McError::Cancelled);
    }

    // 3) Java
    let java_bin = java::ensure(d, installed.java_major, sink)?;

    // 4) mods
    let game_dir = d.instance(&inst.id);
    std::fs::create_dir_all(&game_dir)?;
    // Primer arranque: sin la pantalla de narrador/accesibilidad de Minecraft (si ya hay options.txt no se toca).
    let options = game_dir.join("options.txt");
    if !options.exists() {
        let _ = std::fs::write(&options, "onboardAccessibility:false\n");
    }
    if inst.loader == Loader::Fabric {
        sink.stage("mods", "Instalando mods");
        let mods_dir = game_dir.join("mods");
        mods::ensure_fabric_api(&mods_dir, &inst.mc)?;
        if inst.performance {
            let ok = mods::ensure_performance(&mods_dir, &inst.mc, &|l| sink.log(l));
            if ok.len() < mods::PERFORMANCE_SET.len() {
                warnings.push(format!("Solo {} de {} mods de rendimiento existen para Minecraft {}.", ok.len(), mods::PERFORMANCE_SET.len(), inst.mc));
            }
        }
        if inst.fused {
            match mods::ensure_fused(&mods_dir, &inst.mc, "fabric") {
                Ok(Some(_)) => {}
                Ok(None) => warnings.push(format!("DrakeStar Fused aun no tiene una version publicada para Minecraft {}: se abre sin Fused.", inst.mc)),
                Err(e) => warnings.push(format!("No se pudo instalar DrakeStar Fused: {e}")),
            }
        } else {
            mods::remove_fused(&mods_dir);
        }
    }

    // 5) plan
    sink.stage("launch", "Iniciando Minecraft");
    let mut o = Options { game_dir: game_dir.clone(), java_bin, xmx_mb: inst.xmx_mb, xms_mb: (inst.xmx_mb / 4).max(512), extra_jvm: inst.extra_jvm.clone(), ..Default::default() };
    match &inst.target {
        Target::Menu => {}
        Target::Server { address } => o.join_server = Some(address.clone()),
        Target::World { name } => o.open_world = Some(name.clone()),
    }
    let plan = launch::plan(d, &installed, &session, &o)?;
    Ok(Prepared { plan, session, game_dir, warnings, java_major: installed.java_major, version_id })
}
