//! Cliente Rust de DrakeStar (modo DrakesRust): conecta a un servidor, hace el
//! login en modo offline y lee el flujo de paquetes. Sin dependencia de Tauri:
//! el launcher (`drakestar-app`) solo descarga este binario y lo ejecuta.

pub mod auth;
pub mod client;
pub mod crypto;
pub mod proto;
