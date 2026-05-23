//! SlateDuck Core — data model, key layout, and encoding for the SlateDuck catalog.

pub mod encoding;
pub mod error;
pub mod latency;
pub mod tags;
pub mod validation;

pub use error::{Result, SlateDuckError};
