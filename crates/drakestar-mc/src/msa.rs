//! Login de Microsoft/Minecraft (device code + refresh token): Microsoft -> Xbox Live -> XSTS -> Minecraft Services.
//!
//! Necesita el client ID de una app de Azure PROPIA (cuentas personales, "public client flows" activado) aprobada por Mojang
//! (https://aka.ms/mce-reviewappid). El ID se toma de `DRAKESTAR_MSFT_CLIENT_ID`, de los ajustes o del valor por defecto de la compilacion.

use crate::{McError, McResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const DEVICE_CODE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";
const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    /// URL con el codigo ya puesto (si Microsoft la entrega): se abre directo en el navegador.
    pub verification_uri_complete: Option<String>,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsaTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

/// Cuenta de Minecraft lista para jugar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McProfile {
    pub name: String,
    /// UUID con guiones.
    pub uuid: String,
    pub access_token: String,
    pub xuid: Option<String>,
}

pub enum Poll {
    Pending(u64),
    Done(MsaTokens),
}

fn form(url: &str, params: &[(&str, &str)]) -> Result<Value, (u16, Value)> {
    let enc = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    let body = params.iter().map(|(k, v)| format!("{k}={}", enc(v))).collect::<Vec<_>>().join("&");
    match ureq::post(url).timeout(std::time::Duration::from_secs(30)).set("Content-Type", "application/x-www-form-urlencoded").send_string(&body) {
        Ok(r) => Ok(r.into_json().unwrap_or(Value::Null)),
        Err(ureq::Error::Status(code, r)) => Err((code, r.into_json().unwrap_or(Value::Null))),
        Err(e) => Err((0, json!({ "error": "network", "error_description": e.to_string() }))),
    }
}

fn post_json(url: &str, body: &Value) -> McResult<Value> {
    match ureq::post(url).timeout(std::time::Duration::from_secs(30)).set("Accept", "application/json").send_json(body) {
        Ok(r) => Ok(r.into_json().map_err(|e| McError::Json(e.to_string()))?),
        Err(ureq::Error::Status(code, r)) => {
            let v: Value = r.into_json().unwrap_or(Value::Null);
            Err(McError::Invalid(describe_http(url, code, &v)))
        }
        Err(e) => Err(McError::Net(e.to_string())),
    }
}

fn describe_http(url: &str, code: u16, v: &Value) -> String {
    if url == MC_LOGIN && code == 403 {
        return "Mojang aun no aprobo la app de Azure de este launcher (\"Invalid app registration\"). Registra tu app y pide la aprobacion en https://aka.ms/mce-reviewappid, o usa una cuenta sin premium mientras tanto.".into();
    }
    if let Some(x) = v["XErr"].as_u64() {
        return xerr(x);
    }
    format!("HTTP {code} de {url}: {}", v.to_string().chars().take(200).collect::<String>())
}

pub fn xerr(code: u64) -> String {
    match code {
        2148916233 => "Esta cuenta de Microsoft no tiene perfil de Xbox. Entra una vez a minecraft.net/login para crearlo.".into(),
        2148916235 => "Xbox Live no esta disponible en tu pais.".into(),
        2148916236 | 2148916237 => "La cuenta necesita verificacion de edad/adulto en Xbox.".into(),
        2148916238 => "Es una cuenta de menor de edad: un adulto debe agregarla a una familia de Microsoft.".into(),
        n => format!("Xbox Live rechazo el inicio de sesion (XErr {n})."),
    }
}

pub fn hyphens(id: &str) -> String {
    if id.len() == 32 {
        format!("{}-{}-{}-{}-{}", &id[0..8], &id[8..12], &id[12..16], &id[16..20], &id[20..32])
    } else {
        id.to_string()
    }
}

pub fn start(client_id: &str) -> McResult<DeviceCode> {
    let v = form(DEVICE_CODE, &[("client_id", client_id), ("scope", SCOPE)]).map_err(|(c, v)| {
        McError::Invalid(format!("Microsoft rechazo el client ID ({c}): {}", v["error_description"].as_str().unwrap_or("sin detalle")))
    })?;
    let user_code: String = v["user_code"].as_str().ok_or_else(|| McError::Json("sin user_code".into()))?.into();
    let verification_uri: String = v["verification_uri"].as_str().unwrap_or("https://www.microsoft.com/link").into();
    // Las cuentas personales no devuelven la URL completa: se intenta prellenar el codigo con `?otc=` (si la pagina lo ignora, el usuario lo escribe).
    let complete = v["verification_uri_complete"].as_str().map(String::from).or_else(|| verification_uri.starts_with("https://").then(|| format!("{verification_uri}?otc={user_code}")));
    Ok(DeviceCode {
        device_code: v["device_code"].as_str().ok_or_else(|| McError::Json("sin device_code".into()))?.into(),
        user_code,
        verification_uri,
        verification_uri_complete: complete,
        interval: v["interval"].as_u64().unwrap_or(5).max(5),
        expires_in: v["expires_in"].as_u64().unwrap_or(900),
    })
}

fn tokens(v: &Value) -> McResult<MsaTokens> {
    Ok(MsaTokens {
        access_token: v["access_token"].as_str().ok_or_else(|| McError::Json("sin access_token".into()))?.into(),
        refresh_token: v["refresh_token"].as_str().unwrap_or("").into(),
        expires_in: v["expires_in"].as_u64().unwrap_or(3600),
    })
}

pub fn poll(client_id: &str, device_code: &str) -> McResult<Poll> {
    match form(
        TOKEN,
        &[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("device_code", device_code), ("client_id", client_id)],
    ) {
        Ok(v) if v.get("access_token").is_some() => Ok(Poll::Done(tokens(&v)?)),
        Ok(_) => Ok(Poll::Pending(5)),
        Err((_, v)) => match v["error"].as_str().unwrap_or("") {
            "authorization_pending" => Ok(Poll::Pending(v["interval"].as_u64().unwrap_or(5).max(5))),
            "slow_down" => Ok(Poll::Pending(v["interval"].as_u64().unwrap_or(10).max(10))),
            "authorization_declined" => Err(McError::Invalid("rechazaste el acceso en Microsoft".into())),
            "expired_token" | "bad_verification_code" => Err(McError::Invalid("el codigo caduco: vuelve a iniciar el login".into())),
            other => Err(McError::Invalid(format!("Microsoft: {other} {}", v["error_description"].as_str().unwrap_or("")))),
        },
    }
}

pub fn refresh(client_id: &str, refresh_token: &str) -> McResult<MsaTokens> {
    let v = form(TOKEN, &[("grant_type", "refresh_token"), ("refresh_token", refresh_token), ("client_id", client_id), ("scope", SCOPE)])
        .map_err(|(_, v)| McError::Invalid(format!("la sesion de Microsoft caduco, vuelve a iniciar sesion ({})", v["error"].as_str().unwrap_or("?"))))?;
    let mut t = tokens(&v)?;
    if t.refresh_token.is_empty() {
        t.refresh_token = refresh_token.into();
    }
    Ok(t)
}

fn xbl_token(ms_access: &str) -> McResult<(String, String)> {
    // los tokens de la API v2 de Microsoft llevan el prefijo "d=" como RpsTicket; si falla se reintenta sin el
    let mut last = None;
    for ticket in [format!("d={ms_access}"), ms_access.to_string()] {
        match post_json(
            XBL,
            &json!({ "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": ticket }, "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT" }),
        ) {
            Ok(v) => {
                let t = v["Token"].as_str().ok_or_else(|| McError::Json("sin token de Xbox Live".into()))?.to_string();
                let uhs = v["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or_else(|| McError::Json("sin uhs".into()))?.to_string();
                return Ok((t, uhs));
            }
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap())
}

/// Microsoft access token -> perfil de Minecraft (nombre, uuid y token de Minecraft Services).
pub fn minecraft_profile(ms_access: &str) -> McResult<McProfile> {
    let (xbl, uhs) = xbl_token(ms_access)?;
    let xsts = post_json(XSTS, &json!({ "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl] }, "RelyingParty": "rp://api.minecraftservices.com/", "TokenType": "JWT" }))?;
    let xsts_token = xsts["Token"].as_str().ok_or_else(|| McError::Json("sin token XSTS".into()))?;
    let xuid = xsts["DisplayClaims"]["xui"][0]["xid"].as_str().map(String::from);
    let mc = post_json(MC_LOGIN, &json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") }))?;
    let access = mc["access_token"].as_str().ok_or_else(|| McError::Json("sin token de Minecraft".into()))?.to_string();
    let prof = match ureq::get(MC_PROFILE).timeout(std::time::Duration::from_secs(30)).set("Authorization", &format!("Bearer {access}")).call() {
        Ok(r) => r.into_json::<Value>().map_err(|e| McError::Json(e.to_string()))?,
        Err(ureq::Error::Status(404, _)) => return Err(McError::Invalid("esta cuenta no tiene Minecraft: Java Edition".into())),
        Err(e) => return Err(McError::Net(e.to_string())),
    };
    let id = prof["id"].as_str().ok_or_else(|| McError::Invalid("esta cuenta no tiene Minecraft: Java Edition".into()))?;
    let name = prof["name"].as_str().ok_or_else(|| McError::Invalid("la cuenta no tiene nombre de jugador".into()))?;
    Ok(McProfile { name: name.into(), uuid: hyphens(id), access_token: access, xuid })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xerr_messages_are_friendly() {
        assert!(xerr(2148916233).contains("perfil de Xbox"));
        assert!(xerr(2148916238).contains("menor"));
        assert!(xerr(1).contains("XErr 1"));
    }

    #[test]
    fn invalid_app_registration_is_explained() {
        let m = describe_http(MC_LOGIN, 403, &json!({}));
        assert!(m.contains("aprobo"));
    }

    #[test]
    fn hyphens_formats_ids() {
        assert_eq!(hyphens("069a79f444e94726a5befca90e38aaf5"), "069a79f4-44e9-4726-a5be-fca90e38aaf5");
    }
}
