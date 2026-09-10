//! Encryption support: configuring SlateDB block transformers for at-rest encryption.
//!
//! Uses SlateDB's block-level encryption for catalog values.
//! Parquet encryption is a separate, Parquet-native concern.

#![allow(missing_docs)]

use async_trait::async_trait;
use bytes::Bytes;

/// Current on-disk block envelope version.
pub const ENCRYPTION_ENVELOPE_VERSION: u8 = 1;
const ENVELOPE_MAGIC: &[u8; 4] = b"RLK1";
const NONCE_LEN: usize = 12;
const MAX_KEY_ID_LEN: usize = 64;

/// A named AES-256 key in an encryption key ring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionKey {
    /// Non-secret identifier persisted in encrypted block envelopes.
    pub id: String,
    /// Raw AES-256 key material.
    pub key: [u8; 32],
}

/// Encryption configuration for the catalog store.
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// AES-256 encryption key (32 bytes).
    pub key: [u8; 32],
    /// Identifier written into new encrypted block envelopes.
    pub key_id: String,
    /// Older keys retained for reads during rotation.
    pub read_keys: Vec<EncryptionKey>,
}

impl EncryptionConfig {
    /// Create encryption config from a hex-encoded key string.
    pub fn from_hex(hex_key: &str) -> Result<Self, EncryptionError> {
        Self::from_hex_with_id("default", hex_key)
    }

    /// Create an encryption config from a hex key and non-secret key ID.
    pub fn from_hex_with_id(key_id: &str, hex_key: &str) -> Result<Self, EncryptionError> {
        let bytes = hex_decode(hex_key)?;
        if bytes.len() != 32 {
            return Err(EncryptionError::InvalidKeyLength {
                expected: 32,
                actual: bytes.len(),
            });
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Self::from_bytes_with_id(key_id, key)
    }

    /// Create encryption config from raw bytes.
    pub fn from_bytes(key: [u8; 32]) -> Self {
        Self::from_bytes_with_id("default", key).expect("static key ID is valid")
    }

    /// Create encryption config from raw bytes and a non-secret key ID.
    pub fn from_bytes_with_id(key_id: &str, key: [u8; 32]) -> Result<Self, EncryptionError> {
        validate_key_id(key_id)?;
        Ok(Self {
            key,
            key_id: key_id.to_string(),
            read_keys: Vec::new(),
        })
    }

    /// Return a copy configured to write with a different key ID.
    pub fn with_key_id(mut self, key_id: &str) -> Result<Self, EncryptionError> {
        validate_key_id(key_id)?;
        self.key_id = key_id.to_string();
        Ok(self)
    }

    /// Retain an older key for reading during rotation.
    pub fn with_read_key(mut self, key_id: &str, key: [u8; 32]) -> Result<Self, EncryptionError> {
        validate_key_id(key_id)?;
        if key_id == self.key_id || self.read_keys.iter().any(|entry| entry.id == key_id) {
            return Err(EncryptionError::DuplicateKeyId(key_id.to_string()));
        }
        self.read_keys.push(EncryptionKey {
            id: key_id.to_string(),
            key,
        });
        Ok(self)
    }
}

/// A SlateDB `BlockTransformer` that applies AES-256-GCM encryption.
///
/// Layout on disk: `[RLK1][version][key-id length][key ID][12-byte nonce][ciphertext + tag]`.
pub struct AesGcmTransformer {
    active: EncryptionKey,
    read_keys: Vec<EncryptionKey>,
}

impl AesGcmTransformer {
    pub fn new(config: &EncryptionConfig) -> Self {
        Self {
            active: EncryptionKey {
                id: config.key_id.clone(),
                key: config.key,
            },
            read_keys: config.read_keys.clone(),
        }
    }

    fn key(&self, id: &str) -> Option<&[u8; 32]> {
        if id == self.active.id {
            return Some(&self.active.key);
        }
        self.read_keys
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| &entry.key)
    }

    fn all_keys(&self) -> impl Iterator<Item = &[u8; 32]> {
        std::iter::once(&self.active.key).chain(self.read_keys.iter().map(|entry| &entry.key))
    }
}

#[async_trait]
impl slatedb::BlockTransformer for AesGcmTransformer {
    async fn encode(&self, data: Bytes) -> Result<Bytes, slatedb::Error> {
        use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
        use aes_gcm::Aes256Gcm;

        let cipher = Aes256Gcm::new((&self.active.key).into());
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let header = envelope_header(&self.active.id).map_err(slatedb::Error::data)?;
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: data.as_ref(),
                    aad: &header,
                },
            )
            .map_err(|_| slatedb::Error::data("AES-GCM encryption failed".to_string()))?;

        let mut out = Vec::with_capacity(header.len() + NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&header);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(Bytes::from(out))
    }

    async fn decode(&self, data: Bytes) -> Result<Bytes, slatedb::Error> {
        use aes_gcm::aead::{Aead, KeyInit, Payload};
        use aes_gcm::{Aes256Gcm, Nonce};

        if data.starts_with(ENVELOPE_MAGIC) {
            let envelope = parse_envelope(&data)?;
            let key = self.key(envelope.key_id).ok_or_else(|| {
                slatedb::Error::data(format!(
                    "encryption key ID is unavailable: {}",
                    envelope.key_id
                ))
            })?;
            let cipher = Aes256Gcm::new(key.into());
            let plaintext = cipher
                .decrypt(
                    Nonce::from_slice(envelope.nonce),
                    Payload {
                        msg: envelope.ciphertext,
                        aad: envelope.header,
                    },
                )
                .map_err(|_| slatedb::Error::data("AES-GCM decryption failed".to_string()))?;
            return Ok(Bytes::from(plaintext));
        }

        // v0.58 and earlier stored [nonce][ciphertext]. Keep that format
        // readable so rotation can be introduced without a rewrite-first step.
        if data.len() < NONCE_LEN {
            return Err(slatedb::Error::data(
                "encrypted block too short to contain nonce".to_string(),
            ));
        }
        let nonce = Nonce::from_slice(&data[..NONCE_LEN]);
        for key in self.all_keys() {
            let cipher = Aes256Gcm::new(key.into());
            if let Ok(plaintext) = cipher.decrypt(nonce, &data[NONCE_LEN..]) {
                return Ok(Bytes::from(plaintext));
            }
        }
        Err(slatedb::Error::data(
            "AES-GCM decryption failed (wrong key or corrupt block)".to_string(),
        ))
    }
}

/// Errors related to encryption configuration.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EncryptionError {
    #[error("invalid key length: expected {expected} bytes, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },
    #[error("invalid hex encoding: {0}")]
    InvalidHex(String),
    #[error("invalid encryption key ID: {0}")]
    InvalidKeyId(String),
    #[error("duplicate encryption key ID: {0}")]
    DuplicateKeyId(String),
}

fn validate_key_id(key_id: &str) -> Result<(), EncryptionError> {
    if key_id.is_empty()
        || key_id.len() > MAX_KEY_ID_LEN
        || !key_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
    {
        return Err(EncryptionError::InvalidKeyId(key_id.to_string()));
    }
    Ok(())
}

fn envelope_header(key_id: &str) -> Result<Vec<u8>, String> {
    validate_key_id(key_id).map_err(|error| error.to_string())?;
    let key_id = key_id.as_bytes();
    let mut header = Vec::with_capacity(6 + key_id.len());
    header.extend_from_slice(ENVELOPE_MAGIC);
    header.push(ENCRYPTION_ENVELOPE_VERSION);
    header.push(key_id.len() as u8);
    header.extend_from_slice(key_id);
    Ok(header)
}

struct ParsedEnvelope<'a> {
    header: &'a [u8],
    key_id: &'a str,
    nonce: &'a [u8],
    ciphertext: &'a [u8],
}

fn parse_envelope(data: &[u8]) -> Result<ParsedEnvelope<'_>, slatedb::Error> {
    if data.len() < 6 || &data[..4] != ENVELOPE_MAGIC {
        return Err(slatedb::Error::data(
            "invalid encrypted block envelope".to_string(),
        ));
    }
    if data[4] != ENCRYPTION_ENVELOPE_VERSION {
        return Err(slatedb::Error::data(format!(
            "unsupported encrypted block envelope version {}",
            data[4]
        )));
    }
    let key_id_len = data[5] as usize;
    let header_len = 6 + key_id_len;
    if data.len() < header_len + NONCE_LEN {
        return Err(slatedb::Error::data(
            "encrypted block envelope is truncated".to_string(),
        ));
    }
    let key_id = std::str::from_utf8(&data[6..header_len])
        .map_err(|_| slatedb::Error::data("encrypted block key ID is not UTF-8".to_string()))?;
    let nonce_end = header_len + NONCE_LEN;
    Ok(ParsedEnvelope {
        header: &data[..header_len],
        key_id,
        nonce: &data[header_len..nonce_end],
        ciphertext: &data[nonce_end..],
    })
}

/// Decode a hex string to bytes.
fn hex_decode(s: &str) -> Result<Vec<u8>, EncryptionError> {
    if !s.len().is_multiple_of(2) {
        return Err(EncryptionError::InvalidHex(
            "odd length hex string".to_string(),
        ));
    }
    let mut bytes = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        let byte = u8::from_str_radix(&s[i..i + 2], 16)
            .map_err(|e| EncryptionError::InvalidHex(e.to_string()))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_hex_valid() {
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let config = EncryptionConfig::from_hex(hex).unwrap();
        assert_eq!(config.key[0], 0x01);
        assert_eq!(config.key[31], 0xef);
    }

    #[test]
    fn test_from_hex_invalid_length() {
        let hex = "0123456789abcdef";
        let err = EncryptionConfig::from_hex(hex).unwrap_err();
        assert!(matches!(err, EncryptionError::InvalidKeyLength { .. }));
    }

    #[test]
    fn test_from_hex_invalid_chars() {
        let hex = "zz23456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let err = EncryptionConfig::from_hex(hex).unwrap_err();
        assert!(matches!(err, EncryptionError::InvalidHex(_)));
    }

    #[tokio::test]
    async fn versioned_envelope_round_trips_and_reads_rotated_key() {
        use slatedb::BlockTransformer;

        let old = EncryptionConfig::from_bytes_with_id("old", [1; 32]).unwrap();
        let transformer = AesGcmTransformer::new(&old);
        let encoded = transformer
            .encode(Bytes::from_static(b"catalog"))
            .await
            .unwrap();
        assert!(encoded.starts_with(ENVELOPE_MAGIC));

        let rotated = EncryptionConfig::from_bytes_with_id("new", [2; 32])
            .unwrap()
            .with_read_key("old", [1; 32])
            .unwrap();
        let rotated_transformer = AesGcmTransformer::new(&rotated);
        assert_eq!(
            rotated_transformer.decode(encoded).await.unwrap(),
            Bytes::from_static(b"catalog")
        );
        let new_encoded = rotated_transformer
            .encode(Bytes::from_static(b"new catalog"))
            .await
            .unwrap();
        assert_eq!(
            transformer
                .decode(new_encoded)
                .await
                .unwrap_err()
                .to_string(),
            "Data error: encryption key ID is unavailable: new"
        );
    }

    #[test]
    fn key_ids_are_validated_and_unique() {
        let config = EncryptionConfig::from_bytes_with_id("active", [0; 32]).unwrap();
        assert!(matches!(
            config.clone().with_read_key("bad key", [1; 32]),
            Err(EncryptionError::InvalidKeyId(_))
        ));
        assert!(matches!(
            config.with_read_key("active", [1; 32]),
            Err(EncryptionError::DuplicateKeyId(_))
        ));
    }
}
