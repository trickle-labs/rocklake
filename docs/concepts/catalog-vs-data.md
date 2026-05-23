# Catalog vs Data

SlateDuck draws a sharp architectural line between **catalog** (metadata about your lakehouse) and **data** (the actual analytical records stored in Parquet files). Understanding this separation is essential for reasoning about SlateDuck's resource usage, failure modes, scaling characteristics, and operational model.

## What Lives in the Catalog

The catalog contains everything DuckDB needs to know *about* your data without actually reading it:

- **Schemas:** Named containers for organizing tables (like PostgreSQL schemas or Hive databases)
- **Tables:** Definitions including table ID, name, associated schema, and data path
- **Columns:** Column names, types, ordering, nullability, default values, and version history
- **Data files:** Paths to Parquet files, file sizes, row counts, and the snapshot at which they were registered
- **Delete files:** References to deletion vectors (for row-level deletes without rewriting Parquet files)
- **Column statistics:** Min/max values per column per file, null counts, and NaN presence (for predicate pushdown)
- **Snapshots:** Historical record of every catalog mutation, with timestamps and optional author/message
- **Views and macros:** SQL view definitions and macro implementations

All of this metadata is stored as protobuf-encoded key-value pairs in SlateDB. The total size of a typical catalog is small relative to the data it describes: a catalog tracking 10,000 Parquet files with 50 columns each might occupy 50-100 MB of storage, while the Parquet files themselves could be terabytes.

## What Lives Outside the Catalog

The actual analytical data — the rows you query with DuckDB — is stored in Parquet files that DuckDB reads directly from object storage. SlateDuck never sees this data. It never reads Parquet files, never parses them, and never caches their contents. Its only interaction with data files is recording their existence in the catalog (path, size, row count, statistics).

This separation has profound implications:

**SlateDuck's resource usage is bounded by catalog size, not data size.** A catalog tracking 1 TB of data uses the same memory and CPU as one tracking 1 PB of data (assuming similar numbers of files and columns). You can scale your data storage independently of your catalog infrastructure.

**SlateDuck never becomes a bottleneck for read queries.** Once DuckDB has obtained the list of relevant data files from the catalog, it reads them directly from object storage in parallel. SlateDuck is out of the critical path for the actual data scan.

**Catalog operations and data operations have different failure domains.** If SlateDuck is temporarily unavailable, existing DuckDB sessions that have already cached the file list can continue reading data. New sessions or DDL operations will fail until SlateDuck recovers, but reads in progress are unaffected.

## The Interaction Pattern

The interaction between DuckDB, SlateDuck, and object storage follows a consistent pattern:

1. **DuckDB asks SlateDuck:** "What tables exist in schema X? What columns does table Y have? What Parquet files contain data for table Y? What are the min/max statistics for column Z in each file?"

2. **SlateDuck responds** with metadata from the catalog, filtered to the appropriate snapshot.

3. **DuckDB reads data directly** from the Parquet files listed by SlateDuck, using the column statistics for predicate pushdown to skip irrelevant files.

4. **For writes, DuckDB writes Parquet files** to object storage and then tells SlateDuck about them by issuing INSERT statements that register the file path, row count, size, and statistics in the catalog.

This pattern means SlateDuck handles many small metadata requests (typically returning a few hundred rows at most) while DuckDB handles the heavy lifting of scanning potentially gigabytes of Parquet data.

## Why This Separation Matters

**Scaling reads is trivial.** You can run hundreds of DuckDB instances concurrently, all reading from the same catalog and the same Parquet files. SlateDuck serves metadata from an LSM-tree that supports unlimited concurrent readers. The Parquet files are immutable objects that support unlimited concurrent GET requests.

**Failure recovery is simple.** If SlateDuck crashes, you restart it. The catalog state is fully persistent in object storage. There is nothing to recover, replay, or reconcile. DuckDB sessions in progress may get a connection error and need to reconnect, but no data is lost.

**Security boundaries are clean.** SlateDuck only needs access to the catalog prefix in your bucket. It does not need read access to your Parquet files. You can configure IAM policies that give SlateDuck write access only to the catalog prefix and give DuckDB read access to the data prefix. This limits the blast radius of a compromised SlateDuck process.

**Cost accounting is transparent.** You can separately measure and optimize the cost of catalog operations (small, frequent requests to a small storage footprint) versus data operations (large, parallel reads from a large storage footprint). They have different optimization strategies and different cost profiles.
