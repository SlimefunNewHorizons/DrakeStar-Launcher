//! Descarga paralela con reintentos y verificacion SHA-1; indice y objetos de assets de Minecraft.

use crate::progress::ProgressSink;
use crate::version::AssetIndexRef;
use crate::{Dirs, McError, McResult};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

/// Un archivo por bajar.
#[derive(Debug, Clone)]
pub struct Job {
    pub url: String,
    pub dest: PathBuf,
    /// SHA-1 (40 hex) esperado; vacio = sin verificar (librerias de Fabric sin hash).
    pub sha1: String,
    pub size: u64,
}

fn already_ok(j: &Job) -> bool {
    let Ok(meta) = std::fs::metadata(&j.dest) else { return false };
    if j.size != 0 && meta.len() != j.size {
        return false;
    }
    if j.sha1.is_empty() {
        return true;
    }
    drakestar_net::sha1_file(&j.dest).map(|h| h.eq_ignore_ascii_case(&j.sha1)).unwrap_or(false)
}

fn fetch_one(j: &Job) -> McResult<u64> {
    let mut last = String::new();
    for attempt in 0..3 {
        let r = if j.sha1.is_empty() {
            drakestar_net::download_direct(&j.url, &j.dest)
        } else {
            drakestar_net::download_verified(&j.url, &j.dest, &j.sha1, Some(j.size), |_| {})
        };
        match r {
            Ok(n) => return Ok(n),
            Err(e) => {
                last = e.to_string();
                std::thread::sleep(std::time::Duration::from_millis(400 * (attempt + 1)));
            }
        }
    }
    Err(McError::Net(format!("{} ({last})", j.url)))
}

/// Baja todos los trabajos pendientes en paralelo; devuelve cuantos bytes bajo.
pub fn download_all(jobs: Vec<Job>, workers: usize, sink: &dyn ProgressSink) -> McResult<u64> {
    let pending: Vec<Job> = jobs.into_iter().filter(|j| !already_ok(j)).collect();
    if pending.is_empty() {
        sink.fraction(1.0);
        return Ok(0);
    }
    let total: u64 = pending.iter().map(|j| j.size.max(1)).sum();
    let done = AtomicU64::new(0);
    let next = AtomicUsize::new(0);
    let bytes = AtomicU64::new(0);
    let err: Mutex<Option<McError>> = Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..workers.max(1).min(pending.len()) {
            s.spawn(|| loop {
                if sink.cancelled() || err.lock().unwrap().is_some() {
                    return;
                }
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(j) = pending.get(i) else { return };
                match fetch_one(j) {
                    Ok(n) => {
                        bytes.fetch_add(n, Ordering::SeqCst);
                        let d = done.fetch_add(j.size.max(1), Ordering::SeqCst) + j.size.max(1);
                        sink.fraction((d as f64 / total as f64).min(1.0) as f32);
                    }
                    Err(e) => {
                        *err.lock().unwrap() = Some(e);
                        return;
                    }
                }
            });
        }
    });
    if sink.cancelled() {
        return Err(McError::Cancelled);
    }
    if let Some(e) = err.into_inner().unwrap() {
        return Err(e);
    }
    Ok(bytes.load(Ordering::SeqCst))
}

#[derive(Debug, Deserialize)]
struct IndexFile {
    #[serde(default)]
    objects: BTreeMap<String, Obj>,
}

#[derive(Debug, Deserialize)]
struct Obj {
    hash: String,
    size: u64,
}

const RESOURCES: &str = "https://resources.download.minecraft.net";

/// Descarga el indice de assets y todos sus objetos (sonidos, idiomas, texturas de fondo...).
pub fn ensure_assets(d: &Dirs, idx: &AssetIndexRef, sink: &dyn ProgressSink) -> McResult<()> {
    let index_path = d.assets().join("indexes").join(format!("{}.json", idx.id));
    download_all(
        vec![Job { url: idx.url.clone(), dest: index_path.clone(), sha1: idx.sha1.clone(), size: idx.size }],
        1,
        &crate::NullSink,
    )?;
    let text = std::fs::read_to_string(&index_path)?;
    let index: IndexFile = serde_json::from_str(&text)?;
    let jobs: Vec<Job> = index
        .objects
        .values()
        .map(|o| {
            let p = &o.hash[..2];
            Job { url: format!("{RESOURCES}/{p}/{}", o.hash), dest: d.assets().join("objects").join(p).join(&o.hash), sha1: o.hash.clone(), size: o.size }
        })
        .collect();
    sink.log(&format!("assets: {} objetos en el indice {}", jobs.len(), idx.id));
    download_all(jobs, 12, sink)?;
    Ok(())
}
