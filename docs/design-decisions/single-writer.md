# Single-Writer Model (Design Decision)

This page documents the decision to use a single-writer concurrency model for catalog mutations, the alternatives that were evaluated, and the specific trade-offs involved.

## The Decision

At any given time, exactly one SlateDuck process is authorized to write to a catalog. Multiple readers are supported concurrently without coordination. Writer identity is enforced through an epoch counter stored in the catalog.

## Alternatives Considered

**Multi-writer with pessimistic locking.** Multiple writers coordinate via distributed locks (e.g., DynamoDB lock table, ZooKeeper). Rejected because it introduces a dependency on an external coordination service, defeating SlateDuck's "object storage only" promise.

**Multi-writer with optimistic concurrency control.** Multiple writers attempt to commit, detect conflicts, and retry. Rejected because SlateDB does not natively support conditional writes at the application level (compare-and-swap on arbitrary keys). Implementing OCC on top of SlateDB would require either external coordination or complex conflict detection logic.

**Raft/Paxos consensus.** Multiple replicas agree on the order of mutations. Rejected because it requires 3+ running instances, introduces consensus latency, and is massively over-engineered for a metadata catalog that processes a few writes per minute.

**Single writer elected via object storage leasing.** Use S3 conditional PUT or DynamoDB-backed lease to elect a writer. Considered viable but adds complexity and a failure mode (lease expiration during long operations). The simpler epoch-based approach was preferred.

## Why Single-Writer Works

**DuckLake writes are infrequent.** A typical analytics workload writes to the catalog a few times per minute (registering new data files after ETL jobs complete). A single writer handling tens of writes per second (SlateDuck's practical limit on S3 Standard) is not a bottleneck.

**Catalog writes are small.** A typical transaction registers 1-100 files and creates one snapshot. The write batch is under 100 KB. There is no need for write parallelism to achieve throughput.

**Correctness is trivial.** With one writer, the snapshot sequence is linear and deterministic. There are no conflicts, no retries, no disambiguation of concurrent mutations. Testing is straightforward.

**Operational simplicity.** There is no cluster to manage, no split-brain to resolve, no quorum to maintain. One process writes. If it fails, another takes over (after incrementing the epoch).

## The Cost

**Write availability depends on one process.** If the writer crashes, writes are unavailable until a replacement starts. The recovery time is typically seconds (start a new process, it increments the epoch and becomes the writer).

**No write parallelism.** Large bulk operations (registering 10,000 files) must go through the single writer sequentially. In practice, DuckDB already batches these into transactions of reasonable size.

**Writer failover is not automatic.** SlateDuck does not include built-in leader election. Operators must run a health-check/restart mechanism (systemd, Kubernetes liveness probe, etc.) to restart the writer on failure.

## Multi-Writer via Partitioning

For workloads that genuinely need concurrent writers, SlateDuck provides dataset partitioning: one independent catalog per dataset, each with its own writer. This gives you write parallelism across datasets without the complexity of multi-writer within a single catalog. See [Concepts: Single Writer, Many Readers](../concepts/single-writer-many-readers.md) for details.
