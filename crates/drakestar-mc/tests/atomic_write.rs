use drakestar_mc::write_atomic;
use std::path::PathBuf;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("ds-atomic-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn replaces_destination_and_removes_temporary_file() {
    let f = Fixture::new();
    let path = f.0.join("nested/catalog.json");
    write_atomic(&path, b"old").unwrap();
    write_atomic(&path, b"new").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
    assert!(!f.0.join("nested/catalog.json.tmp").exists());
}

#[test]
fn preserves_existing_temporary_file_and_destination() {
    let f = Fixture::new();
    let path = f.0.join("catalog.json");
    let tmp = f.0.join("catalog.json.tmp");
    std::fs::write(&path, b"old").unwrap();
    std::fs::write(&tmp, b"other writer").unwrap();
    assert!(write_atomic(&path, b"new").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"old");
    assert_eq!(std::fs::read(&tmp).unwrap(), b"other writer");
}

#[test]
fn failed_rename_cleans_up_only_its_temporary_file() {
    let f = Fixture::new();
    let path = f.0.join("catalog.json");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("keep"), b"existing").unwrap();
    assert!(write_atomic(&path, b"new").is_err());
    assert_eq!(std::fs::read(path.join("keep")).unwrap(), b"existing");
    assert!(!f.0.join("catalog.json.tmp").exists());
}

#[cfg(unix)]
#[test]
fn refuses_temporary_symlinks_without_changing_external_files() {
    let f = Fixture::new();
    let path = f.0.join("catalog.json");
    let victim = f.0.join("outside.txt");
    std::fs::write(&victim, b"preserve").unwrap();
    std::os::unix::fs::symlink(&victim, f.0.join("catalog.json.tmp")).unwrap();
    assert!(write_atomic(&path, b"new").is_err());
    assert_eq!(std::fs::read(&victim).unwrap(), b"preserve");
    assert!(std::fs::symlink_metadata(f.0.join("catalog.json.tmp"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!path.exists());
}

#[test]
fn refuses_temporary_hardlinks_without_changing_external_files() {
    let f = Fixture::new();
    let path = f.0.join("catalog.json");
    let victim = f.0.join("outside.txt");
    std::fs::write(&victim, b"preserve").unwrap();
    std::fs::hard_link(&victim, f.0.join("catalog.json.tmp")).unwrap();
    assert!(write_atomic(&path, b"new").is_err());
    assert_eq!(std::fs::read(&victim).unwrap(), b"preserve");
    assert!(!path.exists());
}
