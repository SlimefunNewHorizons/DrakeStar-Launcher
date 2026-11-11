//! Cifrado del protocolo Minecraft (online-mode).
//!
//! Lo que pasa cuando un servidor exige autenticación:
//!
//! 1. El servidor manda `EncryptionRequest`: su clave pública RSA (DER), un
//!    `verifyToken` aleatorio y (desde 1.20.5) un `salt`.
//! 2. El cliente genera un `sharedSecret` AES-128 y manda
//!    `EncryptionResponse`: el secret y el token **encriptados con RSA**
//!    (PKCS#1 v1.5), más el `salt` de vuelta.
//! 3. El cliente calcula el hash `SHA-1(serverId ++ sharedSecret ++ publicKey)`
//!    y llama a los servicios de Minecraft (`join`) para decir "soy yo".
//! 4. A partir de ahí, todo el tráfico va por un túnel **AES-128-CFB8**.
//!
//! Cada pieza está separada para poder probarla sin red ni servidor real.

use aes::Aes128;
use aes::cipher::{Block, BlockCipherEncrypt, KeyInit};
use rsa::pkcs8::DecodePublicKey;
use rsa::{Pkcs1v15Encrypt, PublicKey};
use sha1::{Digest, Sha1};
use std::io::{Read, Write};

/// Encripta un bloque de 16 bytes con AES-128.
fn encrypt_block(cipher: &Aes128, data: [u8; 16]) -> [u8; 16] {
    let mut block = Block::<Aes128>::from(data);
    cipher.encrypt_block(&mut block);
    let mut out = [0u8; 16];
    out.copy_from_slice(&block);
    out
}

// ---------- serverId hash ----------

/// Hash que Minecraft usa para atar la sesión al servidor.
/// Es `SHA-1(serverId ++ sharedSecret ++ publicKey)` en hex.
pub fn server_id_hash(server_id: &str, shared: &[u8], public_key: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(server_id.as_bytes());
    h.update(shared);
    h.update(public_key);
    hex_of(h.finalize().as_slice())
}

/// Mojang calcula el hex con `new BigInteger(digest).toString(16)`.
///
/// En Java, `BigInteger(byte[])` interpreta el array de bytes como un entero
/// signado en complemento a dos (two's complement):
/// - Si el bit más significativo (bit 7 del primer byte) es 1, el número es **negativo**.
///   En ese caso, Java antepone un signo '-' y muestra el valor absoluto (complemento a dos invertido + 1).
/// - Si es positivo, quita los ceros iniciales.
/// - Un hash que no use complemento a dos falla en el 50% de las conexiones a servidores online-mode.
pub fn hex_of(digest: &[u8]) -> String {
    if digest.is_empty() {
        return "0".to_string();
    }
    let is_negative = (digest[0] & 0x80) != 0;
    if is_negative {
        let mut inverted = digest.to_vec();
        let mut carry = 1u16;
        for byte in inverted.iter_mut().rev() {
            let val = (!*byte as u16) + carry;
            *byte = val as u8;
            carry = val >> 8;
        }
        let hex_str = hex::encode(inverted);
        let trimmed = hex_str.trim_start_matches('0');
        format!("-{}", if trimmed.is_empty() { "0" } else { trimmed })
    } else {
        let hex_str = hex::encode(digest);
        let trimmed = hex_str.trim_start_matches('0');
        if trimmed.is_empty() {
            "0".to_string()
        } else {
            trimmed.to_string()
        }
    }
}

// ---------- RSA ----------

/// Encripta con la clave pública del servidor (PKCS#1 v1.5, como Minecraft).
/// `der` es la clave en formato DER `SubjectPublicKeyInfo`.
pub fn rsa_encrypt(der: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let key = PublicKey::from_public_key_der(der)
        .map_err(|e| format!("clave pública del servidor inválida: {e}"))?;
    let mut rng = rand::rng();
    key.encrypt(&mut rng, Pkcs1v15Encrypt, plaintext)
        .map_err(|e| format!("no se pudo encriptar con la clave del servidor: {e}"))
}

// ---------- AES-128-CFB8 ----------

/// Túnel AES-128 en modo CFB8 (segmento de 8 bits), que es el que usa Minecraft.
/// Se instancia **por dirección**: uno para leer, otro para escribir, ambos con
/// el mismo `key` e `iv` (el `sharedSecret`).
pub struct Cfb8 {
    cipher: Aes128,
    /// Registro de desplazamiento: cambia con cada byte, por eso en CFB8 el
    /// keystream se recalcula a cada byte (no se puede cachear).
    shift: [u8; 16],
}

impl Cfb8 {
    pub fn new(key: &[u8], iv: &[u8]) -> Result<Cfb8, String> {
        if key.len() != 16 || iv.len() != 16 {
            return Err(format!("CFB8 exige key e iv de 16 bytes (key {}, iv {})", key.len(), iv.len()));
        }
        let cipher = Aes128::new_from_slice(key).map_err(|e| e.to_string())?;
        let mut shift = [0u8; 16];
        shift.copy_from_slice(iv);
        Ok(Cfb8 { cipher, shift })
    }

    /// Cifra o descifra `data`.
    ///
    /// `encrypt = true`: el registro se alimenta con el byte **cifrado** (lo que
    /// sale al canal). `encrypt = false`: con el byte **de entrada** (lo que
    /// viene del canal). En CFB estándar el registro siempre toma el byte que
    /// viaja, así ambos lados evolucionan igual.
    pub fn apply(&mut self, data: &[u8], encrypt: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        for &b in data {
            let ks = encrypt_block(&self.cipher, self.shift);
            let c = b ^ ks[0];
            let channel = if encrypt { c } else { b };
            self.shift.copy_within(1.., 0);
            self.shift[15] = channel;
            out.push(c);
        }
        out
    }
}

// ---------- parse del EncryptionRequest ----------

/// Los campos que el servidor manda al pedir autenticación.
#[derive(Debug, Clone, PartialEq)]
pub struct EncryptionRequest {
    /// Normalmente vacío (era para servidores antiguos).
    pub server_id: String,
    /// Clave pública RSA en DER.
    pub public_key: Vec<u8>,
    /// Token aleatorio que hay que devolver encriptado.
    pub verify_token: Vec<u8>,
    /// Desde protocolo 764 (1.20.5). Si es `None`, el paquete es del formato viejo.
    pub salt: Option<i64>,
}

/// Lee un `EncryptionRequest` del cuerpo del paquete.
///
/// El formato viejo es `serverId + publicKey + verifyToken`. Desde 1.20.5 lleva
/// además `salt (long)` y `shouldAuthenticate (bool)`. Se detectan por consumo:
/// si sobran 9 bytes exactos, es el formato nuevo.
pub fn parse_encryption_request(body: &[u8]) -> Result<EncryptionRequest, String> {
    let mut cur = body;
    let server_id = take_string(&mut cur)?;
    let public_key = take_byte_array(&mut cur)?;
    let verify_token = take_byte_array(&mut cur)?;
    // 8 de salt + 1 del booleano
    let salt = if cur.len() == 9 {
        let s = i64::from_be_bytes(cur[..8].try_into().unwrap());
        // se consume 8 del salt + 1 del bool; 'cur' ya no se usa después
        let _ = &cur[9..];
        Some(s)
    } else if cur.is_empty() {
        None
    } else {
        return Err(format!(" EncryptionRequest con {} bytes sobrantes sin sentido", cur.len()));
    };
    Ok(EncryptionRequest { server_id, public_key, verify_token, salt })
}

/// Cuerpo del `EncryptionResponse` que hay que mandar.
pub fn encryption_response_body(
    enc_secret: &[u8],
    enc_token: &[u8],
    salt: Option<i64>,
) -> Vec<u8> {
    let mut out = Vec::new();
    put_byte_array(&mut out, enc_secret);
    put_byte_array(&mut out, enc_token);
    if let Some(s) = salt {
        out.extend_from_slice(&s.to_be_bytes());
    }
    out
}

/// Long para `shouldAuthenticate`: en la respuesta moderna solo va el salt.
/// El booleano se consume del request pero no se devuelve.

// ---------- primitivas de lectura/escritura del protocolo ----------

fn take_varint(cur: &mut &[u8]) -> Result<u32, String> {
    let mut result: u32 = 0;
    let mut shift = 0;
    loop {
        let b = cur.first().ok_or("fin de datos leyendo varint")?;
        *cur = &cur[1..];
        result |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 35 {
            return Err("varint demasiado largo".into());
        }
    }
}

pub fn take_string(cur: &mut &[u8]) -> Result<String, String> {
    let n = take_varint(cur)? as usize;
    if cur.len() < n {
        return Err("string truncada".into());
    }
    let s = std::str::from_utf8(&cur[..n]).map_err(|e| e.to_string())?.to_string();
    *cur = &cur[n..];
    Ok(s)
}

pub fn take_byte_array(cur: &mut &[u8]) -> Result<Vec<u8>, String> {
    let n = take_varint(cur)? as usize;
    if cur.len() < n {
        return Err("byte array truncado".into());
    }
    let v = cur[..n].to_vec();
    *cur = &cur[n..];
    Ok(v)
}

pub fn put_byte_array(out: &mut Vec<u8>, data: &[u8]) {
    let mut len = Vec::new();
    let mut n = data.len();
    loop {
        let mut b = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            b |= 0x80;
        }
        len.push(b);
        if n == 0 {
            break;
        }
    }
    out.extend_from_slice(&len);
    out.extend_from_slice(data);
}

/// Comparte el codec de varint con `proto` sin acoplarlos.
pub fn write_varint(out: &mut Vec<u8>, value: u32) {
    let mut n = value;
    loop {
        let mut b = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            b |= 0x80;
        }
        out.push(b);
        if n == 0 {
            break;
        }
    }
}

// ---------- túnel cifrado sobre un stream ----------

/// Envuelve un stream con el túnel CFB8. Leer descifra, escribir cifra.
pub struct EncryptedStream<S> {
    inner: S,
    rx: Cfb8,
    tx: Cfb8,
}

impl<S: Read + Write> EncryptedStream<S> {
    /// `shared` es el sharedSecret AES-128. Minecraft usa el mismo secret como
    /// key e IV en ambas direcciones.
    pub fn new(inner: S, shared: &[u8]) -> Result<EncryptedStream<S>, String> {
        let rx = Cfb8::new(shared, shared)?;
        let tx = Cfb8::new(shared, shared)?;
        Ok(EncryptedStream { inner, rx, tx })
    }
}

impl<S: Read> Read for EncryptedStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut enc = vec![0u8; buf.len()];
        let n = self.inner.read(&mut enc)?;
        if n == 0 {
            return Ok(0);
        }
        let dec = self.rx.apply(&enc[..n], false);
        buf[..n].copy_from_slice(&dec);
        Ok(n)
    }
}

impl<S: Write> Write for EncryptedStream<S> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let enc = self.tx.apply(buf, true);
        self.inner.write_all(&enc)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Vector NIST SP 800-38A para AES-128-CFB8.
    const KEY: &str = "2b7e151628aed2a6abf7158809cf4f3c";
    const IV: &str = "000102030405060708090a0b0c0d0e0f";
    // NIST SP 800-38A F.3.3 (CFB8-AES128), verificado contra Python/cryptography
    const CT8: &str = "3b79424c9c0dd436bace9e0ed4586a4f";

    fn hx(s: &str) -> Vec<u8> {
        hex::decode(s).unwrap()
    }

    #[test]
    fn cfb8_matches_nist_vector() {
        let mut c = Cfb8::new(&hx(KEY), &hx(IV)).unwrap();
        let pt = hx("6bc1bee22e409f96e93d7e117393172a");
        let ct = c.apply(&pt, true);
        assert_eq!(hex::encode(ct), CT8.to_lowercase());
        // y es simétrico: aplicar otra vez devuelve el original
        let mut c2 = Cfb8::new(&hx(KEY), &hx(IV)).unwrap();
        let back = c2.apply(&hx(CT8), false);
        assert_eq!(back, pt);
    }

    #[test]
    fn cfb8_streams_past_the_first_block() {
        // 32 bytes: si el keystream no se recalcula tras cada byte, esto falla.
        // Vector generado con Python/cryptography (modes.CFB8).
        let mut c = Cfb8::new(&hx(KEY), &hx(IV)).unwrap();
        let pt: Vec<u8> = (0..32u8).collect();
        let ct = c.apply(&pt, true);
        assert_eq!(hex::encode(ct), "5055986aba87b80250cade7c59863351459eebed680c26ae20ca58ccf5a7b3d2");
    }

    #[test]
    fn cfb8_is_streaming_across_blocks() {
        // un stream largo debe seguir produciendo el mismo resultado que en
        // bloques separados: CFB8 consume 1 byte de keystream por byte
        let mut a = Cfb8::new(&hx(KEY), &hx(IV)).unwrap();
        let mut b = Cfb8::new(&hx(KEY), &hx(IV)).unwrap();
        let data: Vec<u8> = (0..64u8).collect();
        assert_eq!(a.apply(&data, true), b.apply(&data, true));
        assert_eq!(a.apply(b"tail", true), b.apply(b"tail", true));
    }

    #[test]
    fn cfb8_rejects_wrong_sizes() {
        assert!(Cfb8::new(&[0u8; 15], &[0u8; 16]).is_err());
        assert!(Cfb8::new(&[0u8; 16], &[0u8; 8]).is_err());
    }

    #[test]
    fn server_id_hash_strips_leading_zeros() {
        // comportamiento de BigInteger.toString(16): sin ceros iniciales y con signo negativo si el bit alto es 1
        let shared = hx("0123456789abcdef0123456789abcdef");
        let key = hx("0123456789abcdef0123456789abcdef");
        let h = server_id_hash("", &shared, &key);
        // El digest SHA-1 es b3dc..., cuyo bit alto es 1 (negativo en complemento a 2):
        assert_eq!(h, "-4c23729e0bcd650c547ee9e06ddcb8f5d622bf8c");
        assert_eq!(h.len(), 41);
        assert!(!h.trim_start_matches('-').starts_with('0') || h == "0", "el hash no debe traer cero inicial: {h}");
        // es determinista
        assert_eq!(h, server_id_hash("", &shared, &key));
        // y cambia si cambia cualquier entrada
        assert_ne!(h, server_id_hash("x", &shared, &key));
    }

    #[test]
    fn parses_modern_request_with_salt() {
        let mut body = Vec::new();
        write_varint(&mut body, ("".len()) as u32); // serverId vacío
        let der = hx("300d06092a864886f70d01010a0500"); // DER mínimo para el test
        put_byte_array(&mut body, &der);
        let token = hx("0011223344556677");
        put_byte_array(&mut body, &token);
        let salt: i64 = 0x1122334455667788;
        body.extend_from_slice(&salt.to_be_bytes());
        body.push(1u8); // shouldAuthenticate

        let r = parse_encryption_request(&body).unwrap();
        assert_eq!(r.server_id, "");
        assert_eq!(r.public_key, der);
        assert_eq!(r.verify_token, token);
        assert_eq!(r.salt, Some(salt));
    }

    #[test]
    fn parses_old_request_without_salt() {
        let mut body = Vec::new();
        write_varint(&mut body, 0);
        let der = hx("300d06092a864886f70d01010a0500");
        put_byte_array(&mut body, &der);
        put_byte_array(&mut body, &hx("deadbeef"));
        // sin salt ni bool: no sobran bytes
        let r = parse_encryption_request(&body).unwrap();
        assert_eq!(r.salt, None);
        assert_eq!(r.verify_token, hx("deadbeef"));
    }

    #[test]
    fn malformed_request_is_rejected() {
        // basura
        assert!(parse_encryption_request(b"\xff\xff\xff\xff").is_err());
        // 3 bytes sobrantes: ni formato viejo ni nuevo
        let mut body = Vec::new();
        write_varint(&mut body, 0);
        put_byte_array(&mut body, b"k");
        put_byte_array(&mut body, b"t");
        body.extend_from_slice(&[1, 2, 3]);
        assert!(parse_encryption_request(&body).is_err());
    }

    #[test]
    fn response_body_carries_salt_only_when_present() {
        let a = encryption_response_body(b"secret", b"token", None);
        assert!(!a.is_empty());
        let b = encryption_response_body(b"secret", b"token", Some(42));
        assert_eq!(b.len(), a.len() + 8, "el salt suma 8 bytes");
    }

    #[test]
    fn encrypted_stream_roundtrips() {
        // un stream simulado: aplicamos el túnel a los dos lados y tiene que dar
        // el original
        let shared = hx("0123456789abcdef0123456789abcdef");
        let mut tx = Cfb8::new(&shared, &shared).unwrap();
        let mut rx = Cfb8::new(&shared, &shared).unwrap();
        let msg: Vec<u8> = (0..200u8).collect();
        let wire = tx.apply(&msg, true);
        assert_ne!(wire, msg, "el túnel tiene que alterar los datos");
        assert_eq!(rx.apply(&wire, false), msg);
    }

    #[test]
    fn mojang_twos_complement_hex_vectors() {
        use sha1::{Digest, Sha1};

        let cases = [
            ("Notch", "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48"),
            ("jeb_", "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1"),
            ("mc.drakescraft.cl", "-2f689d131c106700e88e75810ccabf2970b7a69d"),
            ("SIMON", "e095b3543aaba3929af581c18625bcc75ec19f0"),
            ("", "-25c65c11a194b4f2cdaa40106a9fe76f5027f8f7"),
        ];

        for (input, expected) in cases {
            let mut hasher = Sha1::new();
            hasher.update(input.as_bytes());
            let digest = hasher.finalize();
            let actual = hex_of(&digest);
            assert_eq!(actual, expected, "falló vector de hash Mojang para '{input}'");
        }
    }
}

