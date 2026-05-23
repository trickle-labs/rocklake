# Operations

This section covers the operational procedures for running SlateDuck in production. From routine tasks like monitoring and garbage collection to emergency procedures like repair and restore, these guides provide step-by-step instructions with explanations of what each operation does internally and when you should use it.

## Routine Operations

- **[Monitoring](monitoring.md)** — Prometheus metrics, health endpoints, and what to alert on
- **[Logging](logging.md)** — Log levels, structured output, and debugging sessions
- **[Garbage Collection](garbage-collection.md)** — Managing catalog growth with retention policies
- **[Health Checks](health-checks.md)** — Verifying catalog integrity and operational readiness

## Data Management

- **[Backup & Restore](backup-restore.md)** — NDJSON export, checkpoints, and disaster recovery
- **[Export](export.md)** — Extracting catalog data for migration or analysis
- **[Excision](excision.md)** — Physical deletion of historical data
- **[Inspect](inspect.md)** — Examining internal catalog state

## Maintenance

- **[Verify & Repair](verify-repair.md)** — Integrity checks and conservative repair
- **[Upgrades](upgrades.md)** — Version upgrades and format migrations
- **[Troubleshooting](troubleshooting.md)** — Common problems and their solutions
