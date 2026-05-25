# IVM Retrospective — v0.11 through v0.15

This document captures design decisions, lessons learned, and trade-offs
across the IVM implementation from v0.11 (initial) through v0.15 (operational
hardening).

## Timeline

| Version | Focus | Key Deliverable |
|---------|-------|-----------------|
| v0.11 | Foundation | Single-input GROUP BY, append-only CDC |
| v0.12 | Scale-out | Multi-shard, lease-based workers |
| v0.13 | Joins | Hash-join with 3 strategies |
| v0.14 | Correctness | Phantom-row fix, aggregate tiers, volatility |
| v0.15 | Hardening | Durability, cost control, operational tooling |

## Key Design Decisions

### Why not DBSP/Feldera directly?

The Feldera runtime (`dbsp` crate) requires:
- Its own thread pool and scheduling
- `rkyv` serialization on all data types
- `feldera-storage` for persistence

SlateDuck uses SlateDB + serde_json + single-writer leases, making integration
infeasible without forking. We implement the DBSP *algebraic model* directly.

### Why single-writer per shard?

- Eliminates coordination overhead
- SlateDB's MVCC provides consistent reads
- Lease-based fencing prevents split-brain
- Simpler correctness reasoning

### Why flush coalescing?

S3 PUTs are expensive ($5/million). Without coalescing:
- 1 event/sec × 1 flush/event = 2.6M PUTs/month = $13/shard/month
- With 5s coalescing window: ~520K PUTs/month = $2.60/shard/month

The flush coalesce window is the primary cost lever.

### Why not stream-to-Parquet directly?

v0.15 considered writing Parquet directly from the IVM engine. Deferred because:
- DuckDB's Parquet writer is more optimized
- Row-group statistics require buffering
- Sort-key ordering adds complexity
- NDJSON → Parquet conversion is fast enough for v0.15 scale

### Why exactly-once via CAS?

The output plane uses catalog CAS (compare-and-swap) for exactly-once:
- Worker writes Parquet file to S3
- Worker commits file reference to catalog with CAS on frontier
- If CAS fails → another worker already committed (duplicate detected)
- If worker crashes after Parquet write but before CAS → safe to retry

This avoids distributed transactions while guaranteeing no duplicate output.

## Lessons Learned

1. **Frontier tracking is the foundation** — every correctness guarantee
   reduces to "skip events ≤ frontier."

2. **Cost mode should be set at creation time** — changing modes mid-stream
   requires careful state migration.

3. **Diamond detection matters** — without it, multi-view refresh can produce
   inconsistent intermediate states.

4. **Append-only fast path is significant** — most production workloads are
   append-heavy; detecting this enables simpler state management.

5. **Rate limiting belongs at the protocol layer** — not the application layer.
   PG-Wire is the natural enforcement point.

## Future Directions (post-v0.15)

- Stream-to-Parquet direct writer (v0.16+)
- Cross-region state replication
- Adaptive freshness based on query patterns
- Window functions in IVM plans
- Temporal joins with watermarks
