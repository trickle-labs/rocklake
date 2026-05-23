# Error Codes Reference

This page lists all SQLSTATE error codes that SlateDuck may return, organized by error class.

## Class 00 — Successful Completion

| Code | Name | Description |
|------|------|-------------|
| 00000 | successful_completion | Operation completed successfully |

## Class 08 — Connection Exception

| Code | Name | Description |
|------|------|-------------|
| 08000 | connection_exception | General connection failure |
| 08003 | connection_does_not_exist | Client attempted to use a closed connection |
| 08006 | connection_failure | Network error during operation |

## Class 22 — Data Exception

| Code | Name | Description |
|------|------|-------------|
| 22023 | invalid_parameter_value | Invalid parameter (e.g., snapshot ID before retain_from) |

## Class 25 — Invalid Transaction State

| Code | Name | Description |
|------|------|-------------|
| 25001 | active_sql_transaction | Cannot perform operation while in a transaction |
| 25006 | read_only_sql_transaction | Write attempted on a read-only instance |

## Class 3F — Invalid Schema Name

| Code | Name | Description |
|------|------|-------------|
| 3F000 | invalid_schema_name | Referenced schema does not exist |

## Class 42 — Syntax Error or Access Rule Violation

| Code | Name | Description |
|------|------|-------------|
| 42000 | syntax_error_or_access_rule_violation | General syntax error |
| 42601 | syntax_error | SQL statement not recognized by bounded classifier |
| 42P01 | undefined_table | Referenced table does not exist |
| 42P06 | duplicate_schema | Schema already exists |
| 42P07 | duplicate_table | Table already exists |

## Class 57 — Operator Intervention

| Code | Name | Description |
|------|------|-------------|
| 57P04 | writer_fenced | Another writer has taken over (epoch incremented) |

## Class 58 — System Error (External)

| Code | Name | Description |
|------|------|-------------|
| 58000 | system_error | Object storage error (timeout, permission denied, etc.) |

## Class 0A — Feature Not Supported

| Code | Name | Description |
|------|------|-------------|
| 0A000 | feature_not_supported | Catalog format version not recognized |

## Class XX — Internal Error

| Code | Name | Description |
|------|------|-------------|
| XX000 | internal_error | Unexpected internal error (protobuf decode failure, invariant violation) |

## Handling Errors

Clients should use SQLSTATE codes (not error messages) for programmatic error handling. Error messages are human-readable descriptions that may change between SlateDuck versions without notice. SQLSTATE codes are stable across versions.

Common error handling patterns:

- **42601 (syntax_error):** The SQL statement is not in SlateDuck's bounded set. This is expected for arbitrary queries. Use the DuckLake-supported patterns.
- **57P04 (writer_fenced):** The connection is invalid because another writer took over. Reconnect and retry.
- **58000 (system_error):** Object storage is unavailable. Retry with exponential backoff.
- **42P01 (undefined_table):** The table does not exist at the requested snapshot. Check the snapshot ID and table name.
