//! Error types for SlateDuck.

use thiserror::Error;

/// Result type alias for SlateDuck operations.
pub type Result<T> = std::result::Result<T, SlateDuckError>;

/// Top-level error type for SlateDuck.
#[derive(Debug, Error)]
pub enum SlateDuckError {
    #[error("SlateDB error: {0}")]
    SlateDb(String),

    #[error("Object store error: {0}")]
    ObjectStore(#[from] object_store::Error),

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("Magic mismatch: expected SDKV, got {0:?}")]
    MagicMismatch(Vec<u8>),

    #[error("Unknown encoding version: {0}")]
    UnknownEncodingVersion(u8),

    #[error("Unknown table tag: 0x{0:02X}")]
    UnknownTag(u8),

    #[error("Catalog format version mismatch: expected {expected}, got {actual}")]
    FormatVersionMismatch { expected: u32, actual: u32 },

    #[error("Catalog not initialized")]
    CatalogNotInitialized,

    #[error("Writer fenced: another writer has taken over")]
    WriterFenced,

    #[error("Transaction conflict: {0}")]
    TransactionConflict(String),

    #[error("Value too large: {size} bytes exceeds limit of {limit} bytes")]
    ValueTooLarge { size: usize, limit: usize },

    #[error("Feature not supported: {0}")]
    FeatureNotSupported(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<slatedb::Error> for SlateDuckError {
    fn from(e: slatedb::Error) -> Self {
        SlateDuckError::SlateDb(e.to_string())
    }
}
