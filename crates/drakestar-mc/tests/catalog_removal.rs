//! Regresiones independientes: retirar contenido debe conservar la coherencia con el índice.
use drakestar_mc::{catalog, instances::Instance, Dirs};
use sha2::{Digest, Sha512};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const PAYLOAD: &[u8] = b"fixture: not an executable jar";

struct Fixture {
    dirs: Dirs,
    instance: Instance,
    root: PathBuf,
    index: Vec<u8>,
}

impl Fixture {
    fn new(owned: bool) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dirs = Dirs::new(
            std::env::temp_dir().join(format!("catalog-788-{}-{nonce}", std::process::id())),
        );
        let instance: Instance = serde_json::from_value(serde_json::json!({
            "id": "fixture", "name": "Fixture", "mc": "26.2",
            "loader": "fabric", "target": {"kind": "menu"}
        }))
        .unwrap();
        let root = dirs.instance(&instance.id);
        fs::create_dir_all(root.join("mods")).unwrap();
        fs::write(root.join("mods/fixture.jar"), PAYLOAD).unwrap();
        let index = serde_json::to_vec(&vec![catalog::Entry {
            project_id: "fixture".into(),
            version_id: "v1".into(),
            title: "Fixture".into(),
            kind: catalog::Kind::Mod,
            filename: "fixture.jar".into(),
            sha512: Sha512::digest(PAYLOAD)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            minecraft: "26.2".into(),
            dependencies: vec![],
            owned,
        }])
        .unwrap();
        fs::write(root.join(".drakestar-catalog.json"), &index).unwrap();
        Self {
            dirs,
            instance,
            root,
            index,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700));
        }
        let _ = fs::remove_dir_all(self.dirs.root());
    }
}

#[test]
fn successful_removal_updates_file_and_index() {
    let f = Fixture::new(true);
    assert!(catalog::remove(&f.dirs, &f.instance, "fixture")
        .unwrap()
        .is_empty());
    assert!(!f.root.join("mods/fixture.jar").exists());
    assert!(catalog::list(&f.dirs, &f.instance).unwrap().is_empty());
}

#[test]
fn external_file_survives_removal_from_index() {
    let f = Fixture::new(false);
    assert!(catalog::remove(&f.dirs, &f.instance, "fixture")
        .unwrap()
        .is_empty());
    assert_eq!(fs::read(f.root.join("mods/fixture.jar")).unwrap(), PAYLOAD);
    assert!(catalog::list(&f.dirs, &f.instance).unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn failed_index_write_preserves_installed_file() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new(true);
    // El índice sigue legible; mods/ permite retirar archivos, pero el índice no se puede reemplazar.
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o500)).unwrap();
    assert!(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(f.root.join("permission-probe"))
            .is_err(),
        "El ensayo exige un usuario sin bypass de permisos"
    );
    let result = catalog::remove(&f.dirs, &f.instance, "fixture");
    assert!(
        result.is_err(),
        "El índice sin permiso de escritura debe fallar"
    );
    assert_eq!(
        fs::read(f.root.join(".drakestar-catalog.json")).unwrap(),
        f.index
    );
    assert!(
        f.root.join("mods/fixture.jar").exists(),
        "Una retirada fallida debe conservar el archivo que el índice aún declara instalado"
    );
    assert_eq!(fs::read(f.root.join("mods/fixture.jar")).unwrap(), PAYLOAD);
}
