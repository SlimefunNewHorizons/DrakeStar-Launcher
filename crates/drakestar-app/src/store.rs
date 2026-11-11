//! Almacen de perfiles en disco (`profiles/<id>/profile.json`). Toda escritura
//! pasa por `Profile::validate` de drakestar-core: la regla de compatibilidad
//! vive en un solo sitio.

use crate::{layout::write_atomic, AppError, AppResult, Layout};
use drakestar_core::Profile;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ProfileStore {
    layout: Layout,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvalidProfile {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileList {
    pub profiles: Vec<Profile>,
    /// Carpetas con `profile.json` ilegible o invalido (no se ocultan).
    pub invalid: Vec<InvalidProfile>,
}

impl ProfileStore {
    pub fn new(layout: Layout) -> Self {
        Self { layout }
    }

    pub fn list(&self) -> ProfileList {
        let mut profiles = vec![];
        let mut invalid = vec![];
        if let Ok(rd) = std::fs::read_dir(self.layout.profiles_dir()) {
            for e in rd.flatten() {
                if !e.path().is_dir() {
                    continue;
                }
                let id = e.file_name().to_string_lossy().to_string();
                let json = e.path().join("profile.json");
                if !json.exists() {
                    continue; // carpeta de datos huerfana, no es un perfil
                }
                match std::fs::read_to_string(&json)
                    .map_err(|e| e.to_string())
                    .and_then(|s| Profile::from_json(&s).map_err(|e| e.to_string()))
                {
                    Ok(p) if p.id == id => profiles.push(p),
                    Ok(p) => invalid.push(InvalidProfile {
                        id,
                        error: format!("el id del perfil ('{}') no coincide con la carpeta", p.id),
                    }),
                    Err(error) => invalid.push(InvalidProfile { id, error }),
                }
            }
        }
        profiles.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
        invalid.sort_by(|a, b| a.id.cmp(&b.id));
        ProfileList { profiles, invalid }
    }

    pub fn get(&self, id: &str) -> AppResult<Profile> {
        check_id(id)?;
        let s = std::fs::read_to_string(self.layout.profile_json(id))
            .map_err(|_| AppError::NotFound(format!("perfil '{id}'")))?;
        Profile::from_json(&s).map_err(|e| AppError::Invalid(e.to_string()))
    }

    pub fn exists(&self, id: &str) -> bool {
        check_id(id).is_ok() && self.layout.profile_json(id).exists()
    }

    fn write(&self, p: &Profile) -> AppResult<()> {
        p.validate()?;
        let json = serde_json::to_string_pretty(p)?;
        write_atomic(&self.layout.profile_json(&p.id), json.as_bytes())?;
        Ok(())
    }

    pub fn create(&self, p: &Profile) -> AppResult<()> {
        p.validate()?;
        if self.exists(&p.id) {
            return Err(AppError::Exists(format!("perfil '{}'", p.id)));
        }
        self.write(p)
    }

    pub fn update(&self, p: &Profile) -> AppResult<()> {
        p.validate()?;
        if !self.exists(&p.id) {
            return Err(AppError::NotFound(format!("perfil '{}'", p.id)));
        }
        self.write(p)
    }

    /// `remove_data = true` borra tambien servidor/instancia/logs; si no, solo `profile.json`.
    pub fn delete(&self, id: &str, remove_data: bool) -> AppResult<()> {
        check_id(id)?;
        if !self.exists(id) {
            return Err(AppError::NotFound(format!("perfil '{id}'")));
        }
        if remove_data {
            std::fs::remove_dir_all(self.layout.profile_dir(id))?;
        } else {
            std::fs::remove_file(self.layout.profile_json(id))?;
        }
        Ok(())
    }

    /// Copia solo la configuracion (no los datos del servidor/instancia).
    pub fn duplicate(&self, id: &str, new_id: &str, new_name: &str) -> AppResult<Profile> {
        let mut p = self.get(id)?;
        p.id = new_id.to_string();
        p.name = new_name.to_string();
        self.create(&p)?;
        Ok(p)
    }

    /// Importa un perfil desde JSON. Si el id existe y `overwrite` es falso -> `Exists`.
    pub fn import_json(&self, json: &str, overwrite: bool) -> AppResult<Profile> {
        let p = Profile::from_json(json).map_err(|e| AppError::Invalid(e.to_string()))?;
        if self.exists(&p.id) {
            if !overwrite {
                return Err(AppError::Exists(format!("perfil '{}'", p.id)));
            }
            self.update(&p)?;
        } else {
            self.create(&p)?;
        }
        Ok(p)
    }

    pub fn export_json(&self, id: &str) -> AppResult<String> {
        Ok(serde_json::to_string_pretty(&self.get(id)?)?)
    }

    /// Escribe `exports/<id>.json` y devuelve la ruta.
    pub fn export_to_file(&self, id: &str) -> AppResult<std::path::PathBuf> {
        let json = self.export_json(id)?;
        let path = self.layout.exports_dir().join(format!("{id}.json"));
        write_atomic(&path, json.as_bytes())?;
        Ok(path)
    }
}

fn check_id(id: &str) -> AppResult<()> {
    let ok = !id.is_empty()
        && id.len() <= 48
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(AppError::Invalid(format!("id invalido '{id}' (usa a-z, 0-9, '-' y '_')")))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use drakestar_core::{profile::*, Channel, Mode};

    pub fn sample(id: &str, mode: Mode) -> Profile {
        Profile {
            schema: 1,
            id: id.into(),
            name: format!("Perfil {id}"),
            mode,
            channel: Channel::Stable,
            minecraft: "26.2".into(),
            loader: Loader::Vanilla,
            java: JavaSettings::default(),
            server: if mode == Mode::Online { Some("play.example.net".into()) } else { None },
            preset: None,
            modpack: None,
            graphics: GraphicsSettings::default(),
            notes: String::new(),
        }
    }
    fn store(tag: &str) -> ProfileStore {
        ProfileStore::new(Layout::new(crate::testutil::tmpdir(tag)))
    }

    #[test]
    fn crud_and_persistence_across_instances() {
        let l = Layout::new(crate::testutil::tmpdir("crud"));
        let s = ProfileStore::new(l.clone());
        s.create(&sample("b", Mode::DrakesJava)).unwrap();
        s.create(&sample("a", Mode::LocalServer)).unwrap();
        assert!(matches!(s.create(&sample("a", Mode::LocalServer)), Err(AppError::Exists(_))));
        // otro store sobre el mismo directorio ve los datos (persistencia real en disco)
        let s2 = ProfileStore::new(l);
        let list = s2.list();
        assert_eq!(list.profiles.len(), 2);
        assert_eq!(list.profiles[0].id, "a");
        let mut p = s2.get("a").unwrap();
        p.name = "Renombrado".into();
        s2.update(&p).unwrap();
        assert_eq!(s.get("a").unwrap().name, "Renombrado");
        s.delete("a", true).unwrap();
        assert!(matches!(s.get("a"), Err(AppError::NotFound(_))));
        assert_eq!(s.list().profiles.len(), 1);
    }

    #[test]
    fn invalid_profiles_are_rejected_by_core_rules() {
        let s = store("invalid");
        let mut p = sample("r", Mode::DrakesRust);
        p.loader = drakestar_core::profile::Loader::Fabric;
        assert!(matches!(s.create(&p), Err(AppError::Invalid(_))));
        let mut p = sample("r", Mode::DrakesRust);
        p.modpack = Some("x".into());
        assert!(s.create(&p).is_err());
        assert!(s.create(&sample("Bad Id", Mode::DrakesJava)).is_err());
        assert!(s.get("../etc").is_err());
        assert!(s.delete("../x", true).is_err());
        assert!(s.list().profiles.is_empty());
    }

    #[test]
    fn import_export_roundtrip_and_conflicts() {
        let s = store("impexp");
        s.create(&sample("x", Mode::Online)).unwrap();
        let json = s.export_json("x").unwrap();
        assert!(matches!(s.import_json(&json, false), Err(AppError::Exists(_))));
        let p = s.import_json(&json, true).unwrap();
        assert_eq!(p.id, "x");
        s.delete("x", false).unwrap();
        s.import_json(&json, false).unwrap();
        let path = s.export_to_file("x").unwrap();
        assert!(path.ends_with("exports/x.json"));
        let s2 = store("impexp2");
        s2.import_json(&std::fs::read_to_string(path).unwrap(), false).unwrap();
        // JSON invalido o perfil que viola reglas no se importa
        assert!(s2.import_json("{nope", false).is_err());
        assert!(s2
            .import_json(r#"{"schema":1,"id":"z","name":"Z","mode":"drakes-rust","channel":"stable","minecraft":"26.2","loader":"forge"}"#, false)
            .is_err());
    }

    #[test]
    fn delete_keeps_or_removes_data_and_corrupt_is_reported() {
        let l = Layout::new(crate::testutil::tmpdir("del"));
        let s = ProfileStore::new(l.clone());
        s.create(&sample("keep", Mode::LocalServer)).unwrap();
        std::fs::create_dir_all(l.server_dir("keep")).unwrap();
        s.delete("keep", false).unwrap();
        assert!(l.server_dir("keep").exists());
        assert!(!s.exists("keep"));
        std::fs::create_dir_all(l.profile_dir("broken")).unwrap();
        std::fs::write(l.profile_json("broken"), "{ no").unwrap();
        let list = s.list();
        assert!(list.profiles.is_empty());
        assert_eq!(list.invalid.len(), 1);
        assert_eq!(list.invalid[0].id, "broken");
    }

    #[test]
    fn duplicate_copies_config_only() {
        let s = store("dup");
        s.create(&sample("o", Mode::DrakesJava)).unwrap();
        let d = s.duplicate("o", "o-copy", "Copia").unwrap();
        assert_eq!(d.name, "Copia");
        assert!(s.duplicate("o", "o-copy", "x").is_err());
    }
}
