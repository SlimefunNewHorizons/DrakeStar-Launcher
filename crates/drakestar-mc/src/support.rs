//! Logs del launcher y reportes de soporte: archivo diario con rotacion, hook de panic, paquete .zip con los logs del juego
//! (siempre con datos personales tachados) y envio opcional a un recolector.
//!
//! Solo se envia lo necesario para depurar: version de la app y del juego, SO, mods por nombre y las ultimas lineas de los logs.
//! Nunca tokens, contrasenas ni rutas con el nombre de usuario del PC.

use crate::{Dirs, McError, McResult};
use regex::Regex;
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn log_dir(d: &Dirs) -> PathBuf {
    d.root().join("logs")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|x| x.as_secs()).unwrap_or(0)
}

/// Fecha civil UTC (anio, mes, dia) a partir de segundos desde 1970 (algoritmo de Howard Hinnant).
fn civil(secs: u64) -> (i64, i64, i64) {
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, day)
}

fn today() -> String {
    let (y, m, d) = civil(now_secs());
    format!("{y:04}-{m:02}-{d:02}")
}

/// `2026-11-12 03:04:05Z` (UTC): legible en los registros y ordenable.
fn stamp() -> String {
    let s = now_secs();
    let (y, m, d) = civil(s);
    let t = s % 86_400;
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}Z", t / 3600, t % 3600 / 60, t % 60)
}

/// Escribe una linea en `logs/launcher-<fecha>.log` y borra los de mas de 14 dias.
pub fn app_log(d: &Dirs, level: &str, msg: &str) {
    let dir = log_dir(d);
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join(format!("launcher-{}.log", today()));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{} [{level}] {}", stamp(), redact(msg));
    }
}

pub fn rotate(d: &Dirs, keep_days: u64) {
    let Ok(rd) = std::fs::read_dir(log_dir(d)) else { return };
    let limit = std::time::Duration::from_secs(keep_days * 86_400);
    for e in rd.flatten() {
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|a| a > limit).unwrap_or(false);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Registra los panics del launcher en el log (con el mensaje y la ubicacion).
pub fn install_panic_hook(d: Dirs) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        app_log(&d, "PANIC", &info.to_string());
        prev(info);
    }));
}

fn patterns() -> &'static Vec<(Regex, &'static str)> {
    static P: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    P.get_or_init(|| {
        vec![
            (Regex::new(r"(?i)(accessToken|access_token|authorization|bearer|refresh_token|session_token|password|passwd)(\s*[=: ]\s*)\S+").unwrap(), "$1$2***"),
            (Regex::new(r"eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{5,}").unwrap(), "<jwt>"),
            (Regex::new(r"(?i)[A-Z]:\\Users\\[^\\/\s]+").unwrap(), r"C:\Users\<user>"),
            (Regex::new(r"/home/[^/\s]+").unwrap(), "/home/<user>"),
            (Regex::new(r"/Users/[^/\s]+").unwrap(), "/Users/<user>"),
            (Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap(), "<email>"),
            (Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap(), "<ip>"),
        ]
    })
}

/// Tacha datos personales de una linea de log: tokens, rutas con el usuario del PC, correos e IPs.
pub fn redact(line: &str) -> String {
    let mut out = line.to_string();
    for (re, rep) in patterns() {
        out = re.replace_all(&out, *rep).into_owned();
    }
    out
}

fn tail_lines(p: &Path, max: usize) -> String {
    let text = std::fs::read(p).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max);
    lines[start..].iter().map(|l| redact(l)).collect::<Vec<_>>().join("\n")
}

fn newest(dir: &Path, ext: &str, n: usize) -> Vec<PathBuf> {
    let mut v: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some(ext)).filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().take(n).map(|x| x.1).collect()
}

/// Datos de contexto para el reporte (sin nada personal).
pub struct Context {
    pub app_version: String,
    pub reason: String,
    pub instance: Option<String>,
    pub minecraft: Option<String>,
    pub exit_code: Option<i32>,
    pub game_dir: Option<PathBuf>,
}

/// Crea `support/report-<timestamp>.zip` con los logs recientes (tachados) y el contexto.
pub fn bundle(d: &Dirs, c: &Context) -> McResult<PathBuf> {
    let out = d.root().join("support").join(format!("report-{}.zip", now_secs()));
    std::fs::create_dir_all(out.parent().unwrap())?;
    let mut z = zip::ZipWriter::new(std::fs::File::create(&out)?);
    let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut add = |name: &str, body: String| -> McResult<()> {
        z.start_file(name, opt)?;
        z.write_all(body.as_bytes())?;
        Ok(())
    };
    let mods: Vec<String> = c
        .game_dir
        .as_ref()
        .and_then(|g| std::fs::read_dir(g.join("mods")).ok())
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    add(
        "context.json",
        serde_json::to_string_pretty(&json!({
            "app": "DrakeStar", "version": c.app_version, "reason": c.reason, "instance": c.instance, "minecraft": c.minecraft,
            "exit_code": c.exit_code, "os": crate::os_name(), "arch": crate::arch_name(), "mods": mods,
        }))?,
    )?;
    for p in newest(&log_dir(d), "log", 3) {
        add(&format!("launcher/{}", p.file_name().unwrap().to_string_lossy()), tail_lines(&p, 2000))?;
    }
    if let Some(g) = &c.game_dir {
        let latest = g.join("logs").join("latest.log");
        if latest.exists() {
            add("game/latest.log", tail_lines(&latest, 3000))?;
        }
        let session = g.join("logs").join("launcher-session.log");
        if session.exists() {
            add("game/launcher-session.log", tail_lines(&session, 1500))?;
        }
        for p in newest(&g.join("crash-reports"), "txt", 2) {
            add(&format!("game/crash-reports/{}", p.file_name().unwrap().to_string_lossy()), tail_lines(&p, 600))?;
        }
    }
    z.finish()?;
    Ok(out)
}

/// Envia el reporte al recolector (POST binario). Tope de 6 MB; `token` va en `X-DrakeStar-Token`.
pub fn upload(zip: &Path, endpoint: &str, token: &str, device_id: &str, app_version: &str) -> McResult<()> {
    let data = std::fs::read(zip)?;
    if data.len() > 6 * 1024 * 1024 {
        return Err(McError::Invalid("el reporte pasa de 6 MB".into()));
    }
    if !drakestar_net::url_allowed(endpoint) {
        return Err(McError::Invalid("el recolector debe ser https".into()));
    }
    ureq::post(endpoint)
        .set("Content-Type", "application/zip")
        .set("X-DrakeStar-Token", token)
        .set("X-DrakeStar-Device", device_id)
        .set("X-DrakeStar-Version", app_version)
        .timeout(std::time::Duration::from_secs(30))
        .send_bytes(&data)
        .map_err(|e| McError::Net(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_tokens_paths_emails_and_ips() {
        let r = redact(r"--accessToken eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abcdefghijk user C:\Users\Pablo\AppData ip 181.58.88.6 mail a@b.cl");
        assert!(!r.contains("eyJhbGci"), "{r}");
        assert!(!r.contains("Pablo"), "{r}");
        assert!(!r.contains("181.58.88.6"), "{r}");
        assert!(!r.contains("a@b.cl"), "{r}");
        assert!(r.contains(r"C:\Users\<user>"));
        assert_eq!(redact("/home/jack/.drakestar/x"), "/home/<user>/.drakestar/x");
        assert!(!redact("accessToken: abc123def").contains("abc123def"));
    }

    #[test]
    fn civil_dates_are_right() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(1_794_455_870), (2026, 11, 12));
        assert_eq!(civil(951_782_400), (2000, 2, 29));
    }

    #[test]
    fn bundle_contains_redacted_game_log() {
        let d = Dirs::new(std::env::temp_dir().join(format!("dsmc-sup-{}", std::process::id())));
        let g = d.instance("t");
        std::fs::create_dir_all(g.join("logs")).unwrap();
        std::fs::write(g.join("logs/latest.log"), "Setting user: X\naccessToken SECRETVALUE123\nruta /home/jack/mundo\n").unwrap();
        app_log(&d, "INFO", "arranque");
        let z = bundle(&d, &Context { app_version: "t".into(), reason: "crash".into(), instance: Some("t".into()), minecraft: Some("1.21.1".into()), exit_code: Some(1), game_dir: Some(g) }).unwrap();
        let mut a = zip::ZipArchive::new(std::fs::File::open(&z).unwrap()).unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut a.by_name("game/latest.log").unwrap(), &mut s).unwrap();
        assert!(!s.contains("SECRETVALUE123") && !s.contains("/home/jack"));
        assert!(a.by_name("context.json").is_ok());
        let _ = std::fs::remove_dir_all(d.root());
    }
}
