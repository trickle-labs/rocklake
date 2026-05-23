# Phase 0 Go/No-Go Decisions

## Summary

All Phase 0 gates pass. The design is validated. Proceed to Phase 1 (v0.2).

## Decision Record

| # | Decision | Outcome | Rationale |
|---|----------|---------|-----------|
| 1 | GlueSQL vs. custom dispatcher | **Custom dispatcher** | Bounded SQL surface (< 20 shapes); 7 shims vs. 9 for GlueSQL; better testability |
| 2 | Transaction API | **`db.begin(SerializableSnapshot)`** | All gates pass; atomic counter + row in single txn |
| 3 | Conditional init | **`DbTransaction` get-then-put** | Insert-if-absent works reliably with SSI |
| 4 | `flush()` barrier | **Reliable** | Write → flush → reader sees key; use as visibility barrier |
| 5 | `pgwire` crate extended-protocol | **Supported** | pgwire v0.28 supports Parse/Bind/Execute/Sync |
| 6 | Counter allocation | **Single DbTransaction** | Counter + consumed row commit atomically; monotonicity guaranteed |
| 7 | Writer fencing | **SlateDB manifest-level** | Map to SQLSTATE 57P04; SlateDuck-own epoch as defense-in-depth |
| 8 | WriteBatch size | **No observed SlateDB limit** | Enforce SlateDuck's own 64 MiB limit unconditionally |
| 9 | Prefix scan semantics | **Latest merged values** | Safe for MVCC filter layer; no dedup needed |
| 10 | Credential isolation | **Separate IAM policies** | catalog-only / data-only / gc-both validated |

## Blocked Items

None. All assumptions validated successfully.

## Next Steps

Proceed to v0.2 — Catalog Core:
- Implement full binary key layout for all 28 tables
- Implement Protobuf value encoding
- Implement counter allocation with proptest
- Implement MVCC filter layer
- Implement CatalogStore public API
