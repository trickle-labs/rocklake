#![cfg(feature = "minio-tests")]

use rocklake_testkit::{CatalogHarness, MinioHarness, PgWireHarness};

static MINIO: tokio::sync::OnceCell<MinioHarness> = tokio::sync::OnceCell::const_new();

async fn minio() -> &'static MinioHarness {
    MINIO
        .get_or_init(|| async {
            MinioHarness::start("rocklake-pgwire-tests")
                .await
                .expect("MinIO should start for PG-Wire E2E tests")
        })
        .await
}

async fn pgwire(prefix: &str) -> PgWireHarness {
    let catalog = CatalogHarness::on_minio(minio().await, prefix)
        .await
        .expect("catalog should open on MinIO");
    let harness = PgWireHarness::start_with_catalog(catalog.store.clone())
        .await
        .expect("PG-Wire server should start");
    drop(catalog);
    harness
}

#[tokio::test]
async fn minio_pgwire_accepts_connections_against_minio_catalog() {
    let harness = pgwire("pgwire/version_state").await;
    let client = harness.connect().await.expect("connect should succeed");
    drop(client);
}
