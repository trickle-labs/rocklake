# Native Extension (Strategy C)

The SlateDuck native extension loads directly into DuckDB's process as a shared library. This eliminates network overhead entirely — catalog operations are in-process function calls with microsecond latency. This is Strategy C in SlateDuck's deployment model.

## How It Works

The extension is built as a C-compatible shared library using the `slateduck-ffi` crate. It exports functions matching DuckDB's extension ABI (version 5000):

- `slateduck_init` — Initialize the extension, register functions
- `slateduck_open` — Open a catalog at a given storage path
- `slateduck_close` — Close a previously opened catalog
- `slateduck_list_schemas` — Return all schemas in the catalog
- `slateduck_describe_table` — Return columns for a specific table

DuckDB loads the extension at runtime and calls these functions directly, bypassing all network layers.

## Building

```bash
cd crates/slateduck-ffi
cargo build --release
```

The output is a shared library:
- Linux: `target/release/libslateduck_ffi.so`
- macOS: `target/release/libslateduck_ffi.dylib`
- Windows: `target/release/slateduck_ffi.dll`

## Loading in DuckDB

```sql
-- Load the extension
LOAD '/path/to/libslateduck_ffi.so';

-- Open a catalog
SELECT slateduck_open('s3://my-bucket/catalog/');

-- Use the catalog
SELECT * FROM slateduck_list_schemas();
SELECT * FROM slateduck_describe_table('public', 'events');
```

## When to Use Strategy C

Strategy C is appropriate when:

- **Latency is critical:** Sub-millisecond catalog operations for interactive workloads
- **Deployment simplicity:** Single process (DuckDB + catalog) rather than two processes
- **Resource efficiency:** No network serialization overhead, no separate memory footprint for the sidecar

Strategy C is NOT appropriate when:

- **Multiple DuckDB instances share a catalog:** Each would need its own extension instance, complicating writer coordination
- **Independent upgrades:** You want to upgrade SlateDuck without touching DuckDB
- **Process isolation:** A bug in the catalog code could crash DuckDB
- **Debugging:** In-process debugging is harder than observing a separate sidecar

## Limitations

The native extension currently exposes a subset of SlateDuck's capabilities. Operations like GC, excision, export, and repair are not available through the FFI — they require the CLI tool or PG-wire interface. The extension focuses on the hot-path catalog operations that DuckDB needs for query planning and execution.

## ABI Stability

The extension targets DuckDB's extension ABI version 5000. When DuckDB releases a new ABI version, the extension must be recompiled and potentially updated to match the new interface. This is the primary maintenance cost of Strategy C compared to Strategy B (which uses the stable PostgreSQL wire protocol).
