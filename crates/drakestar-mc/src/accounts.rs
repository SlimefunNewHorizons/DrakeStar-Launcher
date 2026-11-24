//! Gestion de cuentas: varias cuentas Microsoft (premium) y sin premium por PC, una por defecto.
//!
//! `accounts/accounts.json` guarda solo datos publicos (nombre, UUID). Los refresh tokens van aparte en
//! `accounts/secrets.json` (modo 0600 en Unix; en Windows queda bajo el perfil del usuario). Nunca se loguean.

use crate::launch::Session;
use crate::msa::{self, McProfile};
use crate::{offline, write_atomic, Dirs, McError, McResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Microsoft,
    Elyby,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub uuid: String,
    pub added_at: u64,
    #[serde(default)]
    pub last_used: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountList {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub default: Option<String>,
}

pub struct Accounts {
    dir: PathBuf,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Accounts {
    pub fn new(d: &Dirs) -> Self {
        Self { dir: d.root().join("accounts") }
    }
    fn list_path(&self) -> PathBuf {
        self.dir.join("accounts.json")
    }
    fn secrets_path(&self) -> PathBuf {
        self.dir.join("secrets.json")
    }

    pub fn list(&self) -> AccountList {
        self.read_list().unwrap_or_default()
    }

    fn read_list(&self) -> McResult<AccountList> {
        match std::fs::read(self.list_path()) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AccountList::default()),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&self, l: &AccountList) -> McResult<()> {
        write_atomic(&self.list_path(), serde_json::to_string_pretty(l)?.as_bytes())?;
        Ok(())
    }

    #[cfg(test)]
    fn secrets(&self) -> BTreeMap<String, String> {
        self.read_secrets().unwrap_or_default()
    }

    fn read_secrets(&self) -> McResult<BTreeMap<String, String>> {
        match std::fs::read(self.secrets_path()) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e.into()),
        }
    }

    // Si falla el listado, restaura los bytes anteriores del almacén privado.
    // No es una transacción resistente a la caída del proceso entre los dos archivos.
    fn save_with_secrets(&self, list: &AccountList, secrets: &BTreeMap<String, String>) -> McResult<()> {
        let previous = match std::fs::read(self.secrets_path()) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        if let Err(error) = self.save_secrets(secrets).and_then(|()| self.save(list)) {
            // Una escritura rechazada antes del rename ya conserva el almacén.
            let unchanged = match std::fs::read(self.secrets_path()) {
                Ok(bytes) => previous.as_ref() == Some(&bytes),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => previous.is_none(),
                Err(_) => false,
            };
            if unchanged { return Err(error); }
            let restored = match previous {
                Some(bytes) => self.save_secret_bytes(&bytes),
                None => std::fs::remove_file(self.secrets_path()).map_err(McError::from),
            };
            if restored.is_err() {
                return Err(McError::Invalid("No se pudo guardar la cuenta ni restaurar accounts/secrets.json. Conserva la carpeta accounts para recuperarla antes de reintentar.".into()));
            }
            return Err(error);
        }
        Ok(())
    }

    fn save_secrets(&self, m: &BTreeMap<String, String>) -> McResult<()> {
        self.save_secret_bytes(serde_json::to_string(m)?.as_bytes())
    }

    fn save_secret_bytes(&self, bytes: &[u8]) -> McResult<()> {
        std::fs::create_dir_all(&self.dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let p = self.secrets_path();
        write_atomic(&p, bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<Account> {
        self.list().accounts.into_iter().find(|a| a.id == id)
    }

    /// Cuenta por defecto (la marcada, o la primera).
    pub fn default_account(&self) -> Option<Account> {
        let l = self.list();
        l.default.as_ref().and_then(|d| l.accounts.iter().find(|a| &a.id == d).cloned()).or_else(|| l.accounts.first().cloned())
    }

    pub fn add_offline(&self, name: &str) -> McResult<Account> {
        if !offline::valid_name(name) {
            return Err(McError::Invalid("el nombre debe tener 3 a 16 letras, numeros o guion bajo".into()));
        }
        let mut l = self.read_list()?;
        let uuid = offline::offline_uuid(name);
        let id = format!("offline-{uuid}");
        if let Some(a) = l.accounts.iter().find(|a| a.id == id) {
            return Ok(a.clone());
        }
        let acc = Account { id: id.clone(), kind: Kind::Offline, name: name.into(), uuid, added_at: now(), last_used: None };
        l.accounts.push(acc.clone());
        if l.default.is_none() {
            l.default = Some(id);
        }
        self.save(&l)?;
        Ok(acc)
    }

    pub fn add_elyby(&self, session: &Session, token: &crate::ely::Token) -> McResult<Account> {
        let mut l = self.read_list()?;
        let id = format!("ely-{}", session.uuid);
        let acc = Account { id: id.clone(), kind: Kind::Elyby, name: session.name.clone(), uuid: session.uuid.clone(), added_at: now(), last_used: None };
        if let Some(existing) = l.accounts.iter_mut().find(|a| a.id == id) { existing.name = session.name.clone(); }
        else { l.accounts.push(acc.clone()); }
        if l.default.is_none() { l.default = Some(id.clone()); }
        let mut secrets = self.read_secrets()?;
        secrets.insert(id.clone(), serde_json::to_string(token)?);
        self.save_with_secrets(&l, &secrets)?;
        self.get(&id).ok_or(McError::NotFound(id))
    }

    pub fn add_microsoft(&self, p: &McProfile, refresh_token: &str) -> McResult<Account> {
        let mut l = self.read_list()?;
        let id = format!("msa-{}", p.uuid);
        let acc = Account { id: id.clone(), kind: Kind::Microsoft, name: p.name.clone(), uuid: p.uuid.clone(), added_at: now(), last_used: None };
        match l.accounts.iter_mut().find(|a| a.id == id) {
            Some(existing) => {
                existing.name = p.name.clone();
            }
            None => l.accounts.push(acc.clone()),
        }
        if l.default.is_none() {
            l.default = Some(id.clone());
        }
        let mut s = self.read_secrets()?;
        s.insert(id.clone(), refresh_token.to_string());
        self.save_with_secrets(&l, &s)?;
        self.get(&id).ok_or(McError::NotFound(id))
    }

    pub fn remove(&self, id: &str) -> McResult<()> {
        let mut l = self.read_list()?;
        l.accounts.retain(|a| a.id != id);
        if l.default.as_deref() == Some(id) {
            l.default = l.accounts.first().map(|a| a.id.clone());
        }
        let mut s = self.read_secrets()?;
        if s.remove(id).is_some() {
            self.save_with_secrets(&l, &s)
        } else {
            self.save(&l)
        }
    }

    pub fn set_default(&self, id: &str) -> McResult<()> {
        let mut l = self.read_list()?;
        if !l.accounts.iter().any(|a| a.id == id) {
            return Err(McError::NotFound(format!("cuenta {id}")));
        }
        l.default = Some(id.into());
        self.save(&l)
    }

    pub fn touch(&self, id: &str) {
        self.touch_with_name(id, None);
    }

    fn touch_with_name(&self, id: &str, new_name: Option<&str>) {
        let Ok(mut l) = self.read_list() else { return; };
        if let Some(a) = l.accounts.iter_mut().find(|a| a.id == id) {
            if let Some(name) = new_name {
                a.name = name.to_string();
            }
            a.last_used = Some(now());
            let _ = self.save(&l);
        }
    }

    fn persist_refreshed_elyby(&self, id: &str, expected_uuid: &str, session: Session, token: &crate::ely::Token) -> McResult<Session> {
        if session.uuid != expected_uuid {
            return Err(McError::Invalid("La identidad Ely.by cambió: vuelve a iniciar sesión.".into()));
        }
        // Persistir primero el token rotado en secrets.json sin rollback al token anterior ya invalidado por Ely.by.
        let mut s = self.read_secrets()?;
        s.insert(id.to_string(), serde_json::to_string(token)?);
        self.save_secrets(&s)?;
        self.touch_with_name(id, Some(&session.name));
        Ok(session)
    }

    /// Sesion lista para lanzar. Las cuentas Microsoft y Ely.by guardan primero el token rotado en secrets.json.
    pub fn session(&self, id: &str, client_id: &str) -> McResult<Session> {
        let acc = self.get(id).ok_or_else(|| McError::NotFound(format!("cuenta {id}")))?;
        match acc.kind {
            Kind::Offline => Ok(Session::offline(&acc.name)),
            Kind::Elyby => {
                let stored = self.read_secrets()?.get(id).cloned().ok_or_else(|| McError::Invalid("Vuelve a iniciar sesión con Ely.by.".into()))?;
                let token: crate::ely::Token = serde_json::from_str(&stored).map_err(|_| McError::Invalid("Vuelve a iniciar sesión con Ely.by.".into()))?;
                let (session, token) = crate::ely::refresh(&token)?;
                self.persist_refreshed_elyby(id, &acc.uuid, session, &token)
            }
            Kind::Microsoft => {
                let rt = self.read_secrets()?.get(id).cloned().ok_or_else(|| McError::Invalid("falta el acceso guardado de esta cuenta: vuelve a iniciar sesion".into()))?;
                let t = msa::refresh(client_id, &rt)?;
                let mut s = self.read_secrets()?;
                s.insert(id.to_string(), t.refresh_token.clone());
                self.save_secrets(&s)?;
                let p = msa::minecraft_profile(&t.access_token)?;
                self.touch_with_name(id, Some(&p.name));
                Ok(Session { name: p.name, uuid: p.uuid, access_token: p.access_token, user_type: "msa".into(), xuid: p.xuid, client_id: Some(client_id.into()) })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> Dirs {
        let p = std::env::temp_dir().join(format!("dsmc-acc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        Dirs::new(p)
    }

    #[test]
    fn offline_accounts_and_default() {
        let d = tmp("a");
        let a = Accounts::new(&d);
        assert!(a.list().accounts.is_empty());
        let x = a.add_offline("Alex").unwrap();
        let y = a.add_offline("Steve").unwrap();
        assert_eq!(a.default_account().unwrap().id, x.id);
        a.set_default(&y.id).unwrap();
        assert_eq!(a.default_account().unwrap().name, "Steve");
        assert_eq!(a.add_offline("Alex").unwrap().id, x.id); // sin duplicados
        assert!(a.add_offline("no valido!").is_err());
        let s = a.session(&x.id, "cid").unwrap();
        assert_eq!(s.user_type, "legacy");
        a.remove(&y.id).unwrap();
        assert_eq!(a.default_account().unwrap().id, x.id);
        let _ = std::fs::remove_dir_all(d.root());
    }

    #[test]
    fn elyby_tokens_are_private_and_removed_with_account() {
        let d = tmp("ely"); let a = Accounts::new(&d);
        let session = Session { name:"Alex".into(), uuid:"069a79f444e94726a5befca90e38aaf5".into(), access_token:"TEST-ELY-TOKEN".into(), user_type:"mojang".into(), xuid:None, client_id:None };
        let token = crate::ely::Token { access_token:session.access_token.clone(), client_token:"TEST-CLIENT".into() };
        let account = a.add_elyby(&session, &token).unwrap();
        assert_eq!(account.kind, Kind::Elyby);
        assert_eq!(a.add_elyby(&session, &token).unwrap().id, account.id);
        assert_eq!(a.list().accounts.len(), 1);
        let public = std::fs::read_to_string(a.list_path()).unwrap();
        assert!(!public.contains("TEST-ELY-TOKEN")); assert!(!public.contains("TEST-CLIENT"));
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(a.secrets_path()).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(std::fs::metadata(&a.dir).unwrap().permissions().mode() & 0o777, 0o700);
        }
        a.remove(&account.id).unwrap();
        assert!(a.secrets().is_empty());
        std::fs::remove_dir_all(d.root()).unwrap();
    }

    #[test]
    fn microsoft_secret_is_stored_apart() {
        let d = tmp("b");
        let a = Accounts::new(&d);
        let acc = a.add_microsoft(&McProfile { name: "Pro".into(), uuid: "00112233-4455-6677-8899-aabbccddeeff".into(), access_token: "tok".into(), xuid: None }, "REFRESH-SECRET").unwrap();
        let public = std::fs::read_to_string(d.root().join("accounts/accounts.json")).unwrap();
        assert!(!public.contains("REFRESH-SECRET"));
        let secrets = std::fs::read_to_string(d.root().join("accounts/secrets.json")).unwrap();
        assert!(secrets.contains("REFRESH-SECRET"));
        a.remove(&acc.id).unwrap();
        assert!(!std::fs::read_to_string(d.root().join("accounts/secrets.json")).unwrap().contains("REFRESH-SECRET"));
        let _ = std::fs::remove_dir_all(d.root());
    }

    #[test]
    fn elyby_refresh_persists_rotated_secret_without_rollback_on_list_failure() {
        let d = tmp("ely-refresh");
        let a = Accounts::new(&d);
        let uuid = "069a79f444e94726a5befca90e38aaf5";
        let initial_session = Session {
            name: "Alex".into(),
            uuid: uuid.into(),
            access_token: "OLD-TOKEN".into(),
            user_type: "mojang".into(),
            xuid: None,
            client_id: None,
        };
        let initial_token = crate::ely::Token {
            access_token: "OLD-TOKEN".into(),
            client_token: "CLIENT-1".into(),
        };
        let acc = a.add_elyby(&initial_session, &initial_token).unwrap();

        // 1) Un cambio de UUID en refresh se rechaza sin pisar el token almacenado.
        let wrong_uuid_session = Session {
            name: "Impostor".into(),
            uuid: "ffffffffffffffffffffffffffffffff".into(),
            access_token: "WRONG-TOKEN".into(),
            user_type: "mojang".into(),
            xuid: None,
            client_id: None,
        };
        let wrong_token = crate::ely::Token {
            access_token: "WRONG-TOKEN".into(),
            client_token: "CLIENT-1".into(),
        };
        assert!(a.persist_refreshed_elyby(&acc.id, &acc.uuid, wrong_uuid_session, &wrong_token).is_err());
        assert!(std::fs::read_to_string(a.secrets_path()).unwrap().contains("OLD-TOKEN"));

        // 2) Rotación normal actualiza secrets.json, nombre y last_used en una sola pasada.
        let rotated_session = Session {
            name: "AlexNew".into(),
            uuid: uuid.into(),
            access_token: "ROTATED-TOKEN-1".into(),
            user_type: "mojang".into(),
            xuid: None,
            client_id: None,
        };
        let rotated_token = crate::ely::Token {
            access_token: "ROTATED-TOKEN-1".into(),
            client_token: "CLIENT-1".into(),
        };
        let out = a.persist_refreshed_elyby(&acc.id, &acc.uuid, rotated_session, &rotated_token).unwrap();
        assert_eq!(out.access_token, "ROTATED-TOKEN-1");
        assert!(std::fs::read_to_string(a.secrets_path()).unwrap().contains("ROTATED-TOKEN-1"));
        let updated = a.get(&acc.id).unwrap();
        assert_eq!(updated.name, "AlexNew");
        assert!(updated.last_used.is_some());

        // 3) Si accounts.json falla al actualizar metadatos tras rotar en el servidor remoto,
        // secrets.json conserva el token nuevo (ROTATED-TOKEN-2) y nunca revierte al invalidado.
        std::fs::remove_file(a.list_path()).unwrap();
        std::fs::create_dir_all(a.list_path()).unwrap();
        let rotated_session_2 = Session {
            name: "AlexNew2".into(),
            uuid: uuid.into(),
            access_token: "ROTATED-TOKEN-2".into(),
            user_type: "mojang".into(),
            xuid: None,
            client_id: None,
        };
        let rotated_token_2 = crate::ely::Token {
            access_token: "ROTATED-TOKEN-2".into(),
            client_token: "CLIENT-1".into(),
        };
        let out2 = a.persist_refreshed_elyby(&acc.id, &acc.uuid, rotated_session_2, &rotated_token_2).unwrap();
        assert_eq!(out2.access_token, "ROTATED-TOKEN-2");
        let saved_secrets = std::fs::read_to_string(a.secrets_path()).unwrap();
        assert!(saved_secrets.contains("ROTATED-TOKEN-2"));
        assert!(!saved_secrets.contains("ROTATED-TOKEN-1"));

        let _ = std::fs::remove_dir_all(d.root());
    }
}
