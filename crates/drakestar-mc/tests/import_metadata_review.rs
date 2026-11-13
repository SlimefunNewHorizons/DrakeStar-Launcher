//! Independent regression probes for PR #5. No network or real instances.
//! Run the ignored probes explicitly; they describe unresolved ticket #785.
use drakestar_mc::{importing, Dirs};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);
const METADATA_LIMIT: u64 = 8 * 1024 * 1024;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir();
        loop {
            let path = root.join(format!(
                "drakestar-metadata-review-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("fixture: {e}"),
            }
        }
    }

    fn prism(&self) -> PathBuf {
        let source = self.0.join("source");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"26.2"}]}"#,
        )
        .unwrap();
        source
    }

    fn dirs(&self) -> Dirs {
        Dirs::new(self.0.join("launcher"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn preview_accepts_regular_small_config_without_writing_instance() {
    let f = Fixture::new();
    let source = f.prism();
    fs::write(source.join("instance.cfg"), "name=Review control\n").unwrap();
    let preview = importing::preview(&f.dirs(), &source).unwrap();
    assert_eq!(preview.instance.name, "Review control");
    assert_eq!(preview.instance.mc, "26.2");
    assert!(!f.dirs().instances().exists());
}

// Exercise the shared JSON reader through each public format, independently
// of the instance.cfg probes above. All metadata and link targets are synthetic.
const JSON_FORMATS: &[(&str, &str, &str)] = &[
    (
        "Prism",
        "mmc-pack.json",
        r#"{"components":[{"uid":"net.minecraft","version":"26.2"}]}"#,
    ),
    (
        "CurseForge",
        "minecraftinstance.json",
        r#"{"name":"JSON review","gameVersion":"26.2"}"#,
    ),
    (
        "Modrinth",
        "modrinth.index.json",
        r#"{"formatVersion":1,"game":"minecraft","name":"JSON review","dependencies":{"minecraft":"26.2"},"files":[]}"#,
    ),
];

#[test]
fn json_metadata_regular_files_preview_all_formats_without_side_effects() {
    for (format, filename, content) in JSON_FORMATS {
        let f = Fixture::new();
        let source = f.0.join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join(filename), content).unwrap();
        let preview = importing::preview(&f.dirs(), &source).unwrap();
        assert_eq!(&preview.format, format, "{filename}");
        assert_eq!(preview.instance.mc, "26.2", "{filename}");
        assert_eq!(fs::read_to_string(source.join(filename)).unwrap(), *content);
        assert!(!f.dirs().instances().exists(), "{filename}");
    }
}

#[test]
fn json_metadata_size_limit_rejects_all_formats() {
    for (_, filename, _) in JSON_FORMATS {
        let f = Fixture::new();
        let source = f.0.join("source");
        fs::create_dir(&source).unwrap();
        let metadata = fs::File::create(source.join(filename)).unwrap();
        metadata.set_len(METADATA_LIMIT + 1).unwrap();
        drop(metadata);
        let error = importing::preview(&f.dirs(), &source).unwrap_err();
        assert!(
            error.to_string().contains("metadatos demasiado grandes"),
            "{filename}: expected size rejection before JSON parsing, got {error}"
        );
        assert!(!f.dirs().instances().exists(), "{filename}");
    }
}

#[cfg(unix)]
#[test]
#[ignore = "#785: the shared JSON reader follows external metadata symlinks"]
fn json_metadata_rejects_external_symlinks_in_all_formats() {
    let mut accepted = Vec::new();
    for (format, filename, content) in JSON_FORMATS {
        let f = Fixture::new();
        let source = f.0.join("source");
        fs::create_dir(&source).unwrap();
        let outside = f.0.join("outside.json");
        fs::write(&outside, content).unwrap();
        std::os::unix::fs::symlink(&outside, source.join(filename)).unwrap();
        if let Ok(preview) = importing::preview(&f.dirs(), &source) {
            assert_eq!(&preview.format, format, "{filename}");
            accepted.push(*filename);
        }
        assert_eq!(fs::read_to_string(&outside).unwrap(), *content);
        assert!(!f.dirs().instances().exists(), "{filename}");
    }
    assert!(
        accepted.is_empty(),
        "preview followed external JSON metadata symlinks: {accepted:?}"
    );
}

#[test]
#[ignore = "#785: Prism instance.cfg currently has no size limit"]
fn preview_rejects_oversized_prism_config() {
    let f = Fixture::new();
    let source = f.prism();
    let mut config = fs::File::create(source.join("instance.cfg")).unwrap();
    config
        .write_all(b"name=Oversized review fixture\n")
        .unwrap();
    config.set_len(METADATA_LIMIT + 1).unwrap();
    drop(config);
    let result = importing::preview(&f.dirs(), &source);
    assert!(
        result.is_err(),
        "preview accepted instance.cfg larger than the JSON metadata limit"
    );
}

#[cfg(unix)]
#[test]
#[ignore = "#785: Prism instance.cfg currently follows symbolic links"]
fn preview_rejects_prism_config_symlink() {
    let f = Fixture::new();
    let source = f.prism();
    let outside = f.0.join("outside.cfg");
    fs::write(&outside, "name=Outside synthetic sentinel\n").unwrap();
    std::os::unix::fs::symlink(&outside, source.join("instance.cfg")).unwrap();
    let result = importing::preview(&f.dirs(), &source);
    assert!(
        result.is_err(),
        "preview followed instance.cfg outside the selected source: {result:?}"
    );
}

// Child process only: a blocked FIFO read must never hang the test runner.
#[cfg(unix)]
#[test]
#[ignore = "subprocess helper for preview_rejects_prism_config_fifo_promptly"]
fn fifo_preview_worker() {
    let Some(root) = std::env::var_os("DRAKESTAR_METADATA_REVIEW_ROOT") else {
        return;
    };
    let root = Path::new(&root);
    fs::write(root.join("worker-ready"), b"ready").unwrap();
    assert!(
        importing::preview(&Dirs::new(root.join("launcher")), &root.join("source")).is_err(),
        "preview accepted a FIFO config"
    );
}

#[cfg(unix)]
#[test]
#[ignore = "#785: opening a FIFO instance.cfg currently blocks preview"]
fn preview_rejects_prism_config_fifo_promptly() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let f = Fixture::new();
    let source = f.prism();
    assert!(Command::new("mkfifo")
        .arg(source.join("instance.cfg"))
        .status()
        .unwrap()
        .success());
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "fifo_preview_worker", "--ignored"])
        .env("DRAKESTAR_METADATA_REVIEW_ROOT", &f.0)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let mut ready = None;
    let exit = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if ready.is_none() && f.0.join("worker-ready").exists() {
            ready = Some(Instant::now());
        }
        if ready.is_some_and(|t| t.elapsed() >= Duration::from_secs(1))
            || start.elapsed() >= Duration::from_secs(10)
        {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        f.0.join("worker-ready").exists(),
        "FIFO worker never reached the preview call"
    );
    assert!(
        exit.is_some_and(|s| s.success()),
        "preview blocked on instance.cfg FIFO instead of rejecting it promptly"
    );
}
