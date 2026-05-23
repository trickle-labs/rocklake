//! Value encoding/decoding with magic bytes and version prefix.

use crate::error::{Result, SlateDuckError};

/// Magic bytes prefix for all SlateDuck values.
pub const VALUE_MAGIC: &[u8; 4] = b"SDKV";

/// Current encoding version.
pub const ENCODING_VERSION: u8 = 1;

/// Encode a value with the standard SlateDuck prefix:
/// `encoding_version: u8 | magic: b"SDKV" | payload`
pub fn encode_value(payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(1 + VALUE_MAGIC.len() + payload.len());
    buf.push(ENCODING_VERSION);
    buf.extend_from_slice(VALUE_MAGIC);
    buf.extend_from_slice(payload);
    buf
}

/// Decode a value, verifying the magic bytes and version prefix.
/// Returns the payload bytes after the prefix.
pub fn decode_value(data: &[u8]) -> Result<&[u8]> {
    if data.is_empty() {
        return Err(SlateDuckError::Encoding("empty value".to_string()));
    }

    let version = data[0];
    if version != ENCODING_VERSION {
        return Err(SlateDuckError::UnknownEncodingVersion(version));
    }

    if data.len() < 1 + VALUE_MAGIC.len() {
        return Err(SlateDuckError::Encoding(
            "value too short for magic".to_string(),
        ));
    }

    let magic = &data[1..5];
    if magic != VALUE_MAGIC {
        return Err(SlateDuckError::MagicMismatch(magic.to_vec()));
    }

    Ok(&data[5..])
}

/// Encode a u64 counter value.
pub fn encode_counter(value: u64) -> Vec<u8> {
    encode_value(&value.to_be_bytes())
}

/// Decode a u64 counter value.
pub fn decode_counter(data: &[u8]) -> Result<u64> {
    let payload = decode_value(data)?;
    if payload.len() != 8 {
        return Err(SlateDuckError::Encoding(format!(
            "counter payload must be 8 bytes, got {}",
            payload.len()
        )));
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(payload);
    Ok(u64::from_be_bytes(bytes))
}

/// Encode a u32 value (used for format version).
pub fn encode_u32(value: u32) -> Vec<u8> {
    encode_value(&value.to_be_bytes())
}

/// Decode a u32 value.
pub fn decode_u32(data: &[u8]) -> Result<u32> {
    let payload = decode_value(data)?;
    if payload.len() != 4 {
        return Err(SlateDuckError::Encoding(format!(
            "u32 payload must be 4 bytes, got {}",
            payload.len()
        )));
    }
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(payload);
    Ok(u32::from_be_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_value() {
        let payload = b"hello world";
        let encoded = encode_value(payload);
        let decoded = decode_value(&encoded).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn roundtrip_counter() {
        for val in [0u64, 1, 42, u64::MAX] {
            let encoded = encode_counter(val);
            let decoded = decode_counter(&encoded).unwrap();
            assert_eq!(decoded, val);
        }
    }

    #[test]
    fn roundtrip_u32() {
        for val in [0u32, 1, 42, u32::MAX] {
            let encoded = encode_u32(val);
            let decoded = decode_u32(&encoded).unwrap();
            assert_eq!(decoded, val);
        }
    }

    #[test]
    fn bad_magic_rejected() {
        let mut data = encode_value(b"test");
        data[2] = b'X'; // corrupt magic
        assert!(matches!(
            decode_value(&data),
            Err(SlateDuckError::MagicMismatch(_))
        ));
    }

    #[test]
    fn unknown_version_rejected() {
        let mut data = encode_value(b"test");
        data[0] = 99; // unknown version
        assert!(matches!(
            decode_value(&data),
            Err(SlateDuckError::UnknownEncodingVersion(99))
        ));
    }

    #[test]
    fn empty_value_rejected() {
        assert!(matches!(
            decode_value(&[]),
            Err(SlateDuckError::Encoding(_))
        ));
    }
}
