# DuckDB Compatibility

This page documents the SQL compatibility between SlateDuck and DuckDB's `ducklake` extension. SlateDuck aims to be a transparent replacement for DuckLake's built-in PostgreSQL/SQLite catalog backends — DuckDB should not be able to tell the difference.

## Compatibility Matrix

### Schema Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| CREATE SCHEMA | Supported | |
| DROP SCHEMA | Supported | Cascades to contained tables |
| ALTER SCHEMA RENAME | Supported | |

### Table Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| CREATE TABLE | Supported | All DuckDB types supported |
| DROP TABLE | Supported | |
| ALTER TABLE ADD COLUMN | Supported | |
| ALTER TABLE DROP COLUMN | Supported | |
| ALTER TABLE RENAME COLUMN | Supported | |
| ALTER TABLE SET SCHEMA | Supported | Move table between schemas |
| ALTER TABLE RENAME | Supported | |

### Data Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| INSERT (file registration) | Supported | Registers data file metadata |
| DELETE (file deregistration) | Supported | Marks files as deleted |
| File statistics | Supported | Min/max/null count per column |
| Partition pruning | Supported | Via column statistics |

### Transaction Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| BEGIN TRANSACTION | Supported | Implicit snapshot allocation |
| COMMIT | Supported | Atomic catalog update |
| ROLLBACK | Supported | Discard pending changes |
| Time travel (AT SNAPSHOT) | Supported | Read historical catalog state |

### View and Macro Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| CREATE VIEW | Supported | |
| DROP VIEW | Supported | |
| CREATE MACRO | Supported | |
| DROP MACRO | Supported | |

## DuckDB Version Compatibility

SlateDuck's bounded SQL dispatcher is validated against specific DuckDB versions using a wire corpus test suite. The corpus records actual SQL emitted by each DuckDB version and verifies that SlateDuck classifies and handles it correctly.

| DuckDB Version | SlateDuck Compatibility |
|----------------|------------------------|
| 1.2.0+ | Full compatibility |
| 1.1.x | Partial (older ducklake SQL patterns) |
| < 1.1 | Not supported (no ducklake extension) |

## Known Differences

**Transaction isolation:** DuckLake on PostgreSQL provides true SERIALIZABLE isolation. SlateDuck provides snapshot isolation (readers see a consistent snapshot, writers serialize via single-writer model). The practical difference is negligible for catalog workloads.

**Error messages:** SlateDuck returns SQLSTATE error codes compatible with PostgreSQL but with different human-readable messages. Well-behaved clients should use SQLSTATE codes, not message text, for error handling.

**Connection state:** Some PostgreSQL session variables (e.g., `search_path`, `client_encoding`) are acknowledged but have no effect in SlateDuck. They are accepted to maintain protocol compatibility.
