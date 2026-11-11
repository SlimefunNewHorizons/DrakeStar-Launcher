use crate::channel::Channel;
use serde::{Deserialize, Serialize};

/// Los cuatro modos del launcher. Son productos distintos con requisitos distintos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Cliente Rust (Horizon-like). No carga mods Java.
    DrakesRust,
    /// Cliente Java vanilla/Fabric/Forge/NeoForge gestionado.
    DrakesJava,
    /// Servidor local DrakesPurpur de pruebas.
    LocalServer,
    /// Acceso rapido a servidores remotos (p. ej. DrakesCraft).
    Online,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Loader {
    Vanilla,
    Fabric,
    Forge,
    Neoforge,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JavaSettings {
    /// Major requerido (p. ej. 25 para Minecraft 26.x). `None` = deducir de la version.
    #[serde(default)]
    pub major: Option<u32>,
    #[serde(default = "default_min_mem")]
    pub min_mem_mb: u32,
    #[serde(default = "default_max_mem")]
    pub max_mem_mb: u32,
    #[serde(default)]
    pub extra_jvm_args: Vec<String>,
}
fn default_min_mem() -> u32 {
    1024
}
fn default_max_mem() -> u32 {
    4096
}
impl Default for JavaSettings {
    fn default() -> Self {
        Self {
            major: None,
            min_mem_mb: 1024,
            max_mem_mb: 4096,
            extra_jvm_args: vec![],
        }
    }
}

/// Un perfil es la unidad de configuracion. Se serializa a `profiles/<id>/profile.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub mode: Mode,
    pub channel: Channel,
    /// Version de Minecraft objetivo (p. ej. "26.2").
    pub minecraft: String,
    #[serde(default = "default_loader")]
    pub loader: Loader,
    #[serde(default)]
    pub java: JavaSettings,
    /// Solo `online` / clientes: servidor al que conectar.
    #[serde(default)]
    pub server: Option<String>,
    /// Solo `local-server`: preset (p. ej. "slimefun").
    #[serde(default)]
    pub preset: Option<String>,
    /// Solo `drakes-java`: id de modpack gestionado.
    #[serde(default)]
    pub modpack: Option<String>,
    /// Configuración de renderizado visual (OpenGL / Vulkan, resolución, GPU dedicada).
    #[serde(default)]
    pub graphics: GraphicsSettings,
    #[serde(default)]
    pub notes: String,
}

/// Backend de renderizado visual del cliente Minecraft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GraphicsBackend {
    /// OpenGL nativo (estándar de Minecraft, Sodium, Iris Shaders).
    Opengl,
    /// Vulkan nativo (VulkanMod, pipeline de bajo overhead para GPUs modernas).
    Vulkan,
}

impl Default for GraphicsBackend {
    fn default() -> Self {
        Self::Opengl
    }
}

/// Opciones de pantalla y aceleración gráfica para Minecraft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphicsSettings {
    #[serde(default)]
    pub backend: GraphicsBackend,
    #[serde(default = "default_width")]
    pub width: Option<u32>,
    #[serde(default = "default_height")]
    pub height: Option<u32>,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default = "default_true")]
    pub discrete_gpu: bool,
}

fn default_width() -> Option<u32> {
    Some(1280)
}
fn default_height() -> Option<u32> {
    Some(720)
}
fn default_true() -> bool {
    true
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            backend: GraphicsBackend::Opengl,
            width: Some(1280),
            height: Some(720),
            fullscreen: false,
            discrete_gpu: true,
        }
    }
}

fn default_loader() -> Loader {
    Loader::Vanilla
}

pub const PROFILE_SCHEMA: u32 = 1;

#[derive(Debug, PartialEq)]
pub enum ProfileError {
    Schema(u32),
    BadId(String),
    Memory,
    Field(&'static str),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Schema(v) => write!(f, "schema {v} no soportado (esperado {PROFILE_SCHEMA})"),
            Self::BadId(i) => write!(f, "id invalido '{i}' (usa a-z, 0-9, '-' y '_')"),
            Self::Memory => write!(f, "min_mem_mb no puede superar max_mem_mb"),
            Self::Field(s) => write!(f, "{s}"),
        }
    }
}
impl std::error::Error for ProfileError {}

impl Profile {
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.schema != PROFILE_SCHEMA {
            return Err(ProfileError::Schema(self.schema));
        }
        let ok = !self.id.is_empty()
            && self.id.len() <= 48
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        if !ok {
            return Err(ProfileError::BadId(self.id.clone()));
        }
        if self.java.min_mem_mb > self.java.max_mem_mb {
            return Err(ProfileError::Memory);
        }
        match self.mode {
            Mode::DrakesRust => {
                if self.loader != Loader::Vanilla {
                    return Err(ProfileError::Field(
                        "drakes-rust no admite loaders Java (fabric/forge/neoforge)",
                    ));
                }
                if self.modpack.is_some() {
                    return Err(ProfileError::Field("drakes-rust no admite modpacks Java"));
                }
            }
            Mode::DrakesJava => {
                if self.preset.is_some() {
                    return Err(ProfileError::Field("'preset' solo aplica a local-server"));
                }
                if self.modpack.is_some() && self.loader == Loader::Vanilla {
                    return Err(ProfileError::Field(
                        "un modpack requiere loader fabric/forge/neoforge",
                    ));
                }
            }
            Mode::LocalServer => {
                if self.loader != Loader::Vanilla || self.modpack.is_some() {
                    return Err(ProfileError::Field(
                        "local-server (Purpur/Paper) no usa loaders ni modpacks de cliente",
                    ));
                }
            }
            Mode::Online => {
                if self.loader != Loader::Vanilla || self.modpack.is_some() {
                    return Err(ProfileError::Field(
                        "online es un perfil de conexion: sin loader ni modpack",
                    ));
                }
                if self.server.as_deref().unwrap_or("").is_empty() {
                    return Err(ProfileError::Field("online requiere 'server'"));
                }
            }
        }
        Ok(())
    }

    pub fn from_json(s: &str) -> Result<Profile, Box<dyn std::error::Error>> {
        let p: Profile = serde_json::from_str(s)?;
        p.validate()?;
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(mode: Mode) -> Profile {
        Profile {
            schema: 1,
            id: "demo".into(),
            name: "Demo".into(),
            mode,
            channel: Channel::Stable,
            minecraft: "26.2".into(),
            loader: Loader::Vanilla,
            java: JavaSettings::default(),
            server: Some("play.example.net".into()),
            preset: None,
            modpack: None,
            graphics: GraphicsSettings::default(),
            notes: String::new(),
        }
    }

    #[test]
    fn rust_profile_rejects_java_mods() {
        let mut p = base(Mode::DrakesRust);
        assert!(p.validate().is_ok());
        p.loader = Loader::Fabric;
        assert!(p.validate().is_err());
        p.loader = Loader::Vanilla;
        p.modpack = Some("x".into());
        assert!(p.validate().is_err());
    }

    #[test]
    fn modpack_needs_loader() {
        let mut p = base(Mode::DrakesJava);
        p.modpack = Some("pack".into());
        assert!(p.validate().is_err());
        p.loader = Loader::Neoforge;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn online_needs_server_and_ids_are_checked() {
        let mut p = base(Mode::Online);
        p.server = None;
        assert!(p.validate().is_err());
        let mut q = base(Mode::DrakesJava);
        q.id = "Bad Id".into();
        assert!(matches!(q.validate(), Err(ProfileError::BadId(_))));
    }

    #[test]
    fn online_has_no_loader_or_modpack() {
        let mut p = base(Mode::Online);
        p.loader = Loader::Forge;
        assert!(p.validate().is_err());
        p.loader = Loader::Vanilla;
        p.modpack = Some("m".into());
        assert!(p.validate().is_err());
    }

    #[test]
    fn json_roundtrip() {
        let p = base(Mode::Online);
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(Profile::from_json(&s).unwrap(), p);
    }
}
