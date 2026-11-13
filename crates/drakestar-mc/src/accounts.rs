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
        std::fs::read_to_string(self.list_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    fn save(&self, l: &AccountList) -> McResult<()> {
        write_atomic(&self.list_path(), serde_json::to_string_pretty(l)?.as_bytes())?;
        Ok(())
    }

    fn secrets(&self) -> BTreeMap<String, String> {
        std::fs::read_to_string(self.secrets_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    fn save_secrets(&self, m: &BTreeMap<String, String>) -> McResult<()> {
        std::fs::create_dir_all(&self.dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let p = self.secrets_path();
        write_atomic(&p, serde_json::to_string(m)?.as_bytes())?;
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
        let mut l = self.list();
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
        let mut l = self.list();
        let id = format!("ely-{}", session.uuid);
        let acc = Account { id: id.clone(), kind: Kind::Elyby, name: session.name.clone(), uuid: session.uuid.clone(), added_at: now(), last_used: None };
        if let Some(existing) = l.accounts.iter_mut().find(|a| a.id == id) { existing.name = session.name.clone(); }
        else { l.accounts.push(acc.clone()); }
        if l.default.is_none() { l.default = Some(id.clone()); }
        let mut secrets = self.secrets();
        secrets.insert(id.clone(), serde_json::to_string(token)?);
        self.save_secrets(&secrets)?;
        self.save(&l)?;
        self.get(&id).ok_or_else(|| McError::NotFound(id))
    }

    pub fn add_microsoft(&self, p: &McProfile, refresh_token: &str) -> McResult<Account> {
        let mut l = self.list();
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
        self.save(&l)?;
        let mut s = self.secrets();
        s.insert(id.clone(), refresh_token.to_string());
        self.save_secrets(&s)?;
        self.get(&id).ok_or_else(|| McError::NotFound(id))
    }

    pub fn remove(&self, id: &str) -> McResult<()> {
        let mut l = self.list();
        l.accounts.retain(|a| a.id != id);
        if l.default.as_deref() == Some(id) {
            l.default = l.accounts.first().map(|a| a.id.clone());
        }
        self.save(&l)?;
        let mut s = self.secrets();
        if s.remove(id).is_some() {
            self.save_secrets(&s)?;
        }
        Ok(())
    }

    pub fn set_default(&self, id: &str) -> McResult<()> {
        let mut l = self.list();
        if !l.accounts.iter().any(|a| a.id == id) {
            return Err(McError::NotFound(format!("cuenta {id}")));
        }
        l.default = Some(id.into());
        self.save(&l)
    }

    pub fn touch(&self, id: &str) {
        let mut l = self.list();
        if let Some(a) = l.accounts.iter_mut().find(|a| a.id == id) {
            a.last_used = Some(now());
            let _ = self.save(&l);
        }
    }

    /// Sesion lista para lanzar. Las cuentas Microsoft renuevan su token y guardan el nuevo refresh token.
    pub fn session(&self, id: &str, client_id: &str) -> McResult<Session> {
        let acc = self.get(id).ok_or_else(|| McError::NotFound(format!("cuenta {id}")))?;
        match acc.kind {
            Kind::Offline => Ok(Session::offline(&acc.name)),
            Kind::Elyby => {
                let stored = self.secrets().get(id).cloned().ok_or_else(|| McError::Invalid("Vuelve a iniciar sesión con Ely.by.".into()))?;
                let token: crate::ely::Token = serde_json::from_str(&stored).map_err(|_| McError::Invalid("Vuelve a iniciar sesión con Ely.by.".into()))?;
                let (session, token) = crate::ely::refresh(&token)?;
                if session.uuid != acc.uuid { return Err(McError::Invalid("La identidad Ely.by cambió: vuelve a iniciar sesión.".into())); }
                self.add_elyby(&session, &token)?;
                self.touch(id);
                Ok(session)
            }
            Kind::Microsoft => {
                let rt = self.secrets().get(id).cloned().ok_or_else(|| McError::Invalid("falta el acceso guardado de esta cuenta: vuelve a iniciar sesion".into()))?;
                let t = msa::refresh(client_id, &rt)?;
                let p = msa::minecraft_profile(&t.access_token)?;
                let mut s = self.secrets();
                s.insert(id.to_string(), t.refresh_token.clone());
                self.save_secrets(&s)?;
                self.touch(id);
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
        let acc = a.add_microsoft(&McProfile { name: "Pro".into(), uuid: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(), access_token: "tok".into(), xuid: None }, "REFRESH-SECRET").unwrap();
        let public = std::fs::read_to_string(d.root().join("accounts/accounts.json")).unwrap();
        assert!(!public.contains("REFRESH-SECRET"));
        let secrets = std::fs::read_to_string(d.root().join("accounts/secrets.json")).unwrap();
        assert!(secrets.contains("REFRESH-SECRET"));
        a.remove(&acc.id).unwrap();
        assert!(!std::fs::read_to_string(d.root().join("accounts/secrets.json")).unwrap().contains("REFRESH-SECRET"));
        let _ = std::fs::remove_dir_all(d.root());
    }
}
