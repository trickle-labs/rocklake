# Upgrades

Install the v0.51.4 binary, stop the current process, and restart it with the
same catalog URL and flags. Catalog state remains in the configured object
store.

```bash
# Verify new binary version
rocklake --version

# Preflight check before restarting service
rocklake doctor --catalog s3://bucket/catalog/

# Inspect catalog readiness and snapshot state
rocklake status --catalog s3://bucket/catalog/

# Start the service
rocklake serve --catalog s3://bucket/catalog/ --bind 127.0.0.1:5432
```

Before upgrading production workloads:
1. Run `rocklake status --catalog ...` to verify current snapshot ID and format version.
2. Create a backup via `rocklake catalog backup create --catalog <url> --out <backup-dir>`.
3. Check `rocklake doctor --catalog ...` with the new binary to confirm permissions and credentials.
4. Replace the binary and restart `rocklake serve`.
5. Verify client connectivity via DuckDB or `rocklake status`.

There is no published Docker image or RockLake-specific container upgrade path; use the release binary.
