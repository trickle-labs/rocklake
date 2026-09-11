# Upgrades

The v0.61.0 binary can be upgraded directly from v0.60.0. Stop the current
process and restart it with the same catalog URL and flags. Catalog state
remains in the configured object store; v0.61.0 does not change the catalog
storage format.

```bash
# Verify new binary version
rocklake --version

# Preflight check before restarting service
rocklake doctor --catalog s3://bucket/catalog/

# Inspect catalog readiness and snapshot state
rocklake status --catalog s3://bucket/catalog/ --output json

# Preview the typed migration plan; this is read-only
rocklake catalog migrate --catalog s3://bucket/catalog/ --dry-run

# Start the service
rocklake serve --catalog s3://bucket/catalog/ --bind 127.0.0.1:5432
```

Before upgrading production workloads:
1. Run `rocklake status --catalog ... --output json` to record all version domains and the read/write compatibility contract.
2. Create a backup via `rocklake catalog backup create --catalog <url> --out <backup-dir>`.
3. Check `rocklake doctor --catalog ...` with the new binary to confirm permissions and credentials.
4. Replace the binary and restart `rocklake serve`.
5. Verify client connectivity via DuckDB or `rocklake status`.

`rocklake catalog migrate --apply` records an administrative migration job and
verifies the catalog before activation. In v0.61.0 the registered migration is
a verified no-op. Unknown targets and downgrades fail before a write. After a
future format-changing migration, rollback is restore-only from a compatible
backup; an older binary must not write the activated format.

The supported mixed-version window is v0.60.0 readers during a v0.61.0 binary
rollout. Keep one writer active and do not let an older writer publish state
after the rollout has crossed a documented format point of no return. Use
`rocklake capacity report` to record the measured envelope before rollout.

There is no published Docker image or RockLake-specific container upgrade path; use the release binary.
