//! Primitivas del protocolo (varint, cadenas, frames). Sin I/O propio: solo
//! codifican/decodifican sobre buffers, asi se prueban con vectores fijos.
//!
//! Los tamaños y formatos son los del protocolo de Minecraft (linea 1.20.5+ /
//! 26.x): VarInt de complemento a dos, cadenas UTF-8 con longitud VarInt y
//! paquetes enmarcados como `[VarInt longitud][VarInt id][cuerpo]`.

/// Longitud maxima de un paquete: el protocolo usa 21 bits (2 MiB).
pub const MAX_PACKET_BYTES: usize = 2 << 20;

/// Escribe un VarInt (entero de tamano variable, 7 bits por byte, little-endian).
pub fn write_varint(buf: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        // el ultimo byte no lleva el bit de continuacion
        if v & 0xFFFFFF80 == 0 {
            buf.push(v as u8);
            return;
        }
        buf.push((v & 0x7F) as u8 | 0x80);
        v >>= 7;
    }
}

/// Bytes que ocupa `value` codificado.
pub fn varint_len(value: i32) -> usize {
    let mut v = value as u32;
    let mut n = 1;
    while v & 0xFFFFFF80 != 0 {
        v >>= 7;
        n += 1;
    }
    n
}

/// Lee un VarInt. Error si se pasa de 5 bytes (seria basura, no un varint).
pub fn read_varint(r: &mut (impl std::io::Read + ?Sized)) -> std::io::Result<i32> {
    let mut result: u32 = 0;
    let mut b = [0u8; 1];
    for i in 0..5 {
        r.read_exact(&mut b)?;
        result |= ((b[0] & 0x7F) as u32) << (7 * i);
        if b[0] & 0x80 == 0 {
            return Ok(result as i32);
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "varint de mas de 5 bytes"))
}

/// Cadena: VarInt de longitud en bytes UTF-8 + los bytes.
pub fn write_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    write_varint(buf, bytes.len() as i32);
    buf.extend_from_slice(bytes);
}

pub fn read_string(r: &mut impl std::io::Read) -> std::io::Result<String> {
    let len = read_varint(r)?;
    if len < 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "longitud negativa"));
    }
    let mut bytes = vec![0u8; len as usize];
    r.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn write_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_be_bytes());
}

pub fn read_u16(r: &mut impl std::io::Read) -> std::io::Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_be_bytes(b))
}

pub fn write_i64(buf: &mut Vec<u8>, v: i64) {
    buf.extend_from_slice(&v.to_be_bytes());
}

/// Marco completo: `[longitud][id][cuerpo]`.
pub fn write_packet(buf: &mut Vec<u8>, id: i32, body: &[u8]) {
    let mut inner = Vec::with_capacity(varint_len(id) + body.len());
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    write_varint(buf, inner.len() as i32);
    buf.extend_from_slice(&inner);
}

/// Escribe un marco comprimido según el umbral `threshold`.
/// Si `threshold < 0`, se comporta como paquete sin compresión.
/// Si `threshold >= 0`:
/// - Si el paquete sin comprimir (`id` + `body`) < threshold: `Data Length = 0` y payload sin comprimir.
/// - Si el paquete sin comprimir >= threshold: `Data Length = longitud sin comprimir` y payload comprimido con zlib.
pub fn write_packet_compressed(buf: &mut Vec<u8>, id: i32, body: &[u8], threshold: i32) -> std::io::Result<()> {
    if threshold < 0 {
        write_packet(buf, id, body);
        return Ok(());
    }

    let uncompressed_len = varint_len(id) + body.len();
    let mut payload = Vec::with_capacity(uncompressed_len);
    write_varint(&mut payload, id);
    payload.extend_from_slice(body);

    if (uncompressed_len as i32) < threshold {
        // No comprimido: Data Length = 0
        let mut inner = Vec::with_capacity(varint_len(0) + payload.len());
        write_varint(&mut inner, 0);
        inner.extend_from_slice(&payload);
        write_varint(buf, inner.len() as i32);
        buf.extend_from_slice(&inner);
    } else {
        // Comprimido con zlib
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&payload)?;
        let compressed_bytes = encoder.finish()?;

        let mut inner = Vec::with_capacity(varint_len(uncompressed_len as i32) + compressed_bytes.len());
        write_varint(&mut inner, uncompressed_len as i32);
        inner.extend_from_slice(&compressed_bytes);
        write_varint(buf, inner.len() as i32);
        buf.extend_from_slice(&inner);
    }
    Ok(())
}

/// Escribe un marco directo a un escritorio (socket, buffer, ...).
pub fn send_packet(w: &mut (impl std::io::Write + ?Sized), id: i32, body: &[u8]) -> std::io::Result<()> {
    let mut buf = Vec::new();
    write_packet(&mut buf, id, body);
    w.write_all(&buf)
}

pub fn send_packet_compressed(
    w: &mut (impl std::io::Write + ?Sized),
    id: i32,
    body: &[u8],
    threshold: i32,
) -> std::io::Result<()> {
    let mut buf = Vec::new();
    write_packet_compressed(&mut buf, id, body, threshold)?;
    w.write_all(&buf)
}

/// Lee un marco. Devuelve `(id, cuerpo)`; el cuerpo es propiedad del llamante.
/// Si el paquete se pasa de `MAX_PACKET_BYTES` se descarta el flujo: no se puede
/// seguir sincronizado con un servidor que manda marcos corruptos.
pub fn read_packet(r: &mut (impl std::io::Read + ?Sized)) -> std::io::Result<(i32, Vec<u8>)> {
    read_packet_compressed(r, -1)
}

/// Lee un marco con o sin compresión activa (`threshold >= 0`).
pub fn read_packet_compressed(
    r: &mut (impl std::io::Read + ?Sized),
    threshold: i32,
) -> std::io::Result<(i32, Vec<u8>)> {
    let len = read_varint(r)?;
    if len < 0 || len as usize > MAX_PACKET_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("paquete de {len} bytes: fuera de rango"),
        ));
    }
    let mut frame = vec![0u8; len as usize];
    r.read_exact(&mut frame)?;

    if threshold < 0 {
        let mut body = std::io::Cursor::new(frame);
        let id = read_varint(&mut body)?;
        let pos = body.position() as usize;
        let payload = body.into_inner().split_off(pos);
        return Ok((id, payload));
    }

    // Modo comprimido activo
    let mut cursor = std::io::Cursor::new(frame);
    let data_len = read_varint(&mut cursor)?;
    let pos = cursor.position() as usize;
    let raw = cursor.into_inner().split_off(pos);

    if data_len == 0 {
        // Paquete no comprimido (menor que el threshold)
        let mut body = std::io::Cursor::new(raw);
        let id = read_varint(&mut body)?;
        let p = body.position() as usize;
        let payload = body.into_inner().split_off(p);
        Ok((id, payload))
    } else {
        // Paquete comprimido con Zlib
        use flate2::read::ZlibDecoder;
        use std::io::Read;

        if data_len < 0 || (data_len as usize) > MAX_PACKET_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("data_len descomprimido invalido: {data_len}"),
            ));
        }

        let mut decoder = ZlibDecoder::new(&raw[..]);
        let mut decompressed = Vec::with_capacity(data_len as usize);
        decoder.read_to_end(&mut decompressed)?;

        let mut body = std::io::Cursor::new(decompressed);
        let id = read_varint(&mut body)?;
        let p = body.position() as usize;
        let payload = body.into_inner().split_off(p);
        Ok((id, payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_known_vectors() {
        // vectores clasicos del protocolo
        let cases: &[(i32, &[u8])] = &[
            (0, &[0x00]),
            (1, &[0x01]),
            (2, &[0x02]),
            (127, &[0x7f]),
            (128, &[0x80, 0x01]),
            (255, &[0xff, 0x01]),
            (300, &[0xac, 0x02]),
            (2147483647, &[0xff, 0xff, 0xff, 0xff, 0x07]), // 31 bits: 5 bytes
            (2097151, &[0xff, 0xff, 0x7f]),                // 21 bits: 3 bytes
            (-1, &[0xff, 0xff, 0xff, 0xff, 0x0f]),
            (-2147483648, &[0x80, 0x80, 0x80, 0x80, 0x08]),
        ];
        for (v, bytes) in cases {
            let mut buf = Vec::new();
            write_varint(&mut buf, *v);
            assert_eq!(buf, *bytes, "codificando {v}");
            assert_eq!(varint_len(*v), bytes.len());
            let mut cur = std::io::Cursor::new(bytes.to_vec());
            assert_eq!(read_varint(&mut cur).unwrap(), *v, "decodificando {v}");
        }
    }

    #[test]
    fn strings_are_length_prefixed_utf8() {
        let mut buf = Vec::new();
        write_string(&mut buf, "Hello");
        assert_eq!(buf, b"\x05Hello");
        let mut cur = std::io::Cursor::new(buf);
        assert_eq!(read_string(&mut cur).unwrap(), "Hello");
        // multi-byte: la longitud es de BYTES, no de caracteres
        let mut buf = Vec::new();
        write_string(&mut buf, "héllo");
        assert_eq!(buf.len(), 1 + "héllo".len());
        assert_eq!(read_string(&mut std::io::Cursor::new(buf)).unwrap(), "héllo");
    }

    #[test]
    fn packet_frame_roundtrip() {
        let mut buf = Vec::new();
        write_packet(&mut buf, 0x2a, b"abc");
        assert_eq!(buf, vec![0x04, 0x2a, 0x61, 0x62, 0x63]);
        let (id, body) = read_packet(&mut std::io::Cursor::new(buf)).unwrap();
        assert_eq!((id, body.as_slice()), (0x2a, b"abc".as_slice()));
        // dos paquetes seguidos en el mismo buffer
        let mut buf = Vec::new();
        write_packet(&mut buf, 0x00, b"");
        write_packet(&mut buf, 0x01, b"xy");
        let mut cur = std::io::Cursor::new(buf);
        assert_eq!(read_packet(&mut cur).unwrap(), (0, Vec::new()));
        assert_eq!(read_packet(&mut cur).unwrap(), (1, b"xy".to_vec()));
    }

    #[test]
    fn oversized_and_truncated_fail() {
        let mut bad = std::io::Cursor::new(vec![0xff, 0xff, 0xff, 0xff, 0x07]);
        assert!(read_packet(&mut bad).is_err());
        assert!(read_packet(&mut std::io::Cursor::new(vec![0x05, 0x2a])).is_err());
    }

    #[test]
    fn compressed_packet_roundtrips_above_and_below_threshold() {
        // 1) Pequeño (< threshold 256): no se comprime (Data Length = 0)
        let mut buf_small = Vec::new();
        write_packet_compressed(&mut buf_small, 0x05, b"hello", 256).unwrap();
        let (id, body) = read_packet_compressed(&mut std::io::Cursor::new(buf_small), 256).unwrap();
        assert_eq!(id, 0x05);
        assert_eq!(body, b"hello");

        // 2) Grande (>= threshold 256): se comprime con zlib
        let big_payload = vec![0x42u8; 500];
        let mut buf_big = Vec::new();
        write_packet_compressed(&mut buf_big, 0x07, &big_payload, 256).unwrap();
        // El frame comprimido debe ser sustancialmente menor a 500 bytes
        assert!(buf_big.len() < 100);

        let (id_big, body_big) = read_packet_compressed(&mut std::io::Cursor::new(buf_big), 256).unwrap();
        assert_eq!(id_big, 0x07);
        assert_eq!(body_big, big_payload);
    }
}
