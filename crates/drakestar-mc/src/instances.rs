//! Instancias de juego: que version, con que loader, con que mods y a donde entrar. Incluye las plantillas de DrakesCraft.

use crate::{write_atomic, Dirs, McError, McResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric,
}

/// Que hace el boton JUGAR de la instancia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Target {
    /// Abre el juego en el menu principal.
    Menu,
    /// Entra directo a un servidor.
    Server { address: String },
    /// Abre un mundo guardado (un jugador / LAN).
    World { name: String },
    /// Servidor Purpur local gestionado (con Slimefun si hay build): el launcher lo arranca y entra solo.
    /// `lan` lo abre a la red local; si no, solo escucha en este equipo.
    Local { port: u16, lan: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc: String,
    pub loader: Loader,
    pub target: Target,
    /// Instala Sodium, Lithium, FerriteCore... (solo Fabric).
    #[serde(default)]
    pub performance: bool,
    /// Instala DrakeStar Fused (Bukkit/Purpur/Slimefun dentro del servidor integrado). Solo Fabric.
    #[serde(default)]
    pub fused: bool,
    #[serde(default = "default_ram")]
    pub xmx_mb: u32,
    #[serde(default)]
    pub extra_jvm: Vec<String>,
    #[serde(default)]
    pub note: String,
}

fn default_ram() -> u32 {
    3072
}

impl Instance {
    pub fn validate(&self) -> McResult<()> {
        if self.id.is_empty() || self.id.len() > 48 || !self.id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_') {
            return Err(McError::Invalid("el id de instancia solo admite minusculas, numeros, '-' y '_'".into()));
        }
        if !crate::manifest::supported(&self.mc) {
            return Err(McError::Invalid(format!("Minecraft {} esta fuera del rango soportado ({} a {})", self.mc, crate::manifest::MIN_SUPPORTED, crate::manifest::MAX_SUPPORTED)));
        }
        if (self.performance || self.fused) && self.loader != Loader::Fabric {
            return Err(McError::Invalid("los mods de rendimiento y Fused necesitan Fabric".into()));
        }
        if matches!(self.target, Target::Local { .. }) && (self.fused || self.performance && self.loader != Loader::Fabric) {
            return Err(McError::Invalid("un servidor local no usa Fused (los plugins van en el servidor)".into()));
        }
        if self.xmx_mb < 1024 || self.xmx_mb > 65536 {
            return Err(McError::Invalid("la memoria debe estar entre 1024 y 65536 MB".into()));
        }
        Ok(())
    }
}

/// Plantillas que trae el launcher.
pub fn presets() -> Vec<Instance> {
    vec![
        Instance {
            id: "drakescraft".into(),
            name: "DrakesCraft (online)".into(),
            mc: "1.21.11".into(),
            loader: Loader::Fabric,
            target: Target::Server { address: "mc.drakescraft.cl:25565".into() },
            performance: true,
            fused: false,
            xmx_mb: 4096,
            extra_jvm: vec![],
            note: "Entra directo al servidor oficial (Java y Bedrock comparten la IP).".into(),
        },
        Instance {
            id: "slimefun-solo".into(),
            name: "Slimefun en solitario / LAN".into(),
            mc: "1.21.11".into(),
            loader: Loader::Fabric,
            target: Target::Local { port: 25570, lan: false },
            performance: true,
            fused: false,
            xmx_mb: 4096,
            extra_jvm: vec![],
            note: "Servidor propio en tu equipo con Slimefun: el launcher lo arranca y entras solo. Al jugar aceptas el EULA de Mojang (aka.ms/MinecraftEULA).".into(),
        },
        Instance {
            id: "vanilla-latest".into(),
            name: "Minecraft vanilla".into(),
            mc: "1.21.11".into(),
            loader: Loader::Vanilla,
            target: Target::Menu,
            performance: false,
            fused: false,
            xmx_mb: 3072,
            extra_jvm: vec![],
            note: "Sin mods.".into(),
        },
    ]
}

pub struct Instances {
    path: PathBuf,
}

impl Instances {
    pub fn new(d: &Dirs) -> Self {
        Self { path: d.root().join("instances.json") }
    }

    /// Lista guardada; la primera vez crea las plantillas.
    pub fn list(&self) -> Vec<Instance> {
        match std::fs::read_to_string(&self.path).ok().and_then(|t| serde_json::from_str::<Vec<Instance>>(&t).ok()) {
            Some(mut v) => {
                // la plantilla antigua de Slimefun solitario (Fused alfa en el menu) pasa al servidor local
                if let Some(x) = v.iter_mut().find(|x| x.id == "slimefun-solo" && x.fused && x.target == Target::Menu) {
                    if let Some(p) = presets().into_iter().find(|p| p.id == "slimefun-solo") {
                        *x = p;
                        let _ = self.save(&v);
                    }
                }
                v
            }
            None => {
                let p = presets();
                let _ = self.save(&p);
                p
            }
        }
    }

    fn save(&self, v: &[Instance]) -> McResult<()> {
        write_atomic(&self.path, serde_json::to_string_pretty(v)?.as_bytes())?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> McResult<Instance> {
        self.list().into_iter().find(|i| i.id == id).ok_or_else(|| McError::NotFound(format!("instancia {id}")))
    }

    pub fn upsert(&self, i: Instance) -> McResult<Instance> {
        i.validate()?;
        let mut v = self.list();
        match v.iter_mut().find(|x| x.id == i.id) {
            Some(x) => *x = i.clone(),
            None => v.push(i.clone()),
        }
        self.save(&v)?;
        Ok(i)
    }

    pub fn remove(&self, id: &str) -> McResult<()> {
        let mut v = self.list();
        v.retain(|i| i.id != id);
        self.save(&v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_valid_and_in_range() {
        for p in presets() {
            p.validate().unwrap_or_else(|e| panic!("{}: {e}", p.id));
        }
    }

    #[test]
    fn rejects_out_of_range_and_vanilla_with_mods() {
        let mut p = presets().remove(0);
        p.mc = "1.19.4".into();
        assert!(p.validate().is_err());
        p.mc = "26.3".into();
        assert!(p.validate().is_err());
        let mut v = presets().remove(2);
        v.performance = true;
        assert!(v.validate().is_err());
    }

    #[test]
    fn store_roundtrip() {
        let d = Dirs::new(std::env::temp_dir().join(format!("dsmc-inst-{}", std::process::id())));
        let s = Instances::new(&d);
        assert_eq!(s.list().len(), 3);
        let mut x = s.get("drakescraft").unwrap();
        x.xmx_mb = 6144;
        s.upsert(x).unwrap();
        assert_eq!(s.get("drakescraft").unwrap().xmx_mb, 6144);
        s.remove("vanilla-latest").unwrap();
        assert_eq!(s.list().len(), 2);
        let _ = std::fs::remove_dir_all(d.root());
    }
}
