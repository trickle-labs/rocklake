# Why SlateDB?

The most fundamental architectural decision in SlateDuck is the choice of persistence engine. SlateDB is a Rust-native LSM-tree key-value store that writes directly to object storage, with no local disk required. This page explains why SlateDB was chosen over alternatives and what consequences follow from that choice.

## The Requirements

SlateDuck's persistence layer needed these properties:

1. **Object storage as the durable layer.** No local disk dependency. The catalog must survive instance termination.
2. **Atomic multi-key writes.** Multiple catalog entries must be committed together (transactions).
3. **Prefix scans.** Listing tables in a schema, columns in a table, and files for a table must be efficient.
4. **Point reads.** Fetching a specific counter or system key must be fast.
5. **Crash safety.** No corruption on crash, no recovery procedure beyond restarting.
6. **Concurrent readers.** Multiple processes must read the catalog simultaneously.
7. **Rust-native.** First-class Rust API with async support for integration with tokio.

## Alternatives Considered

**FoundationDB:** Excellent transaction support but requires a cluster of servers (violates requirement 1). Not embeddable.

**RocksDB:** Mature LSM-tree but writes to local disk. Would require a separate replication mechanism for durability on object storage. The `rust-rocksdb` bindings are C++ FFI with complex build requirements.

**SQLite:** Requires local filesystem with POSIX locking. Does not support concurrent readers from different processes without network filesystem semantics. DuckLake already has a SQLite backend; SlateDuck exists specifically to avoid this dependency.

**Custom implementation:** Building an LSM-tree from scratch on object storage is a multi-year effort. SlateDB provides this already with a focused API.

**DynamoDB / Cloud Firestore / Cosmos DB:** Cloud-native but proprietary, not self-hostable, and would tie SlateDuck to a specific provider.

## Why SlateDB Won

SlateDB meets all seven requirements directly:

- It writes WAL segments and SSTs to any `object_store` backend (S3, GCS, Azure, local FS)
- Its `WriteBatch` provides atomic multi-key commits
- Keys are sorted, enabling efficient prefix scans via iterator seek
- Point reads use bloom filters and binary search within SSTs
- The WAL provides crash safety (atomic PUT semantics)
- Readers can open the same manifest independently (immutable SSTs)
- It is a pure Rust library with native async/tokio support

Additionally, SlateDB is actively maintained by the same team building SlateDB Cloud, which means continued investment in correctness, performance, and cloud-native features (compaction, caching, garbage collection of old SSTs).

## Consequences of This Choice

**Positive:**
- Zero operational infrastructure for the persistence layer
- Durability backed by cloud provider SLAs (11 nines for S3)
- No local state to manage, backup, or migrate
- Simple deployment: one binary + one bucket path
- Horizontal read scale-out without replication

**Negative:**
- Higher per-operation latency than local disk (20-100ms vs. microseconds for S3 Standard)
- Dependency on cloud provider availability (if S3 is down, catalog is unavailable)
- Limited control over compaction timing and resource usage
- Newer project than RocksDB (less battle-tested, smaller ecosystem)
- No built-in secondary indexes (SlateDuck must implement its own)

## Mitigating the Negatives

The latency cost is mitigated by write batching (one PUT per transaction, not per row), hot key caching (one GET for the most common metadata), and S3 Express One Zone (3-10ms latency for latency-sensitive workloads).

The availability dependency is acceptable because SlateDuck targets cloud-native deployments where object storage availability is a given. If S3 is down, your Parquet files are also inaccessible, so the catalog being unavailable is not an additional failure mode.

The maturity concern is addressed by SlateDuck's extensive test suite, including property-based tests that exercise SlateDB under various failure conditions, and by SlateDB's own CI which runs correctness tests continuously.
