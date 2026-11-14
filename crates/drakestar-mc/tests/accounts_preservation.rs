//! La autenticación no debe convertir archivos dañados en una lista vacía y sobrescribirlos.
use drakestar_mc::{accounts::Accounts, ely::Token, launch::Session, Dirs};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture(Dirs);

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(Dirs::new(std::env::temp_dir().join(format!(
            "accounts-preservation-788-{}-{nonce}",
            std::process::id()
        ))))
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.root().join("accounts").join(name)
    }

    fn login(&self) -> drakestar_mc::McResult<drakestar_mc::accounts::Account> {
        // Identidad y tokens sintéticos; no se llama al proveedor ni se usa el perfil real.
        let session = Session {
            name: "FixtureEly".into(),
            uuid: "00000000000000000000000000000788".into(),
            access_token: "fixture-access-not-a-credential".into(),
            user_type: "mojang".into(),
            xuid: None,
            client_id: None,
        };
        let token = Token {
            access_token: session.access_token.clone(),
            client_token: "fixture-client-not-a-credential".into(),
        };
        Accounts::new(&self.0).add_elyby(&session, &token)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.root());
    }
}

#[test]
fn new_profile_can_add_elyby_without_network() {
    let f = Fixture::new();
    let account = f.login().unwrap();
    assert_eq!(Accounts::new(&f.0).list().accounts.len(), 1);
    assert_eq!(
        Accounts::new(&f.0).default_account().unwrap().id,
        account.id
    );
    let public = fs::read_to_string(f.path("accounts.json")).unwrap();
    assert!(!public.contains("fixture-access"));
    assert!(!public.contains("fixture-client"));
}

#[test]
fn malformed_account_list_is_preserved_and_login_rejected() {
    let f = Fixture::new();
    Accounts::new(&f.0).add_offline("FixtureOld").unwrap();
    let damaged = b"{\"accounts\":[{\"name\":\"FixtureOld\"";
    fs::write(f.path("accounts.json"), damaged).unwrap();

    let result = f.login();

    assert_eq!(
        fs::read(f.path("accounts.json")).unwrap(),
        damaged,
        "Una lectura inválida debe conservar los bytes recuperables del perfil"
    );
    assert!(
        !f.path("secrets.json").exists(),
        "Un perfil público inválido no debe dejar credenciales huérfanas"
    );
    assert!(
        result.is_err(),
        "El acceso debe informar el daño, sin reemplazar el perfil"
    );
}

#[test]
fn malformed_secret_store_is_preserved_with_existing_accounts() {
    let f = Fixture::new();
    Accounts::new(&f.0).add_offline("FixtureOld").unwrap();
    let original_accounts = fs::read(f.path("accounts.json")).unwrap();
    let damaged = b"{\"msa-fixture\":\"fixture-old-not-a-credential\"";
    fs::write(f.path("secrets.json"), damaged).unwrap();

    let result = f.login();

    assert_eq!(
        fs::read(f.path("secrets.json")).unwrap(),
        damaged,
        "Un almacén inválido debe conservar tokens recuperables de otras cuentas"
    );
    assert_eq!(
        fs::read(f.path("accounts.json")).unwrap(),
        original_accounts,
        "El error no debe añadir una cuenta parcialmente persistida"
    );
    assert!(
        result.is_err(),
        "El acceso debe informar el almacén inválido"
    );
}

#[test]
fn failed_elyby_secret_write_keeps_existing_account_files() {
    let f = Fixture::new();
    Accounts::new(&f.0).add_offline("FixtureOld").unwrap();
    let original = fs::read(f.path("accounts.json")).unwrap();
    // Una colisión de directorio provoca EISDIR, incluso ejecutando como root.
    fs::create_dir(f.path("secrets.json.tmp")).unwrap();

    assert!(f.login().is_err());

    assert_eq!(fs::read(f.path("accounts.json")).unwrap(), original);
    assert!(!f.path("secrets.json").exists());
}

#[test]
fn failed_elyby_account_write_must_not_leave_orphan_credentials() {
    let f = Fixture::new();
    Accounts::new(&f.0).add_offline("FixtureOld").unwrap();
    let original = fs::read(f.path("accounts.json")).unwrap();
    // Se puede leer el perfil; solo falla su posterior persistencia.
    fs::create_dir(f.path("accounts.json.tmp")).unwrap();

    assert!(f.login().is_err());

    assert_eq!(fs::read(f.path("accounts.json")).unwrap(), original);
    assert!(
        !f.path("secrets.json").exists(),
        "Una alta rechazada no debe persistir credenciales sin cuenta"
    );
}

#[test]
fn failed_account_removal_must_preserve_public_profile() {
    let f = Fixture::new();
    let account = f.login().unwrap();
    let original_accounts = fs::read(f.path("accounts.json")).unwrap();
    let original_secrets = fs::read(f.path("secrets.json")).unwrap();
    fs::create_dir(f.path("secrets.json.tmp")).unwrap();

    assert!(Accounts::new(&f.0).remove(&account.id).is_err());

    // Evita mostrar incluso los tokens sintéticos si esta guarda falla.
    assert!(fs::read(f.path("secrets.json")).unwrap() == original_secrets);
    assert_eq!(
        fs::read(f.path("accounts.json")).unwrap(),
        original_accounts,
        "Una retirada rechazada debe conservar la cuenta y su selección"
    );
}

#[test]
fn failed_relogin_restores_previous_credentials_and_permissions() {
    let f = Fixture::new();
    let account = f.login().unwrap();
    let original_accounts = fs::read(f.path("accounts.json")).unwrap();
    // Conservar también el formato original al restaurar, sin reserializarlo.
    let secrets = fs::read_to_string(f.path("secrets.json")).unwrap();
    let original_secrets = format!("  {secrets}\n").into_bytes();
    fs::write(f.path("secrets.json"), &original_secrets).unwrap();
    fs::create_dir(f.path("accounts.json.tmp")).unwrap();
    let session = Session {
        name: "FixtureRenamed".into(),
        uuid: account.uuid.clone(),
        access_token: "fixture-replacement-not-a-credential".into(),
        user_type: "mojang".into(),
        xuid: None,
        client_id: None,
    };
    let token = Token {
        access_token: session.access_token.clone(),
        client_token: "fixture-client".into(),
    };

    assert!(Accounts::new(&f.0).add_elyby(&session, &token).is_err());

    assert_eq!(
        fs::read(f.path("accounts.json")).unwrap(),
        original_accounts
    );
    assert!(fs::read(f.path("secrets.json")).unwrap() == original_secrets);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(f.path("secrets.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn failed_removal_account_write_restores_credentials_and_default() {
    let f = Fixture::new();
    let account = f.login().unwrap();
    let original_accounts = fs::read(f.path("accounts.json")).unwrap();
    let original_secrets = fs::read(f.path("secrets.json")).unwrap();
    fs::create_dir(f.path("accounts.json.tmp")).unwrap();

    assert!(Accounts::new(&f.0).remove(&account.id).is_err());

    assert_eq!(
        fs::read(f.path("accounts.json")).unwrap(),
        original_accounts
    );
    assert!(fs::read(f.path("secrets.json")).unwrap() == original_secrets);
}

#[test]
fn failed_microsoft_secret_write_preserves_public_profile() {
    let f = Fixture::new();
    let accounts = Accounts::new(&f.0);
    accounts.add_offline("FixtureOld").unwrap();
    let original = fs::read(f.path("accounts.json")).unwrap();
    fs::create_dir(f.path("secrets.json.tmp")).unwrap();
    let profile = drakestar_mc::msa::McProfile {
        name: "FixtureMsa".into(),
        uuid: "00000000000000000000000000000789".into(),
        access_token: "fixture-access".into(),
        xuid: None,
    };

    assert!(accounts.add_microsoft(&profile, "fixture-refresh").is_err());

    assert_eq!(fs::read(f.path("accounts.json")).unwrap(), original);
    assert!(!f.path("secrets.json").exists());
}

#[test]
fn corrupt_public_profile_rejects_offline_mutations() {
    let f = Fixture::new();
    let accounts = Accounts::new(&f.0);
    let account = accounts.add_offline("FixtureOld").unwrap();
    let damaged = b"{broken";
    fs::write(f.path("accounts.json"), damaged).unwrap();

    assert!(accounts.add_offline("FixtureNew").is_err());
    assert!(accounts.set_default(&account.id).is_err());
    assert!(accounts.remove(&account.id).is_err());
    accounts.touch(&account.id);

    assert_eq!(fs::read(f.path("accounts.json")).unwrap(), damaged);
}
