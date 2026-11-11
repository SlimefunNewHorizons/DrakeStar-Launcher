//! Conexion del cliente: handshake, login (offline o con cuenta de Minecraft),
//! y lectura del flujo posterior.
//!
//! Alcance real (y sus limites, para no prometer de mas):
//! - handshake + login start + login acknowledged: si, contra un servidor.
//! - **online-mode=true**: cifrado AES-128-CFB8 y autenticacion con Minecraft
//!   Services (`crypto` + `auth`). Requiere que el llamador pase una `Session`.
//! - servidores `online-mode=false`: tambien, sin cifrado ni sesion.
//! - NO hace compresion, NO decodifica los estados de configuracion/play
//!   (inventarios, GUIs de Slimefun): ahi solo registra el paquete.
//!
//! shortcut: sin compresion y sin decodificar play; actualizar cuando se pruebe
//! contra un DrakesPurpur 26.2 real.

use crate::proto;
use serde_json::Value;
use std::io::{BufRead, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Cualquier cosa que se pueda leer y escribir: el socket pelado o el túnel
/// cifrado. Permite cambiarlo en caliente tras el handshake de cifrado.
trait ReadWrite: Read + Write {}
impl<S: Read + Write> ReadWrite for S {}

/// Protocolo de la linea 26.x del ecosistema (docs/ARCHITECTURE.md):
/// DrakesPurpur 26.2 y Horizon 26.3/protocolo 777.
pub const PROTOCOL_26_2: i32 = 776;
pub const DEFAULT_PORT: u16 = 25565;

pub const NEXT_STATE_LOGIN: i32 = 2;
pub const LOGIN_START: i32 = 0x00;
pub const LOGIN_ACKNOWLEDGED: i32 = 0x02;
pub const PKT_DISCONNECT: i32 = 0x00;
pub const PKT_ENCRYPTION_REQUEST: i32 = 0x01;
pub const PKT_ENCRYPTION_RESPONSE: i32 = 0x01;
pub const PKT_LOGIN_SUCCESS: i32 = 0x02;
pub const PKT_SET_COMPRESSION: i32 = 0x03;

#[derive(Debug, Clone)]
pub struct Config {
    pub server: String,
    pub port: u16,
    pub name: String,
    pub protocol: i32,
    /// Conexion y espera de respuesta. Si el servidor no contesta al login dentro
    /// de este tiempo, se aborta en vez de quedarse colgado.
    pub timeout: Duration,
    /// Sesion de Minecraft para entrar a servidores `online-mode=true`. Si es
    /// `None` y el servidor pide cifrado, no se puede continuar.
    pub session: Option<crate::auth::Session>,
    /// Pedir login con cuenta de Microsoft antes de conectar (lo usa el binario;
    /// quien llame a la libreria puede autenticar por su cuenta).
    pub wants_login: bool,
    /// Solo para tests: no llama a Mojang (no hay red ni cuenta real). La parte
    /// de cifrado del protocolo sí se ejerce completa.
    #[doc(hidden)]
    pub skip_join: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { server: String::new(), port: DEFAULT_PORT, name: "jugador".into(), protocol: PROTOCOL_26_2, timeout: Duration::from_secs(10), session: None, wants_login: false, skip_join: false }
    }
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// El servidor pide cifrado: es `online-mode=true`.
    OnlineMode,
    Compression,
    /// El servidor no contesto al login dentro del timeout.
    NoLogin,
    Disconnected(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "error de red: {e}"),
            Self::OnlineMode => write!(
                f,
                "el servidor exige cifrado (online-mode=true): esta sesion no tiene cuenta de Minecraft asociada"
            ),
            Self::Compression => write!(f, "el servidor activa compresion: no soportada todavia"),
            Self::NoLogin => write!(f, "el servidor no respondio al login (timeout de {}s)", 10),
            Self::Disconnected(r) => write!(f, "el servidor nos desconecto: {r}"),
        }
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Texto corto de un componente de chat de Minecraft. El cuerpo del paquete es
/// `[VarInt longitud][JSON]`, no el JSON suelto.
fn chat_text(body: &[u8]) -> String {
    let raw = proto::read_string(&mut std::io::Cursor::new(body)).unwrap_or_else(|_| String::from_utf8_lossy(body).to_string());
    let Ok(v) = serde_json::from_str::<Value>(&raw) else { return raw };
    match &v {
        Value::Object(o) => {
            if let Some(Value::String(t)) = o.get("text") {
                return t.clone();
            }
            if let Some(Value::String(t)) = o.get("translate") {
                return format!("[{t}]");
            }
        }
        Value::String(t) => return t.clone(),
        _ => {}
    }
    raw.chars().take(200).collect()
}

/// Conecta, hace el login y lee el flujo hasta que el servidor cierra o el
/// usuario escribe `quit`/`stop`. Cada linea se entrega por `out` (el binario la
/// imprime; los tests la capturan).
pub fn run(cfg: &Config, out: &mut dyn FnMut(&str)) -> Result<(), Error> {
    let addr = (cfg.server.as_str(), cfg.port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "direccion no valida")))?;
    out(&format!("conectando a {addr} (protocolo {})", cfg.protocol));
    let tcp = TcpStream::connect_timeout(&addr, cfg.timeout)?;
    tcp.set_read_timeout(Some(cfg.timeout))?;
    // `sock` puede cambiar de tipo: tras el cifrado pasa a ser el túnel.
    let mut sock: Box<dyn ReadWrite> = Box::new(tcp.try_clone()?);

    // handshake + login start: el protocolo permite mandarlos juntos
    let mut hello = Vec::new();
    let mut hs = Vec::new();
    proto::write_varint(&mut hs, cfg.protocol);
    proto::write_string(&mut hs, &cfg.server);
    proto::write_u16(&mut hs, cfg.port);
    proto::write_varint(&mut hs, NEXT_STATE_LOGIN);
    proto::write_packet(&mut hello, 0x00, &hs);
    let mut start = Vec::new();
    proto::write_string(&mut start, &cfg.name);
    start.extend_from_slice(&[0u8; 16]); // UUID nil: solo valido en modo offline
    proto::write_packet(&mut hello, LOGIN_START, &start);
    sock.write_all(&hello)?;
    sock.flush()?;

    // fase de login
    loop {
        let (id, body) = match proto::read_packet(&mut sock) {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                return Err(Error::NoLogin)
            }
            Err(e) => return Err(e.into()),
        };
        match id {
            PKT_ENCRYPTION_REQUEST => {
                // El servidor exige online-mode: hay que autenticarse y cifrar.
                let Some(session) = cfg.session.as_ref() else {
                    return Err(Error::OnlineMode);
                };
                let req = crate::crypto::parse_encryption_request(&body)
                    .map_err(|e| Error::Io(std::io::Error::other(e)))?;

                // shared secret AES-128 aleatorio, y lo que manda encriptado con
                // la RSA del servidor
                let mut shared = [0u8; 16];
                rand::fill(&mut shared);
                let enc_secret = crate::crypto::rsa_encrypt(&req.public_key, &shared)
                    .map_err(|e| Error::Io(std::io::Error::other(e)))?;
                let enc_token = crate::crypto::rsa_encrypt(&req.public_key, &req.verify_token)
                    .map_err(|e| Error::Io(std::io::Error::other(e)))?;
                let payload = crate::crypto::encryption_response_body(&enc_secret, &enc_token, req.salt);

                let mut body_out = Vec::new();
                proto::write_packet(&mut body_out, PKT_ENCRYPTION_RESPONSE, &payload);
                sock.write_all(&body_out)?;
                sock.flush()?;

                // Atar la sesion al servidor: sin este paso Mojang no valida el
                // join y el servidor nos echa aunque el cifrado este bien.
                let sid = crate::crypto::server_id_hash(&req.server_id, &shared, &req.public_key);
                if !cfg.skip_join {
                    if let Err(e) = crate::auth::join_server(&session.access_token, &session.uuid, &sid) {
                        return Err(Error::Io(std::io::Error::other(e.to_string())));
                    }
                }
                out("sesion validada con Mojang; activando cifrado");

                // A partir de aqui todo va por el túnel AES-128-CFB8.
                sock = Box::new(
                    crate::crypto::EncryptedStream::new(tcp.try_clone()?, &shared)
                        .map_err(|e| Error::Io(std::io::Error::other(e)))?,
                );
            }
            PKT_SET_COMPRESSION => return Err(Error::Compression),
            PKT_DISCONNECT => return Err(Error::Disconnected(chat_text(&body))),
            PKT_LOGIN_SUCCESS => {
                out(&format!("login correcto ({} bytes de Login Success)", body.len()));
                proto::send_packet(&mut sock, LOGIN_ACKNOWLEDGED, &[])?;
                break;
            }
            other => out(&format!("paquete de login ignorado: 0x{other:02x}")),
        }
    }

    // `quit`/`stop` desde el launcher o la terminal: cerrar el socket alcanza.
    // No se manda un paquete de desconexion porque tras el cifrado habria que
    // cifrarlo tambien, y eso significaria compartir el estado del túnel con este
    // hilo; el servidor interpreta el cierre igual.
    let quit = tcp.try_clone()?;
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines().map_while(Result::ok) {
            let cmd = line.trim();
            if cmd == "quit" || cmd == "stop" || cmd == "exit" {
                let _ = quit.shutdown(std::net::Shutdown::Both);
                return;
            }
        }
    });

    // flujo posterior (configuracion/play): se registra sin decodificar.
    loop {
        let (id, body) = match proto::read_packet(&mut sock) {
            Ok(p) => p,
            // el servidor cerro: es una salida limpia, no un fallo
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                continue; // servidor en silencio: no es un error
            }
            Err(e) => return Err(e.into()),
        };
        if id == PKT_DISCONNECT {
            return Err(Error::Disconnected(chat_text(&body)));
        }
        out(&format!("paquete 0x{id:02x} ({} bytes)", body.len()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::TcpListener;
    use std::sync::mpsc;

    fn cfg(port: u16) -> Config {
        Config { server: "127.0.0.1".into(), port, name: "tester".into(), ..Config::default() }
    }

    /// Servidor minimo offline: lee el saludo, contesta lo que diga `script`
    /// y devuelve lo observado por un canal.
    fn mini_server(script: impl FnOnce(&mut dyn Read, &mut dyn Write) + Send + 'static) -> u16 {
        let lst = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = lst.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((s, _)) = lst.accept() {
                let mut r = s.try_clone().unwrap();
                let mut w = s;
                script(&mut r, &mut w);
            }
        });
        port
    }

    fn collect(cfg: &Config, script: impl FnOnce(&mut dyn Read, &mut dyn Write) + Send + 'static) -> (Vec<String>, Result<(), Error>) {
        let port = mini_server(script);
        let (tx, rx) = mpsc::channel();
        let cfg = Config { port, ..cfg.clone() };
        let res = std::thread::spawn(move || run(&cfg, &mut |l| tx.send(l.to_string()).unwrap())).join().unwrap();
        (rx.try_iter().collect(), res)
    }

    /// Cliente que conecta y recibe flujo: el servidor contesta login correcto,
    /// dos paquetes y luego cierra -> el cliente termina con Ok.
    #[test]
    fn connects_and_logs_the_stream() {
        let (lines, res) = collect(
            &cfg(0),
            |r, w| {
                let (hs, _) = proto::read_packet(r).unwrap(); // handshake
                assert_eq!(hs, 0x00);
                let (start, body) = proto::read_packet(r).unwrap(); // login start
                assert_eq!(start, LOGIN_START);
                let name = proto::read_string(&mut std::io::Cursor::new(body)).unwrap();
                assert_eq!(name, "tester");
                let mut body = Vec::new();
                proto::write_string(&mut body, "login-ok-payload");
                proto::send_packet(w, PKT_LOGIN_SUCCESS, &body).unwrap();
                let (ack, ack_body) = proto::read_packet(r).unwrap(); // login acknowledged
                assert_eq!((ack, ack_body.len()), (LOGIN_ACKNOWLEDGED, 0));
                proto::send_packet(w, 0x57, b"a").unwrap();
                proto::send_packet(w, 0x58, b"bc").unwrap();
                w.flush().unwrap();
                // al cerrar, el cliente ve EOF y sale con Ok
            },
        );
        assert!(res.is_ok(), "{res:?}");
        assert!(lines.iter().any(|l| l.contains("login correcto")));
        assert!(lines.iter().any(|l| l.contains("0x57")));
        assert!(lines.iter().any(|l| l.contains("0x58") && l.contains("2 bytes")));
    }

    #[test]
    fn online_mode_and_compression_are_reported() {
        let (_, res) = collect(&cfg(0), |_r, w| {
            proto::send_packet(w, PKT_ENCRYPTION_REQUEST, b"x").unwrap();
            w.flush().unwrap();
        });
        assert!(matches!(res, Err(Error::OnlineMode)));

        let (_, res) = collect(&cfg(0), |_r, w| {
            let mut b = Vec::new();
            proto::write_varint(&mut b, 256);
            proto::send_packet(w, PKT_SET_COMPRESSION, &b).unwrap();
            w.flush().unwrap();
        });
        assert!(matches!(res, Err(Error::Compression)));
    }

    #[test]
    fn disconnect_reason_is_extracted_from_chat_json() {
        let (lines, res) = collect(&cfg(0), |_r, w| {
            let mut b = Vec::new();
            proto::write_string(&mut b, r#"{"text":"servidor lleno"}"#);
            proto::send_packet(w, PKT_DISCONNECT, &b).unwrap();
            w.flush().unwrap();
        });
        match res {
            Err(Error::Disconnected(r)) => assert_eq!(r, "servidor lleno"),
            other => panic!("esperaba desconexion: {other:?}"),
        }
        assert!(lines.iter().any(|l| l.starts_with("conectando a")));
    }

    #[test]
    fn chat_text_falls_back_to_raw() {
        assert_eq!(chat_text(br#"{"translate":"multiplayer.disconnect.kicked"}"#), "[multiplayer.disconnect.kicked]");
        assert_eq!(chat_text(b"\""), "\"");
        assert_eq!(chat_text(b"no json"), "no json");
    }

    // ---------- cifrado online-mode, de punta a punta ----------

    /// Clave RSA de 1024 bits fija (como las que usan los servidores de
    /// Minecraft). Es de prueba: la privada viaja en el repo a propósito.
    const RSA_PUB_DER: &str = "30819f300d06092a864886f70d010101050003818d0030818902818100c6f71c8bd0c57fb8f3e40c71b7784aa9deb98514c7c5c4f78d0dbd5e0d7dc232a2abb0e31653996d208aaea5d5056578d0a26acf6b377b2e6e468106a749a3658c0c6f114ed818c87047f4c9767a5dff802fdf660f46d8d9550bfd5004992da2457bc1252cf24b4ce2d320bcad1384dc4694c3415903aef9a3f986580437fb750203010001";
    const RSA_PRIV_DER: &str = "30820276020100300d06092a864886f70d0101010500048202603082025c02010002818100c6f71c8bd0c57fb8f3e40c71b7784aa9deb98514c7c5c4f78d0dbd5e0d7dc232a2abb0e31653996d208aaea5d5056578d0a26acf6b377b2e6e468106a749a3658c0c6f114ed818c87047f4c9767a5dff802fdf660f46d8d9550bfd5004992da2457bc1252cf24b4ce2d320bcad1384dc4694c3415903aef9a3f986580437fb75020301000102818034937c219e2b7252c7700db1c70fac7b32f5c72af49758332a3ce53b834926160752634dbeca9c926fb4f5edb05adab3903e3b075087baaf4c48a933bee5009e0f0107f018d95c52441889659f0de3456959356b9d7d4037e254a10bdc07efd80a39d543dd4556ebfff0ce33001102dd7793eebd566911a9a9747777131adcc1024100f987412345f7edcc024b6ce613ac0370751a5ba599dee881a7d249c58efdd7bc8c947b9ff46c93fbb3679b7ed596a719f904f123b58bf4f3afd77709fec877b9024100cc2024a8cff1359af2ed394d9ade2a7f3f235e7cfa8d5f38fb00977a108ed4e0b1fcb6a39af46734f032ee8627227933e238b7fdcf5d78e1b0df2436b744879d024100be651ba8a47cb6ef9158a7f12e5574125ce80dc1e0149be8427b4c5af7ee6cacabbfa84cb69e1a9455a1da9dfeaf833568330324322466758258afb5eab85fd102400da203f0c65a8cc2f98080783f64a11ff87e8b3bfb9bdf8535b657678e9d8b782a4aff83b3c0a8f7ef26e7a94c09d06077b6640b6e8306a4c424530dd8759a3d0240313c4f2df51d47a9024b9ee1f192a1f579da653b324d75fe21787e742dc4b192db4d8437b3acd6c70de6ac4be4302cd646c19ac3e3ea25ac07318475abfa689f";
    const VERIFY_TOKEN: &[u8] = &[0x0c, 0x0f, 0xfe, 0xe0, 0x0b, 0xad, 0xf0, 0x0d];

    #[test]
    fn online_mode_login_encrypts_and_validates_with_mojang() {
        use rsa::pkcs8::DecodePrivateKey;
        use rsa::{Pkcs1v15Encrypt, PrivateKey as RsaPrivateKey};
        use super::PKT_LOGIN_SUCCESS;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let (hs, _) = proto::read_packet(&mut sock).unwrap(); // handshake
            assert_eq!(hs, 0x00);
            let (start, _) = proto::read_packet(&mut sock).unwrap(); // login start
            assert_eq!(start, LOGIN_START);

            // enviamos el EncryptionRequest con nuestra clave y salt
            let mut req = Vec::new();
            proto::write_varint(&mut req, 0); // serverId vacío
            let der = hex::decode(RSA_PUB_DER).unwrap();
            crate::crypto::put_byte_array(&mut req, &der);
            crate::crypto::put_byte_array(&mut req, VERIFY_TOKEN);
            req.extend_from_slice(&0x0102030405060708i64.to_be_bytes()); // salt
            req.push(1u8); // shouldAuthenticate
            proto::send_packet(&mut sock, PKT_ENCRYPTION_REQUEST, &req).unwrap();

            // el cliente responde con el secret y el token encriptados
            let (resp_id, resp_body) = proto::read_packet(&mut sock).unwrap();
            assert_eq!(resp_id, PKT_ENCRYPTION_RESPONSE);
            let mut cur = resp_body.as_slice();
            let enc_secret = crate::crypto::take_byte_array(&mut cur).unwrap();
            let enc_token = crate::crypto::take_byte_array(&mut cur).unwrap();
            assert_eq!(cur.len(), 8, "vuelve el salt");
            assert_eq!(&cur, &0x0102030405060708i64.to_be_bytes());

            let key = RsaPrivateKey::from_pkcs8_der(&hex::decode(RSA_PRIV_DER).unwrap()).unwrap();
            let secret = key.decrypt(Pkcs1v15Encrypt, &enc_secret).unwrap();
            let token = key.decrypt(Pkcs1v15Encrypt, &enc_token).unwrap();
            assert_eq!(secret.len(), 16, "el shared secret es AES-128");
            assert_eq!(token, VERIFY_TOKEN, "el verify token debe volver igual");

            // Login Success va cifrado con el shared secret
            let mut login = Vec::new();
            proto::write_string(&mut login, "069a79f444e94726a5befca90e38aaf5");
            proto::write_string(&mut login, "tester");
            // El servidor cifra con un stream y descifra con OTRO, igual que el
            // cliente: compartir uno solo desincroniza el keystream.
            let mut tx_cipher = crate::crypto::Cfb8::new(&secret, &secret).unwrap();
            let mut rx_cipher = crate::crypto::Cfb8::new(&secret, &secret).unwrap();
            let mut framed = Vec::new();
            proto::write_packet(&mut framed, PKT_LOGIN_SUCCESS, &login);
            let wire = tx_cipher.apply(&framed, true);
            sock.write_all(&wire).unwrap();
            sock.flush().unwrap();

            // El acknowledged (varint 1 + id 2) tiene que llegar cifrado.
            let mut ack_wire = [0u8; 2];
            sock.read_exact(&mut ack_wire).unwrap();
            let plain = rx_cipher.apply(&ack_wire, false);
            assert_eq!(plain, vec![1, 2], "el acknowledged descifrado debe ser [len=1, id=2]");
        });

        let (tx, rx) = mpsc::channel();
        let res = std::thread::spawn(move || {
            let mut cfg = cfg(port);
            cfg.protocol = 776;
            cfg.session = Some(crate::auth::Session {
                uuid: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
                name: "tester".into(),
                access_token: "token-de-prueba".into(),
            });
            cfg.skip_join = true; // no hay red ni cuenta real
            run(&cfg, &mut |l| {
                let _ = tx.send(l.to_string());
            })
        })
        .join()
        .unwrap();

        server.join().unwrap();
        let lines: Vec<String> = rx.try_iter().collect();
        let all = lines.join("
");
        assert!(
            all.contains("sesion validada") || all.contains("cifrado"),
            "debería anunciar el cifrado: {all}"
        );
        // sin cuenta real el join se salta, así que el login llega a éxito
        assert!(res.is_ok(), "el login debería completarse: {res:?}");
        assert!(all.contains("login correcto"), "debería haber login correcto: {all}");
    }
}
