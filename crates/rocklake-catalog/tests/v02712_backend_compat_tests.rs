//! v0.27.12 catalog backend compatibility tests.
//!
//! Tests the unified `catalog_backend_compat_test!` macro against:
//!  - In-memory backend (always runs in CI)
//!  - GCS emulator backend (requires `--features gcs-emulator` + Docker)
//!  - Azure Blob Storage emulator backend (requires `--features azure-emulator` + Docker)
//!  - MinIO backend (requires `--features minio-tests` + Docker)
//!
//! ## Running emulator tests
//!
//! ```sh
//! # GCS only
//! cargo test -p rocklake-catalog --test v02712_backend_compat_tests \
//!   --features gcs-emulator
//!
//! # Azure only
//! cargo test -p rocklake-catalog --test v02712_backend_compat_tests \
//!   --features azure-emulator
//!
//! # All emulators
//! cargo test -p rocklake-catalog --test v02712_backend_compat_tests \
//!   --features gcs-emulator,azure-emulator
//! ```

use rocklake_testkit::catalog_backend_compat_test;

// ── In-memory (always runs) ───────────────────────────────────────────────────

catalog_backend_compat_test!(
    inmem,
    std::sync::Arc::new(object_store::memory::InMemory::new())
);

// ── GCS emulator (requires --features gcs-emulator + Docker) ─────────────────

#[cfg(feature = "gcs-emulator")]
mod gcs_compat {
    use rocklake_testkit::catalog_backend_compat_test;
    use rocklake_testkit::GcsEmulatorHarness;
    use std::sync::OnceLock;

    static HARNESS: OnceLock<Result<GcsEmulatorHarness, String>> = OnceLock::new();

    /// Run the GCS emulator and return an `Arc<dyn ObjectStore>`.
    ///
    /// If Docker is unavailable, the test is skipped gracefully via panic with
    /// a descriptive message.
    async fn gcs_store() -> std::sync::Arc<dyn object_store::ObjectStore> {
        let harness = HARNESS
            .get_or_init(|| {
                // Spawn a new thread with its own runtime to avoid blocking the executor thread
                std::thread::spawn(|| {
                    tokio::runtime::Runtime::new()
                        .unwrap()
                        .block_on(async {
                            GcsEmulatorHarness::start().await
                                .map_err(|e| e.to_string())
                        })
                })
                .join()
                .unwrap_or_else(|_| Err("failed to initialize GCS harness thread".to_string()))
            })
            .as_ref()
            .map_err(|e| format!("GCS emulator unavailable (skipping emulator tests): {e}. \n                         Ensure Docker is installed and fake-gcs-server image is accessible."))
            .expect("failed to initialize GCS emulator");

        let bucket_name = format!("rocklake-test-{}", uuid::Uuid::new_v4());
        harness
            .create_bucket(&bucket_name)
            .await
            .expect("failed to create GCS emulator bucket");
        harness.object_store(&bucket_name)
    }

    catalog_backend_compat_test!(gcs, super::gcs_store().await);
}

// ── Azure emulator (requires --features azure-emulator + Docker) ──────────────

#[cfg(feature = "azure-emulator")]
mod azure_compat {
    use rocklake_testkit::catalog_backend_compat_test;
    use rocklake_testkit::AzureEmulatorHarness;
    use std::sync::OnceLock;

    static HARNESS: OnceLock<Result<AzureEmulatorHarness, String>> = OnceLock::new();

    /// Run the Azurite emulator and return an `Arc<dyn ObjectStore>`.
    ///
    /// If Docker is unavailable, the test panics with a descriptive message.
    async fn azure_store() -> std::sync::Arc<dyn object_store::ObjectStore> {
        let harness = HARNESS
            .get_or_init(|| {
                // Spawn a new thread with its own runtime to avoid blocking the executor thread
                std::thread::spawn(|| {
                    tokio::runtime::Runtime::new()
                        .unwrap()
                        .block_on(async {
                            AzureEmulatorHarness::start().await
                                .map_err(|e| e.to_string())
                        })
                })
                .join()
                .unwrap_or_else(|_| Err("failed to initialize Azure harness thread".to_string()))
            })
            .as_ref()
            .map_err(|e| format!("Azure emulator unavailable (skipping emulator tests): {e}. \n                         Ensure Docker is installed and Azurite image is accessible."))
            .expect("failed to initialize Azure emulator");

        // Container names in Azure must be lowercase, alphanumeric plus dash only
        let container_name = format!("rocklake-test-{}", uuid::Uuid::new_v4());
        harness
            .create_container(&container_name)
            .await
            .expect("failed to create Azure emulator container");
        harness.object_store(&container_name)
    }

    catalog_backend_compat_test!(azure, super::azure_store().await);
}
