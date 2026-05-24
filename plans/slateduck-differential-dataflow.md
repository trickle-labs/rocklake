# Incremental View Maintenance for SlateDuck/DuckLake

> **Status:** Research and design proposal. Not on any current roadmap.
> **Companion to:** [plans/slatedb-differential-dataflow.md](slatedb-differential-dataflow.md), which covers the general SlateDB+DD architecture. This document narrows the scope to SlateDuck specifically and to the scale-out story.
> **Thesis:** Because DuckLake snapshots, SlateDB SSTs, SlateDuck catalog facts, and differential dataflow (DD) batches are *all immutable*, incremental view maintenance (IVM) on SlateDuck can scale out almost embarrassingly well: stateless workers, no cross-worker coordination beyond a watermark, deterministic re-execution, and unlimited read fan-out.

---

## 1. The Layered Immutability Argument

The case for IVM on SlateDuck rests on four layers of immutability that already exist in the stack, plus a fifth layer DD adds on top:

| Layer | Substrate | Immutable atom | Mutation rule |
|---|---|---|---|
| 1. Object store | S3 / GCS / Azure | Object | Write once; PUT-if-absent; delete via lifecycle |
| 2. SlateDB | LSM on object store | SST | Sealed at flush; only "delete after retention" mutation |
| 3. SlateDuck catalog | DuckLake schema | MVCC fact row | Born at `begin_snapshot`; sealed by terminal `end_snapshot` mark; never mutated thereafter |
| 4. DuckLake data | Parquet | Data file | Written once at `data_file_id`; superseded by delete-file overlays, never edited |
| 5. **IVM state (proposed)** | DD trace on SlateDB | Batch of `(K, V, T, R)` updates | Sealed at frontier; compacted by frontier advance; never edited |

The structural consequence: **any IVM computation that depends only on data at or before a snapshot `S` is a pure function of immutable inputs.** It is therefore:

- **Deterministic.** Two workers reading the same `(base_snapshot, view_def)` *must* produce the same `(record, time, diff)` stream.
- **Idempotent.** Re-running the computation produces the same output regardless of how many times it ran.
- **Cacheable.** Intermediate arrangements are content-addressable by `(view_id, input_snapshot, operator_id)`.
- **Re-executable.** A failed worker can be replaced by any other worker without recovery dance.
- **Splittable.** Work can be sharded by key range and merged exactly because addition of diffs is commutative.

Every property in that list is normally hard-won in stream processing systems. In SlateDuck they fall out for free from the storage model. The scale-out story (§5) leverages all five.

---

## 2. The Problem Statement, Precisely

Given:

- A DuckLake catalog hosted on SlateDuck at warehouse `W`.
- A user-defined view: `CREATE INCREMENTAL MATERIALIZED VIEW v AS <select>`.
- A continuously growing input: new DuckLake snapshots `S_1, S_2, …` of the base tables referenced by `<select>`.

Produce:

- A second DuckLake table `v` whose contents, at every catalog snapshot `S_i`, equal the result of running `<select>` against the inputs as they exist at `S_i`.
- A freshness lag (delay between input snapshot `S_i` being committed and `v`'s corresponding snapshot being committed) that is bounded by a configurable target (e.g. 10 s p99).
- A read path that exposes `v` exactly like any other DuckLake table — through the existing pgwire surface, via the DuckDB `ducklake` extension, or via direct Parquet/Arrow access.

Non-goals for v1:

- Exactly-once guarantees stronger than DuckLake's natural snapshot-level idempotence.
- Generalized recursive/Datalog queries (DBSP's SQL surface is sufficient).
- Sub-second freshness (we are bounded by S3 PUT latency).
- Cross-warehouse joins (single-warehouse first).

---

## 3. Background: DuckLake and DD in Two Paragraphs

**DuckLake** ([ducklake.select](https://ducklake.select/)) is a lakehouse format where the *catalog* (snapshots, tables, columns, file references, statistics) lives in an SQL database and the *data* lives as Parquet files. Each commit is a new immutable snapshot with a monotonically increasing `snapshot_id`. SlateDuck implements the catalog on top of SlateDB instead of PostgreSQL/SQLite, with MVCC fact rows (`begin_snapshot`, `end_snapshot`) replacing in-place updates. All 28 DuckLake v1.0 tables are present; the catalog is queryable at any historical snapshot.

**Differential dataflow** ([TimelyDataflow/differential-dataflow](https://github.com/TimelyDataflow/differential-dataflow)) and its SQL-flavored cousin **DBSP** ([feldera.com](https://www.feldera.com/)) maintain the result of a relational/functional computation as inputs change. Input collections are streams of `(record, time, diff)` triples; operators (`map`, `filter`, `join`, `reduce`, `iterate`) propagate diffs through a dataflow graph. The runtime maintains *arrangements* — sorted indexed views of collections that are physically layered immutable *batches* compacted by *frontier advancement*. From a 15-second cold compute, DD typically absorbs a single-update change in tens to hundreds of microseconds.

The key insight (developed at length in the companion document) is that DD's batch+frontier discipline is structurally an LSM tree, so SlateDB is a natural durable substrate for DD arrangements. This document focuses on what that enables specifically for SlateDuck-hosted DuckLake.

---

## 4. Architecture: Three Planes

The IVM system decomposes into three logically separable planes that share the SlateDB substrate but run as distinct processes.

```
                              ┌──────────────────────────────┐
                              │      CONTROL PLANE           │
                              │  (slateduck-pgwire + new     │
                              │   matview catalog entries)   │
                              └─────────────┬────────────────┘
                                            │ catalog facts
                                            ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CATALOG (SlateDB)                              │
│   tables, columns, snapshots, data_files, …, MATVIEWS, MATVIEW_DEPS,        │
│   MATVIEW_CHECKPOINTS, MATVIEW_SHARDS                                       │
└─────────────────────────────────────────────────────────────────────────────┘
                ▲                                              │
                │ derived snapshots                            │ definitions, watermarks
                │                                              ▼
┌───────────────┴──────────────┐              ┌────────────────────────────────┐
│       OUTPUT PLANE           │              │       COMPUTE PLANE            │
│  Parquet writer:             │◀─arrangement─│  N stateless DBSP workers      │
│  materialize current state   │   snapshot   │  each owning a key-range shard │
│  → new DuckLake snapshot     │              │  of every view's arrangements  │
└──────────────────────────────┘              └────────────────┬───────────────┘
                                                               │
                                              ┌────────────────▼───────────────┐
                                              │  STATE STORE (SlateDB)         │
                                              │  per (view, shard) database    │
                                              │  arrangements as keyed batches │
                                              └────────────────────────────────┘
```

### 4.1 Control plane

Lives inside the existing `slateduck-pgwire` binary. Adds two SQL surfaces:

```sql
CREATE INCREMENTAL MATERIALIZED VIEW v AS <select>;
DROP   INCREMENTAL MATERIALIZED VIEW v;
ALTER  INCREMENTAL MATERIALIZED VIEW v SET (shard_count = 8, freshness = '5s');
REFRESH INCREMENTAL MATERIALIZED VIEW v FULL;   -- rebuild from scratch
```

Catalog state added (new tags under the SlateDuck `0xFD`-range or as a new top-level allocation):

- **`matviews`**: `(matview_id, name, schema, view_sql, output_table_id, shard_count, freshness_target, state_uri, created_at_snapshot, dropped_at_snapshot)` — versioned in the same MVCC scheme as `tables`.
- **`matview_deps`**: `(matview_id, base_table_id, used_columns)` — append-only; tells the compute plane which inputs to subscribe to.
- **`matview_checkpoints`**: `(matview_id, shard_id, last_input_snapshot, last_output_snapshot, frontier_time, durable_at)` — append-only; the watermark log.
- **`matview_shards`**: `(matview_id, shard_id, key_range_lo, key_range_hi, owner_worker, lease_expires_at)` — single-writer-per-shard lease via SlateDB CAS.

The control plane does *not* execute the view. It just records intent and shard ownership. Compute workers tail this state.

### 4.2 Compute plane

A new binary, `slateduck-ivm`, runs independently of `slateduck-pgwire`. Each instance:

1. **Subscribes** to the catalog: polls `matviews` and `matview_shards` for shards it owns or could acquire.
2. **Acquires shards** by CAS on the `matview_shards` lease (same fencing pattern SlateDB uses for writers).
3. **For each owned shard**, runs a DBSP circuit:
   - Sources: `MatviewInputSource` reads the base table's Parquet files between `last_input_snapshot` and the current `latest_snapshot`, *filtered to the shard's key range*.
   - Operators: the user's SQL compiled to DBSP.
   - Sink: writes arrangement deltas to a per-shard SlateDB database at `{warehouse}/matviews/{matview_id}/shards/{shard_id}/`.
4. **Advances the checkpoint**: once a batch is durable in SlateDB, append a new `matview_checkpoints` row.
5. **Triggers materialization**: signals the output plane (via another append to a queue table) that shard `(v, s)` has new state at frontier `T`.

Workers are fully stateless other than their local SlateDB block cache. Losing a worker means another worker eventually acquires the dropped lease and continues from the last durable checkpoint — no replay of the source from scratch.

### 4.3 Output plane

Either a dedicated binary or a thread in `slateduck-ivm`. Periodically (driven by freshness target):

1. Read the union of all shards' current arrangement state at a chosen frontier `T`.
2. Write Parquet files (one per shard, or rebalanced) representing the materialized contents of `v`.
3. Commit a new catalog snapshot adding those data files to the output table `v`.

Because catalog commits are SlateDuck's existing primitive, the output plane reuses `CatalogWriter` unchanged. The materialized view's *Parquet files are first-class DuckLake data files* — they get statistics, can be queried by any DuckDB client, support time travel, and participate in normal catalog GC.

---

## 5. Scale-Out: The Centerpiece

This is the section that pays off the immutability argument.

### 5.1 What's being parallelized

Three orthogonal axes of parallelism:

1. **Across views.** Each materialized view is independent. N views ⇒ N independent dataflows. No coordination.
2. **Within a view, across key-range shards.** A DBSP circuit can be partitioned by *any key that is preserved through all reduce/join operators* — typically a GROUP BY column or a join key. Each shard processes a disjoint key range and writes to its own SlateDB database. Aggregations within a shard are local; cross-shard work is zero for shard-local queries (the common case) and a final merge for global queries.
3. **Within a shard, across worker generations.** Because checkpoints are durable and inputs are immutable, a worker can be killed and replaced mid-batch. Replacement worker resumes from checkpoint. This is fault tolerance via replacement, not via in-place recovery.

### 5.2 Why immutability makes (2) cheap

Sharding a streaming computation is normally hard because the shuffle is stateful: when a row moves between shards, you must atomically update both sides' state. In an immutable world this disappears:

- **Inputs never move.** A DuckLake data file is partitioned by its row contents at write time. The shard owner reads only the rows in its key range; rows for other shards are filtered out at the Parquet reader. The same data file may be read by many workers; that's a benefit, not a cost — SlateDB's block cache amortizes it.
- **Arrangements never move.** Each shard owns its own SlateDB database. Workers do not write to each other's databases. Re-sharding (changing `shard_count`) is a *new view version*, computed in parallel with the old one until cutover.
- **Checkpoints are local.** A shard's watermark is per-shard. There is no global barrier. Slow shards do not block fast shards from publishing new snapshots of *their portion* of the output.

The result: adding workers gives linear throughput improvement up to the point where the *output materialization* (Parquet write of N shards into N data files) becomes the bottleneck — and that bottleneck is itself trivially parallel, because each shard writes its own Parquet file independently.

### 5.3 Why immutability makes read fan-out free

The output of an IVM view *is a DuckLake table*. SlateDuck's existing single-writer-many-readers story (see [docs/concepts/single-writer-many-readers.md](../docs/concepts/single-writer-many-readers.md)) applies verbatim:

- Any number of read replicas can serve `SELECT * FROM v WHERE …` queries against any historical snapshot.
- Adding readers requires no coordination with writers or with each other.
- Pruning, projection pushdown, and time travel all work because `v` looks like every other table.

This means the read fan-out for materialized views is *the same as for base tables*. You scale reads of derived data the same way you scale reads of raw data: spin up more pgwire sidecars pointed at the same bucket.

### 5.4 Why immutability makes compute fan-out cheap

Two scenarios show up in practice:

**Scenario A: backfill / initial materialization.** A new view is created over a 10 TB base table. With M workers, the cold compute can be parallelized across M shards, each reading 10/M TB. Workers commit shard-local snapshots independently. The final view goes live when the slowest shard reaches the target frontier. Cost is linear in 1/M.

**Scenario B: large incremental batch.** A bulk load adds 100 GB to a base table in one DuckLake snapshot. Each shard's source reader filters to its key range; the per-shard work is ~100/M GB. Aggregations propagate as `(K, +n)` deltas; for SUM/COUNT/MIN/MAX, the per-shard delta computation is O(distinct keys in shard). No global synchronization needed; each shard publishes its own delta to its own SlateDB and signals the output plane independently.

**Scenario that is hard:** a `JOIN` between two large tables where neither is small enough to broadcast. This requires both tables to be partitioned on the join key, *or* a re-shuffle of one side. The first case is trivially parallel and is the design recommendation; the second requires a more complex exchange (see §8).

### 5.5 Coordination ceiling

The only globally coordinated state in the entire scale-out story is the watermark frontier per matview, plus the catalog snapshot commit (which is already single-writer in SlateDuck). Workers within a view exchange:

- Lease heartbeats on `matview_shards` (one CAS per worker per lease interval).
- Append-only watermark advances on `matview_checkpoints` (one PUT per shard per batch).
- A small signal queue for the output plane.

None of that scales with data volume. Adding TBs of data does not increase coordination traffic. Adding shards adds O(shards) heartbeats and watermarks. This is the same coordination shape SlateDB itself has and it is known to scale to large fleets.

### 5.6 Concrete scaling target

A reasonable v1 design point:

- **Single warehouse, up to ~1000 matviews.** Limit imposed by catalog overhead and S3 listing cost, not by compute.
- **Per matview: 1 to 64 shards.** Configurable; default 1 for small views, auto-bumped at creation time based on base-table size.
- **Per shard: one writer worker.** Multiple workers may *contend* for a shard's lease across failover but only one holds it at a time.
- **Per warehouse: arbitrary read replica count.**
- **Freshness: 1 s (best effort, single shard) to 30 s (large fan-out, conservative).**

These are not aspirational; they are achievable with the architecture described above without inventing new distributed primitives.

---

## 6. Mapping to the Existing SlateDuck Codebase

Concrete changes by crate. The bullets are sized so that a reasonable v1 milestone is one or two months of focused work.

### 6.1 `slateduck-core`

- Allocate new system tag(s) for matview state. Options:
  - Add to the `0xFD` family used for inlined rows, with a distinct subtype byte.
  - Or, allocate fresh top-level tags (e.g. `0x20`-`0x24`) for `matviews`, `matview_deps`, `matview_checkpoints`, `matview_shards`, and `matview_arrangement_index`.
- Extend `tags.rs` `TagDescriptor` for each new tag with appropriate `MvccBehavior` (mostly `AppendOnly`, except `matviews` which is `Versioned`).
- Add protobuf row types in `rows.rs` for each new table.
- No change to MVCC or key encoding; these are reuses of existing patterns.

### 6.2 `slateduck-catalog`

- `writer.rs`: add `create_matview`, `drop_matview`, `update_matview_checkpoint`, `claim_matview_shard`. The first three are straightforward MVCC writes. `claim_matview_shard` uses the existing `DbTransaction` with `SerializableSnapshot` to do compare-and-set on the lease row.
- `reader.rs`: add `list_matviews`, `get_matview`, `list_shards_for_worker`, `read_checkpoint_history`.
- Tests: extend `tests/integration_tests.rs` with matview lifecycle scenarios.

### 6.3 `slateduck-sql`

- Extend the bounded SQL surface to recognize `CREATE/DROP/ALTER/REFRESH INCREMENTAL MATERIALIZED VIEW`. These are the *only* new statement shapes; the inner `<select>` is parsed but not evaluated by pgwire — it is stored verbatim in the catalog row and interpreted by `slateduck-ivm`.
- Add `SHOW MATERIALIZED VIEWS`, `SHOW MATVIEW SHARDS`, `EXPLAIN MATERIALIZED VIEW v` for observability.

### 6.4 `slateduck-pgwire`

- Route the new statements to the new catalog writer methods.
- `SELECT * FROM v` already works because `v` is a normal DuckLake table — no change required on the read path.

### 6.5 `slateduck-ivm` (new crate)

The substantial new component. Major modules:

- `source.rs`: `MatviewInputSource` — reads Parquet files filtered to a shard's key range, emits `(row, snapshot_id, +1)` deltas. For append-only base tables this is a forward scan; for tables with delete files, it emits matching `(row, snapshot_id, -1)`.
- `circuit.rs`: thin wrapper around [DBSP](https://github.com/feldera/feldera). Compiles the stored view SQL to a DBSP circuit. Handles operator registration, schema validation.
- `trace.rs`: `SlateDbTrace` — implementation of DBSP's persistent trace trait on top of SlateDB. (Could initially use DBSP's existing RocksDB-backed persistent traces, then migrate to a native impl. See companion document §4.2.)
- `worker.rs`: per-process event loop. Acquires shard leases, drives circuits, advances checkpoints, signals the output plane.
- `output.rs`: per-shard Parquet writer. Reads current arrangement state, writes a new data file, commits to the output table's catalog.
- `bin/slateduck-ivm.rs`: binary entry point with CLI mirroring `slateduck-pgwire`'s shape:
  ```
  slateduck-ivm serve \
    --catalog-path s3://bucket/catalogs/warehouse-a \
    --state-prefix  s3://bucket/matview-state/ \
    --worker-id     ivm-0 \
    --shard-limit   16
  ```

### 6.6 New crate dependencies

- `dbsp` (or `feldera`): the IVM engine.
- `parquet` + `arrow`: already in the workspace (via DataFusion) — reused.
- `datafusion-sql`: for parsing the inner view query into a logical plan that DBSP can consume.

No changes to existing public APIs of `slateduck-core` / `slateduck-catalog` / `slateduck-pgwire` are required. The IVM system is a pure addition.

---

## 7. End-to-End Worked Example

A user runs against a SlateDuck deployment:

```sql
-- 1. There is a base table being continuously appended.
SELECT count(*) FROM events;
-- 5_000_000 rows, last snapshot S_42.

-- 2. Define an incremental view.
CREATE INCREMENTAL MATERIALIZED VIEW events_by_day AS
SELECT date_trunc('day', occurred_at) AS day,
       event_type,
       count(*)                       AS n
FROM events
GROUP BY 1, 2
WITH (shard_count = 8, freshness = '5s');
```

What happens:

1. **Pgwire** parses, validates, calls `catalog.create_matview(...)`. New rows appear in `matviews`, `matview_deps`, `matview_shards` (8 shards spanning the hash range of `event_type`). Output table `events_by_day` is created empty. Catalog snapshot advances to `S_43`. Returns to client in ~10 ms.
2. **An IVM worker** polls the catalog, sees 8 unclaimed shards. It claims as many as its `--shard-limit` and acquires leases. Other workers claim the rest.
3. **Each worker** opens its per-shard SlateDB database under `s3://bucket/matview-state/events_by_day/shard-{N}/`. Reads the base table's data files between snapshot 0 and 42, filtered to the shard's key range. Builds the initial arrangement (a `BTreeMap`-like structure on disk: `(day, event_type) → count`). Writes durable batches into SlateDB. Each shard commits a per-shard checkpoint at `last_input_snapshot = 42`.
4. **The output plane** sees 8 shards reach frontier 42, reads each shard's arrangement contents, writes 8 Parquet files (one per shard, naturally partitioned), and commits a new catalog snapshot `S_44` that adds those files to `events_by_day`. View is now live.
5. **A producer appends** 10 000 new events. Commits as `S_45`.
6. **Workers wake up** (configurable: polling or change-feed). Each shard scans only the new data files in `S_45` that intersect its key range. Pushes ~100 new `(row, +1)` updates per shard through the DBSP circuit. The aggregation operator emits per-key delta: for keys that already existed, two updates `(K, old_count, -1)`, `(K, new_count, +1)`; for new keys, one update `(K, count, +1)`.
7. **Per shard**, the delta is written to SlateDB (one small batch per shard). Checkpoint advances to 45. Total work per shard: ~hundreds of microseconds of CPU, plus one S3 PUT.
8. **The output plane** sees 8 shards at frontier 45, decides whether to publish a new snapshot of `events_by_day` (yes — freshness budget exceeded). It writes 8 small Parquet files (one per shard), commits snapshot `S_46`. View now reflects the new events.
9. **A reader** runs `SELECT * FROM events_by_day WHERE day = '2026-05-24'`. Pgwire serves it from the latest snapshot. The reader has no knowledge that this is a materialized view; it's just a DuckLake table.

Total elapsed time from producer commit (step 5) to reader-visible result (step 8): bounded by `freshness` target, typically 1-5 s.

---

## 8. Hard Problems and Mitigations

The architecture above sidesteps most distributed-systems pain via immutability, but a handful of real problems remain.

### 8.1 Cross-shard joins

If a view joins two tables on different keys than the shard key (or joins on a key that is not the GROUP BY key), the shard partitioning breaks down. Three mitigations, in order of preference:

1. **Broadcast small side.** If one input is small enough, replicate it to all shards. Common for dimension tables.
2. **Re-shard at the join.** Insert an exchange operator that re-partitions one side. Adds an extra SlateDB write/read cycle but preserves the shard-local model elsewhere.
3. **Single-shard mode.** For complex multi-join views, set `shard_count = 1` and scale by deploying multiple identical views with different filters. Crude but works.

Pick option (1) for v1; (2) is a v2 elaboration; (3) is always available as an escape hatch.

### 8.2 Backpressure and slow shards

If one shard's compute lags (slow disk, hot key skew), the output plane has two choices:

- **Wait for all shards.** Simple but bounded by tail. Default for "consistent snapshot" semantics.
- **Publish per-shard.** Each shard publishes its portion independently; query-time merges them. Faster freshness but the view has per-shard skew in its watermark.

Expose as `WITH (output_mode = 'consistent' | 'per_shard')`. Default consistent.

### 8.3 Schema evolution on base tables

If `events` gains a column or a column changes type, what happens to `events_by_day`?

- If the view doesn't reference the changed column: no-op.
- If it does: invalidate the view and require explicit `REFRESH ... FULL` (rebuilds from scratch). v1 does not attempt incremental schema migration.

### 8.4 Deletes and updates in base tables

DuckLake supports deletion via *delete files*. The input source must emit `(row, -1)` updates for rows newly covered by a delete file. This is a straightforward filter against delete-file overlays, but it makes the input source non-trivial. For v1, support delete files for non-aggregating views (where `(-1)` deltas pass through correctly) and document that aggregations over deletable base tables require `REFRESH ... FULL` after large deletion campaigns.

### 8.5 Compaction interactions

Two compactors are now running: SlateDB's SST compaction (per state-store database) and the matview's frontier-advance compaction (DBSP-driven). They are not coordinated. In practice this is fine because each operates on disjoint data, but it does mean ~2x the background CPU cost compared to a unified compactor. A future optimization is to fuse them.

### 8.6 Cost of small frequent writes

A naive implementation flushes a SlateDB batch on every input snapshot, generating thousands of small SSTs per day per shard. Mitigations:

- Coalesce: only flush when `time-since-last-flush > freshness/2` *and* there is buffered work.
- Use SlateDB's `await_durable = false` for non-checkpoint writes; only `await_durable = true` at checkpoint boundaries.
- Configure aggressive SlateDB compaction for matview state stores.

### 8.7 Exactly-once on the output

DuckLake snapshot commits are transactional via SlateDuck's catalog. The output plane commits at-most-one snapshot per `(matview, target_frontier)` tuple by including the frontier in the snapshot's metadata and using catalog CAS to ensure no duplicate snapshot is created for the same frontier. This gives exactly-once *output snapshots* with respect to the input watermark.

---

## 9. Phased Delivery

A pragmatic delivery plan that lets each phase deliver standalone value.

### Phase A — `slateduck-ivm` minimum viable shape

Goal: a single-shard, single-table, append-only, GROUP BY view, end-to-end, with persistence.

- Catalog support for `matviews`, `matview_deps`, `matview_checkpoints` (no shards yet).
- `slateduck-ivm` binary, single-shard hardcoded.
- DBSP circuit for `SELECT k, sum(v) FROM t GROUP BY k`.
- SlateDB-backed persistent trace (initially using DBSP's bundled object-store storage; native `SlateDbTrace` later).
- Output plane writes a single Parquet per cycle.
- Demo: append to base table, see view update within freshness target.

Exit criteria: TPC-H Q1 maintained against a streaming `lineitem` source, with sub-second freshness for small batches.

### Phase B — Sharding and lease management

Goal: scale-out within a single view.

- Add `matview_shards` table and lease CAS.
- Worker shard claim/heartbeat/release lifecycle.
- Per-shard SlateDB state stores.
- Per-shard Parquet outputs.
- Restart safety: kill -9 a worker, verify another picks up its shards.

Exit criteria: TPC-H Q1 with 8 shards, 8x ingest throughput vs Phase A.

### Phase C — Joins

Goal: support two-table joins with shard-aware sources.

- Broadcast small-side join support.
- Co-partitioned join support (same shard key both sides).
- DBSP circuit compilation for `JOIN ... GROUP BY`.

Exit criteria: TPC-H Q3 or Q5 maintained incrementally.

### Phase D — Operational hardening

Goal: production-ready.

- Backpressure & per-shard publication modes.
- Delete-file support in input source.
- Cost-of-S3-writes optimization (write batching, await_durable tuning).
- Metrics, tracing, alerts.
- `REFRESH ... FULL` and schema-evolution handling.
- Documentation and operator playbook.

### Phase E — Beyond SQL (optional, v2)

- Native `SlateDbTrace` implementation (move off DBSP's bundled persistence).
- Re-sharding without full rebuild.
- Cross-warehouse views.
- Recursive CTE / Datalog (lift to raw DD).
- Continuous integrity-constraint checking as a special case of IVM.

---

## 10. Strategic Considerations

### 10.1 Why this fits SlateDuck's identity

SlateDuck's pitch is "your entire lakehouse in an S3 bucket, no server required" (see [README.md](../README.md)). Incremental materialized views are the single feature most lakehouses lack that SlateDuck can deliver *without* breaking that pitch — because the IVM workers are stateless sidecars, the state lives in the same bucket, and the output is just more DuckLake. There is no new operational story to learn: same single-writer-many-readers, same checkpointing, same backup, same time travel.

### 10.2 Why not just defer to Materialize/Feldera

Existing IVM systems require their own storage (Materialize's `persist`, Feldera's RocksDB), their own deployment story (Kubernetes operators, separate clusters), and their own consistency model that does not align with the lakehouse's snapshots. SlateDuck-native IVM aligns all three — and inherits SlateDuck's hard-won correctness properties (catalog MVCC, single-writer fencing, immutable facts) instead of duplicating them.

### 10.3 Where this sits in the roadmap

This is firmly post-v1.0 work. The v0.9.x correctness/security/operational tracks (see [ROADMAP.md](../ROADMAP.md)) must land first. But two pieces of *non-IVM* architectural plumbing are cheap to add now and would unlock IVM later:

- **Stable change-log shape for catalog snapshots.** Make sure `read_at(snapshot_id)` plus a *diff* between snapshots is a primitive operation, not a re-derivation. This benefits replication, audit, and IVM equally.
- **Per-warehouse subdirectory layout that admits sibling state stores.** The current layout already does this (catalogs live under `catalogs/<warehouse>/`); a future addition of `matviews/<warehouse>/...` is uncontroversial. Verify that nothing in `slateduck-core` assumes the only thing in a warehouse bucket is the catalog itself.

### 10.4 Risks worth naming

- **DBSP API churn.** DBSP is younger than DD and still evolving. Pinning to a version and writing a thin adaptation layer (`slateduck-ivm::circuit`) is essential.
- **Operator fluency.** "Materialized view freshness" is a new concept for SlateDuck users; documentation and good defaults matter as much as the implementation.
- **Cost.** Frequent S3 writes from many shards can balloon API costs. The Phase D optimizations are not optional.
- **Scope discipline.** It is tempting to chase graph algorithms, Datalog, exactly-once exactly-everything. The wedge is *SQL materialized views for DuckLake*. Stay focused.

---

## 11. Summary

The argument in one paragraph: SlateDuck's catalog, SlateDB's SSTs, DuckLake's data files, and DBSP's batches are all immutable. Stack them and you get an IVM system where compute workers are stateless, state is content-addressable in object storage, sharding is trivial because nothing ever moves between shards, and read fan-out is unlimited because the materialized view is itself just another DuckLake table. The hardest distributed-systems problems in conventional stream processing — exactly-once recovery, state migration, shuffle correctness — are degenerate cases when every layer is immutable. The implementation cost is one new crate (`slateduck-ivm`), a handful of new catalog tables, no breaking changes to existing crates, and a small SQL surface extension. The strategic payoff is a capability that Iceberg and Delta achieve only with external streaming systems, delivered as a single binary against an S3 bucket.

If we are going to build IVM at all, we should build it here, because SlateDuck's existing properties make it dramatically easier than building it anywhere else.

---

## Appendix: Open Questions for Design Review

1. Tag allocation: distinct top-level tags or a packed `0xFD` subspace for matview catalog state?
2. Should `slateduck-ivm` share a binary with `slateduck-pgwire` for ops simplicity, or stay separate for clear failure-domain boundaries? (Recommendation: separate.)
3. Default `shard_count`: fixed at 1, auto-tuned from base-table size, or always require explicit user choice?
4. Should view outputs be written as DuckLake data files of the *output table* `v`, or as a parallel "matview snapshot" namespace? (Recommendation: regular data files — preserves the "v looks like any table" property.)
5. How to expose freshness/lag in observability — as a per-matview `last_published_snapshot_lag_ms` gauge? As a SQL function `MATVIEW_LAG('v')`?
6. Initial DBSP integration strategy: depend on the `dbsp` crate directly, or vendor a fork? (Recommendation: direct dependency, pin version, abstract behind `slateduck-ivm::circuit`.)
7. Failure mode: what happens if a worker holds a lease but cannot make progress? Need a heartbeat-based eviction policy.
8. Cost model: at what input throughput does S3 PUT cost dominate compute cost? Need empirical numbers before recommending defaults.
