# Changelog

All notable changes to SlateDuck are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.8.0] - 2025-01-15

### Added
- Complete documentation site with 80+ pages covering all aspects of SlateDuck
- Performance metrics collection (Prometheus-compatible)
- Hot key caching for frequently-read system keys
- Secondary index support for partition-based access patterns
- Encryption module (AES-256-GCM) for value-level encryption at rest
- Partitioned writer support for multi-dataset workloads
- Audit logging for destructive operations (excision, GC)
- DataFusion integration via CatalogProvider trait
- Health check endpoints for Kubernetes deployments
- NDJSON export/import for backup and migration

### Changed
- Improved error messages with SQLSTATE codes throughout
- Write batching optimization reduces S3 PUT count by 3-5x for bulk operations
- Upgraded SlateDB dependency to 0.13 for improved compaction
- Upgraded pgwire dependency to 0.28 for protocol compliance fixes

### Fixed
- MVCC visibility filter edge case for snapshot ID 0
- Key encoding correctness for maximum u64 values
- Session cleanup on abrupt client disconnect
- Wire corpus compatibility with DuckDB 1.2.2 column ordering changes

## [0.7.0] - 2024-12-01

### Added
- GC (garbage collection) command with configurable retention
- Excision command for physical deletion of superseded rows
- Verify command for catalog integrity checking
- Repair command for conservative auto-repair
- Checkpoint command for named restore points
- Wire corpus test suite for DuckDB 1.2.0 and 1.2.2

### Changed
- Improved SQL classifier accuracy for edge cases in DuckDB output
- Reduced memory usage by 40% through arena allocation for key encoding
- Better error reporting for object storage connectivity issues

### Fixed
- Writer fencing race condition during rapid failover
- Protobuf decode error for columns with very long default expressions
- Counter overflow handling for catalogs with > 2^53 snapshots

## [0.6.0] - 2024-10-15

### Added
- PG-wire protocol implementation (Strategy B)
- SQL statement classifier with ~50 recognized patterns
- Session management with configurable max connections
- TLS support for encrypted client connections
- Password authentication support

### Changed
- Migrated from custom binary protocol to PostgreSQL wire protocol
- Unified error handling with SQLSTATE codes

## [0.5.0] - 2024-08-01

### Added
- Native DuckDB extension (Strategy C) via FFI
- CatalogStore, CatalogReader, CatalogWriter abstractions
- Complete DuckLake protocol table support (28 table types)
- Property-based tests for key encoding

## [0.4.0] - 2024-06-01

### Added
- Initial SlateDB integration
- Key encoding scheme (tag + big-endian u64)
- Value envelope format (SDKV)
- Protobuf row serialization via prost
- MVCC visibility filter
- Counter-based ID allocation
