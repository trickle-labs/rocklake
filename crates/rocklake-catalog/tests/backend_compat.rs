//! v0.36.0 — Catalog backend compatibility tests.
//!
//! Wires the shared `catalog_backend_compat_test!` macro against:
//!  - In-memory backend (always runs in CI)
//!  - LocalFS backend (always runs in CI)
//!  - GCS emulator backend (requires `--features gcs-emulator` + Docker)
//!  - Azure Blob Storage emulator backend (requires `--features azure-emulator` + Docker)
//!  - MinIO backend (requires `--features minio-tests` + Docker)
//!
//! This file is the canonical `crates/rocklake-catalog/tests/backend_compat.rs`
//! entry point referenced in the v0.36.0 roadmap.
//!
//! ## Running emulator tests
//!
//! ```sh
//! # In-memory + LocalFS (default CI)
//! cargo test -p rocklake-catalog --test backend_compat
//!
//! # GCS emulator (requires Docker)
//! cargo test -p rocklake-catalog --test backend_compat --features gcs-emulator
//!
//! # Azure emulator (requires Docker)
//! cargo test -p rocklake-catalog --test backend_compat --features azure-emulator

//! # MinIO backend (requires Docker + Testcontainers)
//! cargo test -p rocklake-catalog --test backend_compat --features minio-tests
//!
//! # All emulators
//! cargo test -p rocklake-catalog --test backend_compat \
//!   --features gcs-emulator,azure-emulator
//! ```

use rocklake_testkit::catalog_backend_compat_test;

// ── In-memory (always runs) ────────────────────────────────────────────────

catalog_backend_compat_test!(
    inmem,
    std::sync::Arc::new(object_store::memory::InMemory::new())
);

// ── LocalFS (always runs) ─────────────────────────────────────────────────
//
// Uses a temporary directory on the local filesystem.  This covers the
// real I/O path that development and single-host deployments use.

catalog_backend_compat_test!(localfs, {
    let tmp = tempfile::TempDir::new().expect("localfs: tempdir failed");
    // Leak the TempDir so it is not cleaned up while the tests run.
    let path = tmp.keep();
    std::sync::Arc::new(
        object_store::local::LocalFileSystem::new_with_prefix(&path)
            .expect("localfs: LocalFileSystem::new_with_prefix failed"),
    )
});

// ── GCS emulator (requires --features gcs-emulator + Docker) ──────────────

#[cfg(feature = "gcs-emulator")]
mod gcs_compat {
    use rocklake_testkit::catalog_backend_compat_test;
    use rocklake_testkit::GcsEmulatorHarness;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    static HARNESS: Mutex<Option<Arc<GcsEmulatorHarness>>> = Mutex::const_new(None);

    async fn gcs_store() -> std::sync::Arc<dyn object_store::ObjectStore> {
        let mut harness_lock = HARNESS.lock().await;

        if harness_lock.is_none() {
            let harness = GcsEmulatorHarness::start()
                .await
                .map_err(|e| format!("GCS emulator unavailable (requires Docker + fake-gcs-server): {e}. \n                         Run: docker pull fsouza/fake-gcs-server:latest"))
                .expect("failed to initialize GCS emulator");
            *harness_lock = Some(Arc::new(harness));
        }

        let harness = harness_lock
            .as_ref()
            .expect("harness should be initialized");
        let bucket_name = format!("rocklake-test-{}", uuid::Uuid::new_v4());
        harness.create_bucket(&bucket_name).await.ok();
        harness.object_store(&bucket_name)
    }

    catalog_backend_compat_test!(gcs, super::gcs_store().await);
}

// ── Azure Blob Storage emulator (requires --features azure-emulator + Docker) ─

#[cfg(feature = "azure-emulator")]
mod azure_compat {
    use rocklake_testkit::catalog_backend_compat_test;
    use rocklake_testkit::AzureEmulatorHarness;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    static HARNESS: Mutex<Option<Arc<AzureEmulatorHarness>>> = Mutex::const_new(None);

    async fn azure_store() -> std::sync::Arc<dyn object_store::ObjectStore> {
        let mut harness_lock = HARNESS.lock().await;

        if harness_lock.is_none() {
            let harness = AzureEmulatorHarness::start()
                .await
                .map_err(|e| format!("Azure emulator unavailable (requires Docker + Azurite): {e}. \n                         Run: docker pull mcr.microsoft.com/azure-storage/azurite:latest"))
                .expect("failed to initialize Azure emulator");
            *harness_lock = Some(Arc::new(harness));
        }

        let harness = harness_lock
            .as_ref()
            .expect("harness should be initialized");
        let container_name = format!("rocklake-test-{}", uuid::Uuid::new_v4());
        harness.create_container(&container_name).await.ok();
        harness.object_store(&container_name)
    }

    catalog_backend_compat_test!(azure, super::azure_store().await);
}

// ── MinIO backend (requires --features minio-tests + Docker + Testcontainers) ─

#[cfg(feature = "minio-tests")]
mod minio_compat {
    use rocklake_testkit::catalog_backend_compat_test;
    use rocklake_testkit::MinioHarness;

    static HARNESS: tokio::sync::OnceCell<MinioHarness> = tokio::sync::OnceCell::const_new();

    async fn minio_store() -> std::sync::Arc<dyn object_store::ObjectStore> {
        let harness = HARNESS
            .get_or_init(|| async {
                match MinioHarness::start("rocklake-test").await {
                    Ok(h) => h,
                    Err(e) => {
                        panic!(
                            "MinIO harness unavailable (requires Docker + Testcontainers): {e}. \
                         Run: cargo test -p rocklake-catalog --test backend_compat --features minio-tests"
                        );
                    }
                }
            })
            .await;

        harness.object_store()
    }

    catalog_backend_compat_test!(minio, super::minio_store().await);
}
