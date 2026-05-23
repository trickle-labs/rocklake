# Bounded SQL (Design Decision)

This page documents the decision to implement a bounded SQL dispatcher rather than a general-purpose SQL engine. For a detailed explanation of what bounded SQL is and how it works, see [Concepts: Bounded SQL](../concepts/bounded-sql.md). This page focuses on the trade-off analysis.

## The Decision

SlateDuck's SQL layer recognizes exactly the SQL statement shapes emitted by DuckDB's `ducklake` extension (approximately 50 patterns) and rejects everything else. It does not support arbitrary queries, joins, subqueries, aggregations, or user-defined functions.

## Alternatives Considered

**Embed a full SQL engine (GlueSQL, DataFusion SQL).** This would allow arbitrary queries against the catalog. However, it would massively increase the attack surface, introduce query planning complexity, require a query optimizer, and create an expectation of general SQL support that we could not maintain.

**Implement a subset SQL engine.** Support SELECT with WHERE and basic operators but not joins or subqueries. This middle ground would be worse than either extreme: complex enough to have bugs but limited enough to frustrate users who expect more.

**No SQL at all — binary protocol.** Define a custom binary protocol for catalog operations. This would be maximally efficient but would require a custom DuckDB extension that speaks the binary protocol, eliminating compatibility with existing DuckLake tooling.

## Why Bounded?

**The DuckLake protocol surface is finite and well-defined.** DuckDB's `ducklake` extension emits a fixed set of SQL patterns. These patterns change only with new DuckDB releases, and each new pattern can be added explicitly. We do not need to handle arbitrary SQL because our clients do not send arbitrary SQL.

**Finite surface = provable security.** With approximately 50 statement kinds, every single code path is testable and auditable. This is not true for a general SQL engine where the statement space is combinatorially explosive.

**Zero query planning overhead.** Classification is O(1) pattern matching. There is no cost model, no optimizer rules, no plan enumeration. This matters for workloads with thousands of small catalog queries per second.

**Maintenance burden stays constant.** A general SQL engine requires ongoing investment in optimizer rules, type coercion logic, and edge case handling. The bounded dispatcher's maintenance is proportional to the number of supported patterns, which grows slowly (a few per DuckDB major version).

## The Cost

**Not usable as a general-purpose catalog query tool.** You cannot run `SELECT COUNT(*) FROM ducklake_table` or `SELECT t.name, COUNT(f.id) FROM tables t JOIN files f ON ...`. If you need ad-hoc catalog queries, export the catalog to NDJSON and query it with DuckDB directly.

**Tight coupling to DuckDB's SQL patterns.** If DuckDB changes how it formats a query (e.g., reorders columns in a SELECT list), SlateDuck's classifier must be updated. This coupling is managed through a wire corpus test suite that records actual DuckDB sessions.

**Cannot serve non-DuckLake PostgreSQL clients.** A general PostgreSQL client (psql, pgAdmin, Grafana) will find that most queries fail. SlateDuck is not a PostgreSQL replacement — it only speaks the specific subset that DuckLake needs.
