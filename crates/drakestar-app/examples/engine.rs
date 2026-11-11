//! Motor de demostración: el `Engine` real cargando mods nativos.
//!
//! Publica hooks, llama `init` a los mods, los engancha a `client:tick`, corre
//! N ticks y los apaga. Es la mitad que faltaba del ABI, del lado del motor.
//!
//! Uso: `cargo run -p drakestar-app --example engine -- <carpeta-de-mods> [ticks]`

use drakestar_app::engine::{log_level_label, Engine};
use drakestar_app::Layout;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match args.first() {
        Some(d) => d.clone(),
        None => {
            eprintln!("uso: engine <carpeta-de-mods> [ticks]");
            std::process::exit(2);
        }
    };
    let ticks: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    let dir = std::path::PathBuf::from(&dir);

    // el cargador de verdad: descubre, verifica SHA-256 y resuelve
    let data = std::env::temp_dir().join("drakestar-engine");
    let l = Layout::new(&data);
    std::fs::create_dir_all(&data).unwrap();
    let svc = drakestar_app::service::Service::new(l.clone(), std::sync::Arc::new(PrintSink));
    let perfil = drakestar_core::profile::Profile {
        schema: 1,
        id: "motor".into(),
        name: "Motor de demostracion".into(),
        mode: drakestar_core::Mode::DrakesJava,
        channel: drakestar_core::Channel::Stable,
        minecraft: "26.2".into(),
        loader: drakestar_core::profile::Loader::Fabric,
        java: drakestar_core::profile::JavaSettings::default(),
        server: None,
        preset: None,
        modpack: None,
        graphics: drakestar_core::profile::GraphicsSettings::default(),
        notes: String::new(),
    };
    let _ = svc.create_profile(&perfil);
    let mods = svc.layout.instance_dir("motor").join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    let mut jars = 0;
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if e.path().extension().map(|x| x == "jar").unwrap_or(false) {
                std::fs::copy(e.path(), mods.join(e.file_name())).unwrap();
                jars += 1;
            }
        }
    }
    let report = match svc.mods_decide("motor") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("ERROR cargador: {e}");
            std::process::exit(1);
        }
    };
    println!("cargador: {jars} jar(s) -> {} descubiertos, {} verificados, {} cargan", report.discovered, report.verified, report.order.len());
    for (id, why) in &report.not_loaded {
        println!("  x [{id}] {why}");
    }
    let cargados = match svc.native_load("motor") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERROR adaptador nativo: {e}");
            std::process::exit(1);
        }
    };
    println!("adaptador nativo: {} mod(s): {:?}", cargados.len(), cargados);
    if cargados.is_empty() {
        println!("(no habia mods nativos: los Java los carga Fabric Loader)");
        return;
    }

    // el motor
    let engine = Engine::new(svc.native.clone());
    for h in ["mod:init", "mod:load", "client:tick", "client:render"] {
        engine.publish(h);
    }
    println!("motor: hooks publicados: {:?}", engine.published());

    let (ok, fail) = engine.start();
    println!("motor: init -> ok {:?}", ok);
    for f in &fail {
        println!("  x {f}");
    }
    for id in &ok {
        if engine.hook("client:tick", id) {
            println!("motor: '{id}' enganchado a client:tick");
        }
    }
    let corridos = engine.run(ticks);
    println!("motor: {corridos} llamadas a mods en {ticks} ticks");
    engine.stop();
    println!("motor: shutdown");

    println!();
    println!("--- lo que dijeron los mods ({}) ---", engine.logs().len());
    for (lvl, msg) in engine.logs() {
        println!("  [{}] {msg}", log_level_label(lvl));
    }
}

struct PrintSink;
impl drakestar_app::process::EventSink for PrintSink {
    fn emit(&self, _ev: drakestar_app::process::ProcEvent) {}
}
