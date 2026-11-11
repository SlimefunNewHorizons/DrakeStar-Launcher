//! Gestor de procesos con salida en streaming, entrada por stdin, historial en
//! memoria y archivo de log en disco. No conoce Tauri: emite eventos a un
//! `EventSink` (la capa Tauri los reenvia a la UI).

use crate::{now_ms, AppError, AppResult};
use drakestar_core::launch::LaunchPlan;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const HISTORY_CAP: usize = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Stdout,
    Stderr,
    /// Mensajes del propio launcher.
    System,
    /// Comandos enviados por el usuario.
    Stdin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcState {
    Stopped,
    Running,
    Stopping,
    /// Termino con codigo != 0 sin haberlo pedido el usuario.
    Crashed,
}

#[derive(Debug, Clone, Serialize)]
pub struct LineRec {
    pub ts: u64,
    pub stream: Stream,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProcEvent {
    Line { profile: String, ts: u64, stream: Stream, line: String },
    State { profile: String, state: ProcState, code: Option<i32> },
}

pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, ev: ProcEvent);
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcStatus {
    pub state: ProcState,
    pub pid: Option<u32>,
    pub started_at: Option<u64>,
    pub exit_code: Option<i32>,
    pub log_file: Option<String>,
}

struct Entry {
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    status: ProcStatus,
    stop_requested: bool,
    log: SharedLog,
}

struct Inner {
    sink: Arc<dyn EventSink>,
    procs: Mutex<HashMap<String, Entry>>,
    history: Mutex<HashMap<String, VecDeque<LineRec>>>,
}

#[derive(Clone)]
pub struct ProcessManager {
    inner: Arc<Inner>,
}

type SharedLog = Arc<Mutex<Option<File>>>;

impl Inner {
    fn push_line(&self, id: &str, log: &SharedLog, stream: Stream, line: String) {
        let ts = now_ms();
        if let Ok(mut g) = log.lock() {
            if let Some(f) = g.as_mut() {
                let tag = match stream {
                    Stream::Stdout => "out",
                    Stream::Stderr => "err",
                    Stream::System => "sys",
                    Stream::Stdin => "in ",
                };
                let _ = writeln!(f, "{ts} [{tag}] {line}");
            }
        }
        {
            let mut h = self.history.lock().unwrap();
            let q = h.entry(id.to_string()).or_default();
            if q.len() >= HISTORY_CAP {
                q.pop_front();
            }
            q.push_back(LineRec { ts, stream, line: line.clone() });
        }
        self.sink.emit(ProcEvent::Line { profile: id.to_string(), ts, stream, line });
    }

    fn set_state(&self, id: &str, state: ProcState, code: Option<i32>) {
        if let Some(e) = self.procs.lock().unwrap().get_mut(id) {
            e.status.state = state;
            if code.is_some() {
                e.status.exit_code = code;
            }
        }
        self.sink.emit(ProcEvent::State { profile: id.to_string(), state, code });
    }
}

fn pump<R: std::io::Read + Send + 'static>(
    inner: Arc<Inner>,
    id: String,
    log: SharedLog,
    r: R,
    stream: Stream,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut br = BufReader::new(r);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match br.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let s = String::from_utf8_lossy(&buf);
                    inner.push_line(&id, &log, stream, s.trim_end_matches(['\n', '\r']).to_string());
                }
            }
        }
    })
}

impl ProcessManager {
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        Self {
            inner: Arc::new(Inner {
                sink,
                procs: Mutex::new(HashMap::new()),
                history: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn status(&self, id: &str) -> ProcStatus {
        self.inner
            .procs
            .lock()
            .unwrap()
            .get(id)
            .map(|e| e.status.clone())
            .unwrap_or(ProcStatus {
                state: ProcState::Stopped,
                pid: None,
                started_at: None,
                exit_code: None,
                log_file: None,
            })
    }

    pub fn is_active(&self, id: &str) -> bool {
        matches!(self.status(id).state, ProcState::Running | ProcState::Stopping)
    }

    pub fn history(&self, id: &str) -> Vec<LineRec> {
        self.inner.history.lock().unwrap().get(id).map(|q| q.iter().cloned().collect()).unwrap_or_default()
    }

    pub fn clear_history(&self, id: &str) {
        self.inner.history.lock().unwrap().remove(id);
    }

    /// Arranca el proceso del plan. El log se escribe en `log_dir/launcher-<ts>.log`.
    pub fn start(&self, id: &str, plan: &LaunchPlan, log_dir: &Path) -> AppResult<ProcStatus> {
        if self.is_active(id) {
            return Err(AppError::Busy(format!("'{id}' ya esta en ejecucion")));
        }
        std::fs::create_dir_all(&plan.cwd)?;
        std::fs::create_dir_all(log_dir)?;
        let ts = now_ms();
        let log_path: PathBuf = log_dir.join(format!("launcher-{ts}.log"));
        let log: SharedLog = Arc::new(Mutex::new(Some(File::create(&log_path)?)));

        let mut cmd = Command::new(&plan.program);
        cmd.args(&plan.args)
            .envs(&plan.envs)
            .current_dir(&plan.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| {
            AppError::Io(format!("no se pudo ejecutar '{}': {e} (cwd {})", plan.program, plan.cwd))
        })?;
        let pid = child.id();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let stdin = child.stdin.take();
        let child = Arc::new(Mutex::new(child));
        let stdin = Arc::new(Mutex::new(stdin));
        let status = ProcStatus {
            state: ProcState::Running,
            pid: Some(pid),
            started_at: Some(ts),
            exit_code: None,
            log_file: Some(log_path.to_string_lossy().to_string()),
        };
        self.inner.procs.lock().unwrap().insert(
            id.to_string(),
            Entry { child: child.clone(), stdin, status: status.clone(), stop_requested: false, log: log.clone() },
        );
        self.inner.push_line(id, &log, Stream::System, format!("--- inicio (pid {pid}) ---"));
        self.inner.push_line(
            id,
            &log,
            Stream::System,
            format!("$ {} {}  [cwd {}]", plan.program, plan.args.join(" "), plan.cwd),
        );
        self.inner.sink.emit(ProcEvent::State { profile: id.to_string(), state: ProcState::Running, code: None });

        let h_out = pump(self.inner.clone(), id.to_string(), log.clone(), stdout, Stream::Stdout);
        let h_err = pump(self.inner.clone(), id.to_string(), log.clone(), stderr, Stream::Stderr);
        let inner = self.inner.clone();
        let id2 = id.to_string();
        thread::spawn(move || {
            let code = loop {
                {
                    let mut c = child.lock().unwrap();
                    match c.try_wait() {
                        Ok(Some(st)) => break st.code(),
                        Ok(None) => {}
                        Err(_) => break None,
                    }
                }
                thread::sleep(Duration::from_millis(40));
            };
            // Un nieto que herede las tuberias podria mantenerlas abiertas: esperar con limite.
            let t0 = std::time::Instant::now();
            while !(h_out.is_finished() && h_err.is_finished()) && t0.elapsed() < Duration::from_secs(2) {
                thread::sleep(Duration::from_millis(20));
            }
            let requested = inner.procs.lock().unwrap().get(&id2).map(|e| e.stop_requested).unwrap_or(false);
            let state = if requested || code == Some(0) { ProcState::Stopped } else { ProcState::Crashed };
            inner.push_line(
                &id2,
                &log,
                Stream::System,
                format!("--- proceso terminado (codigo {}) ---", code.map(|c| c.to_string()).unwrap_or_else(|| "senal".into())),
            );
            inner.set_state(&id2, state, code);
            if let Ok(mut g) = log.lock() {
                *g = None;
            }
        });
        Ok(status)
    }

    /// Escribe una linea en el stdin del proceso (consola del servidor).
    pub fn send_line(&self, id: &str, line: &str) -> AppResult<()> {
        if line.contains(['\n', '\r']) {
            return Err(AppError::Invalid("el comando no puede contener saltos de linea".into()));
        }
        let (stdin, log) = {
            let g = self.inner.procs.lock().unwrap();
            let e = g.get(id).ok_or_else(|| AppError::Busy(format!("'{id}' no esta en ejecucion")))?;
            if !matches!(e.status.state, ProcState::Running | ProcState::Stopping) {
                return Err(AppError::Busy(format!("'{id}' no esta en ejecucion")));
            }
            (e.stdin.clone(), e.log.clone())
        };
        let mut g = stdin.lock().unwrap();
        let w = g.as_mut().ok_or_else(|| AppError::Busy("stdin cerrado".into()))?;
        writeln!(w, "{line}")?;
        w.flush()?;
        drop(g);
        self.inner.push_line(id, &log, Stream::Stdin, line.to_string());
        Ok(())
    }

    /// Parada ordenada: envia `graceful` (p. ej. "stop") por stdin y, si el proceso
    /// sigue vivo tras `timeout`, lo mata. Retorna de inmediato (no bloquea).
    pub fn stop(&self, id: &str, graceful: Option<&str>, timeout: Duration) -> AppResult<()> {
        {
            let mut g = self.inner.procs.lock().unwrap();
            let e = g.get_mut(id).ok_or_else(|| AppError::Busy(format!("'{id}' no esta en ejecucion")))?;
            if e.status.state != ProcState::Running {
                return Err(AppError::Busy(format!("'{id}' no esta en ejecucion")));
            }
            e.stop_requested = true;
            e.status.state = ProcState::Stopping;
        }
        self.inner.sink.emit(ProcEvent::State { profile: id.to_string(), state: ProcState::Stopping, code: None });
        if let Some(cmd) = graceful {
            let _ = self.send_line(id, cmd);
        }
        let this = self.clone();
        let id = id.to_string();
        thread::spawn(move || {
            let deadline = std::time::Instant::now() + timeout;
            while std::time::Instant::now() < deadline {
                if !this.is_active(&id) {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            let _ = this.kill(&id);
        });
        Ok(())
    }

    /// Mata el proceso de inmediato.
    pub fn kill(&self, id: &str) -> AppResult<()> {
        let child = {
            let mut g = self.inner.procs.lock().unwrap();
            let e = g.get_mut(id).ok_or_else(|| AppError::Busy(format!("'{id}' no esta en ejecucion")))?;
            e.stop_requested = true;
            e.child.clone()
        };
        let mut c = child.lock().unwrap();
        let _ = c.kill();
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Instant;

    #[derive(Default)]
    struct Collect(Mutex<Vec<ProcEvent>>);
    impl EventSink for Collect {
        fn emit(&self, ev: ProcEvent) {
            self.0.lock().unwrap().push(ev);
        }
    }

    fn plan(script: &str, cwd: &Path) -> LaunchPlan {
        LaunchPlan::new("sh", vec!["-c".into(), script.into()], cwd.to_string_lossy())
    }
    fn wait_until(f: impl Fn() -> bool) {
        let t = Instant::now();
        while !f() {
            assert!(t.elapsed() < Duration::from_secs(10), "timeout esperando condicion");
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn streams_stdout_stderr_and_writes_log() {
        let d = crate::testutil::tmpdir("proc1");
        let sink = Arc::new(Collect::default());
        let pm = ProcessManager::new(sink.clone());
        let st = pm.start("p", &plan("echo hola; echo problema >&2; exit 0", &d.join("srv")), &d.join("logs")).unwrap();
        wait_until(|| pm.status("p").state == ProcState::Stopped);
        let h = pm.history("p");
        assert!(h.iter().any(|l| l.stream == Stream::Stdout && l.line == "hola"));
        assert!(h.iter().any(|l| l.stream == Stream::Stderr && l.line == "problema"));
        let log = std::fs::read_to_string(st.log_file.unwrap()).unwrap();
        assert!(log.contains("[out] hola") && log.contains("[err] problema"));
        let evs = sink.0.lock().unwrap();
        assert!(evs.iter().any(|e| matches!(e, ProcEvent::Line { line, .. } if line == "hola")));
        assert!(matches!(evs.last().unwrap(), ProcEvent::State { state: ProcState::Stopped, code: Some(0), .. }));
    }

    #[test]
    fn stdin_commands_and_graceful_stop() {
        let d = crate::testutil::tmpdir("proc2");
        let pm = ProcessManager::new(Arc::new(Collect::default()));
        pm.start("s", &plan("while read l; do echo \"eco:$l\"; [ \"$l\" = stop ] && exit 0; done", &d), &d.join("l")).unwrap();
        assert!(pm.is_active("s"));
        assert!(pm.start("s", &plan("true", &d), &d.join("l")).is_err()); // ya corriendo
        pm.send_line("s", "list").unwrap();
        wait_until(|| pm.history("s").iter().any(|l| l.line == "eco:list"));
        assert!(pm.send_line("s", "a\nb").is_err());
        pm.stop("s", Some("stop"), Duration::from_secs(5)).unwrap();
        wait_until(|| pm.status("s").state == ProcState::Stopped);
        assert!(pm.history("s").iter().any(|l| l.line == "eco:stop"));
        assert_eq!(pm.status("s").exit_code, Some(0));
        assert!(pm.send_line("s", "x").is_err());
    }

    #[test]
    fn stop_escalates_to_kill_and_crash_is_detected() {
        let d = crate::testutil::tmpdir("proc3");
        let pm = ProcessManager::new(Arc::new(Collect::default()));
        pm.start("k", &plan("sleep 30", &d), &d.join("l")).unwrap();
        pm.stop("k", None, Duration::from_millis(200)).unwrap();
        wait_until(|| pm.status("k").state == ProcState::Stopped);
        pm.start("c", &plan("echo boom >&2; exit 3", &d), &d.join("l")).unwrap();
        wait_until(|| pm.status("c").state == ProcState::Crashed);
        assert_eq!(pm.status("c").exit_code, Some(3));
    }

    #[test]
    fn missing_program_is_a_clear_error() {
        let d = crate::testutil::tmpdir("proc4");
        let pm = ProcessManager::new(Arc::new(Collect::default()));
        let p = LaunchPlan::new("no-existe-drakestar", vec![], d.to_string_lossy());
        let e = pm.start("m", &p, &d.join("l")).unwrap_err();
        assert!(e.to_string().contains("no se pudo ejecutar"));
        assert_eq!(pm.status("m").state, ProcState::Stopped);
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use std::time::Instant;

    #[derive(Default)]
    struct Collect(Mutex<Vec<ProcEvent>>);
    impl EventSink for Collect {
        fn emit(&self, ev: ProcEvent) {
            self.0.lock().unwrap().push(ev);
        }
    }

    fn plan(script: &str, cwd: &Path) -> LaunchPlan {
        LaunchPlan::new("cmd", vec!["/C".into(), script.into()], cwd.to_string_lossy())
    }

    fn wait_until(f: impl Fn() -> bool) {
        let t = Instant::now();
        while !f() {
            assert!(t.elapsed() < Duration::from_secs(10), "timeout esperando condicion");
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn windows_streams_stdout_and_detects_exit() {
        let d = crate::testutil::tmpdir("wproc1");
        let sink = Arc::new(Collect::default());
        let pm = ProcessManager::new(sink);
        let st = pm.start("wp", &plan("echo hola_win & exit /b 0", &d.join("srv")), &d.join("logs")).unwrap();
        wait_until(|| pm.status("wp").state == ProcState::Stopped);
        let h = pm.history("wp");
        assert!(h.iter().any(|l| l.stream == Stream::Stdout && l.line.trim() == "hola_win"));
        assert!(Path::new(&st.log_file.unwrap()).is_file());
    }
}
