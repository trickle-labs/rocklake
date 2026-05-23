# Immutability Trade-offs

SlateDuck's append-only, immutable data model is its most distinctive architectural property. Catalog entries are never modified in place; updates create new versions, and old versions remain until explicitly excised. This page honestly examines the costs of this approach and how they are managed.

## What You Get

The benefits of immutability are covered in detail in [Concepts: Immutability](../concepts/immutability.md):
- Free time travel (query any historical snapshot)
- Automatic crash safety (no partial updates)
- Lock-free readers (never conflict with writers)
- Horizontal read scale-out (immutable SSTs support unlimited concurrent GET)

## What You Pay

### Storage Growth

Every schema change, table creation, column addition, and data file registration creates new rows that are never automatically reclaimed. A heavily-modified catalog accumulates "dead weight" — superseded rows that consume storage but serve no purpose for current queries.

**Quantifying the cost:** A typical catalog row is 50-200 bytes. A table with 50 columns that undergoes 10 schema changes creates ~500 superseded column rows (50 columns x 10 changes), occupying approximately 50 KB. This is negligible for a single table but adds up across thousands of tables with frequent changes.

**Mitigation:** Run `slateduck gc` to advance the retention horizon, then `slateduck excise` to physically remove superseded rows. For most workloads, running GC weekly with a 30-day retention window keeps storage growth well-bounded.

### Read Amplification

Every prefix scan must examine all versions of each entity and filter for visibility. If a column has been modified 100 times, the reader examines 100 rows to find the one visible version.

**Quantifying the cost:** The MVCC filter is two integer comparisons per row — trivially fast. The real cost is I/O: reading 100 rows from SlateDB when only 1 is needed means 99% wasted bytes fetched from object storage.

**Mitigation:** For most catalog entities, the number of versions is small (1-5). Garbage collection removes old versions, capping the worst case. For the pathological case, secondary indexes provide version-free access paths.

### Operational Complexity

Operators must understand and run the two-phase GC process (advance retention, then excise). This is an additional operational task that PostgreSQL-backed DuckLake does not require (PostgreSQL has automatic VACUUM).

**Mitigation:** GC can be automated via cron or a sidecar process. The commands are simple and idempotent. The documentation provides clear guidance on retention policies.

### No True Delete

You cannot immediately and permanently remove a catalog entry. Even after GC + excision, there is a window between creation and excision where the data exists. For GDPR compliance or similar regulations that require prompt deletion, this window must be acceptable.

**Mitigation:** The window can be made as short as needed by running GC with `--retain-days 0` followed by immediate excision. This is not recommended for normal operation (it destroys all time travel capability) but is available for compliance scenarios.

## The Trade-off Matrix

| Concern | Immutable (SlateDuck) | Mutable (PostgreSQL) |
|---------|----------------------|---------------------|
| Time travel | Free, unlimited history | Expensive, limited by WAL retention |
| Crash safety | Automatic | Requires WAL + checkpoint + recovery |
| Read scale-out | Unlimited, no coordination | Requires replication + lag |
| Storage efficiency | Grows until GC | Automatic via VACUUM |
| Delete latency | Eventual (GC + excise) | Immediate |
| Operational burden | GC scheduling | VACUUM tuning, replication monitoring |

## Our Assessment

For the lakehouse catalog use case, the trade-off favors immutability. Catalogs are small (megabytes, not gigabytes), schema changes are infrequent (daily, not per-second), and time travel is a core user requirement. The costs are manageable with periodic GC, and the benefits (crash safety, read scale-out, time travel) are fundamental to the product's value proposition.

If your use case involves very high catalog churn (thousands of schema changes per second) or strict immediate-deletion requirements, SlateDuck's immutability model may not be appropriate. Consider PostgreSQL-backed DuckLake instead.
