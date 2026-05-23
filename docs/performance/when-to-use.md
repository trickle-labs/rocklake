# When to Use SlateDuck

SlateDuck is not the right choice for every workload. This page honestly describes the scenarios where SlateDuck excels and where other solutions are better.

## SlateDuck Excels When

### You want a serverless data lakehouse

If your data is already in object storage (S3, GCS, Azure Blob) as Parquet files and you want to manage it as a lakehouse without running database servers, SlateDuck is purpose-built for this scenario. One binary + one bucket path gives you a full DuckLake catalog with durability, time travel, and horizontal read scaling.

### You are building on DuckDB

SlateDuck is designed specifically for DuckDB's `ducklake` extension. If DuckDB is your query engine, SlateDuck provides the most operationally simple catalog backend. No PostgreSQL to manage, no SQLite file to distribute, no MySQL cluster to monitor.

### Your catalog workload is moderate

Most analytics catalogs see a few writes per minute (ETL jobs registering new data files) and a few hundred reads per minute (DuckDB instances planning queries). SlateDuck handles this comfortably. If your workload fits this profile, SlateDuck's simplicity outweighs its latency limitations.

### You need time travel

SlateDuck's immutable architecture provides free time travel — query any historical catalog state by snapshot ID. This is invaluable for debugging data pipelines, auditing changes, and reproducing query results from the past.

### You operate in multiple regions

SlateDuck leverages cloud storage cross-region replication for multi-region read replicas with zero additional configuration. If you need globally-distributed catalog reads, SlateDuck achieves this through infrastructure features rather than application-level replication.

## SlateDuck Is NOT Ideal When

### You need sub-millisecond catalog latency

If every millisecond counts (interactive BI dashboards with sub-second query times), the 20-100ms object storage latency of S3 Standard is a problem. Mitigation options exist (S3 Express, native extension) but if latency is your primary concern, an in-process SQLite catalog or a local PostgreSQL instance will be faster.

### You have extreme write throughput

If your catalog sees hundreds of writes per second (continuous streaming ingestion with per-second file registration), SlateDuck's single-writer model may be a bottleneck. Consider PostgreSQL for write-heavy workloads.

### You need arbitrary catalog queries

If you regularly run ad-hoc analytics against catalog metadata (e.g., "which tables have grown fastest this month?"), SlateDuck's bounded SQL means you must export to NDJSON first. PostgreSQL allows these queries directly.

### You already have managed PostgreSQL

If your organization already operates managed PostgreSQL (RDS, Cloud SQL, Azure Database) with established backup, monitoring, and failover procedures, there may be little operational benefit to switching to SlateDuck. The marginal complexity of another database in your stack is low.

### You need multi-writer

If multiple independent processes must write to the same catalog concurrently without coordination, SlateDuck's single-writer model is a fundamental limitation. You can work around it with dataset partitioning, but if true multi-writer is a requirement, PostgreSQL with row-level locking is more appropriate.
