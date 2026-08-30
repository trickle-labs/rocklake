# rocklake-client

Idiomatic async Rust client for the [RockLake](https://github.com/trickle-labs/rocklake) catalog.

## Usage

```rust
use rocklake_client::{CatalogClient, CatalogClientBuilder, SnapshotRef};

#[tokio::main]
async fn main() {
    let client = CatalogClientBuilder::new("file:///path/to/catalog")
        .build()
        .await
        .unwrap();

    let schemas = client.list_schemas(SnapshotRef::Latest).await.unwrap();
    println!("schemas: {schemas:?}");

    client.close().await;
}
```

For synchronous contexts:

```rust
use rocklake_client::{CatalogClientSync, SnapshotRef};

let client = CatalogClientSync::open("file:///path/to/catalog").unwrap();
let schemas = client.list_schemas(SnapshotRef::Latest).unwrap();
client.close();
```

## License

Apache-2.0
