//! Autenticación de Minecraft (online-mode).
//!
//! Sin esto no se puede entrar a un servidor con `online-mode=true`. La cadena
//! es la estándar que usa cualquier launcher:
//!
//! ```text
//! Microsoft (device code)  →  el usuario abre una URL y pone un código
//!        ↓ access_token
//! Xbox Live (RPS)          →  user.auth.xboxlive.com
//!        ↓ XBL token
//! XSTS (Xbox security)     →  relying party de Minecraft
//!        ↓ XSTS token
//! Minecraft Services        →  login_with_xbox
//!        ↓ access_token + perfil
//! sesión lista (uuid, nombre, token)
//! ```
//!
//! **Nada de tokens se guarda en disco.** La sesión vive en memoria durante la
//! conexión. Guardar un refresh token sería un secreto en el disco del usuario,
//! y esta app no tiene dónde ponerlo de forma segura todavía.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Cliente público de Minecraft en Microsoft.
///
/// Por defecto se usa el client_id de Prism Launcher verificado para flujos Device Code
/// en cuentas personales (/consumers/), permitiendo también override vía env `DRAKESTAR_MSFT_CLIENT_ID`.
pub const DEFAULT_MSFT_CLIENT_ID: &str = "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb";

pub fn msft_client_id() -> String {
    std::env::var("DRAKESTAR_MSFT_CLIENT_ID")
        .unwrap_or_else(|_| DEFAULT_MSFT_CLIENT_ID.to_string())
}

const MSFT_DEVICE_CODE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const MSFT_TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_AUTH: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";
const SESSION_JOIN: &str = "https://sessionserver.mojang.com/session/minecraft/join";

/// Sesión autenticada. El `access_token` es de Minecraft Services (no de
/// Microsoft) y solo sirve para el `/join` del servidor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// UUID con guiones, como lo espera el Login Start.
    pub uuid: String,
    pub name: String,
    pub access_token: String,
}

/// Información del código de dispositivo para iniciar sesión en Microsoft.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeAuth {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "payload")]
pub enum PollResult {
    Pending(u64),
    Success(Session),
}

#[derive(Debug)]
pub enum AuthError {
    /// El usuario todavía no puso el código. Lleva los segundos hasta el próximo
    /// sondeo (el servidor pide no insistir antes).
    Pending(u64),
    /// El usuario rechazó el acceso.
    Declined,
    /// El código caducó (15 minutos) o se agotó el tiempo de espera.
    Expired,
    /// La cuenta no tiene Minecraft Java Edition.
    NoGame,
    Network(String),
    Parse(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending(s) => write!(f, "esperando que autorices ({s}s)"),
            Self::Declined => write!(f, "autorizacion rechazada"),
            Self::Expired => write!(f, "el codigo caduco; empieza de nuevo"),
            Self::NoGame => write!(f, "esta cuenta no tiene Minecraft Java Edition"),
            Self::Network(e) => write!(f, "error de red: {e}"),
            Self::Parse(e) => write!(f, "respuesta inesperada del servicio: {e}"),
        }
    }
}
impl std::error::Error for AuthError {}

impl From<ureq::Error> for AuthError {
    fn from(e: ureq::Error) -> Self {
        Self::Network(e.to_string())
    }
}

impl From<std::io::Error> for AuthError {
    fn from(e: std::io::Error) -> Self {
        Self::Parse(format!("lectura de respuesta: {e}"))
    }
}

fn post_form(url: &str, params: &[(&str, &str)]) -> Result<Value, (AuthError, Option<Value>)> {
    let mut form_data = String::new();
    for (i, (k, v)) in params.iter().enumerate() {
        if i > 0 {
            form_data.push('&');
        }
        form_data.push_str(k);
        form_data.push('=');
        // Codificación básica de URL query string
        for b in v.bytes() {
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    form_data.push(b as char);
                }
                _ => {
                    use std::fmt::Write;
                    let _ = write!(form_data, "%{:02X}", b);
                }
            }
        }
    }

    match ureq::post(url)
        .timeout(std::time::Duration::from_secs(30))
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&form_data)
    {
        Ok(r) => r.into_json().map_err(|e| (AuthError::Parse(format!("lectura JSON: {e}")), None)),
        Err(ureq::Error::Status(_code, r)) => {
            // Intenta leer el body JSON con el error (ej: authorization_pending en 400 Bad Request)
            if let Ok(json_body) = r.into_json::<Value>() {
                Err((AuthError::Parse("error de estado".into()), Some(json_body)))
            } else {
                Err((AuthError::Network(format!("HTTP error")), None))
            }
        }
        Err(ureq::Error::Transport(e)) => Err((AuthError::Network(e.to_string()), None)),
    }
}

fn post(url: &str, body: &Value) -> Result<Value, AuthError> {
    let r = ureq::post(url)
        .timeout(std::time::Duration::from_secs(30))
        .send_json(body)?;
    Ok(r.into_json()?)
}

fn get(url: &str, token: &str) -> Result<Value, AuthError> {
    let r = ureq::get(url)
        .timeout(std::time::Duration::from_secs(30))
        .set("Authorization", &format!("Bearer {token}"))
        .call()?;
    Ok(r.into_json()?)
}

/// UUID sin guiones (como lo devuelve Mojang) a la forma con guiones.
pub fn hyphens(id: &str) -> String {
    match id.len() {
        32 => format!("{}-{}-{}-{}-{}", &id[0..8], &id[8..12], &id[12..16], &id[16..20], &id[20..32]),
        _ => id.to_string(),
    }
}

/// Solicita un nuevo código de dispositivo para login con Microsoft.
pub fn request_device_code() -> Result<DeviceCodeAuth, AuthError> {
    let client_id = msft_client_id();
    let dc = post_form(
        MSFT_DEVICE_CODE,
        &[
            ("client_id", &client_id),
            ("scope", "XboxLive.signin offline_access"),
        ],
    ).map_err(|(e, _)| e)?;

    let device_code = dc["device_code"].as_str().ok_or(AuthError::Parse("sin device_code".into()))?.to_string();
    let user_code = dc["user_code"].as_str().ok_or(AuthError::Parse("sin user_code".into()))?.to_string();
    let verification_uri = dc["verification_uri"].as_str().unwrap_or("https://microsoft.com/link").to_string();
    let interval = dc["interval"].as_u64().unwrap_or(5).max(5);
    let expires_in = dc["expires_in"].as_u64().unwrap_or(900);

    Ok(DeviceCodeAuth {
        device_code,
        user_code,
        verification_uri,
        interval,
        expires_in,
    })
}

/// Sondea el estado de autorización del código de dispositivo de Microsoft.
/// Devuelve `Ok(PollResult::Pending(secs))` si el usuario aún no autoriza,
/// o `Ok(PollResult::Success(session))` cuando se completa el ciclo completo.
pub fn poll_device_code(device_code: &str) -> Result<PollResult, AuthError> {
    let client_id = msft_client_id();
    let res = post_form(
        MSFT_TOKEN,
        &[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", device_code),
            ("client_id", &client_id),
        ],
    );

    match res {
        Ok(v) if v.get("access_token").is_some() => {
            let ms_token = v["access_token"].as_str().unwrap();
            let session = exchange_ms_token(ms_token)?;
            Ok(PollResult::Success(session))
        }
        Ok(v) => {
            let err = v["error"].as_str().unwrap_or("");
            let interval = v["interval"].as_u64().unwrap_or(5).max(5);
            match err {
                "authorization_pending" => Ok(PollResult::Pending(interval)),
                "authorization_declined" => Err(AuthError::Declined),
                "expired_token" => Err(AuthError::Expired),
                other => Err(AuthError::Parse(format!("estado inesperado '{other}'"))),
            }
        }
        Err((_e, Some(v))) => {
            let err = v["error"].as_str().unwrap_or("");
            let interval = v["interval"].as_u64().unwrap_or(5).max(5);
            match err {
                "authorization_pending" => Ok(PollResult::Pending(interval)),
                "authorization_declined" => Err(AuthError::Declined),
                "expired_token" => Err(AuthError::Expired),
                other => Err(AuthError::Parse(format!("estado oauth '{other}'"))),
            }
        }
        Err((e, None)) => Err(e),
    }
}

/// Intercambia un access_token de Microsoft por una sesión completa de Minecraft Services.
pub fn exchange_ms_token(ms_token: &str) -> Result<Session, AuthError> {
    // 1) Microsoft → Xbox Live
    let xbl = post(
        XBL_AUTH,
        &json!({
            "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": ms_token },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
    )?;
    let xbl_token = xbl["Token"].as_str().ok_or(AuthError::Parse("sin token de Xbox Live".into()))?.to_string();
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or(AuthError::Parse("sin uhs de Xbox Live".into()))?
        .to_string();

    // 2) Xbox Live → XSTS (el token que Minecraft acepta)
    let xsts = post(
        XSTS_AUTH,
        &json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )?;
    if let Some(err) = xsts["XErr"].as_u64() {
        return Err(AuthError::Parse(format!("XSTS rechazó (XErr {err})")));
    }
    let xsts_token = xsts["Token"].as_str().ok_or(AuthError::Parse("sin token XSTS".into()))?.to_string();

    // 3) XSTS → sesión de Minecraft
    let mc = post(
        MC_LOGIN,
        &json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") }),
    )?;
    let access_token = mc["access_token"].as_str().ok_or(AuthError::Parse("sin token de Minecraft".into()))?.to_string();

    // 4) perfil: uuid y nombre
    let prof = get(MC_PROFILE, &access_token)?;
    let raw_id = prof["id"].as_str().ok_or(AuthError::NoGame)?;
    let name = prof["name"].as_str().ok_or(AuthError::NoGame)?.to_string();

    Ok(Session { uuid: hyphens(raw_id), name, access_token })
}

/// Hace el login completo de forma síncrona/bloqueante (usado por CLI).
pub fn authenticate(out: &mut dyn FnMut(&str)) -> Result<Session, AuthError> {
    let dc = request_device_code()?;
    out(&format!("autoriza en {} con el codigo {}", dc.verification_uri, dc.user_code));

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(dc.expires_in);
    loop {
        if std::time::Instant::now() > deadline {
            return Err(AuthError::Expired);
        }
        match poll_device_code(&dc.device_code) {
            Ok(PollResult::Success(s)) => return Ok(s),
            Ok(PollResult::Pending(secs)) => {
                out(&format!("esperando autorizacion ({secs}s)"));
                std::thread::sleep(std::time::Duration::from_secs(secs));
            }
            Err(e) => {
                out(&format!("reintentando ({e})"));
                std::thread::sleep(std::time::Duration::from_secs(dc.interval));
            }
        }
    }
}

/// Le dice a los servicios de Minecraft "voy a entrar a este servidor". Sin
/// este paso el servidor rechaza la sesión aunque el cifrado esté bien.
/// `uuid` es el de `Session` (con guiones) y `server_id` el hash del handshake.
pub fn join_server(access_token: &str, uuid: &str, server_id: &str) -> Result<(), AuthError> {
    let body = json!({
        "accessToken": access_token,
        "selectedProfile": uuid.replace('-', ""),
        "serverId": server_id
    });
    // Mojang responde 204 sin cuerpo cuando acepta
    ureq::post(SESSION_JOIN)
        .timeout(std::time::Duration::from_secs(30))
        .send_json(&body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_hyphens_to_mojang_ids() {
        assert_eq!(
            hyphens("069a79f444e94726a5befca90e38aaf5"),
            "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        );
        // un UUID que ya tiene guiones queda igual
        assert_eq!(
            hyphens("069a79f4-44e9-4726-a5be-fca90e38aaf5"),
            "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        );
    }

    #[test]
    fn client_id_is_the_known_one() {
        assert_eq!(DEFAULT_MSFT_CLIENT_ID, "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb");
        assert_eq!(msft_client_id(), "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb");
    }
}
