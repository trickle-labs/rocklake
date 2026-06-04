//! rocklake-testkit: shared test utilities for RockLake integration tests.
//!
//! ## Modules
//! - `clock` — `DeterministicClock`: wraps `tokio::time::pause()` for
//!   fully deterministic time-dependent tests without wall-clock sleeps.
//! - `duckdb_harness` — `DuckDbHarness`: reference GROUP BY / join engine
//!   for SQL correctness assertions.
//! - `minio_harness` — `MinioHarness`: manages a MinIO container for
//!   object-store-backed integration tests (Tier 4+).
//! - `duckdb_container_harness` — `DuckDbContainerHarness`: manages a real
//!   DuckDB CLI container for live tutorial loops (Tier 5+).
//! - `catalog_harness` — `CatalogHarness`: lightweight catalog write/read
//!   helper for testing catalog round-trips without a full server.
//! - `pgwire_harness` — `PgWireHarness`: spins up a PG-Wire server on a
//!   random port for client compatibility tests (Tier 5+).
//! - `ivm` — `IvmWorkerHarness`: launches and supervises worker processes for
//!   incremental-view-maintenance integration tests.
//! - `gcs_emulator_harness` — `GcsEmulatorHarness`: manages a fake-gcs-server
//!   container for GCS-backed integration tests (requires `gcs-emulator` feature).
//! - `azure_emulator_harness` — `AzureEmulatorHarness`: manages an Azurite
//!   container for Azure Blob Storage-backed tests (requires `azure-emulator` feature).
//! - `backend_compat` — `catalog_backend_compat_test!` macro for generating
//!   a unified backend compatibility test suite.
//!
//! All timing tests in RockLake use `DeterministicClock` so that:
//! - Tests run in constant CI time regardless of hardware.
//! - Flaky sleep-based assertions are eliminated.

pub mod backend_compat;
pub mod catalog_harness;
pub mod clock;
#[cfg(feature = "minio-tests")]
pub mod duckdb_container_harness;
pub mod duckdb_harness;
pub mod pgwire_harness;
pub mod soak_harness;

#[cfg(feature = "azure-emulator")]
pub mod azure_emulator_harness;
#[cfg(feature = "gcs-emulator")]
pub mod gcs_emulator_harness;
#[cfg(feature = "minio-tests")]
pub mod ivm;
#[cfg(feature = "minio-tests")]
pub mod minio_harness;

pub use catalog_harness::CatalogHarness;
pub use clock::DeterministicClock;
#[cfg(feature = "minio-tests")]
pub use duckdb_container_harness::{
    DuckDbCommandOutput, DuckDbContainerError, DuckDbContainerHarness,
};
pub use duckdb_harness::DuckDbHarness;
pub use pgwire_harness::PgWireHarness;
pub use soak_harness::{SoakConfig, SoakHarness, SoakRunSummary};

#[cfg(feature = "azure-emulator")]
pub use azure_emulator_harness::AzureEmulatorHarness;
#[cfg(feature = "gcs-emulator")]
pub use gcs_emulator_harness::GcsEmulatorHarness;
#[cfg(feature = "minio-tests")]
pub use ivm::IvmWorkerHarness;
#[cfg(feature = "minio-tests")]
pub use minio_harness::MinioHarness;
