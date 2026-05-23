# GlueSQL Spike Results

## Objective

Evaluate GlueSQL as the SQL execution layer for the Strategy B pgwire sidecar.

## Spike Findings

### Decision: Custom AST-matching dispatcher (fewer than 10 shims)

After analyzing the DuckLake wire corpus and the bounded set of SQL statements
emitted by DuckDB's `ducklake` extension, we have determined that a **custom
AST-matching dispatcher** based on `sqlparser-rs` is the correct approach.

### Rationale

1. **Bounded SQL surface**: The wire corpus shows DuckDB emits fewer than 20
   distinct SQL statement shapes against the catalog. These are well-defined
   patterns (INSERT with explicit values, SELECT with MVCC WHERE clauses,
   UPDATE of end_snapshot).

2. **GlueSQL shim count**: GlueSQL would require the following PostgreSQL-specific
   shims to handle DuckDB's expectations:
   - `pg_catalog.pg_type` virtual table (type OID resolution)
   - `current_schema()` function
   - `version()` function
   - `SET`/`SHOW` statement handling
   - PostgreSQL-style `BEGIN`/`COMMIT`/`ROLLBACK`
   - `IS NULL` vs `ISNULL` syntax
   - `TIMESTAMPTZ` type handling
   - `JSONB` type handling
   - Parameter binding (`$1`, `$2`) syntax

   Count: **9 shims** — at the boundary of the decision gate.

3. **Performance concern**: GlueSQL adds a full SQL execution engine layer
   between the wire protocol and the KV store. Since we control both the
   input (well-known SQL shapes) and the output (SlateDB operations), the
   overhead is unnecessary.

4. **Maintenance burden**: Custom dispatcher directly maps AST patterns to
   typed Rust functions. This is easier to test, audit, and extend than
   maintaining compatibility with GlueSQL's evolving API.

### Decision Gate Result

**Adopt custom AST-matching dispatcher** (`slateduck-sql` crate using `sqlparser-rs`).

The bounded SQL surface means we pattern-match on ~20 AST shapes rather than
implementing a general-purpose SQL engine. Each shape maps directly to a typed
`CatalogStore` method call.

### Shims Required for Custom Dispatcher

| # | Shim | Implementation |
|---|------|---------------|
| 1 | `pg_catalog.pg_type` | Hardcoded response table |
| 2 | `current_schema()` | Return "main" |
| 3 | `version()` | Return SlateDuck version string |
| 4 | `SET` statements | Accept and store in session state |
| 5 | `SHOW` statements | Return from session state |
| 6 | `BEGIN`/`COMMIT`/`ROLLBACK` | Map to transaction buffering |
| 7 | Type OID encoding | Lookup table for PG type OIDs |

Total: **7 shims** for the custom dispatcher path (fewer than GlueSQL's 9).
