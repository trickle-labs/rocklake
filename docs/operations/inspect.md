# Inspect

The inspect command provides a quick summary of a catalog's internal state. It is useful for operational verification, debugging, and understanding what a catalog contains without querying through DuckDB.

## Usage

```bash
slateduck inspect --storage s3://bucket/catalog/
```

## Output

The inspect command displays:

```
SlateDuck Catalog Inspection
============================
Storage:           s3://my-bucket/catalog/
Format Version:    1
Writer Epoch:      3
Latest Snapshot:   1,247
Schema Version:    1

Counts:
  Schemas:         4
  Tables:          23
  Columns:         187
  Data Files:      1,892
  Delete Files:    12

Counters:
  Next Snapshot ID:  1,248
  Next Catalog ID:   215
  Next File ID:      1,905

Retention:
  Retain From:     1,100 (snapshots 1-1,099 are GC'd)
  Pinned:          [none]

Last Snapshot:
  ID:              1,247
  Time:            2024-03-15T14:30:22Z
  Author:          etl-pipeline
  Message:         "Registered 15 new data files for orders table"
```

## What Each Field Means

**Format Version:** The catalog format version (currently 1). If this does not match the SlateDuck binary's expected version, operations will fail with `FormatVersionMismatch`.

**Writer Epoch:** Increments each time a new writer process takes over. A high epoch indicates frequent writer restarts (worth investigating).

**Latest Snapshot:** The highest snapshot ID in the catalog. This is the "current" state that new readers see by default.

**Counts:** Total number of live (visible at latest snapshot) entities of each type. Useful for understanding catalog scale.

**Counters:** The next ID that will be allocated for each domain. These should always be higher than any existing ID of that type.

**Retention:** The GC horizon. Snapshots before `retain_from` are not queryable via time travel.

**Last Snapshot:** Details of the most recent catalog mutation. Useful for verifying that writes are happening and identifying the source.

## JSON Output

For programmatic consumption, use the `--format json` flag:

```bash
slateduck inspect --storage s3://bucket/catalog/ --format json
```

This outputs the same information as a JSON object suitable for parsing by monitoring scripts or dashboards.
