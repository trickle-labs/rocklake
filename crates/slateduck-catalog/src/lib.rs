//! SlateDuck Catalog — DuckLake catalog operations backed by SlateDB.
//!
//! This crate implements the full set of DuckLake catalog operations
//! as typed Rust methods:
//! - `CatalogStore`: top-level handle, manages DB lifecycle
//! - `CatalogReader`: point-in-time reads at a DuckLake snapshot
//! - `CatalogWriter`: transactional writes with MVCC, counter allocation, schema_version

mod reader;
mod store;
mod verify;
mod writer;

pub use reader::CatalogReader;
pub use store::{CatalogStore, OpenOptions};
pub use verify::{verify_catalog, VerifyResult};
pub use writer::CatalogWriter;
