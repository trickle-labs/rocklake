# Embedded Client Library

RockLake ships a universal embedded client library that lets any language
ecosystem read and write the catalog without running a PG-wire sidecar.
DuckDB is a first-class consumer, but the library is intentionally
language-neutral.

## Deployment Options

| Option | Use Case |
|--------|----------|
| **Strategy B — PG-wire Sidecar** | DuckDB, psql, any Postgres-compatible client |
| **Embedded Client Library** *(this page)* | Rust, Python, Go, Node.js, any language with C FFI |

The embedded library exposes a stable C ABI (`rocklake.h`) that all language
bindings wrap.  See [docs/reference/c-api.md](../reference/c-api.md) for the
full function reference.

---

## Rust

The `rocklake-client` crate is the idiomatic Rust entry point.  It wraps the
`rocklake-catalog` internals with an async-first API.

### Dependency

```toml
[dependencies]
rocklake-client = "0.48"
```

### Async API

```rust
use rocklake_client::{CatalogClientBuilder, SnapshotRef};

#[tokio::main]
async fn main() {
    let client = CatalogClientBuilder::new("file:///path/to/catalog")
        .build()
        .await
        .unwrap();

    let snapshot = SnapshotRef::Latest;
    let schemas = client.list_schemas(snapshot).await.unwrap();

    for schema in &schemas {
        println!("schema: {}", schema.schema_name);
        let tables = client.list_tables(schema.schema_id, snapshot).await.unwrap();
        for table in &tables {
            let files = client.list_data_files(table.table_id, snapshot).await.unwrap();
            println!("  table {} → {} data files", table.table_name, files.len());
        }
    }

    client.close().await;
}
```

### Sync API

For contexts that cannot use async Rust (C extensions, Python GIL-holding code):

```rust
use rocklake_client::{CatalogClientSync, SnapshotRef};

let client = CatalogClientSync::open("file:///path/to/catalog").unwrap();
let schemas = client.list_schemas(SnapshotRef::Latest).unwrap();
println!("{} schemas", schemas.len());
client.close();
```

---

## Python

The Python binding is built from this repository with `maturin`. v0.51.1 does
not claim a PyPI publication.

### Build from source

```sh
cd bindings/python
pip install maturin
maturin develop
```

### Usage

```python
from rocklake import RockLakeCatalog

cat = RockLakeCatalog.open("/path/to/catalog")

schemas = cat.list_schemas_latest()

for schema in schemas:
    tables = cat.list_tables_latest(schema.schema_id)
    for table in tables:
        files = cat.list_data_files_latest(table.table_id)
        print(f"{table.table_name}: {len(files)} data files")

cat.close()
```

### Polars Integration

`list_data_files()` returns objects with a `.to_dict()` method compatible with
`polars.from_dicts()`:

```python
import polars as pl
from rocklake import RockLakeCatalog

cat = RockLakeCatalog.open("/path/to/catalog")
# Get data file list
files = cat.list_data_files_latest(table_id=1)

# Build a DataFrame of catalog metadata
meta_df = pl.from_dicts([f.to_dict() for f in files])

# Read actual Parquet data
parquet_df = pl.read_parquet([f.path for f in files])
print(parquet_df.head())

cat.close()
```

---

## Go

Build the binding from the repository's `bindings/go` module. v0.51.1 does not
claim a separately published Go module:

```sh
cd bindings/go
go test ./...
```

### Prerequisites

- A pre-built `librocklake_ffi.a` static library for your platform (distributed
  as a GitHub release asset) **or** a local Rust build (`cargo build -p rocklake-ffi`).
- `cgo` enabled (default).

### Usage

```go
package main

import (
    "fmt"
    "log"

    rocklake "github.com/trickle-labs/rocklake-go"
)

func main() {
    cat, err := rocklake.Open("/path/to/catalog")
    if err != nil {
        log.Fatal(err)
    }
    defer cat.Close()

    schemas, err := cat.ListSchemasLatest()
    if err != nil {
        log.Fatal(err)
    }

    for _, s := range schemas {
        fmt.Printf("schema: %s\n", s.SchemaName)
        tables, _ := cat.ListTablesLatest(s.SchemaID)
        for _, t := range tables {
            files, _ := cat.ListDataFilesLatest(t.TableID)
            fmt.Printf("  table %s → %d files\n", t.TableName, len(files))
        }
    }
}
```

---

## Node.js

Build the Node.js package from `bindings/nodejs`; v0.51.1 does not claim an
external npm publication.

```sh
cd bindings/nodejs
npm install
npm run build
```

### Usage

```js
const { Catalog } = require('@rocklake/client');

const cat = Catalog.open('/path/to/catalog');

const schemas = cat.listSchemasLatest();

for (const schema of schemas) {
    const tables = cat.listTablesLatest(schema.schemaId);
    for (const table of tables) {
        const files = cat.listDataFilesLatest(table.tableId);
        console.log(`${table.tableName}: ${files.length} data files`);
    }
}

cat.close();
```

TypeScript type declarations are included (`index.d.ts`).

---

## Non-DuckDB Engine Matrix

| Engine | Integration Path | Status |
|--------|-----------------|--------|
| **Polars** (Python) | `list_data_files()` → `polars.read_parquet()` | ✅ Validated |
| **DataFusion** (Rust) | `rocklake-client` → `list_data_files()` | ✅ Validated |
| **Spark** (PySpark) | Python bindings → `list_data_files()` → `spark.read.parquet()` | Documented |
| **Trino** | Python/Go bindings → `list_data_files()` → Trino catalog connector | Documented |

### Spark

```python
from rocklake import RockLakeCatalog
from pyspark.sql import SparkSession

cat = RockLakeCatalog.open("/path/to/catalog")
files = cat.list_data_files_latest(table_id=1)

spark = SparkSession.builder.getOrCreate()
df = spark.read.parquet(*[f.path for f in files])
df.show()

cat.close()
```

### Trino

For Trino and other JVM-based engines, use the Python or Go bindings to retrieve
the list of Parquet files and register them as external tables, or use the
PG-wire sidecar (Strategy B) which provides a standard PostgreSQL interface that
Trino can query via the `postgresql` connector.

---

## Object-Store URL Format

| Backend | Example URI |
|---------|-------------|
| Local filesystem | `file:///absolute/path` or bare path |
| Amazon S3 | `s3://bucket/prefix` |
| Google Cloud Storage | `gs://bucket/prefix` |
| Azure Blob Storage | `az://container/prefix` |

S3 / GCS / Azure credentials are resolved from environment variables following
the standard `object_store` crate conventions (AWS_ACCESS_KEY_ID, etc.).

---

## Versioning Policy

The C ABI (`ROCKLAKE_ABI_VERSION`) follows semver major bumps for
breaking changes.  Language binding packages follow the RockLake workspace
version.

When `ROCKLAKE_ABI_VERSION` changes, the old constant is kept as a deprecated
alias for one release cycle before removal.

---

## See Also

- [C API Reference](../reference/c-api.md)
- [Architecture: FFI Safety](../architecture/ffi-safety.md)
- [Native DuckDB Extension (unsupported)](native-extension.md)
