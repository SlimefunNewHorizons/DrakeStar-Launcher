//! Prueba real de extremo a extremo: instala (vanilla o Fabric), baja Java, lanza con una cuenta offline y comprueba que el juego llega
//! a la pantalla de titulo. Uso:
//!   cargo run -p drakestar-mc --example play_test -- <mc> [vanilla|fabric] [segundos] [dir]
use drakestar_mc::{fabric, install, java, launch, Dirs, ProgressSink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Console;
impl ProgressSink for Console {
    fn stage(&self, id: &str, text: &str) {
        println!("[{id}] {text}");
    }
    fn fraction(&self, f: f32) {
        if (f * 100.0) as u32 % 25 == 0 {
            println!("   {:.0}%", f * 100.0);
        }
    }
    fn log(&self, line: &str) {
        println!("   {line}");
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mc = a.get(1).cloned().unwrap_or_else(|| "1.21.1".into());
    let loader = a.get(2).cloned().unwrap_or_else(|| "fabric".into());
    let secs: u64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(60);
    let root = a.get(4).cloned().unwrap_or_else(|| format!("{}/.drakestar-test", std::env::var("HOME").unwrap_or_else(|_| ".".into())));
    let d = Dirs::new(&root);
    let sink = Console;

    let id = if loader == "fabric" { fabric::ensure_profile(&d, &mc, None).expect("fabric") } else { mc.clone() };
    println!("version a lanzar: {id}");
    let inst = install::install(&d, &id, &sink).expect("instalacion");
    let java_bin = java::ensure(&d, inst.java_major, &sink).expect("java");
    println!("java {}: {}", inst.java_major, java_bin.display());

    let game_dir = d.instance(&format!("test-{id}"));
    let opts = launch::Options { game_dir: game_dir.clone(), java_bin, xmx_mb: 2048, xms_mb: 512, width: Some(854), height: Some(480), ..Default::default() };
    let plan = launch::plan(&d, &inst, &launch::Session::offline("DrakeTest"), &opts).expect("plan");
    println!("lanzando ({} args)", plan.args.len());

    let title = Arc::new(AtomicBool::new(false));
    let user = Arc::new(AtomicBool::new(false));
    let (t2, u2) = (title.clone(), user.clone());
    let exited = Arc::new(AtomicBool::new(false));
    let e2 = exited.clone();
    let log = game_dir.join("logs").join("launcher-session.log");
    let h = launch::spawn(
        &plan,
        vec![],
        Some(log.clone()),
        move |_s, line| {
            if line.contains("Setting user:") {
                u2.store(true, Ordering::SeqCst);
            }
            // el juego imprime esto al terminar de cargar recursos y mostrar el menu
            if line.contains("Backend library") || line.contains("Created: 1024x") || line.contains("Reloading ResourceManager") || line.contains("Narrator library") {
                t2.store(true, Ordering::SeqCst);
            }
        },
        move |code| {
            println!("el juego termino con codigo {code:?}");
            e2.store(true, Ordering::SeqCst);
        },
    )
    .expect("spawn");

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) && !exited.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(500));
        if title.load(Ordering::SeqCst) && start.elapsed() > Duration::from_secs(20) {
            break;
        }
    }
    println!("usuario configurado: {} | llego al render/menu: {}", user.load(Ordering::SeqCst), title.load(Ordering::SeqCst));
    let ok = user.load(Ordering::SeqCst);
    h.kill();
    std::thread::sleep(Duration::from_millis(800));
    println!("log: {}", log.display());
    std::process::exit(if ok { 0 } else { 1 });
}
