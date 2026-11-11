use crate::{layout::write_atomic, AppResult, Layout};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Ruta al ejecutable de Java. `None` = `java` del PATH.
    #[serde(default)]
    pub java_path: Option<String>,
    /// Nombre de usuario para el modo offline de los clientes.
    #[serde(default)]
    pub username: Option<String>,
    /// Perfil activo en la UI.
    #[serde(default)]
    pub active_profile: Option<String>,
    /// Sesión de Microsoft autenticada (UUID con guiones).
    #[serde(default)]
    pub session_uuid: Option<String>,
    /// Token de acceso para validar la sesión en servidores online-mode.
    #[serde(default)]
    pub session_token: Option<String>,
    /// Nombre de la cuenta de Minecraft autenticada.
    #[serde(default)]
    pub session_name: Option<String>,
}

impl Settings {
    pub fn load(l: &Layout) -> Settings {
        std::fs::read_to_string(l.settings_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, l: &Layout) -> AppResult<()> {
        write_atomic(&l.settings_path(), serde_json::to_string_pretty(self)?.as_bytes())?;
        Ok(())
    }
    pub fn java_bin(&self) -> String {
        match self.java_path.as_deref() {
            Some(p) if !p.trim().is_empty() => p.trim().to_string(),
            _ => "java".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_defaults() {
        let l = Layout::new(crate::testutil::tmpdir("settings"));
        assert_eq!(Settings::load(&l), Settings::default());
        assert_eq!(Settings::default().java_bin(), "java");
        let s = Settings {
            java_path: Some("/opt/j/bin/java".into()),
            username: Some("nix".into()),
            active_profile: Some("a".into()),
            session_uuid: Some("11111111-2222-3333-4444-555555555555".into()),
            session_token: Some("mc_token_123".into()),
            session_name: Some("JackStar".into()),
        };
        s.save(&l).unwrap();
        assert_eq!(Settings::load(&l), s);
        assert_eq!(s.java_bin(), "/opt/j/bin/java");
    }
}
