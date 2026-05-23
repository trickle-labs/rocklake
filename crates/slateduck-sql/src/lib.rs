//! SlateDuck SQL — bounded SQL dispatcher for the pgwire sidecar.
//!
//! This crate will pattern-match on sqlparser AST nodes to dispatch
//! DuckLake catalog SQL into typed CatalogStore operations in v0.3.

pub mod gluesql_spike;
