//! Servidor local gestionado (Purpur) para jugar en solitario/LAN con plugins: el launcher lo baja, lo arranca y entra solo.

use crate::launch::Plan;
use crate::progress::ProgressSink;
use crate::{Dirs, McError, McResult};
use md5::{Digest, Md5};
use std::io::Read;
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const PURPUR: &str = "https://api.purpurmc.org/v2/purpur";

/// Carpeta del servidor de una instancia (mundo, plugins, config).
pub fn dir(d: &Dirs, inst_id: &str) -> PathBuf {
    d.instance(inst_id).join("server")
}

/// Java mayor que pide el servidor para una version de Minecraft.
pub fn java_major(mc: &str) -> u32 {
    let mut it = mc.split('.');
    let _ = it.next();
    let minor: u32 = it.next().and_then(|m| m.parse().ok()).unwrap_or(21);
    // 1.20.5+ y 1.21.x -> 21; 26.x en adelante -> 25
    if mc.starts_with("1.") {
        if minor >= 21 || (minor == 20 && mc.split('.').nth(2).and_then(|p| p.parse::<u32>().ok()).unwrap_or(0) >= 5) { 21 } else { 17 }
    } else {
        25
    }
}

#[derive(serde::Deserialize)]
struct Latest {
    build: String,
    md5: String,
}

/// Baja (una vez) el jar de Purpur de la version y verifica su MD5. Devuelve la ruta.
pub fn ensure_purpur(d: &Dirs, inst_id: &str, mc: &str, sink: &dyn ProgressSink) -> McResult<PathBuf> {
    let sdir = dir(d, inst_id);
    std::fs::create_dir_all(&sdir)?;
    let latest: Latest = ureq::get(&format!("{PURPUR}/{mc}/latest")).timeout(Duration::from_secs(30)).call().map_err(|e| McError::Net(format!("Purpur {mc}: {e}")))?.into_json().map_err(|e| McError::Json(e.to_string()))?;
    let jar = sdir.join(format!("purpur-{mc}-{}.jar", latest.build));
    if jar.exists() && md5_hex(&std::fs::read(&jar)?).eq_ignore_ascii_case(&latest.md5) {
        return Ok(jar);
    }
    sink.stage("server", &format!("Descargando el servidor Purpur {mc} (solo la primera vez)"));
    let mut buf = vec![];
    ureq::get(&format!("{PURPUR}/{mc}/{}/download", latest.build)).timeout(Duration::from_secs(300)).call().map_err(|e| McError::Net(format!("Purpur {mc}: {e}")))?.into_reader().read_to_end(&mut buf)?;
    if !md5_hex(&buf).eq_ignore_ascii_case(&latest.md5) {
        return Err(McError::Invalid("el MD5 del jar de Purpur no coincide".into()));
    }
    // jars viejos fuera
    if let Ok(rd) = std::fs::read_dir(&sdir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("purpur-") && n.ends_with(".jar") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    crate::write_atomic(&jar, &buf)?;
    Ok(jar)
}

fn md5_hex(b: &[u8]) -> String {
    Md5::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// Escribe `eula.txt` y fija en `server.properties` solo lo necesario (respeta lo que el usuario cambie).
/// `lan` abre el puerto a la red local; si no, solo escucha en 127.0.0.1.
pub fn configure(d: &Dirs, inst_id: &str, port: u16, lan: bool) -> McResult<()> {
    let sdir = dir(d, inst_id);
    std::fs::create_dir_all(&sdir)?;
    std::fs::write(sdir.join("eula.txt"), "eula=true\n")?;
    let path = sdir.join("server.properties");
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let ip = if lan { "" } else { "127.0.0.1" };
    let want = [("server-port", port.to_string()), ("server-ip", ip.to_string()), ("online-mode", "false".into()), ("motd", "DrakeStar local".into()), ("enable-query", "false".into()), ("enable-rcon", "false".into()), ("view-distance", "8".into())];
    let mut lines: Vec<String> = old.lines().map(|s| s.to_string()).collect();
    for (k, v) in want {
        let prefix = format!("{k}=");
        // puerto/ip/online-mode siempre se imponen; el resto solo si no estaban
        let force = matches!(k, "server-port" | "server-ip" | "online-mode" | "enable-rcon");
        match lines.iter().position(|l| l.starts_with(&prefix)) {
            Some(i) if force => lines[i] = format!("{k}={v}"),
            Some(_) => {}
            None => lines.push(format!("{k}={v}")),
        }
    }
    std::fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

/// Plan para arrancar el servidor sin ventana.
pub fn plan(d: &Dirs, inst_id: &str, jar: &std::path::Path, java_bin: PathBuf, xmx_mb: u32) -> Plan {
    Plan {
        program: java_bin,
        args: vec![format!("-Xmx{xmx_mb}M"), format!("-Xms{}M", (xmx_mb / 4).max(512)), "-jar".into(), jar.to_string_lossy().into_owned(), "--nogui".into()],
        cwd: dir(d, inst_id),
    }
}

/// Espera a que el servidor acepte conexiones. `alive` corta la espera si el proceso murio.
pub fn wait_ready(port: u16, timeout: Duration, alive: impl Fn() -> bool, cancelled: impl Fn() -> bool) -> McResult<()> {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let t0 = Instant::now();
    while t0.elapsed() < timeout {
        if cancelled() {
            return Err(McError::Cancelled);
        }
        if !alive() {
            return Err(McError::Invalid("el servidor local se cerro antes de abrir el puerto (mira Registros)".into()));
        }
        if TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(McError::Invalid("el servidor local tardo demasiado en arrancar".into()))
}

/// Primer puerto libre desde `start`.
pub fn free_port(start: u16) -> u16 {
    (start..start.saturating_add(50)).find(|p| std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok()).unwrap_or(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_por_version() {
        assert_eq!(java_major("1.21.1"), 21);
        assert_eq!(java_major("1.20.4"), 17);
        assert_eq!(java_major("1.20.6"), 21);
        assert_eq!(java_major("26.2"), 25);
    }

    #[test]
    fn configure_impone_lo_necesario_y_respeta_el_resto() {
        let t = std::env::temp_dir().join(format!("dsmc-srv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let d = Dirs::new(&t);
        std::fs::create_dir_all(dir(&d, "x")).unwrap();
        std::fs::write(dir(&d, "x").join("server.properties"), "view-distance=16\nonline-mode=true\nserver-port=1\n").unwrap();
        configure(&d, "x", 25599, false).unwrap();
        let p = std::fs::read_to_string(dir(&d, "x").join("server.properties")).unwrap();
        assert!(p.contains("view-distance=16") && p.contains("online-mode=false") && p.contains("server-port=25599") && p.contains("server-ip=127.0.0.1"));
        assert_eq!(std::fs::read_to_string(dir(&d, "x").join("eula.txt")).unwrap(), "eula=true\n");
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn wait_ready_detecta_puerto_y_proceso_muerto() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        assert!(wait_ready(port, Duration::from_secs(3), || true, || false).is_ok());
        drop(l);
        assert!(wait_ready(port, Duration::from_secs(3), || false, || false).is_err());
    }
}
