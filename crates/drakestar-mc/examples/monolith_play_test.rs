//! Prueba real End-to-End de "Slimefun en Solitario / LAN" (Servidor DrakesPurpur -> Cliente QuickPlay):
//!   cargo run -p drakestar-mc --example monolith_play_test -- [segundos_cliente] [dir]
use drakestar_mc::accounts::Accounts;
use drakestar_mc::instances;
use drakestar_mc::{launch, play, server, Dirs, ProgressSink};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Console;
impl ProgressSink for Console {
    fn stage(&self, id: &str, text: &str) {
        println!("[stage:{id}] {text}");
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
    let args: Vec<String> = std::env::args().collect();
    let secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(60);
    let root = args.get(2).cloned().unwrap_or_else(|| {
        format!("{}/.drakestar-test", std::env::var("HOME").unwrap_or_else(|_| ".".into()))
    });
    println!("DISPLAY={:?} root={root}", std::env::var("DISPLAY").ok());

    let d = Dirs::new(&root);
    let sink = Console;

    let accs = Accounts::new(&d);
    let acc = accs
        .list()
        .accounts
        .into_iter()
        .find(|a| a.name == "DrakeSoloTest")
        .unwrap_or_else(|| accs.add_offline("DrakeSoloTest").expect("offline account"));

    let inst = instances::presets()
        .into_iter()
        .find(|p| p.id == "slimefun-solo")
        .expect("preset slimefun-solo");
    println!(
        "Preset: {} (mc={}, client_xmx={}M)",
        inst.id, inst.mc, inst.xmx_mb
    );

    let prepared = play::prepare(&d, &inst, &acc.id, "", &sink).expect("play::prepare");
    let ls = prepared.server.expect("LocalServer esperado para slimefun-solo");
    let expected_addr = format!("127.0.0.1:{}", ls.port);

    // Verificar --quickPlayMultiplayer 127.0.0.1:<port>
    let qp_pos = prepared
        .plan
        .args
        .iter()
        .position(|a| a == "--quickPlayMultiplayer")
        .expect("falta --quickPlayMultiplayer en el plan del cliente");
    assert_eq!(
        prepared.plan.args.get(qp_pos + 1).map(String::as_str),
        Some(expected_addr.as_str()),
        "quickPlayMultiplayer no apunta a {expected_addr}"
    );
    println!("OK QuickPlay verificado: --quickPlayMultiplayer {expected_addr}");

    // Verificar eula=true y online-mode=false
    let sdir = server::dir(&d, &inst.id);
    let eula = std::fs::read_to_string(sdir.join("eula.txt")).expect("eula.txt");
    let props = std::fs::read_to_string(sdir.join("server.properties")).expect("server.properties");
    assert!(eula.contains("eula=true"), "eula.txt no tiene eula=true");
    assert!(props.contains("online-mode=false"), "server.properties no tiene online-mode=false");
    println!("OK Zero-Config verificado: eula=true, online-mode=false, server_plan_args={:?}", ls.plan.args);

    // 1) Arrancar servidor local DrakesPurpur
    let sv_dead = Arc::new(AtomicBool::new(false));
    let sv_dead2 = sv_dead.clone();
    let sv_joined = Arc::new(AtomicBool::new(false));
    let sv_joined2 = sv_joined.clone();
    let sv_handle = launch::spawn(
        &ls.plan,
        vec![],
        Some(ls.log_file.clone()),
        move |_stream, line| {
            if line.contains("Done (") || line.contains("Timings Reset") || line.contains("Slimefun") || line.contains("joined the game") || line.contains("logged in with entity") {
                println!("[server] {line}");
            }
            if line.contains("joined the game") || line.contains("logged in with entity") {
                sv_joined2.store(true, Ordering::SeqCst);
            }
        },
        move |code| {
            println!("[server] termino con codigo {code:?}");
            sv_dead2.store(true, Ordering::SeqCst);
        },
    )
    .expect("spawn server");
    let sv_pid = sv_handle.pid();
    println!("Servidor DrakesPurpur lanzado (pid={sv_pid:?}), esperando puerto {}...", ls.port);

    if let Err(e) = server::wait_ready(
        ls.port,
        Duration::from_secs(300),
        || !sv_dead.load(Ordering::SeqCst),
        || false,
    ) {
        sv_handle.kill();
        panic!("wait_ready fallo: {e}");
    }
    println!("OK Servidor escuchando en 127.0.0.1:{}", ls.port);

    // 2) Arrancar cliente Minecraft en DISPLAY=:0 con --quickPlayMultiplayer
    let cli_dead = Arc::new(AtomicBool::new(false));
    let cli_dead2 = cli_dead.clone();
    let cli_connecting = Arc::new(AtomicBool::new(false));
    let cli_connecting2 = cli_connecting.clone();
    let cli_window = Arc::new(AtomicBool::new(false));
    let cli_window2 = cli_window.clone();
    let cli_log = prepared.game_dir.join("logs").join("launcher-session.log");
    let cli_handle = launch::spawn(
        &prepared.plan,
        vec![prepared.session.access_token.clone()],
        Some(cli_log.clone()),
        move |_stream, line| {
            if line.contains("Backend library") || line.contains("LWJGL") || line.contains("Created:") || line.contains("OpenAL") {
                cli_window2.store(true, Ordering::SeqCst);
                println!("[client] {line}");
            }
            if line.contains("Connecting to") {
                cli_connecting2.store(true, Ordering::SeqCst);
                println!("[client] {line}");
            }
        },
        move |code| {
            println!("[client] termino con codigo {code:?}");
            cli_dead2.store(true, Ordering::SeqCst);
        },
    )
    .expect("spawn client");
    println!("Cliente Minecraft lanzado (pid={:?})", cli_handle.pid());

    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(secs) && !cli_dead.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(500));
        if sv_joined.load(Ordering::SeqCst) {
            println!("OK Cliente conectado al servidor local DrakesPurpur!");
            std::thread::sleep(Duration::from_secs(3));
            break;
        }
    }

    let got_window = cli_window.load(Ordering::SeqCst);
    let got_connect = cli_connecting.load(Ordering::SeqCst);
    let got_join = sv_joined.load(Ordering::SeqCst);
    println!(
        "Estado visual/red: window={got_window} connecting={got_connect} joined_server={got_join}"
    );

    // 3) Cerrar cliente y servidor limpiamente (como src-tauri/src/lib.rs)
    cli_handle.kill();
    sv_handle.kill();

    let wait_stop = Instant::now();
    while wait_stop.elapsed() < Duration::from_secs(10) && !sv_dead.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::sleep(Duration::from_millis(500));

    let port_free = TcpListener::bind(("127.0.0.1", ls.port)).is_ok();
    let sv_stopped = sv_dead.load(Ordering::SeqCst);
    println!("Limpieza: server_stopped={sv_stopped} port_free={port_free}");

    assert!(sv_stopped, "el servidor local no termino tras sv.kill()");
    assert!(port_free, "el puerto {} no se libero tras sv.kill()", ls.port);
    assert!(got_window, "el cliente no abrio ventana");
    assert!(got_join, "el cliente no entro al servidor local (sin 'joined the game' en {} s)", secs);
    println!("EXITO TOTAL: End-to-End Slimefun Local verificado!");
}
