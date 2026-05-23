# Troubleshooting

This page covers common problems encountered when running SlateDuck and their solutions.

## Connection Errors

### "connection refused" when DuckDB tries to connect

**Cause:** SlateDuck is not running or is listening on a different address/port.

**Solution:** Verify SlateDuck is running (`ps aux | grep slateduck`), check the bind address and port in the startup output, and ensure there is no firewall blocking the connection.

### "WriterFenced" error (SQLSTATE 57P04)

**Cause:** Another SlateDuck instance has taken over the writer role by incrementing the epoch.

**Solution:** This is expected during failover. The old instance should be terminated. If you see this unexpectedly, check for duplicate SlateDuck processes pointing at the same catalog.

### "FormatVersionMismatch" on startup

**Cause:** The SlateDuck binary does not recognize the catalog's format version. Either the catalog was created by a newer version, or it is corrupted.

**Solution:** Ensure you are running the correct SlateDuck version for your catalog. If you recently downgraded, you may need to upgrade back to the version that performed the migration.

## Performance Issues

### Slow catalog queries (> 500ms)

**Possible causes:**
- High latency to object storage (check network, region mismatch)
- Many superseded rows causing scan amplification (run GC + excision)
- Large number of data files per table (expected for large tables; consider partitioning)
- SlateDB compaction backlog (check compaction metrics)

**Solution:** Run `slateduck inspect` to check row counts. If there are many more rows than expected, run GC. Check object store latency with a simple GET test.

### DuckDB queries are slow after connecting to SlateDuck

**Cause:** DuckDB's `ducklake` extension makes multiple catalog round-trips per query (list files, get stats, etc.). If each round-trip takes 50-100ms, a query with 10 catalog calls adds 500-1000ms of overhead.

**Solution:** This is inherent to the architecture when using S3 Standard. Options: use S3 Express One Zone for lower latency, switch to the native extension (Strategy C) for in-process catalog access, or batch data files into fewer, larger Parquet files.

## Storage Errors

### "ObjectStore: 403 Forbidden"

**Cause:** Insufficient IAM permissions for the configured storage path.

**Solution:** Ensure the IAM role/user has `s3:GetObject`, `s3:PutObject`, `s3:ListBucket`, and `s3:DeleteObject` permissions on the catalog path prefix.

### "ObjectStore: 429 Too Many Requests"

**Cause:** S3 request rate limiting. This can happen during heavy compaction or when scanning very large catalogs.

**Solution:** SlateDuck retries automatically with exponential backoff. If sustained, consider spreading catalog data across multiple S3 prefixes or reducing scan frequency.

## Data Integrity

### Verify reports errors

**Solution:** Run `slateduck repair --dry-run` to see what repairs are proposed. If repairs are available, apply them. If the error is unrecoverable, restore from an NDJSON backup or checkpoint.

### Unexpected empty results from catalog queries

**Possible causes:**
- Reading at a snapshot before the entities were created
- GC has advanced `retain_from` past the target snapshot
- Writer fencing: writes went to a different catalog instance

**Solution:** Check the current snapshot with `slateduck inspect`. Verify `retain_from` is not too aggressive. Ensure all DuckDB instances connect to the same SlateDuck instance.
