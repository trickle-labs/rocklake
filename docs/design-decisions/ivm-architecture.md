# IVM Computation Architecture Decision

**Status:** Decided (May 2026)  
**Decision:** Option A — Extend the hand-rolled Z-difference shim  
**Gate:** Pre-v0.14 Architecture Gate 1

## Context

SlateDuck's IVM engine uses an incremental GROUP BY / JOIN circuit
(`crates/slateduck-ivm/src/circuit.rs`) that implements DBSP's Z-difference
algebraic model.  The `dbsp` crate (Feldera 0.299.0) is declared as a
workspace dependency but is **not imported or used** anywhere in the codebase.

The roadmap references "DBSP operators," "Trace/Batch/Cursor traits," and
"SlateDbTrace" persistence.  Before starting v0.14 we must decide which
compute foundation to extend for the remaining milestones (v0.14–v0.18).

## Options Evaluated

### Option A: Extend the Hand-Rolled Shim (CHOSEN)

Keep the existing `IvmCircuit` / `IvmJoinCircuit` in `circuit.rs` (~539 lines)
and extend it with:
- Full retraction support (already working via `MinMaxState` + weight tracking)
- Aggregate tier classification (algebraic / semi-algebraic / group-rescan)
- Volatility validation layer
- Additional join correctness (EC-01 asymmetric delete branches)

**Pros:**
- Zero new dependencies; already compiles and passes tests
- Full control over persistence (checkpoint state lives in SlateDB directly)
- Worker lifecycle is our own (lease-based, single-writer)
- Extension path is clear: ~200 lines per aggregate tier, ~150 lines for EC-01

**Cons:**
- No pre-built `iterate()` operator for recursive CTEs (v0.16 item)
- Must implement fixed-point detection manually if needed

### Option B: Migrate to DBSP Native API (REJECTED)

Replace `IvmCircuit` with Feldera's `RootCircuit` + `DBSPHandle::step()`.

**Findings from spike:**
1. `DBSPHandle` spawns its own worker threads via `Runtime`; conflicts with
   SlateDuck's lease-based per-shard worker model.
2. `Trace` trait requires `save()` / `restore()` backed by `feldera-storage`
   (`StoragePath`, `FileCommitter`) — incompatible with SlateDB.
3. `BatchReader` requires `Rkyv + SizeOf` on all data types — we use
   protobuf + serde_json, not rkyv.
4. `CollectionHandle` input path requires `DataTrait` bounds that assume
   rkyv-serializable types.
5. 70+ transitive dependencies pulled in (feldera-buffer-cache, feldera-ir,
   feldera-storage, mimalloc, etc.)

**Conclusion:** DBSP is a full streaming platform runtime, not an embeddable
library.  Integrating it would require forking or reimplementing its storage
layer.

### Option C: Switch to differential-dataflow (REJECTED)

Replace `IvmCircuit` with `differential_dataflow::Collection` +
`timely::execute`.

**Findings:**
1. Timely also spawns its own worker threads and communication fabric.
2. Persistence requires the `differential-dataflow` `Trace` to be backed by
   an external compaction layer; no SlateDB adapter exists.
3. Better fit for multi-worker horizontal scale, but SlateDuck targets
   single-writer-per-shard — the coordination overhead adds no value.
4. Would provide `iterate()` for CTEs but at the cost of owning the entire
   scheduling model.

**Conclusion:** Over-engineered for SlateDuck's single-writer lakehouse model.

## Decision Rationale

1. **Simplicity:** The hand-rolled shim is 539 lines, fully understood,
   directly tested.  Extending it to cover v0.14–v0.17 features is
   straightforward and low-risk.
2. **Persistence:** SlateDB checkpoint round-trips are already implemented
   via `IvmTrace`.  No adapter layer needed.
3. **Worker model:** Lease-based single-writer-per-shard is preserved without
   fighting an external runtime's thread model.
4. **Recursive CTEs (v0.16):** Can be implemented as a bounded-iteration loop
   within `IvmCircuit::step()` without a generic `iterate()` operator.  The
   bounded-SQL constraint already limits CTE depth.

## Consequences

- Remove `dbsp` from `[workspace.dependencies]` (dead dependency).
- Update roadmap language: replace "DBSP operators" with "Z-difference shim"
  and "Trace/Batch/Cursor traits" with "IvmTrace checkpoint format."
- v0.15 (persistence): Extend `IvmTrace` serialization, not implement DBSP
  `Trace` trait.
- v0.16 (recursive CTEs): Implement bounded fixed-point iteration directly
  in the circuit step loop.

## References

- `crates/slateduck-ivm/src/circuit.rs` — current Z-difference engine
- `crates/slateduck-ivm/src/trace.rs` — checkpoint/restore metadata
- Feldera DBSP source: `~/.cargo/registry/src/*/dbsp-0.299.0/`
- [DBSP paper (VLDB 2023)](https://www.feldera.com/vldb23.pdf)
