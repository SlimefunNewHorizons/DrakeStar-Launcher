//! Cuentas sin premium: nombre valido y UUID offline (el mismo que calcula un servidor `online-mode=false`).

use md5::{Digest, Md5};

/// 3-16 caracteres `[A-Za-z0-9_]` (regla de nombres de Minecraft Java).
pub fn valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// UUID v3 de `"OfflinePlayer:<name>"`, con guiones.
pub fn offline_uuid(name: &str) -> String {
    let mut h = Md5::new();
    h.update(format!("OfflinePlayer:{name}").as_bytes());
    let mut b: [u8; 16] = h.finalize().into();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    let x: String = b.iter().map(|v| format!("{v:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &x[0..8], &x[8..12], &x[12..16], &x[16..20], &x[20..32])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(valid_name("JackStar6677"));
        assert!(!valid_name("ab"));
        assert!(!valid_name("nombre con espacio"));
        assert!(!valid_name("nombremuuuuuuylargo1"));
    }

    #[test]
    fn offline_uuid_matches_vanilla_servers() {
        // valor conocido de un servidor offline-mode para "Notch" (UUID v3)
        assert_eq!(offline_uuid("Notch"), "b50ad385-829d-3141-a216-7e7d7539ba7f");
        let u = offline_uuid("JackStar6677");
        assert_eq!(u.len(), 36);
        assert_eq!(&u[14..15], "3");
    }
}
