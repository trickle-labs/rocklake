# Object Store Durability

SlateDuck trusts object storage (S3, GCS, Azure Blob) as its durable persistence layer. This is a deliberate architectural choice that eliminates the need for a managed database server, enables serverless deployment, and provides durability guarantees backed by cloud provider SLAs. Understanding how object storage durability works — and how SlateDB bridges the gap between a key-value API and object storage semantics — is important for reasoning about SlateDuck's failure modes and recovery procedures.

## Why Object Storage?

Object stores like S3 offer a unique combination of properties that make them attractive as a persistence layer for metadata:

**Extreme durability.** S3 Standard provides 99.999999999% (11 nines) durability. This means if you store 10 million objects, you can expect to lose one object every 10,000 years. GCS and Azure Blob provide similar guarantees. No self-managed database can match this without heroic effort.

**Zero operational burden.** You do not provision capacity, manage replication, handle failover, patch operating systems, or worry about disk failures. The cloud provider handles all of this transparently.

**Linear cost scaling.** You pay only for what you store and what you access. There is no minimum instance size, no reserved capacity, no idle cost for an underutilized database server.

**Unlimited storage.** There is no practical limit to how much data you can store in a bucket. Your catalog can grow without capacity planning.

**Built-in availability.** S3 Standard provides 99.99% availability. Multi-AZ replication is handled automatically.

## How SlateDB Uses Object Storage

SlateDB, the LSM-tree engine that SlateDuck uses for catalog persistence, maps its storage abstractions onto object storage operations:

| SlateDB Concept | Object Storage Operation |
|-----------------|------------------------|
| Write-Ahead Log (WAL) entry | Single PUT of a segment file |
| Sorted String Table (SST) | Single PUT of a file |
| Manifest | PUT + conditional GET (optimistic) |
| Point read | GET + binary search within SST |
| Prefix scan | Multiple GETs (range of SSTs) |
| Compaction | Read old SSTs + write new SST + update manifest |

The critical insight is that object storage provides **atomic PUT**: a PUT operation either succeeds completely or has no effect. There is no partial write. This gives SlateDB its crash-safety guarantee: a WAL segment is either fully durable or absent.

## Durability Guarantees

Once SlateDuck's `commit` returns successfully, the catalog mutation is durable. Specifically:

1. The write was accepted by the WAL (a PUT to object storage completed successfully)
2. The cloud provider has replicated the bytes to at least two availability zones (for S3 Standard)
3. The data will survive any single-facility failure, including total loss of a data center

This means SlateDuck's durability is bounded by your cloud provider's SLA, not by any aspect of the SlateDuck software. If S3 loses your data, that's an S3 problem, not a SlateDuck problem.

## Latency Implications

The trade-off for extreme durability is latency. Object storage operations are slower than local disk:

| Operation | Local NVMe | S3 Standard | S3 Express |
|-----------|-----------|-------------|------------|
| Single write | 10-100 us | 20-100 ms | 3-10 ms |
| Single read | 10-100 us | 10-50 ms | 2-8 ms |
| Prefix scan (10 keys) | 100 us | 50-150 ms | 10-30 ms |

SlateDuck mitigates this through several strategies:

- **Write batching:** Multiple catalog operations in a single DuckDB transaction become one WAL segment (one PUT)
- **Hot key caching:** The most frequently accessed metadata (current snapshot, file counts) is packed into a single key
- **Secondary indexes:** Snapshot-scoped file lookups use a purpose-built index that avoids scanning all files
- **SlateDB block cache:** Recently read SST blocks are cached in memory, avoiding repeated GETs

For most interactive workloads, the overhead is acceptable: catalog operations take 50-200ms against S3 Standard, which is fast enough for DDL operations that happen infrequently. For latency-sensitive workloads, S3 Express One Zone reduces this to 5-20ms.

## Consistency Model

Object storage provides **read-after-write consistency** for new objects (you can read an object immediately after writing it) and **strong consistency** for list operations (a list returns all objects that have been successfully PUT). SlateDuck relies on both of these guarantees for correct operation.

SlateDB's manifest provides the additional ordering guarantee: readers discover new SSTs by reading the manifest, which is updated atomically after new SSTs are written. This ensures readers never see partial compaction results or orphaned files.

## Failure Modes

Because SlateDuck delegates durability to object storage, its failure modes are:

1. **Object storage unavailable:** SlateDuck cannot read or write. Operations fail with retriable errors. No data is lost. Resume when the outage resolves.
2. **Network partition:** Same as unavailable — SlateDuck cannot reach storage. Fail, retry, resume.
3. **SlateDuck process crash:** Catalog state is fully persistent. Restart the process and it resumes from the latest durable state.
4. **Object storage data loss:** Extremely unlikely (11 nines durability), but if it occurs, there is no local backup to recover from. This is the same risk you accept with any cloud-native architecture.

## Cross-Region Durability

For the highest durability requirements, you can use cross-region replication features of your cloud provider (S3 Cross-Region Replication, GCS Multi-Region, Azure GRS). SlateDuck does not need to know about this — it is handled transparently at the storage layer.
