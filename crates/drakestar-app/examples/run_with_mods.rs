//! Lanza el cliente Fabric real con mods encima.
//!
//! Uso: `cargo run -p drakestar-app --example run_with_mods -- <carpeta-de-mods> [mc]`
//!
//! Instala el cliente (Mojang + Fabric, verificado por hash), copia los jars de
//! la carpeta a `instance/mods/` —donde los busca Fabric Loader, que tambien
//! resuelve sus jar-in-jar— y lo ejecuta. Los logs salen por pantalla.
//!
//! shortcut: mientras el cargador Rust no exista, **quien carga los mods es
//! Fabric Loader**: DrakeStar decide que se descarga y que se lanza, y valida
//! cada mod con el intermediario (`drakestar mod-check`).

use drakestar_app::{
    fabric::{self, ClientStatus},
    Layout,
};
use drakestar_core::{profile::*, Channel, Profile};
use drakestar_core::Mode;
use std::path::PathBuf;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mods_dir = match args.first() {
        Some(d) => PathBuf::from(d),
        None => {
            eprintln!("uso: run_with_mods <carpeta-de-mods> [version-mc]");
            std::process::exit(2);
        }
    };
    let mc = args.get(1).cloned().unwrap_or_else(|| "26.2".into());
    let data_dir = std::env::var("DRAKESTAR_RUN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("drakestar-run"));
    let l = Layout::new(&data_dir);
    std::fs::create_dir_all(&data_dir).unwrap();
    println!("datos: {}", data_dir.display());

    // perfil de prueba
    let p = Profile {
        schema: 1,
        id: "run".into(),
        name: "Run con mods".into(),
        mode: Mode::DrakesJava,
        channel: Channel::Stable,
        minecraft: mc.clone(),
        loader: Loader::Fabric,
        java: JavaSettings { major: Some(25), min_mem_mb: 2048, max_mem_mb: 6144, extra_jvm_args: vec![] },
        server: None,
        preset: None,
        modpack: None,
        graphics: drakestar_core::profile::GraphicsSettings::default(),
        notes: String::new(),
    };

    // 1) instalar el cliente (usa cache: no re-descarga lo que ya esta)
    print!("instalando Minecraft {mc} + Fabric... ");
    let st: ClientStatus = match fabric::install(&l, &p) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ERROR {e}");
            std::process::exit(1);
        }
    };
    println!(
        "listo: {} artefactos, {} bytes (loader {}, MC {})",
        st.artifacts, st.bytes, st.loader_version.clone().unwrap_or_default(), st.minecraft
    );
    if !st.ready {
        eprintln!("el cliente no quedo completo: {}", st.reason.unwrap_or_default());
        std::process::exit(1);
    }

    // 2) copiar los mods (solo los jars de primer nivel: los jar-in-jar los
    //    resuelve Fabric Loader)
    let dest = drakestar_app::instances::data_dir(&l, &p).join("mods");
    std::fs::create_dir_all(&dest).unwrap();
    let mut copied = 0;
    for e in std::fs::read_dir(&mods_dir).unwrap().flatten() {
        if e.path().extension().map(|x| x == "jar").unwrap_or(false) {
            std::fs::copy(e.path(), dest.join(e.file_name())).unwrap();
            copied += 1;
        }
    }
    println!("{copied} mods copiados a {}", dest.display());

    // 3) registrar el perfil (el store trabaja sobre disco) y ejecutar
    let sink = std::sync::Arc::new(PrintSink);
    let svc = drakestar_app::service::Service::new(l.clone(), sink);
    svc.create_profile(&p).unwrap_or_else(|e| {
        // si ya existe de una corrida anterior, se actualiza
        if !matches!(e, drakestar_app::AppError::Exists(_)) {
            eprintln!("ERROR al crear el perfil: {e}");
            std::process::exit(1);
        }
    });
    let started = svc.client_start("run");
    match started {
        Ok(st) => println!("arrancado: pid {:?}", st.pid),
        Err(e) => {
            eprintln!("ERROR al arrancar: {e}");
            std::process::exit(1);
        }
    }
    println!("--- logs (Ctrl-C para salir; el proceso sigue) ---");
    let mut last = 0usize;
    loop {
        std::thread::sleep(Duration::from_millis(700));
        let hist = svc.procs.history("run");
        for h in &hist[last..] {
            println!("{}", h.line);
        }
        last = hist.len();
        if !svc.procs.is_active("run") {
            println!("--- el proceso termino ---");
            break;
        }
    }
}

struct PrintSink;
impl drakestar_app::process::EventSink for PrintSink {
    fn emit(&self, _ev: drakestar_app::process::ProcEvent) {}
}
