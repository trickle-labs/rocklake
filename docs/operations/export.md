# Export and Import

RockLake provides two NDJSON export commands:

- `export` writes the catalog rows understood by the general catalog export
  path.
- `export-catalog` writes the complete DuckLake catalog export used by the
  v0.51.0 quickstart and restore workflow.

Both commands are read-only. Snapshot selection is explicit: omit the option
for the latest state, or pass an exact ID.

## Export

```bash
rocklake export \
  --catalog s3://bucket/catalog/ \
  --output catalog.ndjson \
  --snapshot-id 42
```

## Complete DuckLake export

```bash
rocklake export-catalog \
  --catalog s3://bucket/catalog/ \
  --out catalog-at-42.ndjson \
  --at-snapshot 42
```

The output is newline-delimited JSON. Each line contains a catalog table name
and the row data. The export command does not write to object storage; `--out`
and `--output` are local filesystem paths.

## Import

```bash
rocklake import \
  --catalog s3://bucket/restored-catalog/ \
  --input catalog.ndjson
```

Inspect or verify a restored catalog before serving it:

```bash
rocklake inspect snapshot --catalog s3://bucket/restored-catalog/
rocklake verify catalog --catalog s3://bucket/restored-catalog/
```

Run `rocklake export --help`, `rocklake export-catalog --help`, or
`rocklake import --help` for the current typed argument definitions.
