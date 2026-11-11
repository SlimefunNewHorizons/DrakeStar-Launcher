//! Binario del cliente Rust de DrakeStar. Lo lanza el launcher con
//! `--server <host[:puerto]> --name <usuario>`; tambien se puede usar a mano.
//!
//! `--login` abre el flujo de cuenta de Microsoft (device code), necesario para
//! entrar a servidores con `online-mode=true`.

use drakestar_rust_client::{auth, client::{self, Config, Error}};
use std::process::ExitCode;

fn usage() {
    eprintln!(
        "drakestar-rust-client --server <host[:puerto]> [--name <usuario>] [--protocol <n>] [--login]\n  \
         --login  pide cuenta de Microsoft (necesario en online-mode=true)\n  \
         consola: escribe 'quit' (o 'stop') para salir"
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cfg = Config::default();
    let mut given_uuid: Option<String> = None;
    let mut given_token: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        let value = args.get(i + 1).cloned();
        match flag {
            "--server" => {
                let v = match value {
                    Some(v) => v,
                    None => {
                        eprintln!("ERROR {flag} necesita un valor");
                        return ExitCode::from(2);
                    }
                };
                match v.rsplit_once(':') {
                    Some((host, port)) if !host.is_empty() && !port.is_empty() => {
                        cfg.server = host.to_string();
                        cfg.port = port.parse().unwrap_or_else(|_| {
                            eprintln!("ERROR puerto invalido '{port}'");
                            std::process::exit(2)
                        });
                    }
                    _ => cfg.server = v,
                }
                i += 2;
            }
            "--name" => {
                cfg.name = value.unwrap_or_else(|| {
                    eprintln!("ERROR {flag} necesita un valor");
                    std::process::exit(2)
                });
                i += 2;
            }
            "--protocol" => {
                let v = value.unwrap_or_else(|| {
                    eprintln!("ERROR {flag} necesita un valor");
                    std::process::exit(2)
                });
                cfg.protocol = v.parse().unwrap_or_else(|_| {
                    eprintln!("ERROR protocolo invalido '{v}'");
                    std::process::exit(2)
                });
                i += 2;
            }
            "-h" | "--help" => {
                usage();
                return ExitCode::SUCCESS;
            }
            "--login" => {
                cfg.wants_login = true;
                i += 1;
            }
            "--uuid" => {
                let v = value.unwrap_or_else(|| {
                    eprintln!("ERROR {flag} necesita un valor");
                    std::process::exit(2)
                });
                given_uuid = Some(v);
                i += 2;
            }
            "--token" => {
                let v = value.unwrap_or_else(|| {
                    eprintln!("ERROR {flag} necesita un valor");
                    std::process::exit(2)
                });
                given_token = Some(v);
                i += 2;
            }
            other => {
                eprintln!("ERROR argumento desconocido: {other}");
                usage();
                return ExitCode::from(2);
            }
        }
    }
    if cfg.server.is_empty() {
        eprintln!("ERROR falta --server");
        usage();
        return ExitCode::from(2);
    }

    if let (Some(u), Some(t)) = (given_uuid, given_token) {
        cfg.session = Some(auth::Session {
            uuid: auth::hyphens(&u),
            name: cfg.name.clone(),
            access_token: t,
        });
    }

    // --login: cadena Microsoft -> Xbox -> Minecraft. Bloquea hasta que el
    // usuario autorice en el navegador (o el codigo caduca a los 15 min).
    // Si ya vino una sesion valida via --token y --uuid, se reutiliza directamente.
    if cfg.wants_login && cfg.session.is_none() {
        match auth::authenticate(&mut |l| println!("{l}")) {
            Ok(s) => {
                cfg.session = Some(s.clone());
                // el nombre sale del perfil de Minecraft, no del que se haya pasado
                cfg.name = s.name;
            }
            Err(e) => {
                eprintln!("ERROR login: {e}");
                return ExitCode::from(3);
            }
        }
    }

    match client::run(&cfg, &mut |l| println!("{l}")) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if !matches!(e, Error::Disconnected(_)) {
                eprintln!("ERROR {e}");
            }
            ExitCode::from(1)
        }
    }
}
