# Performance: SlateDuck vs. Alternatives

This page provides an honest comparison of SlateDuck's performance characteristics against alternative DuckLake catalog backends. The goal is to help you make an informed choice, not to claim superiority.

## Comparison Targets

DuckLake supports multiple catalog backends. SlateDuck competes with:

1. **PostgreSQL** — The default production backend for DuckLake
2. **SQLite** — The default local/development backend for DuckLake
3. **MySQL** — An alternative relational backend

## Latency Comparison

| Operation | SlateDuck (S3) | SlateDuck (Express) | PostgreSQL (RDS) | SQLite (local) |
|-----------|---------------|--------------------|--------------------|---------------|
| Point read | 1-50ms | 1-5ms | 1-3ms | < 0.1ms |
| Scan (50 rows) | 5-70ms | 2-10ms | 2-5ms | < 1ms |
| Write (single) | 50-150ms | 3-10ms | 5-15ms | < 1ms |
| Write (batch 100) | 60-160ms | 5-15ms | 20-50ms | 1-5ms |

**Key insight:** For raw latency, SQLite wins (it is in-process with no I/O for cached data). PostgreSQL is second (local network + buffer pool). SlateDuck with S3 Standard is slowest. With S3 Express, SlateDuck is competitive with PostgreSQL.

## Where SlateDuck Wins

**Operational simplicity:** PostgreSQL requires a running database server, connection pooling, backup procedures, failover configuration, and monitoring. SlateDuck requires a single binary and an object storage path. The total operational cost (human time + infrastructure cost) is often lower for SlateDuck despite higher per-operation latency.

**Durability:** S3 provides 11 nines of durability without any configuration. PostgreSQL requires careful WAL archiving, replication, and backup verification to approach similar durability. SlateDuck's durability is free.

**Horizontal read scaling:** SlateDuck supports unlimited concurrent readers without replication lag, connection limits, or read replica configuration. PostgreSQL requires read replicas with associated complexity and lag.

**Zero infrastructure:** No servers to provision, patch, scale, or monitor. No connection limits to tune. No vacuum to configure. No replication to set up.

## Where SlateDuck Loses

**Raw latency:** If your workload is latency-sensitive and you already have PostgreSQL infrastructure, PostgreSQL will be faster for individual operations (1-5ms vs. 20-100ms for S3 Standard).

**Write throughput:** PostgreSQL can handle thousands of writes per second. SlateDuck's single-writer model with S3 Standard limits throughput to ~10-15 writes per second. This is sufficient for catalog workloads but not for high-churn scenarios.

**Query flexibility:** PostgreSQL allows arbitrary SQL against the catalog tables. SlateDuck's bounded SQL means you cannot ad-hoc query the catalog without exporting it first.

## Decision Framework

Choose SlateDuck when:
- You want zero operational overhead for the catalog backend
- You are already using object storage for your data lake
- You value durability and horizontal read scaling
- Your catalog workload is moderate (< 100 writes/minute)

Choose PostgreSQL when:
- You already operate PostgreSQL and have expertise
- Sub-5ms catalog latency is important
- You need high write throughput (> 100 writes/second)
- You want ad-hoc SQL queries against catalog metadata
