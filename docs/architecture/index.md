# Architecture

This section provides a deep technical exploration of SlateDuck's internal architecture. It covers how the system is structured as a set of Rust crates, how keys are laid out in the LSM-tree, how the SQL dispatcher maps statements to catalog operations, how the PostgreSQL wire protocol integration works, how MVCC is implemented at the storage level, and how values are encoded for durability and forward compatibility.

Each page is self-contained but references related concepts. If you are new to SlateDuck, start with the [Overview](overview.md) for the big picture, then drill into specific areas that interest you.

## Architecture Pages

- **[Overview](overview.md)** — The system-level view: how clients connect, how data flows through the sidecar, how catalog state is persisted.

- **[Crate Structure](crate-structure.md)** — The Rust workspace layout. Seven crates with clear dependency boundaries and distinct responsibilities.

- **[Key Layout](key-layout.md)** — The binary encoding scheme that maps relational concepts into lexicographically-ordered byte keys.

- **[Value Encoding](value-encoding.md)** — Protobuf serialization wrapped in a versioned envelope with magic bytes for corruption detection.

- **[SQL Dispatcher](sql-dispatcher.md)** — How incoming SQL is classified into one of ~50 known statement kinds and dispatched to the appropriate catalog operation.

- **[PG-Wire Protocol](pg-wire-protocol.md)** — The PostgreSQL wire protocol implementation: connection handling, authentication, query execution, and type mapping.

- **[Transaction Model](transaction-model.md)** — How DuckDB's logical transactions map to batched SlateDB writes with snapshot isolation.

- **[MVCC Implementation](mvcc-implementation.md)** — The storage-level implementation of multi-version concurrency control, including visibility filtering and version resolution.
