//! Prueba de `play::prepare` completa (sin lanzar): `cargo run -p drakestar-mc --example prepare_test -- <instancia> [carpeta]`.
//! Sirve para comprobar el modo sin conexion (p. ej. `unshare -n`): con todo ya descargado debe preparar igual y avisar.

use drakestar_mc::accounts::Accounts;
use drakestar_mc::instances::Instances;
use drakestar_mc::{play, Dirs, NullSink};

fn main() {
    let mut args = std::env::args().skip(1);
    let id = args.next().unwrap_or_else(|| "vanilla-latest".into());
    let dir = args.next().map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("drakestar-prepare-test"));
    let d = Dirs::new(dir);
    let inst = Instances::new(&d).get(&id).expect("instancia");
    let accounts = Accounts::new(&d);
    let acc = accounts.default_account().unwrap_or_else(|| accounts.add_offline("Tester").expect("cuenta"));
    match play::prepare(&d, &inst, &acc.id, "00000000-0000-0000-0000-000000000000", &NullSink) {
        Ok(p) => {
            println!("OK {} con Java {} ({} argumentos)", p.version_id, p.java_major, p.plan.args.len());
            for w in &p.warnings {
                println!("aviso: {w}");
            }
        }
        Err(e) => {
            println!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}
