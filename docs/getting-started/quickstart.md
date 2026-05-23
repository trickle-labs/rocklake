# Quickstart (Local)

This guide takes you from zero to a working SlateDuck catalog in under five minutes using your local filesystem as the storage backend. By the end, you will have created a catalog, registered a table, inserted data, and queried it through DuckDB.

## Prerequisites

You need two things installed:

- **SlateDuck binary:** Download from the releases page or build from source with `cargo build --release`
- **DuckDB 1.2+:** With the `ducklake` extension. Install it from within DuckDB: `INSTALL ducklake;`

## Step 1: Start SlateDuck

Open a terminal and start the SlateDuck sidecar pointing at a local directory. The directory will be created automatically if it does not exist:

```bash
slateduck --storage file:///tmp/my-lakehouse --bind 127.0.0.1:5432
```

You should see output like:

```
SlateDuck v0.8.0
Catalog: file:///tmp/my-lakehouse
Listening: 127.0.0.1:5432
Writer epoch: 1
```

SlateDuck is now running and ready to accept PostgreSQL wire protocol connections. The catalog has been initialized with format version 1 and all counters set to their starting values.

## Step 2: Connect DuckDB

Open a second terminal and launch DuckDB:

```bash
duckdb
```

Inside DuckDB, load the `ducklake` extension and attach the SlateDuck catalog:

```sql
LOAD ducklake;
ATTACH '' AS lakehouse (TYPE ducklake, PG 'host=127.0.0.1 port=5432');
USE lakehouse;
```

DuckDB is now connected to SlateDuck. Every catalog operation (creating tables, registering files, querying metadata) flows through the PostgreSQL wire protocol to SlateDuck, which persists it to the local filesystem via SlateDB.

## Step 3: Create a Schema and Table

```sql
CREATE SCHEMA analytics;
CREATE TABLE analytics.events (
    event_id BIGINT,
    user_id BIGINT,
    event_type VARCHAR,
    created_at TIMESTAMP,
    payload VARCHAR
);
```

Behind the scenes, DuckDB's `ducklake` extension sent a series of INSERT statements to SlateDuck: one for the schema, one for the table, and one for each column. SlateDuck allocated unique IDs from its counter system, wrote protobuf-encoded rows to SlateDB, and created a new catalog snapshot.

## Step 4: Insert Data

```sql
INSERT INTO analytics.events VALUES
    (1, 100, 'page_view', '2024-01-15 10:30:00', '{"page": "/home"}'),
    (2, 100, 'click', '2024-01-15 10:30:05', '{"button": "signup"}'),
    (3, 101, 'page_view', '2024-01-15 10:31:00', '{"page": "/pricing"}');
```

DuckDB writes the data to a Parquet file in the same storage location and then tells SlateDuck about the file by registering it in the catalog with its path, row count, file size, and column statistics.

## Step 5: Query the Data

```sql
SELECT event_type, COUNT(*) as cnt
FROM analytics.events
GROUP BY event_type
ORDER BY cnt DESC;
```

When DuckDB executes this query, it first asks SlateDuck for the list of data files belonging to `analytics.events`. SlateDuck returns the file paths and column statistics. DuckDB uses the statistics for predicate pushdown (skipping files that cannot contain relevant rows), then reads the Parquet files directly from storage.

## Step 6: Time Travel

Every operation creates a new snapshot. You can query historical states:

```sql
-- See all snapshots
SELECT * FROM ducklake_snapshots();

-- Query the catalog as it was at snapshot 2 (before data was inserted)
ATTACH '' AS lakehouse_v2 (TYPE ducklake, PG 'host=127.0.0.1 port=5432', SNAPSHOT '2');
SELECT * FROM lakehouse_v2.analytics.events;  -- Empty! The data file was registered in snapshot 3.
```

Time travel is free because SlateDuck never overwrites or deletes catalog entries. Every row has a `begin_snapshot` marking when it became visible and an optional `end_snapshot` marking when it was superseded. Querying at a specific snapshot simply filters by these bounds.

## Step 7: Inspect the Catalog

You can inspect the internal state of the catalog using the SlateDuck CLI:

```bash
slateduck inspect --storage file:///tmp/my-lakehouse
```

This displays the current snapshot ID, schema version, object counts, writer epoch, and retention settings. It is useful for debugging and monitoring.

## What Happened Under the Hood

The local directory at `/tmp/my-lakehouse` now contains SlateDB's LSM-tree structure:

```
/tmp/my-lakehouse/
  manifest/          # SlateDB manifest (current state pointer)
  wal/               # Write-ahead log segments
  compacted/         # Sorted String Tables (SSTs) after compaction
  data/              # Parquet files written by DuckDB
```

Every catalog mutation was written first to the WAL (a single PUT to object storage), then compacted in the background into sorted SST files. The catalog state is fully reconstructable from the manifest and SSTs alone.

## Next Steps

- [Quickstart (Cloud)](quickstart-cloud.md) — Run the same workflow against S3, GCS, or Azure
- [Your First Lakehouse](first-lakehouse.md) — A more realistic scenario with schema evolution and GC
- [Concepts](../concepts/index.md) — Understand the theory behind SlateDuck's design
