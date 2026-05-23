# Bounded SQL

SlateDuck's SQL layer is fundamentally different from what you might expect of a database. It is not a general-purpose query engine with a parser, planner, optimizer, and executor. Instead, it is a **bounded dispatcher** that recognizes exactly the SQL statement shapes emitted by supported DuckLake clients and maps each one to a specific catalog operation. Anything outside this bounded set is rejected immediately with SQLSTATE `0A000` (feature not supported).

## What "Bounded" Means

A bounded SQL dispatcher has a finite, enumerable set of supported statement patterns. You can list every single SQL shape it accepts, and that list is complete. There are no edge cases lurking in a complex grammar, no unexpected interactions between features, and no ambiguity about what is supported. The current implementation recognizes approximately 50 distinct statement kinds, covering session management, transactions, reads (selecting metadata), and writes (inserting or updating catalog entries).

This is in stark contrast to a general SQL engine like PostgreSQL, which supports an effectively infinite space of valid SQL statements through the combination of subqueries, CTEs, window functions, lateral joins, and user-defined functions. General engines must handle any valid SQL, which means they need sophisticated query planners and optimizers that introduce complexity and potential for bugs.

## Why Not a Full SQL Engine?

The decision to use a bounded dispatcher rather than a full SQL engine was made for four reinforcing reasons:

**Security through a finite attack surface.** Because the set of accepted statements is finite and enumerable, it is possible to audit every single code path that processes SQL input. There is no SQL injection risk because there is no dynamic query construction. There is no way to exfiltrate data through clever query composition because there are no joins, subqueries, or user-defined functions. The entire surface area is a match statement with approximately 50 arms.

**Correctness through exhaustive coverage.** When the statement space is finite, you can write a test for every single variant. SlateDuck's test suite covers 100% of the statement kinds, including error cases. This level of coverage would be impossible for a general SQL engine where the statement space is infinite.

**Performance through direct dispatch.** Classifying an incoming SQL statement is essentially pattern matching on the AST produced by `sqlparser-rs`. There is no query planning phase, no cost estimation, no optimizer rules. The dispatcher identifies the statement kind in constant time and directly invokes the corresponding catalog operation. This makes the SQL layer essentially zero-overhead.

**Maintainability through simplicity.** The entire SQL classifier is approximately 1500 lines of Rust in a single file. It is straightforward match-and-extract logic with no state machines, no recursion, and no dynamic dispatch. A new developer can understand the entire SQL layer in an afternoon.

## What Is Supported

The bounded set covers everything that DuckDB's `ducklake` extension needs to manage a catalog:

**Session statements:** Version queries (`SELECT version()`), current schema/database queries, `SET` statements for timezone and encoding, and `pg_type` queries for type OID resolution.

**Transaction control:** `BEGIN`, `COMMIT`, and `ROLLBACK`. Transactions buffer multiple write operations and apply them atomically.

**Catalog reads:** Listing schemas, tables, columns, views, and macros. Querying data files and delete files for a table. Reading column statistics for predicate pushdown. Fetching metadata key-value pairs. Reading snapshot history.

**Catalog writes:** Creating schemas, tables, columns, views, and macros. Registering data files and delete files. Updating table statistics. Writing metadata. Creating snapshots and recording snapshot changes.

**Versioning:** Setting `end_snapshot` on superseded rows (for schema evolution, table drops, etc.).

## How Classification Works

When a SQL string arrives over the PostgreSQL wire protocol, SlateDuck parses it using `sqlparser-rs` with the PostgreSQL dialect. The resulting AST is then matched against known patterns:

1. Is it a SELECT? Check the target list and FROM clause against known catalog query shapes.
2. Is it an INSERT? Check the target table name against known catalog table names.
3. Is it an UPDATE? Check the table name and SET clause against known mutation patterns.
4. Is it a transaction statement? Map directly to Begin/Commit/Rollback.

If none of the patterns match, the statement is classified as `Unsupported` and an error is returned to the client. The client never sees partial results or side effects from unsupported statements because classification happens before any catalog operations are performed.

## Extending the Bounded Set

When a new version of DuckDB's `ducklake` extension introduces new catalog operations, SlateDuck must be updated to recognize the new statement shapes. This is a deliberate design choice: compatibility is explicit, not implicit. Each new statement shape is added as a new variant to the `StatementKind` enum, a new arm in the classifier, and a new handler in the executor. The corresponding tests are added at the same time.

This means that SlateDuck's compatibility with DuckDB versions is well-defined and testable. You can look at the `StatementKind` enum and know exactly which DuckDB features are supported.
