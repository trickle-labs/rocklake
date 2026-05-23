# Integration

SlateDuck integrates with several systems in the data ecosystem. This section covers how to connect SlateDuck with DuckDB (the primary client), Apache DataFusion (for Rust-native query execution), and other tools.

## Integration Points

- **[DuckDB](duckdb.md)** — Connecting DuckDB to SlateDuck via the PG-wire sidecar
- **[Native Extension](native-extension.md)** — Loading SlateDuck as a DuckDB extension (Strategy C)
- **[DataFusion](datafusion.md)** — Using SlateDuck as a DataFusion catalog provider
- **[DuckDB Compatibility](duckdb-compatibility.md)** — SQL compatibility matrix with DuckLake
- **[Custom Clients](custom-clients.md)** — Building your own client using the PG-wire protocol
- **[pg-tide-relay](pg-tide-relay.md)** — Relaying DuckLake traffic through existing PG infrastructure
