//! Prueba real del boton JUGAR en una instancia `local` (preset «Slimefun en solitario / LAN»): mismo orden que la app
//! (`play::prepare` -> servidor local -> `server::wait_ready` -> cliente con `join_server`) y exito cuando el servidor
//! anuncia que el jugador entro. Con DISPLAY (p. ej. Xvfb) guarda una captura con `import`. Uso:
//!   cargo run -p drakestar-mc --example local_play_e2e -- [dir] [lan 0|1] [xmx_mb] [performance 0|1] [segundos] [captura.png]
use drakestar_mc::accounts::Accounts;
use drakestar_mc::instances::{presets, Target};
use drakestar_mc::{launch, play, server, Dirs, ProgressSink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Console;
impl ProgressSink for Console {
    fn stage(&self, id: &str, text: &str) {
        println!("[{id}] {text}");
    }
    fn fraction(&self, _: f32) {}
    fn log(&self, line: &str) {
        println!("   {line}");
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let root = a.get(1).cloned().unwrap_or_else(|| format!("{}/.drakestar-e2e", std::env::var("HOME").unwrap_or_else(|_| ".".into())));
    let lan = a.get(2).is_some_and(|s| s == "1");
    let xmx: u32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(4096);
    let perf = a.get(4).map_or(true, |s| s != "0");
    let secs: u64 = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(900);
    let shot = a.get(6).cloned();
    let d = Dirs::new(&root);

    let mut inst = presets().into_iter().find(|i| matches!(i.target, Target::Local { .. })).expect("preset local");
    inst.target = Target::Local { port: 25570, lan };
    inst.xmx_mb = xmx;
    inst.performance = perf;
    let acc = Accounts::new(&d).add_offline("DrakeTest").expect("cuenta offline");
    println!("instancia {} ({} {:?}), lan={lan}", inst.id, inst.mc, inst.loader);

    let p = play::prepare(&d, &inst, &acc.id, "", &Console).expect("prepare");
    for w in &p.warnings {
        println!("AVISO: {w}");
    }
    let ls = p.server.as_ref().expect("Target::Local sin servidor");
    let props = std::fs::read_to_string(server::dir(&d, &inst.id).join("server.properties")).unwrap_or_default();
    println!("server.properties: {}", props.lines().filter(|l| l.starts_with("server-ip") || l.starts_with("server-port") || l.starts_with("online-mode")).collect::<Vec<_>>().join(" | "));

    let dead = Arc::new(AtomicBool::new(false));
    let joined = Arc::new(AtomicBool::new(false));
    let (dead2, j2) = (dead.clone(), joined.clone());
    let sh = launch::spawn(
        &ls.plan,
        vec![],
        Some(ls.log_file.clone()),
        move |_, line| {
            if line.contains("DrakeTest joined the game") || line.contains("DrakeTest[/127.0.0.1") {
                j2.store(true, Ordering::SeqCst);
            }
            if line.contains("joined the game") || line.contains("Done (") || line.contains("ERROR") {
                println!("[servidor] {line}");
            }
        },
        move |c| {
            println!("servidor termino {c:?}");
            dead2.store(true, Ordering::SeqCst);
        },
    )
    .expect("spawn servidor");
    let t0 = Instant::now();
    server::wait_ready(ls.port, Duration::from_secs(600), || !dead.load(Ordering::SeqCst), || false).expect("servidor listo");
    println!("servidor escuchando en {} tras {:?}", ls.port, t0.elapsed());

    let exited = Arc::new(AtomicBool::new(false));
    let e2 = exited.clone();
    let ch = launch::spawn(
        &p.plan,
        vec![p.session.access_token.clone()],
        Some(p.game_dir.join("logs").join("launcher-session.log")),
        |_, line| {
            if line.contains("Connecting to") || line.contains("Exception") {
                println!("[cliente] {line}");
            }
        },
        move |c| {
            println!("cliente termino {c:?}");
            e2.store(true, Ordering::SeqCst);
        },
    )
    .expect("spawn cliente");

    let t1 = Instant::now();
    while t1.elapsed() < Duration::from_secs(secs) && !joined.load(Ordering::SeqCst) && !exited.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(500));
    }
    let ok = joined.load(Ordering::SeqCst);
    println!("jugador dentro del servidor local: {ok} (cliente {:?})", t1.elapsed());
    if ok {
        // deja que carguen los chunks antes de capturar
        std::thread::sleep(Duration::from_secs(40));
        if let Some(f) = shot {
            let st = std::process::Command::new("import").args(["-window", "root", &f]).status();
            println!("captura {f}: {st:?}");
        }
    }
    ch.kill();
    sh.kill();
    std::thread::sleep(Duration::from_secs(2));
    std::process::exit(if ok { 0 } else { 1 });
}
