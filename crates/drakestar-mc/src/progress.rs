use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Receptor de progreso (la capa Tauri lo convierte en eventos para la UI).
pub trait ProgressSink: Send + Sync {
    /// Etapa actual ("java", "minecraft", "assets", "mods", "launch"...), con texto para el usuario.
    fn stage(&self, id: &str, text: &str);
    /// 0.0..=1.0 de la etapa actual.
    fn fraction(&self, f: f32);
    /// Linea informativa (va a los logs).
    fn log(&self, line: &str);
    /// El usuario pidio cancelar.
    fn cancelled(&self) -> bool {
        false
    }
}

pub struct NullSink;
impl ProgressSink for NullSink {
    fn stage(&self, _: &str, _: &str) {}
    fn fraction(&self, _: f32) {}
    fn log(&self, _: &str) {}
}

/// Bandera de cancelacion compartida.
#[derive(Clone, Default)]
pub struct CancelFlag(pub Arc<AtomicBool>);
impl CancelFlag {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
