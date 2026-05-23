# High Availability

SlateDuck achieves high availability through rapid failover rather than active-active replication. Because there is only one writer at a time (the single-writer model), HA focuses on minimizing the time between a writer failure and a replacement taking over.

## Availability Model

SlateDuck's availability depends on:

1. **The SlateDuck process** — must be running to serve requests
2. **Object storage** — must be accessible for reads and writes
3. **Network** — must connect clients to SlateDuck and SlateDuck to storage

Object storage availability is typically 99.99%+ (managed by the cloud provider). Network availability within a region is similarly high. The primary risk is the SlateDuck process itself crashing or becoming unresponsive.

## Failover Strategy

When the SlateDuck process fails:

1. A health check detects the failure (liveness probe, TCP check, or heartbeat timeout)
2. The orchestrator (Kubernetes, systemd, ECS) starts a new instance
3. The new instance reads the catalog manifest from object storage
4. The new instance increments the writer epoch, becoming the active writer
5. The new instance begins accepting connections

Total failover time: 5-30 seconds (dominated by health check interval + container startup).

## Achieving 99.9% Availability

With a 10-second health check interval and 5-second startup time, worst-case recovery is ~15 seconds. Over a month, this allows approximately 2 failures before exceeding the 99.9% availability target (43 minutes of downtime per month).

To achieve 99.9%:
- Run on Kubernetes with aggressive liveness probes (5s interval, 2 failures to trigger restart)
- Use a cloud region with multiple availability zones
- Monitor and alert on writer epoch changes

## Read Availability vs. Write Availability

**Read availability** can be higher than write availability because multiple read-only instances can serve catalog queries simultaneously. If the writer fails, read-only replicas continue serving stale data until the writer recovers and produces new snapshots.

**Write availability** is limited by the single-writer model. During failover, writes are unavailable. For most analytics workloads (where writes are infrequent ETL operations), brief write unavailability is acceptable.

## What SlateDuck Does NOT Provide

SlateDuck does not provide:
- Active-active replication (multiple simultaneous writers)
- Synchronous replication across regions
- Automatic leader election without an external orchestrator
- Sub-second failover

If you need these properties, consider using a managed database service (e.g., Aurora PostgreSQL) with DuckLake's PostgreSQL backend instead.
