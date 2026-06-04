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

#[tokio::test]
async fn minio_pgwire_executes_queries_against_minio_catalog() {
    let catalog = CatalogHarness::on_minio(minio().await, "pgwire/version_state")
        .await
        .expect("catalog should open on MinIO");

    let mut writer = catalog.writer().await;
    writer
        .create_schema("analytics")
        .await
        .expect("create_schema should succeed");
    let snapshot = writer
        .create_snapshot(Some("minio-tests"), Some("seed"))
        .await
        .expect("create_snapshot should succeed");
    catalog.commit_writer(snapshot).await;

    let harness = PgWireHarness::start_with_catalog(catalog.store.clone())
        .await
        .expect("PG-Wire server should start");
    let client = harness.connect().await.expect("connect should succeed");

    let version = client
        .query_one("SELECT version()", &[])
        .await
        .expect("version() query should succeed");
    let version: String = version.get(0);
    assert!(
        version.contains("PostgreSQL"),
        "version() should look like a PostgreSQL server, got: {version}"
    );

    let current_schema = client
        .query_one("SELECT current_schema()", &[])
        .await
        .expect("current_schema() query should succeed");
    let current_schema: String = current_schema.get(0);
    assert_eq!(current_schema, "public");

    client
        .execute(
            "INSERT INTO ducklake_schema (schema_name) VALUES ($1)",
            &[&"reporting"],
        )
        .await
        .expect("INSERT INTO ducklake_schema should succeed over PG-Wire");

    let reader = catalog.reader_latest().await;
    let schemas = reader
        .list_schemas()
        .await
        .expect("list_schemas should succeed");
    assert!(schemas
        .iter()
        .any(|schema| schema.schema_name == "analytics"));
    assert!(schemas
        .iter()
        .any(|schema| schema.schema_name == "reporting"));
}
