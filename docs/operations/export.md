# Export

The export command extracts catalog data as NDJSON (Newline-Delimited JSON) for backup, migration, analysis, or compliance purposes. Each line in the output represents one catalog row with its table name and field values.

## Usage

```bash
# Export current state
slateduck export --storage s3://bucket/catalog/ --output catalog.ndjson

# Export at a specific snapshot
slateduck export --storage s3://bucket/catalog/ --snapshot 1000 --output catalog-at-1000.ndjson

# Export to stdout (for piping)
slateduck export --storage s3://bucket/catalog/
```

## Output Format

Each line is a JSON object with `table` and `data` fields:

```json
{"table":"ducklake_schema","data":{"schema_id":1,"schema_name":"public","begin_snapshot":1}}
{"table":"ducklake_table","data":{"table_id":1,"schema_id":1,"table_name":"events","begin_snapshot":2}}
{"table":"ducklake_column","data":{"column_id":1,"table_id":1,"column_name":"event_id","data_type":"BIGINT","column_index":0,"begin_snapshot":2,"is_nullable":false}}
```

The export includes only rows visible at the specified snapshot (or the latest snapshot if none is specified). Superseded rows (those with `end_snapshot` before the target) are not included. This gives you a clean point-in-time view of the catalog.

## Use Cases

**Disaster recovery:** Export regularly (daily or weekly) and store the NDJSON files in a separate location. If the catalog becomes corrupted, reimport from the most recent export.

**Migration:** Export from one storage backend (e.g., local filesystem) and import to another (e.g., S3). This is useful when moving from development to production.

**Analysis:** Load the NDJSON into DuckDB, pandas, or any other tool for ad-hoc queries about catalog contents. Useful for answering questions like "how many columns does each table have?" or "what is the total data size per schema?"

**Compliance:** Produce a human-readable audit of all catalog state at a specific point in time for regulatory review.

## Import

To restore from an export:

```bash
slateduck import --storage s3://bucket/new-catalog/ --input catalog.ndjson
```

Import creates a fresh catalog and populates it with the exported rows. IDs are reassigned during import (the original IDs are not preserved) because the new catalog has its own counter sequence.
