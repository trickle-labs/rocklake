# Health Checks

Health checks verify that a SlateDuck instance is operational and can serve requests. They are essential for automated deployment systems (Kubernetes liveness/readiness probes, load balancer health checks, monitoring systems) and for manual verification during incident response.

## Types of Health Checks

### Liveness Check

The liveness check verifies that the SlateDuck process is alive and not deadlocked. It does not verify storage connectivity. A liveness check failure should trigger a process restart.

A TCP connection to SlateDuck's PG-wire port that completes the handshake (startup message + AuthenticationOk) constitutes a successful liveness check. Any standard PostgreSQL client health check tool works:

```bash
pg_isready -h localhost -p 5432
```

### Readiness Check

The readiness check verifies that SlateDuck can serve catalog requests. It verifies storage connectivity by reading the hot key or performing a lightweight catalog operation:

```sql
SELECT version();
```

If this returns successfully, the instance is ready to serve traffic. If it times out or returns an error, the instance should be removed from the load balancer until it recovers.

### Deep Health Check

For comprehensive verification, run the inspect command to ensure all catalog state is accessible:

```bash
slateduck inspect --storage s3://bucket/catalog/
```

If this completes without errors, the catalog is healthy. If it fails, investigate the specific error (storage connectivity, format version mismatch, corrupted state).

## Kubernetes Integration

For Kubernetes deployments, configure liveness and readiness probes:

```yaml
livenessProbe:
  tcpSocket:
    port: 5432
  initialDelaySeconds: 5
  periodSeconds: 10
  failureThreshold: 3

readinessProbe:
  exec:
    command: ["slateduck", "inspect", "--storage", "$(CATALOG_STORAGE)", "--format", "json"]
  initialDelaySeconds: 10
  periodSeconds: 30
  failureThreshold: 2
```

## What to Monitor

Beyond binary health (up/down), monitor these operational health indicators:

- **Writer epoch stability:** Frequent changes indicate instability
- **Snapshot creation rate:** If snapshots stop being created, the writer may be stuck
- **Object store error rate:** Rising errors indicate storage connectivity issues
- **Session count trends:** Unexpected drops may indicate client-side problems
