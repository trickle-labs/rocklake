# SQLSTATE Mapping

SlateDuck maps internal Rust error types to PostgreSQL SQLSTATE codes for transmission over the PG-wire protocol. This ensures that DuckDB (and other PostgreSQL-compatible clients) receive standard error codes that they can handle programmatically.

## How It Works

The mapping is implemented in `crates/slateduck-pgwire/src/error.rs`. Each internal error variant is associated with a 5-character SQLSTATE code:

| Internal Error | SQLSTATE | Class | Meaning |
|---------------|----------|-------|---------|
| `StorageError` | 58000 | System Error | Object storage unavailable |
| `WriterFenced` | 57P04 | Operator Intervention | Another writer took over |
| `FormatVersionMismatch` | 0A000 | Feature Not Supported | Catalog format unrecognized |
| `DecodeError` | XX000 | Internal Error | Value deserialization failed |
| `InvalidStatement` | 42601 | Syntax Error | SQL not recognized |
| `TableNotFound` | 42P01 | Undefined Table | Referenced table does not exist |
| `SchemaNotFound` | 3F000 | Invalid Schema Name | Referenced schema does not exist |
| `DuplicateTable` | 42P07 | Duplicate Table | Table already exists |
| `DuplicateSchema` | 42P06 | Duplicate Schema | Schema already exists |
| `TransactionActive` | 25001 | Active SQL Transaction | Cannot perform operation in transaction |
| `ReadOnly` | 25006 | Read Only Transaction | Write attempted in read-only mode |
| `SnapshotTooOld` | 22023 | Invalid Parameter Value | Requested snapshot before retain_from |

## Design Principles

**Use standard codes where possible.** PostgreSQL defines hundreds of SQLSTATE codes covering most error categories. SlateDuck reuses existing codes rather than inventing custom ones.

**Use class-appropriate codes.** The first two characters of SQLSTATE define the error class. SlateDuck ensures errors are in the correct class (e.g., storage errors in class 58 "System Error", not class 42 "Syntax Error").

**Vendor-specific codes for unique conditions.** For error conditions unique to SlateDuck (like `WriterFenced`), the nearest PostgreSQL code is used. `57P04` (database_dropped) is repurposed because it represents a similar "your session is invalid" condition.

## Client Usage

Well-behaved clients should use SQLSTATE codes for error handling, not error message text:

```python
try:
    cursor.execute("CREATE TABLE ...")
except psycopg2.errors.DuplicateTable:
    # Handle duplicate table (SQLSTATE 42P07)
    pass
except psycopg2.errors.ReadOnlySqlTransaction:
    # Handle read-only (SQLSTATE 25006)
    pass
```

## Error Severity

All errors are reported with severity ERROR (not FATAL or PANIC). The connection remains usable after an error — the client can send additional queries. Only authentication failures terminate the connection.
