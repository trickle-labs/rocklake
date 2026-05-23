---
hide:
  - navigation
  - toc
---

# SlateDuck

<div class="hero" markdown>

## Your entire lakehouse catalog in a single S3 bucket

No PostgreSQL. No SQLite file locks. No infrastructure to manage.
Just a bucket, DuckDB, and infinite time travel.

[Get Started](getting-started/quickstart.md){ .md-button .md-button--primary }
[Architecture](architecture/index.md){ .md-button }

</div>

## What is SlateDuck?

SlateDuck is a **DuckLake catalog implementation** backed by [SlateDB](https://slatedb.io) -
an LSM-tree key-value store that uses object storage (S3, GCS, Azure) as its durable layer.

## Why SlateDuck?

| Dimension | PostgreSQL-backed DuckLake | SlateDuck |
|-----------|--------------------------|----------|
| Infrastructure | PostgreSQL server required | Object storage only |
| Catalog durability | WAL + fsync on PostgreSQL | SlateDB LSM on S3/GCS/Azure |
| Catalog size limit | Disk attached to DB host | Unlimited (object storage) |
| Read scale-out | Read replicas | Unlimited readers |
| Write concurrency | Multi-writer (with locks) | Single writer per catalog |
| Catalog log latency | 1-5 ms | 20-50 ms (object store RTT) |
| Operational overhead | DB backups, vacuums, upgrades | None |

## Quick Navigation

- :material-rocket-launch: **[Getting Started](getting-started/index.md)** -
  Install, configure, and run your first query in minutes.
- :material-lightbulb: **[Concepts](concepts/index.md)** -
  Understand the DuckLake model, MVCC, and immutability.
- :material-crane: **[Architecture](architecture/index.md)** -
  Deep-dive into crate structure, key layout, and MVCC implementation.
- :material-server: **[Deployment](deployment/index.md)** -
  Docker, Kubernetes, Fly.io, and bare-metal guides.
- :material-tools: **[Operations](operations/index.md)** -
  Monitoring, GC, excision, backup, and repair.
- :material-puzzle: **[Integration](integration/index.md)** -
  DuckDB, DataFusion, and pg-tide-relay.
- :material-scale-balance: **[Design Decisions](design-decisions/index.md)** -
  Honest trade-off analysis.
- :material-speedometer: **[Performance](performance/index.md)** -
  Benchmarks, latency model, and tuning.
