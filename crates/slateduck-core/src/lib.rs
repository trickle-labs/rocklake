//! SlateDuck Core — data model, key layout, and encoding for the SlateDuck catalog.

pub mod encoding;
pub mod error;
pub mod keys;
pub mod latency;
pub mod mvcc;
pub mod path;
pub mod rows;
pub mod stats;
pub mod tags;
pub mod validation;

pub use error::{Result, SlateDuckError};
pub use keys::MAX_INLINED_ROW_SIZE;
pub use mvcc::{MvccFields, SnapshotId};
pub use path::{CatalogPath, DataPathMode};
pub use rows::CatalogRow;
