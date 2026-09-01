# ADR: route independent catalogs for multi-tenancy

- **Status:** Accepted
- **Date:** 2026-09-01

## Context

RockLake supports one coordinated writer and many readers for an object-store
catalog. A future user may need several logical catalogs behind one service.

Adding a tenant ID to the shared keyspace would change key encoding, writer
fencing, retention, backup, verification, metrics, quotas, and every catalog
API. It would also make isolation a property that every operation must remember
to enforce.

## Decision

Keep the current keyspace single-catalog. When at least two users need multiple
logical catalogs, add a router:

```text
PG database or catalog alias
        |
        v
independent CatalogLocation
        |
        v
independent object-store prefix
        |
        v
independent writer epoch, retention, backup, and quotas
```

The router must apply authentication, connection limits, and metrics per
catalog. Shared cross-catalog transactions remain out of scope until a named
workload requires them.

## Rejected alternative

Do not place tenant IDs in the shared RockLake keyspace. The isolation boundary
would cross storage keys, leases, lifecycle jobs, recovery tools, and metrics.
The router keeps those resources independent and makes the boundary visible in
the request path.

## Trigger

Open a router implementation plan only after two users need multiple logical
catalogs and a maintainer owns the routing, authentication, and operational
contracts.
