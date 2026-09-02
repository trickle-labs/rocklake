# Bounded metadata

RockLake v0.51.3 gives high-cardinality catalog reads an explicit boundedness
contract. A reader is fixed to one DuckLake snapshot for the lifetime of the
operation.

## Data files

All data-file paths use `file_order ASC, data_file_id ASC`, including the Rust
reader, paged reader, streaming reader, PostgreSQL simple and extended queries,
and `COPY TO STDOUT`. Pages accept at most 1,024 rows. Streams are pull-based,
so a slow consumer buffers at most the current decoded row and dropping the
stream cancels the scan.

The continuation token is opaque and is valid only for the same table,
snapshot, and page size. Tokens from another request are rejected.

## Other high-cardinality metadata

File-column statistics, delete files, and snapshot changes each have a
pull-based stream and a page API with the same 1,024-row maximum. Their legacy
`Vec` APIs remain for compatibility and are convenience materializations over
the stream or a pre-v0.51.3 fallback.

Partition, mapping, view, macro, table-statistics, and metadata list methods
remain materializing compatibility APIs. Use the bounded alternatives where
available; their current memory cost is proportional to the returned rows.

## Limits and cancellation

Interactive data-file, delete-file, and file-stat scans use the configured
active-scan admission semaphore. Waiting for a permit is cancellation-safe.
The default total response-byte policy is unlimited; set
`--max-response-bytes` or `ROCKLAKE_MAX_RESPONSE_BYTES` when a total-result
ceiling is required. In-flight stream memory is separate from that optional
policy.

An object-store error after a stream has started is returned as an error. It is
never reported as a successful truncated result. Verification and export are
administrative/offline operations: they may scan the complete catalog, write
progress incrementally where supported, and must not use the interactive
result limit.
