# v0.63.0 beta evidence ledger

This ledger starts with the v0.63.0 tag. It records the design-partner
observation window and is updated with evidence, not estimates. Customer names,
credentials, and customer data do not belong in this repository.

## Observation status

| Field | Value |
|---|---|
| Release | v0.63.0 |
| Observation start | 2026-09-11, after the release tag |
| Required duration | 30 days without an ordinary-maintenance reset |
| Reset conditions | Correctness, security, format, or isolation fix |
| Current status | In progress |
| Release-blocking unresolved P0/P1 findings | 0 recorded at beta entry |

## Representative workloads

| Workload | Deployment | Operational owner | Evidence |
|---|---|---|---|
| Single-catalog design-partner workload | Named design-partner deployment; details recorded privately | Recorded privately | To be appended during observation |
| Multi-catalog design-partner workload | Named design-partner deployment; details recorded privately | Recorded privately | To be appended during observation |

Each entry must record the workload envelope, catalog size, writer and reader
topology, storage backend, request rate, bytes, capacity limits, alerting,
backup age, and verification cadence without exposing customer data.

## Required exercises

Record the command, date, result, recovery time, and follow-up for each
representative deployment:

- backup and restore;
- writer handoff and recovery;
- credential and encryption-key rotation;
- upgrade and rollback before the point of no return;
- registry recovery; and
- continuous metrics, audit, job, capacity, writer-owner, backup-age, and
  verification monitoring.

## Findings

| ID | Severity | Deployment | Finding | Status | Evidence or follow-up |
|---|---|---|---|---|---|
| — | — | — | No findings recorded at beta entry. | Open observation | Append the first field finding here. |

The v0.63.1 readiness audit must close every P0/P1 finding, reconcile the
support envelope with observed behavior, and publish the final evidence links.
