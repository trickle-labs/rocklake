# Glossary

This page defines terms used throughout the SlateDuck documentation. Terms are listed alphabetically.

---

**Bounded SQL** — SlateDuck's approach to SQL support: only a finite, enumerated set of SQL statement patterns are recognized. Statements outside this set are rejected.

**Catalog** — The metadata store that records what schemas, tables, columns, and data files exist in a lakehouse. SlateDuck is a catalog implementation.

**DuckLake** — DuckDB's lakehouse extension that manages data as Parquet files in object storage with metadata in a catalog backend. SlateDuck is one such backend.

**Epoch** — A monotonically increasing counter that identifies the current writer. When a new writer starts, it increments the epoch, fencing (invalidating) the previous writer.

**Excision** — The physical deletion of superseded catalog rows. The second phase of garbage collection. Irreversible.

**Fencing** — The mechanism by which a new writer invalidates an old writer. The old writer's next operation detects the epoch change and refuses to proceed, preventing split-brain writes.

**Garbage Collection (GC)** — The process of advancing the retention horizon to make old snapshots inaccessible, and optionally excising the physical rows of superseded entries.

**Hot Key** — A cached system key that is read on nearly every operation. Caching it avoids a storage round-trip for the most common access pattern.

**Immutability** — The property that catalog rows, once written, are never modified in place. Updates create new rows; old rows are superseded but not deleted (until excision).

**Key** — The binary identifier for a catalog entry in SlateDB. Composed of a tag byte followed by big-endian u64 components.

**MVCC (Multi-Version Concurrency Control)** — The mechanism that allows multiple versions of the same entity to coexist, with readers seeing only the version appropriate to their snapshot.

**Object Storage** — Cloud storage services (S3, GCS, Azure Blob Storage) that provide durable, scalable storage accessible via HTTP. SlateDuck's persistence layer.

**PG-Wire** — The PostgreSQL wire protocol (frontend/backend message format). SlateDuck implements this protocol for compatibility with DuckDB's ducklake extension.

**Retain From** — The snapshot ID below which time travel queries are rejected. Set by garbage collection. Determines what history is still accessible.

**SDKV** — The four-byte magic signature in SlateDuck's value envelope. Stands for "SlateDuck Key-Value." Used for corruption detection.

**Snapshot** — An atomic point-in-time view of the catalog. Each write transaction creates a new snapshot with a unique, monotonically increasing ID.

**SlateDB** — The Rust LSM-tree key-value store that SlateDuck uses for persistence. Writes directly to object storage.

**SST (Sorted String Table)** — An immutable, sorted file in the LSM-tree. Contains key-value pairs in key order. Written by SlateDB during compaction.

**Strategy B** — Deployment as a PG-wire sidecar process. DuckDB connects over TCP.

**Strategy C** — Deployment as a native DuckDB extension (shared library). Catalog operations are in-process function calls.

**Tag** — The first byte of every key, identifying the table type (entity kind) the key belongs to.

**Time Travel** — The ability to query the catalog at any historical snapshot, seeing the state as it existed at that point in time.

**Value Envelope** — The 5-byte wrapper around protobuf-encoded row data: 1 byte format version + 4 bytes "SDKV" magic.

**WAL (Write-Ahead Log)** — SlateDB's mechanism for durably recording writes before they are compacted into SSTs. Each WAL segment is an atomic PUT to object storage.

**Wire Corpus** — A collection of recorded SQL statements from actual DuckDB sessions, used as test fixtures for the SQL classifier.

**Writer** — The single process authorized to create new snapshots (modify the catalog). Identity established by holding the highest epoch.

**Write Batch** — A set of key-value operations (puts and deletes) that are committed atomically in a single SlateDB WAL segment.
