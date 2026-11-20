//! Presupuesto de heap segun RAM fisica, dejando memoria para el SO y memoria nativa de Java.
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct MemoryProfile {
    pub total_mb: u32,
    pub recommended_mb: u32,
    pub max_mb: u32,
}

impl MemoryProfile {
    pub fn for_total(total_mb: u32) -> Self {
        // Nunca se recomienda mas de la mitad de la RAM. 1024 es el minimo del launcher.
        let max_mb = ((total_mb / 2 / 256) * 256).clamp(1024, 65536);
        Self { total_mb, recommended_mb: max_mb.min(6144), max_mb }
    }
    pub fn detect() -> Self {
        let mut system = sysinfo::System::new();
        system.refresh_memory();
        let total = (system.total_memory() / 1024 / 1024).min(u32::MAX as u64) as u32;
        Self::for_total(if total == 0 { 4096 } else { total })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budgets_leave_room_for_os_and_native_memory() {
        for (ram, recommended, max) in [(4096, 2048, 2048), (8192, 4096, 4096), (16384, 6144, 8192), (32768, 6144, 16384), (0, 1024, 1024)] {
            let p = MemoryProfile::for_total(ram);
            assert_eq!((p.recommended_mb, p.max_mb), (recommended, max));
        }
    }
}
