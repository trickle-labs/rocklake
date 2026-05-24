# SlateDuck v2.x: A World-Class Fact Store on Object Storage

> Status: **Exploration / Design**. This document is a forward-looking design
> blueprint for the v2.x line. It builds on the architectural compass in
> [`docs/concepts/fact-store-vision.md`](../docs/concepts/fact-store-vision.md)
> and the substrate already shipped in v0.x/v1.x (see
> [`plans/blueprint.md`](blueprint.md) §1.4 and §5.29). Nothing here is
> committed for implementation until promoted into `ROADMAP.md` with concrete
> milestones.

---

## Introduction (for a non-technical audience)

SlateDuck v1.x ships a **lakehouse catalog**: a way to store metadata about
data files in cloud object storage. The interesting thing about how it does
that is not the catalog itself — it is the **storage engine underneath the
catalog**. That engine treats every change as an immutable *fact* with a
version number, never overwrites or silently deletes anything, and lets
unlimited readers query any point in history without coordinating with a
writer.

That engine is fundamentally schema-agnostic. The lakehouse catalog is
*one application* of it. The 28 catalog tables defined by the lakehouse spec
take up only about 12 % of the available "tag namespace" in the key encoding;
the other 88 % is deliberately reserved for other schemas.

**v2.x is the release line that opens up the substrate.** It extracts the
generic fact-storage layer into a standalone crate, adds the abstractions
needed to host arbitrary schemas on it, and ships first-class query
interfaces (SQL, typed APIs, and a rule-based query language) plus
horizontally-scaling read replicas.

The end state we are aiming at: **a world-class immutable fact store that
runs on nothing but an object-storage bucket**, scales reads horizontally
to thousands of replicas without coordination, supports time travel as a
native read mode, and costs pennies per gigabyte-month to operate.

---

## 1. Vision and Goals

### 1.1 What "world-class" means here

A world-class fact store on object storage must clear every one of these
bars. The bar order matters: correctness gates everything else.

| # | Bar | Concrete commitment |
|---|-----|---------------------|
| 1 | **Correctness** | Every committed fact is durable, every historical version is readable, the audit trail is tamper-evident, and crash recovery is bit-identical to a clean shutdown. |
| 2 | **Time travel** | Point-in-time queries at any historical version are first-class and have the same correctness guarantees as "current" queries. |
| 3 | **Cost** | $0.02–$0.05 per GB-month of stored facts in steady state; no fixed infrastructure cost when idle. |
| 4 | **Operational simplicity** | A bucket and a binary. No external database, no coordination service, no schema registry server. |
| 5 | **Read scale-out** | Linear throughput scaling to ≥ 100 stateless reader replicas on a single fact store. |
| 6 | **Latency** | p50 < 50 ms, p99 < 200 ms for indexed point lookups on warm caches; range scans bounded by object-storage prefetch. |
| 7 | **Query expressiveness** | At least three interfaces (SQL, typed Rust API, rule-based query language) over the same substrate. |
| 8 | **Schema evolution** | Add attributes, change cardinality, rename, and migrate without rewriting history. |
| 9 | **Federation** | Multiple fact stores in different buckets can be queried together with time-aligned semantics. |
| 10 | **Compliance** | Right-to-be-forgotten erasure via the audited excision path; full provenance for every fact. |

v2.x targets bars 1–7 directly; bars 8–10 are stretch goals that may slip
into v2.5+.

### 1.2 Non-goals

- **Replacing operational OLTP databases.** Object storage latency is
  fundamentally tens of milliseconds per round-trip. v2.x is for workloads
  where time travel, audit, and infinite history are worth that cost.
- **General-purpose graph database.** Rule-based queries are supported, but
  graph-specific optimisations (e.g. native shortest-path) are out of scope
  for v2.x.
- **Streaming pub/sub.** Facts are committed in batches; v2.x does not
  attempt to compete with log brokers on per-event latency.
- **Multi-writer per fact store.** v2.x keeps the single-writer model from
  v1.x. Multi-writer is evaluated as a separate exploration in §10.

---

## 2. The Generic Fact Model

### 2.1 Core abstractions

A **fact** is the tuple

```
(entity, attribute, value, version, op)
```

where:

- `entity` — an opaque identifier (u64 or byte-string) for the thing the
  fact is about.
- `attribute` — a named, typed property of the entity (e.g. `user/email`).
- `value` — the asserted value, typed according to the attribute's schema.
- `version` — a monotonically increasing fact-store version (analogous to
  `dl_snapshot_id` in v1.x).
- `op` — `assert` or `retract`. Retraction is a fact too: a new fact at a
  later version that says "this attribute is no longer set on this entity".

A **transaction** is a set of facts committed atomically at one version.
Multiple facts in one transaction share a version and a single audit record.

A **schema** is a set of attribute declarations: name, value type,
cardinality (one vs. many), uniqueness, indexed dimensions, and retention
policy. Schemas are themselves stored as facts in a reserved system
namespace, so schema evolution participates in time travel.

### 2.2 Why entity-attribute-value, not rows

A row-oriented model bakes the column set into the storage key. Adding a
column requires either rewriting old rows or carrying a NULL forest of
absent values. Renaming or splitting a column is even worse.

The entity-attribute-value (EAV) model stores each attribute of each entity
as an independent fact. Adding an attribute is purely additive — no
historical data changes. Renaming is a metadata edit on the schema fact.
Splitting an attribute is a transaction that asserts the new attributes and
retracts the old one. Every change participates in time travel: queries at
older versions still see the old schema and the old data exactly as they
were.

EAV pays a cost at query time: assembling a logical "row" requires joining
several facts. This cost is the central engineering challenge §3 and §4
address with index design and query compilation.

### 2.3 The fact lifecycle

```
        assert(e, a, v, V₁)                retract(e, a, V₂)
            │                                      │
            ▼                                      ▼
[uncommitted]──────► [live: V₁..V₂)─────────► [retracted: V₂..]
                                                   │
                                                   ▼
                                              [excised]
                                          (only via slateduck excise,
                                            audited, irreversible)
```

A fact's `version` (`V₁`) is its **transaction time** — when it was
recorded. The retraction's `version` (`V₂`) marks the upper bound of its
visibility. A query `as_of(V)` with `V₁ ≤ V < V₂` sees the fact as live.

§5 adds an optional second time dimension (**valid time**) for workloads
that need bi-temporal semantics.

---

## 3. Storage Substrate (extracted from `slateduck-core`)

### 3.1 Crate extraction

v2.0 promotes the schema-agnostic primitives from `slateduck-core` into a
new top-level crate, `slateduck-factstore`. The boundary is defined in
[`plans/blueprint.md` §5.29](blueprint.md):

| Moves into `slateduck-factstore`             | Stays in `slateduck-catalog` |
|----------------------------------------------|------------------------------|
| Key encoding utilities                       | 28-table tag allocation      |
| Value header + version byte + Protobuf dispatch | Lakehouse MVCC filter      |
| Counter allocation (transactional RMW)       | Schema-version increment    |
| `retain-from` and visibility advancement     | Inlined-data encoding       |
| Excision primitives + audit log              | Spec-specific operations    |
| Leadership / epoch keys                      | `dl_snapshot_id` semantics  |
| Generic `CatalogStore` with `SnapshotId(u64)`| Lakehouse adapter           |

The lakehouse catalog becomes the **first hosted schema** on the new crate.
v1.x APIs continue to work unchanged; the extraction is purely internal.

### 3.2 Key layout for the generic fact store

The 1-byte tag namespace from v1.x is preserved. Of the 225 still-unused
tags, v2.x reserves a contiguous block for the generic fact-store schema:

```
Tag    Role
----   ----
0x40   Datom: EAVT primary index   (Entity, Attribute, Version, Tx)
0x41   Datom: AEVT secondary       (Attribute, Entity, Version, Tx)
0x42   Datom: AVET unique/range    (Attribute, Value, Entity, Tx)
0x43   Datom: VAET reverse refs    (Value, Attribute, Entity, Tx)
0x44   Tx log: per-transaction audit record
0x45   Schema facts (attribute declarations, indexed at 0x40–0x43)
0x46   Schema migration log
0x47   User-defined system facts (rate limits, retention policies, etc.)
0x48–0x4F  Reserved for future fact-store internals
0x50–0xBF  User-allocated tag ranges for application schemas
```

A fact `(e, a, v, V, op)` is stored as **four physical keys**, one per
index. Each is independently sorted, independently prefix-scannable, and
independently mergeable. Reads choose the index that minimises scan width
for the query at hand (§4.2).

### 3.3 Why four indexes

Each index serves a distinct access pattern:

| Index | Optimised for                                              |
|-------|------------------------------------------------------------|
| EAVT  | "Tell me everything about entity E as of version V"        |
| AEVT  | "Which entities have attribute A set as of version V"      |
| AVET  | "Which entity has attribute A = value X" (uniqueness, range scans) |
| VAET  | "What references entity E via any attribute" (reverse refs) |

This is the same insight LSM-backed systems have rediscovered repeatedly:
*indexes are not a luxury; they are the difference between O(N) scans and
O(log N) lookups*. The substrate already sorts every key — adding three
more sorted projections of the same fact costs 4× the write amplification
in exchange for orders-of-magnitude better read selectivity.

### 3.4 Leveraging SlateDB features

The substrate is designed to make full use of SlateDB capabilities:

**Atomic `WriteBatch`** — All four index writes for one fact, plus the
transaction-log audit fact (tag `0x44`), commit in one batch. Either every
projection is durable or none is. This is the same atomicity v1.x relies on
for catalog correctness.

**`commit_with_options(await_durable)`** — Every transaction commits with
explicit durability semantics. The `Tx` returned to the caller is the one
guaranteed by SlateDB to survive crashes.

**`DbSnapshot` / `DbReader` at checkpoint** — Time travel translates
directly: a query `as_of(V)` opens a reader at the SlateDB checkpoint that
contains version `V`, then filters facts with `tx ≤ V`. No bespoke MVCC
machinery is needed beyond what v1.x already proved.

**Prefix scans** — Every query plan compiles to a (possibly small) set of
prefix scans. SlateDB's tiered storage prefetches SSTs sequentially, which
maps perfectly onto the access pattern of "scan all facts for entity E in
version range V₁..V₂".

**Manifest generations as cache keys** — A reader pinned to a specific
SlateDB manifest generation can be HTTP-cached by the SST's content hash.
This gives us §6 (CDN-friendly read replicas) almost for free.

**Compaction** — Excised facts (the audited deletion path) propagate
through SlateDB compaction the same way `end_snapshot`-marked rows do
today. No new compaction policy is needed.

**Writer fencing** — The single-writer guarantee on the substrate carries
over; multi-writer exploration (§10) builds *on top* of fencing rather than
replacing it.

### 3.5 Value encoding

Every value carries the v1.x SDKV header (encoding version + type tag +
optional compression flag) and a Protobuf-encoded payload. The payload's
schema is a `Value` union covering the primitive types:

```
oneof value {
  bool      bool_val   = 1;
  int64     i64_val    = 2;
  uint64    u64_val    = 3;
  double    f64_val    = 4;  // canonical: NaN normalised, -0.0 == 0.0
  string    str_val    = 5;
  bytes     bytes_val  = 6;
  bytes     uuid_val   = 7;  // 16-byte raw
  int64     instant_us = 8;  // microseconds since epoch
  uint64    ref_val    = 9;  // entity reference
  Decimal   decimal    = 10; // [scale, mantissa bytes]
  Vector    vector     = 11; // [dtype, dim, packed bytes] for ML use cases
}
```

The format is deliberately small and stable. Application schemas that need
richer types compose them from these primitives (e.g. a `Point2D` is two
`f64_val` facts on the same entity under attributes `geo/x` and `geo/y`).

### 3.6 Counter and ID allocation

Entity IDs are allocated from a per-fact-store monotonic counter under tag
`0xFE`, using the same transactional read-modify-write protocol v1.x uses
for `next_catalog_id`. The counter is bumped atomically inside the same
batch that asserts the entity's first fact, so an ID is never burned
without producing a fact.

For high-throughput ingestion, the writer can reserve a **range** of IDs in
one counter bump (`counter += 1000`) and hand them out from memory. The
range itself is durable; only the in-memory cursor is volatile. After a
crash the unallocated tail of the range is permanently skipped — a price
worth paying for batched allocation.

---

## 4. Query Layer

### 4.1 Three query interfaces, one substrate

| Interface | When to use it                                          | Crate |
|-----------|---------------------------------------------------------|-------|
| Typed Rust API | Embedded use, hot paths, library consumers         | `slateduck-factstore` |
| SQL (PG-wire) | Existing SQL tooling, BI dashboards, ad-hoc analysis | `slateduck-pgwire` (extended) |
| Rule-based queries | Recursive, graph-shaped, exploratory queries       | `slateduck-rules` (new) |

All three compile to the same physical operators over the four indexes
(§3.3). A rule-based query and a SQL query that express the same logical
result compile to the same scan plan; the difference is only in surface
syntax.

### 4.2 Index selection

The planner chooses indexes by **estimated scan width**, computed from
inexpensive metadata that the writer maintains as it commits:

- Per-attribute fact count (a counter under tag `0x47`)
- Per-attribute cardinality estimate (HyperLogLog sketch, also `0x47`)
- Min/max value summaries for indexed scalar attributes

These are themselves stored as facts (in the system namespace) so they
participate in time travel and are reconstructible from a rebuild.

Selection rules, in order:

1. If the query binds a unique attribute and a value → AVET point lookup.
2. If the query binds an entity → EAVT prefix scan.
3. If the query binds an attribute and asks for entities → AEVT scan.
4. If the query traverses a reference backward → VAET scan.
5. Otherwise, fall through to a full scan with a covering filter.

### 4.3 The rule-based query language

A small surface, designed to compile efficiently to prefix scans. Example:

```
?ancestor(X, Y) :- parent(X, Y).
?ancestor(X, Y) :- parent(X, Z), ancestor(Z, Y).

?- ancestor(Alice, ?who).
```

Compilation strategy:

- **Non-recursive rules** compile to a sequence of indexed joins. Each
  predicate becomes a scan whose index is chosen by §4.2.
- **Recursive rules** compile to *semi-naïve* evaluation: maintain a
  worklist of newly-derived facts, scan only their neighbourhood at each
  iteration, terminate when the worklist is empty. This is the textbook
  bottom-up evaluation strategy and is well-suited to LSM-backed storage.
- **Negation as failure** is supported only against *stratified* programs
  to keep semantics decidable.
- **Aggregates** (`count`, `sum`, `min`, `max`, `distinct`) are pushed
  down into scans whenever the index supports it.

The compiler emits a query plan that is a DAG of physical operators
(`Scan`, `Filter`, `Join`, `Project`, `Aggregate`). The runtime is
streaming: results flow through operators row-at-a-time, no materialisation
of intermediate results unless an explicit `Aggregate` requires it.

### 4.4 The pull API

A complementary, declarative way to retrieve a hierarchical shape:

```rust
let user = store.as_of(version).pull(user_id, &spec! {
    "user/name",
    "user/email",
    "user/orders": [
        "order/total",
        "order/items": [
            "item/name",
            "item/price",
        ],
    ],
}).await?;
```

The pull spec is compiled into a batched scan: one EAVT scan for the root
entity, one AEVT scan per nested reference attribute, and so on. The
operator tree is fixed by the spec, so the planner has perfect cardinality
information.

This eliminates the N+1-query problem that plagues most graph traversal
APIs — every nested level is a single batched scan, not one round-trip per
parent entity.

### 4.5 SQL surface

The existing PG-wire dispatcher gains a synthetic schema where every
attribute becomes a column on a virtual `entity` table per namespace.
This is enough for BI tools to issue:

```sql
SELECT u.name, COUNT(o.id) AS order_count
FROM   user u
JOIN   order o ON o.user_id = u.id
WHERE  u.created_at > '2026-01-01'
GROUP BY u.name;
```

The SQL planner translates joins into rule-based query fragments under the
hood, so SQL and rule-based queries share an optimiser. Time travel is
expressed via PostgreSQL's `AS OF SYSTEM TIME` extension syntax, the same
clause already used by the lakehouse adapter.

### 4.6 Materialised views

Some queries are too expensive to recompute on every request. The system
supports **incrementally-maintained materialised views**: a view is
declared as a query, and on every committed transaction the view is
updated only for the entities the transaction touched. The view itself is
stored as facts in a reserved namespace, so it participates in time
travel and read replication.

Implementation: each materialised view registers a *change observer* keyed
on the attributes it reads. The writer, after committing a transaction,
runs the observer over the transaction's fact set and emits the view's
delta as a follow-up transaction. The two transactions share a version so
the view is always consistent with its source.

---

## 5. Time, Versioning, and Retention

### 5.1 Version vs. timestamp

The fact-store version (`u64`, monotonic) is the canonical time. Wall
clocks are unreliable across nodes and across history. Each transaction
also records a wall-clock timestamp in its audit fact (tag `0x44`), but
only as **metadata** — queries should use versions, not timestamps.

A helper API translates timestamps to versions:

```rust
let version = store.version_at_or_before(timestamp).await?;
let results = store.as_of(version).query(...).await?;
```

The lookup uses a sparse index of `(timestamp, version)` pairs maintained
by the writer (one entry per N transactions, where N is tunable).

### 5.2 Bi-temporal facts (optional)

For workloads where the question "when did this become true in the real
world?" is distinct from "when did we record it?", facts can carry an
optional `valid_from` / `valid_to` pair:

```rust
txn.assert_at(entity, attribute, value, valid_from..valid_to)?;
```

Queries can then ask either:

- `as_of(version)` — transaction-time view (what we knew on that day)
- `valid_at(instant)` — valid-time view (what was true on that day)
- `bi_temporal(version, instant)` — both (what we knew *then* about *then*)

This unlocks audit, regulatory, and historical-correction use cases that
single-time-axis systems cannot model cleanly.

### 5.3 Retention policies

Per-attribute retention policies are stored as system facts:

```
attribute = "user/login_event"
retention = { keep_versions: 90_days, mode: visibility_only }
```

`mode: visibility_only` advances `retain-from` for that attribute, hiding
old facts from default queries but leaving the bytes in place. `mode:
excise` invokes the audited deletion path on a schedule.

The default is `keep: forever, mode: visibility_only`. Operators must opt
in to byte-level deletion, just as in v1.x.

### 5.4 Excision

Inherited from v1.x unchanged. `slateduck excise --before V --apply`
remains the only path to byte-level deletion, with the same audit
requirements (operator identity + reason, recorded as an immutable fact
under tag `0xFF`).

v2.x adds **per-entity excision** for compliance use cases (right-to-be-
forgotten): `slateduck excise --entity 12345 --apply` removes all facts
for entity 12345 across all indexes and writes a compliance audit record.

---

## 6. Horizontal Read Scale-Out

### 6.1 The architectural argument

Because every fact key is immutable once written, a reader at version V
sees a stable view that no concurrent writer can perturb. This means:

- Readers do not need to talk to writers.
- Readers do not need to talk to each other.
- Readers can be cached, replicated, and pinned to specific manifest
  generations indefinitely.

This is the strongest scale-out story possible: linear throughput by
adding processes, no coordination overhead, no consistency protocol.

### 6.2 The `slateduck reader` binary

A new binary that serves either the lakehouse schema or any registered
application schema, with three deployment modes:

| Mode | Purpose | State |
|------|---------|-------|
| `--mode embedded` | Library use inside another process | None |
| `--mode pod` | Long-running pod behind a load balancer | Warm cache |
| `--mode lambda` | Cold-start serverless function | Cold cache, opens at known manifest |

Mode selection only affects caching and connection pooling; the read path
is identical.

### 6.3 CDN-friendly cache contract

Because keys are content-addressable (the SST that contains them is
identified by content hash), the system can publish recommended HTTP cache
headers:

- `Cache-Control: public, max-age=31536000, immutable` on SST GETs.
- Manifest reads are `max-age=10` (writer-bounded staleness).
- A range-keyed lookup translates to a small number of byte-range GETs
  against immutable URLs — perfect for CDN edge caching.

The CDN guide documents which proxies (CloudFront, Cloudflare, Fastly)
are validated and what request patterns to expect.

### 6.4 Edge / Lambda integration

Documented patterns:

- Open a `DbReader` against a known manifest generation, query, return.
- Cold-start cost dominated by manifest fetch (~50 ms on warm region).
- The manifest URL is published by the writer to a tiny "current-tip"
  pointer; readers tail it.

### 6.5 Scale targets

v2.x ships with reproducible benchmarks demonstrating:

- Linear read-throughput scaling to ≥ 100 reader pods on a single store.
- p99 < 100 ms for indexed point lookups across the entire reader fleet.
- < 1 % p99 degradation when the writer is concurrently committing at
  100 TPS.

---

## 7. Schema and Evolution

### 7.1 Schemas as facts

Attribute declarations are themselves facts in a reserved namespace
(tag `0x45`). A schema change is a transaction; rollback is a query at an
older version; comparing two schemas is a diff between two versions.

This eliminates the perennial "schema registry" problem — there is no
separate service to keep in sync with the data, because the schema *is*
data.

### 7.2 Permitted changes

Without rewriting history:

- Add a new attribute.
- Mark an attribute deprecated (new asserts rejected, old facts still
  queryable).
- Add an index dimension (rebuilt by a background job).
- Add a uniqueness constraint (validated against existing facts before
  commit).
- Rename an attribute (the old name becomes an alias).
- Tighten a value type (e.g. `i64` → `i32`) if all existing facts fit.

With explicit migration:

- Split an attribute into two.
- Merge two attributes into one.
- Change cardinality from `one` to `many` (additive — old facts still
  valid).
- Change cardinality from `many` to `one` (requires conflict resolution).

### 7.3 Migration log

Every migration is recorded as a fact under tag `0x46`. A migration log
entry captures the transformation (as code or as a declarative rule), the
versions it spans, and the operator who authorised it. The log is itself
queryable: "show me every migration that touched `user/email` in the last
year".

---

## 8. Writer Path

### 8.1 The transaction lifecycle

```
caller                writer                  SlateDB                 audit
  │                     │                        │                      │
  │ begin() ────────────►                        │                      │
  │                     │ allocate Tx version ───►                      │
  │ assert(facts) ──────►                        │                      │
  │                     │ validate schema        │                      │
  │                     │ validate uniqueness    │                      │
  │                     │ validate references    │                      │
  │ commit() ───────────►                        │                      │
  │                     │ build WriteBatch       │                      │
  │                     │  ├── 4 index writes    │                      │
  │                     │  ├── tx-log fact       │                      │
  │                     │  └── counter bumps     │                      │
  │                     │ commit_with_options ───► durable ─────────────►
  │ ◄─── version (V) ───┤                        │                      │
```

### 8.2 Validation pipeline

Before a transaction commits, the writer runs three validators in order
of increasing cost:

1. **Type validation** — every asserted value matches its attribute's
   declared type (cheap, in-memory).
2. **Cardinality / uniqueness validation** — uniqueness constraints
   require an AVET probe per asserted value.
3. **Reference integrity** — every `ref_val` points to an entity that
   exists in the EAVT index at the current version.

Each validator can be disabled per-namespace for performance-sensitive
workloads, but the default is "all on".

### 8.3 Batched commits

For high-throughput ingestion, the writer accepts multiple `begin → commit`
calls and groups them into one durable batch. The group commit returns
when SlateDB has flushed the batch; until then, callers see a "pending"
result. Group-commit window is tunable (default 10 ms or 1000 facts,
whichever comes first).

### 8.4 Idempotency

Every transaction carries an optional **idempotency token**. The writer
maintains a small LRU of recently-seen tokens; replays return the original
version without re-committing. This makes the write path safe for clients
behind a load balancer where retries are routine.

---

## 9. Observability

### 9.1 Per-transaction metrics

The writer exposes per-transaction metrics on Prometheus:

- Commit latency (p50, p99, p99.9) per namespace.
- Facts asserted, retracted per transaction.
- Bytes written per transaction.
- Validation rejections by reason.

### 9.2 Per-query metrics

The reader exposes per-query metrics:

- Query latency by interface (typed / SQL / rules).
- Scan width per index per query class.
- Cache hit rate at the SST level.
- Materialised-view freshness lag.

### 9.3 Audit query interface

A SQL view `_audit.transactions` exposes the transaction log:

```sql
SELECT version, committed_at, operator, fact_count
FROM   _audit.transactions
WHERE  committed_at > NOW() - INTERVAL '1 hour'
ORDER  BY version DESC;
```

The view is backed directly by the tag `0x44` index — no separate audit
table needs to be maintained.

---

## 10. Multi-Writer Exploration

### 10.1 Why it might be possible

Because writers only ever **append disjoint keys** (each transaction
allocates a fresh version that prefixes its keys), the substrate can in
principle accept multiple concurrent writers per fact store with conflict
detection at version-allocation time rather than per-key fencing.

### 10.2 The proposed design (provisional)

- A coordinator service (or distributed lock) allocates non-overlapping
  *version ranges* to writers.
- Each writer commits within its assigned range; no two writers can
  produce the same version.
- Schema changes still require single-writer mode (a global lock).
- Uniqueness validation requires cross-writer coordination via an AVET
  probe with a "as of latest committed across all writers" semantics.

### 10.3 The case against

- The operational complexity is large.
- The current "one store per dataset" partitioning pattern (v0.7) is
  cheap and well-understood.
- Multi-writer adds a coordination dependency that the rest of the
  substrate carefully avoids.

### 10.4 Decision

v2.x **evaluates** but does not commit to multi-writer. The deliverable
is a written design and a prototype. Adoption is gated on a real customer
workload that the partitioning pattern cannot serve.

---

## 11. Federation

### 11.1 Cross-store queries

A query in v2.5+ can reference entities in multiple fact stores:

```
?- alice ∈ store_a.user, alice.orders ⊆ store_b.order.
```

Implementation: the query planner identifies cross-store joins,
parallelises scans across stores, and joins results in memory. There is
no global transaction — each store contributes its own version to the
query, and the planner records the cross-store version vector for
reproducibility.

### 11.2 Time alignment

Cross-store queries can specify time alignment:

- `as_of(wall_clock_time)` — each store resolves its own version at the
  given wall-clock instant.
- `as_of(version_vector)` — explicit (store_a@V₁, store_b@V₂) coordinates.

### 11.3 No global coordinator

There is no central federation service. Stores discover each other via
configuration (URLs in a federation manifest), and queries are planned
client-side. This keeps the operational story consistent with everything
else in v2.x: a bucket and a binary, nothing more.

---

## 12. Extraction Boundary and API Surface

### 12.1 The `slateduck-factstore` crate

```rust
pub struct FactStore { /* ... */ }

impl FactStore {
    pub async fn open(uri: &str, opts: OpenOptions) -> Result<Self>;
    pub fn begin(&self) -> Transaction;
    pub fn as_of(&self, version: Version) -> Reader;
    pub fn current(&self) -> Reader;
    pub async fn excise(&self, request: ExcisionRequest) -> Result<ExcisionReceipt>;
}

pub struct Transaction { /* ... */ }

impl Transaction {
    pub fn assert<V: Into<Value>>(
        &mut self, entity: EntityId, attribute: &str, value: V,
    ) -> &mut Self;
    pub fn retract(&mut self, entity: EntityId, attribute: &str) -> &mut Self;
    pub fn assert_at<V: Into<Value>>(
        &mut self, entity: EntityId, attribute: &str, value: V,
        valid: Range<Instant>,
    ) -> &mut Self;
    pub async fn commit(self) -> Result<Version>;
}

pub struct Reader { /* ... */ }

impl Reader {
    pub async fn get(&self, entity: EntityId, attribute: &str) -> Result<Option<Value>>;
    pub async fn entity(&self, entity: EntityId) -> Result<EntityView>;
    pub fn query(&self) -> QueryBuilder;
    pub fn pull(&self, entity: EntityId, spec: PullSpec) -> Pull;
    pub fn rules(&self) -> RulesEngine;
}
```

### 12.2 The lakehouse adapter

The existing `slateduck-catalog` crate becomes a thin **adapter** on top
of `slateduck-factstore`. Each of the 28 lakehouse tables maps to a
schema with attributes named for the spec columns. The adapter exposes
the v1.x API unchanged for backward compatibility.

This is a powerful proof point: if the lakehouse catalog itself runs
cleanly on the generic substrate, every other schema can too.

### 12.3 Compatibility commitment

- v1.x lakehouse catalogs **upgrade in place** to v2.0 — the on-disk
  format is identical, the adapter speaks the same wire protocol.
- v1.x APIs are preserved through the entire v2.x line.
- The generic API is **independently versioned**: `slateduck-factstore`
  may reach 1.0 before or after SlateDuck v2.0 ships.

### 12.4 Possible standalone-project promotion

Once `slateduck-factstore` stabilises (no breaking changes for two minor
releases, ≥ 2 production users beyond the lakehouse adapter), the crate
can be promoted to a **standalone project** with its own repository,
governance, and release cadence. SlateDuck would then depend on it as an
external crate. This is explicitly *not* required for v2.0 to ship —
in-workspace extraction is sufficient.

---

## 13. Implementation Phases

### Phase 2.0 — Extraction (foundational)

- [ ] Carve `slateduck-factstore` out of `slateduck-core`.
- [ ] Re-implement the lakehouse catalog as a `slateduck-factstore`
      adapter.
- [ ] All v1.x tests pass against the adapter.
- [ ] Zero on-disk format changes; in-place upgrade verified.

### Phase 2.1 — Generic fact model

- [ ] EAVT, AEVT, AVET, VAET indexes implemented.
- [ ] Schema-as-facts (tag `0x45`).
- [ ] Transaction log (tag `0x44`) and audit view.
- [ ] Typed Rust API: `assert`, `retract`, `as_of`, `entity`, `query`.
- [ ] Property tests for index consistency: every fact appears in all
      four indexes with identical content.

### Phase 2.2 — Query layer

- [ ] Query planner with index-selection heuristics.
- [ ] Pull API with batched scans.
- [ ] Rule-based query engine with semi-naïve evaluation.
- [ ] SQL surface extension over rule-based engine.
- [ ] Cross-interface query-plan parity tests (same logical query, same
      physical plan).

### Phase 2.3 — Read scale-out

- [ ] `slateduck reader` binary with three deployment modes.
- [ ] CDN cache contract documentation and validated proxy configurations.
- [ ] Linear-scaling benchmark to ≥ 100 reader pods.
- [ ] Lambda / edge cold-start guide.

### Phase 2.4 — Schema evolution

- [ ] All "without rewrite" changes from §7.2.
- [ ] Migration log and replay tool.
- [ ] Schema diff CLI.

### Phase 2.5 — Bi-temporal and retention

- [ ] `valid_from` / `valid_to` storage and query.
- [ ] Per-attribute retention policies.
- [ ] Per-entity excision (right-to-be-forgotten).

### Phase 2.6 — Materialised views

- [ ] Declarative view definition.
- [ ] Incremental maintenance.
- [ ] View staleness metric.

### Phase 2.7 — Federation (stretch)

- [ ] Cross-store query planner.
- [ ] Version-vector coordinates.
- [ ] Multi-store time alignment.

### Phase 2.8 — Multi-writer evaluation (stretch)

- [ ] Written design document.
- [ ] Prototype.
- [ ] Decision: adopt / defer / reject.

---

## 14. Testing Strategy

Inherits the v1.x testing pyramid (property + unit + golden + crash +
performance) and adds:

| Layer | What it tests |
|-------|---------------|
| **Index consistency** | Every fact appears in all four indexes with byte-identical content. Property test, exhaustive. |
| **Query-plan parity** | Logically equivalent queries across SQL, rules, and the typed API produce identical physical plans. |
| **Time-travel determinism** | A query at version V returns identical results regardless of how many versions exist past V. |
| **Schema-evolution safety** | Every permitted change in §7.2 leaves historical queries unchanged. |
| **Scale-out fairness** | Adding readers does not degrade writer latency by more than 1 % p99. |
| **Bi-temporal correctness** | `as_of(V)`, `valid_at(t)`, and `bi_temporal(V, t)` all give correct results under random fact / valid-time interleavings. |
| **Excision auditability** | Every byte-level deletion produces a tamper-evident audit fact. |
| **Federation** | Cross-store query results match the union of per-store query results under aligned versions. |

---

## 15. Performance Targets

### 15.1 Write path

| Workload | Target |
|----------|--------|
| Single-fact transaction (warm)         | p50 < 20 ms, p99 < 80 ms |
| 1000-fact batched transaction (warm)   | p50 < 50 ms, p99 < 200 ms |
| Sustained throughput, single writer    | ≥ 5 000 facts / sec       |
| Sustained throughput, batched commits  | ≥ 50 000 facts / sec      |

### 15.2 Read path

| Workload | Target |
|----------|--------|
| EAVT point lookup (warm cache)         | p50 < 5 ms, p99 < 30 ms   |
| EAVT point lookup (cold cache)         | p50 < 80 ms, p99 < 250 ms |
| AVET unique lookup (warm)              | p50 < 5 ms, p99 < 30 ms   |
| Pull spec, 10 entities × 5 attrs each  | p50 < 50 ms, p99 < 150 ms |
| Recursive rule, 1 000 facts visited    | p50 < 200 ms              |

### 15.3 Scale-out

| Workload | Target |
|----------|--------|
| 100 reader pods, indexed lookups       | aggregate ≥ 50 000 QPS    |
| Writer concurrent with 100 readers     | < 1 % p99 degradation     |
| Cold-start reader (Lambda)             | first-byte < 300 ms       |

These targets are aspirational and serve as the gating criteria for
declaring v2.x "world-class". They are revised as Phase 2.0 benchmarks
land.

---

## 16. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Four-index write amplification dominates write throughput | Medium | High | Benchmark in Phase 2.1; if real, offer "index sets" (subset of indexes per attribute) as an opt-in. |
| Query planner cannot beat hand-written scans in early benchmarks | High | Medium | Ship the typed API first; defer SQL/rules optimisation until the substrate is stable. |
| Bi-temporal semantics confuse early adopters | High | Low | Keep bi-temporal opt-in per attribute; default is single-time. |
| Materialised-view freshness lag breaks user expectations | Medium | High | Bound the lag explicitly (10 ms group-commit window) and expose it as a metric. |
| Schema evolution rule subset is too restrictive | Medium | Medium | Ship the migration log + migration tool as escape hatches. |
| Federation introduces a new failure mode (partial-store unavailability) | Low | Medium | Make federation opt-in; default deployment is single-store. |
| The fact-store API competes for developer mindshare with the lakehouse | Low | High | Position the fact store as the substrate; the lakehouse remains the most polished use case for v2.x. |
| Multi-writer evaluation rabbit-hole consumes Phase 2.8 with no shippable outcome | Medium | Low | Time-box to one quarter; ship the design doc even if no prototype. |

---

## 17. Success Criteria

v2.x succeeds when:

1. The `slateduck-factstore` crate publishes a 1.0 API and the lakehouse
   adapter uses it in production with zero regressions.
2. At least one non-lakehouse schema is built on the substrate (internal
   or external) and reaches a usable state.
3. The reader binary demonstrates linear scaling to 100 pods in a public
   benchmark.
4. The rule-based query engine answers recursive queries correctly under
   property-testing pressure.
5. Bi-temporal queries are correct under randomised workloads.
6. Documentation covers every API, every deployment mode, every
   compliance scenario.
7. A written, evidence-backed decision on multi-writer is recorded.

---

## 18. Open Questions

- What is the right "tag block" allocation policy for user-defined
  schemas? (Tag ranges, dynamic registration, or both?)
- Should the rule-based query language be standardised (e.g. to an
  existing dialect) or stay SlateDuck-specific to leave room for novel
  features?
- How much of the value-type system should be extensible (custom types
  via plugin?) versus closed (the §3.5 list and no more)?
- Is materialised-view incremental maintenance worth the complexity in
  v2.x, or should it slip to v3.0?
- Can the federation design hold across object-storage providers (S3 ↔
  GCS ↔ Azure) without performance cliffs?
- Should counters be per-entity-type or global per-store? (Performance vs.
  conceptual simplicity trade-off.)

---

## 19. References

- [`plans/blueprint.md`](blueprint.md) — v1.x blueprint and the original
  architectural principle (§1.4) and extraction boundary (§5.29).
- [`docs/concepts/fact-store-vision.md`](../docs/concepts/fact-store-vision.md) —
  Conceptual motivation and use-case catalogue.
- [`docs/architecture/key-layout.md`](../docs/architecture/key-layout.md) —
  Current tag namespace allocation and reservations.
- [`docs/architecture/mvcc-implementation.md`](../docs/architecture/mvcc-implementation.md) —
  v1.x MVCC mechanics that v2.x generalises.
- [`ROADMAP.md`](../ROADMAP.md) — v2.x roadmap entry (Exploration).
