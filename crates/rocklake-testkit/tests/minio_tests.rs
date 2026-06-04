#[cfg(feature = "minio-tests")]
mod minio_tests {
    use futures::future::join_all;
    use rocklake_testkit::{CatalogHarness, MinioHarness};

    static MINIO: tokio::sync::OnceCell<MinioHarness> = tokio::sync::OnceCell::const_new();

    async fn minio() -> &'static MinioHarness {
        MINIO
            .get_or_init(|| async {
                MinioHarness::start("rocklake-testkit")
                    .await
                    .expect("MinIO should start for testcontainers-based tests")
            })
            .await
    }

    #[tokio::test]
    async fn minio_harness_roundtrips_objects() {
        let harness = minio().await;
        let prefix = "testkit/minio_harness_roundtrips_objects";
        harness
            .put_object(&format!("{prefix}/hello.txt"), b"hello")
            .await
            .expect("put_object should succeed");

        let keys = harness
            .list_objects(prefix)
            .await
            .expect("list_objects should succeed");
        assert!(keys.iter().any(|key| key.ends_with("hello.txt")));

        harness
            .delete_object(&format!("{prefix}/hello.txt"))
            .await
            .expect("delete_object should succeed");
    }

    #[tokio::test]
    async fn catalog_harness_on_minio_roundtrips_schema() {
        let harness = minio().await;
        let catalog = CatalogHarness::on_minio(harness, "testkit/catalog_roundtrip")
            .await
            .expect("catalog should open on MinIO");

        let mut writer = catalog.writer().await;
        let schema_id = writer
            .create_schema("analytics")
            .await
            .expect("create_schema should succeed");
        let snapshot = writer
            .create_snapshot(Some("minio-tests"), Some("roundtrip"))
            .await
            .expect("create_snapshot should succeed");
        catalog.commit_writer(snapshot).await;

        let reader = catalog.reader_latest().await;
        let schemas = reader
            .list_schemas()
            .await
            .expect("list_schemas should succeed");
        assert!(schemas
            .iter()
            .any(|schema| schema.schema_name == "analytics"));

        let reopened = CatalogHarness::on_minio(harness, "testkit/catalog_roundtrip")
            .await
            .expect("reopen should succeed");
        let reader = reopened.reader_latest().await;
        let schemas = reader
            .list_schemas()
            .await
            .expect("list_schemas should succeed");
        assert!(schemas
            .iter()
            .any(|schema| schema.schema_name == "analytics"));
        let _ = schema_id;
    }

    #[tokio::test]
    async fn concurrent_catalog_opens_on_distinct_minio_prefixes() {
        let harness = minio().await;
        let opens = (0..5).map(|index| async move {
            CatalogHarness::on_minio(harness, &format!("testkit/concurrent_opens/{index}"))
                .await
                .expect("concurrent open should succeed")
        });

        let catalogs = join_all(opens).await;
        for catalog in catalogs {
            let reader = catalog.reader_latest().await;
            let _ = reader
                .list_schemas()
                .await
                .expect("list_schemas should succeed");
        }
    }
}
