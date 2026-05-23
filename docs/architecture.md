# SlateDuck Architecture

## Overview

SlateDuck is a DuckLake catalog backed by SlateDB — both catalog and data
reside in the same object-storage bucket, with no external database server.

## Component Diagram

```
┌─────────────────────────────────────────────────────────┐
│                      DuckDB Client                       │
│                  (ducklake extension)                     │
└──────────────────────────┬──────────────────────────────┘
                           │ PostgreSQL Wire Protocol
                           ▼
┌─────────────────────────────────────────────────────────┐
│                   slateduck-pgwire                        │
│              (Strategy B: PG-wire sidecar)                │
├─────────────────────────────────────────────────────────┤
│                    slateduck-sql                          │
│            (Bounded SQL AST dispatcher)                   │
├─────────────────────────────────────────────────────────┤
│                  slateduck-catalog                        │
│           (DuckLake spec operations in Rust)              │
├─────────────────────────────────────────────────────────┤
│                   slateduck-core                          │
│        (Key layout, encoding, tags, error types)          │
└──────────────────────────┬──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│                       SlateDB                             │
│          (Embedded LSM KV store on object storage)        │
└──────────────────────────┬──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│                    Object Storage                         │
│              (S3 / GCS / Azure / LocalFS)                 │
│                                                           │
│   catalogs/         ← SlateDB WAL, SSTs, manifest         │
│   data/             ← Parquet data files                  │
└─────────────────────────────────────────────────────────┘
```

## Strategies

### Strategy B (v0.3): PG-Wire Sidecar

A small stateless sidecar process translates PostgreSQL wire protocol
(as spoken by DuckDB's `postgres` extension) into `CatalogStore` operations
against SlateDB. Zero configuration; just point DuckDB at the sidecar.

### Strategy C (v0.5): Native DuckDB Extension

A DuckDB extension that implements the DuckLake catalog interface directly
via Rust FFI, eliminating the network hop and SQL emulation entirely.

## Key Design Decisions

1. **Single writer, many readers** — enforced by SlateDB writer fencing
2. **Big-endian key encoding** — natural sort order for prefix scans
3. **Protobuf values** — forward/backward compatible schema evolution
4. **MVCC in application layer** — DuckLake snapshot IDs, not SlateDB-level
5. **Value magic prefix** — `b"SDKV"` for safe format detection
