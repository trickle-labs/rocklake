//! slateduck-testkit: shared test utilities for SlateDuck integration tests.
//!
//! ## Modules
//! - `clock` — `DeterministicClock`: wraps `tokio::time::pause()` for
//!   fully deterministic time-dependent tests without wall-clock sleeps.
//! - `harness` — `IvmWorkerHarness`: drives `IvmWorker` in-process with
//!   helper methods for waiting on lag and asserting output counts.
//! - `duckdb_harness` — `DuckDbHarness`: reference GROUP BY / join engine
//!   for IVM correctness assertions (v0.13).
//! - `oracle` — `IvmOracle`: correctness oracle that compares incremental
//!   IVM output against a batch-recompute reference (v0.14+).
//! - `minio_harness` — `MinioHarness`: manages a MinIO container for
//!   object-store-backed integration tests (Tier 4+).
//! - `catalog_harness` — `CatalogHarness`: lightweight catalog write/read
//!   helper for testing catalog round-trips without a full worker.
//! - `pgwire_harness` — `PgWireHarness`: spins up a PG-Wire server on a
//!   random port for client compatibility tests (Tier 5+).
//!
//! All timing tests in SlateDuck use `DeterministicClock` so that:
//! - Tests run in constant CI time regardless of hardware.
//! - Flaky sleep-based assertions are eliminated.

pub mod catalog_harness;
pub mod clock;
pub mod duckdb_harness;
pub mod harness;
pub mod minio_harness;
pub mod oracle;
pub mod pgwire_harness;

pub use catalog_harness::CatalogHarness;
pub use clock::DeterministicClock;
pub use duckdb_harness::DuckDbHarness;
pub use harness::IvmWorkerHarness;
pub use minio_harness::MinioHarness;
pub use oracle::IvmOracle;
pub use pgwire_harness::PgWireHarness;
