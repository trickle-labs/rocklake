# DataFusion Integration

SlateDuck provides a native integration with Apache DataFusion through the `slateduck-datafusion` crate. This allows Rust applications using DataFusion to use SlateDuck as their catalog provider, querying DuckLake catalogs directly without DuckDB in the picture.

## Architecture

The integration implements three DataFusion traits:

- **`CatalogProvider`** — Lists schemas in the SlateDuck catalog and provides access to them
- **`SchemaProvider`** — Lists tables within a schema and provides access to them
- **`TableProvider`** — Provides table metadata (schema, statistics) and creates scan plans for data files

When DataFusion plans a query against a SlateDuck-backed table, it calls the `TableProvider` to get the list of Parquet data files, then reads those files directly using DataFusion's Parquet reader.

## Usage

```rust
use slateduck_datafusion::SlateDuckCatalog;
use datafusion::prelude::*;

#[tokio::main]
async fn main() {
    let ctx = SessionContext::new();

    // Create a SlateDuck catalog provider
    let catalog = SlateDuckCatalog::open("s3://my-bucket/catalog/").await.unwrap();

    // Register it with DataFusion
    ctx.register_catalog("my_lake", Arc::new(catalog));

    // Query tables through DataFusion
    let df = ctx.sql("SELECT * FROM my_lake.analytics.events WHERE event_type = 'click'").await.unwrap();
    df.show().await.unwrap();
}
```

## What This Enables

**Rust-native analytics:** Build analytics applications in Rust that read DuckLake catalogs without depending on DuckDB or Python.

**Custom query engines:** If you are building a specialized query engine on top of DataFusion (e.g., a streaming analytics system), you can use SlateDuck to manage your table metadata.

**Testing and validation:** Use DataFusion to validate that SlateDuck's catalog is consistent — query the same catalog from both DuckDB and DataFusion and compare results.

## Supported Operations

The DataFusion integration is currently read-only. It supports:

- Listing schemas and tables
- Reading table schemas (column names, types, nullability)
- Providing data file locations for scan planning
- Reporting basic table statistics (row counts, file sizes)

Write operations (CREATE TABLE, INSERT, etc.) are not supported through the DataFusion interface. Use the PG-wire sidecar or CLI for catalog mutations.

## Dependencies

```toml
[dependencies]
slateduck-datafusion = "0.8"
datafusion = "45"
tokio = { version = "1", features = ["full"] }
```

The `slateduck-datafusion` crate transitively depends on `slateduck-catalog` and `slateduck-core`, bringing in the full catalog implementation. The DataFusion dependency is pinned to a specific major version (currently 45) because DataFusion's trait interfaces change between major versions.
