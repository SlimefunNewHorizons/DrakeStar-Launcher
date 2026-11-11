//! Prueba de punta a punta del modo DrakesRust, sin Internet.
//!
//! Sirve el binario del cliente por HTTP local (127.0.0.1, que la politica de
//! `drakestar-net` permite), importa un manifiesto con su SHA-256 real, y hace
//! todo el camino que hace la app: estado -> instalar -> plan -> arrancar ->
//! consola -> parar.
//!
//! Uso: `cargo run -p drakestar-app --example run_rust_client`
//!
//! Ademas levanta un mini-servidor de login (el mismo guion de los tests del
//! cliente) para que el camino completo —launcher -> binario -> servidor—
//! quede probado, no solo el lanzamiento.

use drakestar_app::Layout;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

fn sha256_file(p: &std::path::Path) -> String {
    // se usa la misma funcion que el cargador
    drakestar_net::sha256_file(p).unwrap()
}

/// Sirve un archivo por HTTP en 127.0.0.1 (una vez) y devuelve el puerto.
fn serve_once(path: PathBuf) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut s, _)) = l.accept() {
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf);
            let body = std::fs::read(&path).unwrap();
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&body);
        }
    });
    port
}

/// Mini-servidor de login (protocolo 26.x): lee el saludo y el login start,
/// contesta login success, espera el acknowledged y manda dos paquetes. Es el
/// mismo guion que usan los tests del cliente, pero como proceso vivo.
fn spawn_login_server() -> u16 {
    use drakestar_rust_client::{client, proto};
    use std::io::Write;
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let Ok((s, _)) = l.accept() else { return };
        let mut r = s.try_clone().unwrap();
        let mut w = s;
        let (hs, _) = proto::read_packet(&mut r).unwrap(); // handshake
        assert_eq!(hs, 0x00);
        let (start, body) = proto::read_packet(&mut r).unwrap(); // login start
        assert_eq!(start, client::LOGIN_START);
        let name = proto::read_string(&mut std::io::Cursor::new(body)).unwrap();
        println!("[servidor] login start de '{name}'");
        let mut ok = Vec::new();
        proto::write_string(&mut ok, "login-ok-payload");
        proto::send_packet(&mut w, client::PKT_LOGIN_SUCCESS, &ok).unwrap();
        println!("[servidor] login success enviado");
        let (ack, ack_body) = proto::read_packet(&mut r).unwrap(); // acknowledged
        assert_eq!((ack, ack_body.len()), (client::LOGIN_ACKNOWLEDGED, 0));
        println!("[servidor] login acknowledged recibido");
        proto::send_packet(&mut w, 0x57, b"a").unwrap();
        proto::send_packet(&mut w, 0x58, b"bc").unwrap();
        let _ = w.flush();
        println!("[servidor] dos paquetes enviados, cerrando");
    });
    port
}

fn main() {
    let bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/drakestar-rust-client");
    if !bin.exists() {
        eprintln!("falta compilar el cliente: cargo build -p drakestar-rust-client");
        std::process::exit(1);
    }
    let size = std::fs::metadata(&bin).unwrap().len();
    let sha = sha256_file(&bin);
    println!("cliente: {} bytes, sha256 {}", size, &sha[..16]);

    // 1) servirlo por HTTP local
    let port = serve_once(bin.clone());
    let url = format!("http://127.0.0.1:{port}/drakestar-rust-client");
    println!("sirviendo en {url}");

    // 2) manifiesto con la release (como si estuviera publicada)
    let data = std::env::temp_dir().join("drakestar-rust-run");
    let l = Layout::new(&data);
    std::fs::create_dir_all(&data).unwrap();
    let manifest = format!(
        r#"{{"schema":1,"releases":[{{
            "version":"0.1.0","channel":"stable","minecraft":["26.2"],
            "notes":"cliente Rust local (prueba)",
            "artifacts":[{{"component":"drakes-rust-client","platform":"linux-x86_64",
                           "url":"{url}","sha256":"{sha}","size":{size}}}]
        }}]}}"#
    );
    drakestar_app::release::import_manifest(&l, &manifest).unwrap();
    println!("manifiesto importado");

    // 3b) mini-servidor de login: el cliente se conecta de verdad
    let login_port = spawn_login_server();
    println!("mini-servidor de login en 127.0.0.1:{login_port}");

    // 3) perfil
    let p = drakestar_core::profile::Profile {
        schema: 1,
        id: "rust".into(),
        name: "Prueba Rust".into(),
        mode: drakestar_core::Mode::DrakesRust,
        channel: drakestar_core::Channel::Stable,
        minecraft: "26.2".into(),
        loader: drakestar_core::profile::Loader::Vanilla,
        java: drakestar_core::profile::JavaSettings::default(),
        server: Some(format!("127.0.0.1:{login_port}")),
        preset: None,
        modpack: None,
        graphics: drakestar_core::profile::GraphicsSettings::default(),
        notes: String::new(),
    };
    let sink = Arc::new(PrintSink);
    let svc = drakestar_app::service::Service::new(l.clone(), sink);
    let _ = svc.create_profile(&p);

    // 4) estado -> instalar -> plan
    let st = svc.rust_client_status("rust").unwrap();
    println!(
        "estado: instalado={} listo={} motivo={:?}",
        st.installed, st.ready, st.reason
    );
    let st = svc.rust_client_install("rust").unwrap();
    println!(
        "instalado: version={:?} binario={}",
        st.version,
        st.binary.rsplit('/').next().unwrap_or("")
    );
    assert!(st.installed, "no quedo instalado");
    // el binario quedo ejecutable (en unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&st.binary).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "el binario no quedo ejecutable: {mode:o}");
        println!("permisos: {:o}", mode & 0o777);
    }
    let plan = svc.rust_client_plan("rust").unwrap();
    println!("plan: {} {}", plan.program.rsplit('/').next().unwrap_or(""), plan.args.join(" "));

    // 5) arrancar y ver la consola
    let started = svc.rust_client_start("rust");
    match started {
        Ok(st) => println!("arrancado: pid {:?}", st.pid),
        Err(e) => {
            println!("ERROR al arrancar: {e}");
            std::process::exit(1);
        }
    }
    println!("--- consola ---");
    let mut last = 0usize;
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_secs(25) {
        let hist = svc.procs.history("rust");
        for h in &hist[last..] {
            println!("{}", h.line);
        }
        last = hist.len();
        if !svc.procs.is_active("rust") {
            println!("--- el proceso termino ---");
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }

    // 6) manda un comando por stdin y para
    if svc.procs.is_active("rust") {
        let _ = svc.rust_client_command("rust", "quit");
        std::thread::sleep(std::time::Duration::from_millis(600));
    }
    let hist = svc.procs.history("rust");
    for h in &hist[last..] {
        println!("{}", h.line);
    }
    let _ = svc.rust_client_stop("rust");
    let _ = svc.procs.kill("rust");
    println!("listo: estado final {:?}", svc.procs.status("rust").state);
}

struct PrintSink;
impl drakestar_app::process::EventSink for PrintSink {
    fn emit(&self, _ev: drakestar_app::process::ProcEvent) {}
}
