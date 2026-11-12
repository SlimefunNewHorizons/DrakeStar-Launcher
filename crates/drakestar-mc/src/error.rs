use std::fmt;

#[derive(Debug)]
pub enum McError {
    Net(String),
    Io(std::io::Error),
    Json(String),
    NotFound(String),
    Invalid(String),
    Cancelled,
}

pub type McResult<T> = Result<T, McError>;

impl fmt::Display for McError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Net(e) => write!(f, "error de red: {e}"),
            Self::Io(e) => write!(f, "error de disco: {e}"),
            Self::Json(e) => write!(f, "JSON invalido: {e}"),
            Self::NotFound(e) => write!(f, "no encontrado: {e}"),
            Self::Invalid(e) => write!(f, "{e}"),
            Self::Cancelled => write!(f, "cancelado"),
        }
    }
}

impl std::error::Error for McError {}

impl From<std::io::Error> for McError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<drakestar_net::NetError> for McError {
    fn from(e: drakestar_net::NetError) -> Self {
        Self::Net(e.to_string())
    }
}
impl From<serde_json::Error> for McError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e.to_string())
    }
}
impl From<zip::result::ZipError> for McError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Invalid(format!("zip: {e}"))
    }
}
