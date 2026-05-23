# Wire Corpus

The wire corpus is a test suite of recorded PostgreSQL wire protocol sessions captured from actual DuckDB interactions with DuckLake catalogs. It serves as the ground truth for SlateDuck's SQL classifier — every statement pattern in the corpus must be correctly classified and handled.

## Purpose

DuckDB's `ducklake` extension generates SQL statements that are sent to the catalog backend. These statements follow patterns that vary slightly between DuckDB versions (column ordering, quoting style, parameter formatting). The wire corpus captures these exact patterns so that SlateDuck can be tested against real-world input rather than hand-written test cases.

## Structure

The corpus lives at `tests/fixtures/wire-corpus/` and contains `.sql` files organized by DuckDB version and operation category:

```
tests/fixtures/wire-corpus/
├── duckdb-1.2.0/
│   ├── create-schema.sql
│   ├── create-table.sql
│   ├── insert-data-file.sql
│   ├── list-schemas.sql
│   ├── list-tables.sql
│   └── ...
└── duckdb-1.2.2/
    ├── create-schema.sql
    └── ...
```

Each file contains one or more SQL statements exactly as DuckDB emits them, including whitespace, quoting, and parameter formatting.

## How Corpus Tests Work

The corpus test runner (`tests/golden/`) reads each `.sql` file, passes the SQL through SlateDuck's `classify_statement()` function, and verifies that:

1. The statement is successfully classified (not rejected as unknown)
2. The classification matches the expected `StatementKind` variant
3. The extracted parameters (schema name, table name, column list, etc.) match expected values

If any of these checks fail, the test fails, indicating that SlateDuck would not correctly handle that DuckDB version's output.

## Capturing New Corpus Entries

When a new DuckDB version is released, new corpus entries are captured by:

1. Running DuckDB with the new version against a PostgreSQL-backed DuckLake catalog
2. Enabling wire protocol logging on the PostgreSQL side
3. Extracting the SQL statements from the logs
4. Adding them to the corpus under a new version directory
5. Running the corpus tests to verify SlateDuck handles them

This process ensures that SlateDuck stays compatible with DuckDB as it evolves.

## Relationship to Bounded SQL

The wire corpus defines the boundary of bounded SQL. If a statement pattern exists in the corpus, SlateDuck must handle it. If it does not exist in the corpus, SlateDuck is not required to handle it (and will reject it). This makes the corpus the authoritative specification of SlateDuck's SQL surface.
