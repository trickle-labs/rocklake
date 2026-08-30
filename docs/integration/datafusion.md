# DataFusion Integration

The `rocklake-datafusion` crate exposes a DataFusion `CatalogProvider` backed
by a RockLake `CatalogStore`. This is an embedded Rust integration for
applications that already use DataFusion; it is separate from the supported
DuckDB-over-PostgreSQL-wire path.

## Open a provider

`RockLakeCatalogProvider::open` takes an object store, a catalog prefix, and an
optional exact snapshot ID. With a local catalog:

```rust
use std::sync::Arc;

use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use rocklake_datafusion::RockLakeCatalogProvider;

let object_store = Arc::new(LocalFileSystem::new_with_prefix("./catalog")?);
let provider = RockLakeCatalogProvider::open(
    object_store,
    ObjectPath::from(""),
    None, // None reads the latest committed snapshot.
)
.await?;

let ctx = datafusion::prelude::SessionContext::new();
ctx.register_catalog("lake", Arc::new(provider));
let frame = ctx.sql("SELECT * FROM lake.analytics.events").await?;
frame.show().await?;
```

For an exact historical view, pass `Some(SnapshotId::new(id))` as the third
argument. `None` means latest; zero is never a latest sentinel:

```rust
use rocklake_core::mvcc::SnapshotId;

let provider = RockLakeCatalogProvider::open(
    object_store,
    ObjectPath::from(""),
    Some(SnapshotId::new(1000)),
)
.await?;
```

The provider resolves the Parquet data root from the catalog's `data_path`
metadata. Absolute registered data-file paths do not require that metadata.

## Reuse an existing catalog store

When the application already owns an `Arc<tokio::sync::RwLock<CatalogStore>>`,
reuse it instead of opening the catalog a second time:

```rust
use rocklake_datafusion::RockLakeCatalogProvider;

let provider = RockLakeCatalogProvider::from_catalog_store(store, None).await?;
```

The second argument is the same optional exact `SnapshotId`. A bounded async
bridge connects DataFusion's synchronous provider traits to catalog operations;
the default queue depth is 256 and can be changed with
`RockLakeCatalogProvider::new_with_queue_depth` when constructing from a
`CatalogStore`.

## Scope

- DataFusion 45 is the tested DataFusion version.
- The integration provides catalog metadata and Parquet scan planning; it is
  not a general SQL server.
- Register the matching data object store with DataFusion when data files use a
  remote provider.
- Use the C ABI or language bindings for non-Rust embedded clients.
