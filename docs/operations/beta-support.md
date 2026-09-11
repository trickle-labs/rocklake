# Production beta support

RockLake v0.63.0 is a feature-complete production beta. It is intended for
named design-partner workloads while the observation and evidence gates run.
The binary, PostgreSQL wire protocol, and DuckLake 1.0 path remain the
supported interfaces. Rust, read-only, DataFusion, and language-binding
interfaces keep their existing Preview or Experimental levels.

## Support boundary

- No new feature, query shape, public field, storage format, or backend is
  accepted into the beta branch.
- LocalFS and MinIO evidence is complete for the existing certified envelope.
- AWS S3, GCS, and Azure scale evidence remains deferred to v0.64.0–v0.64.2.
- A support request must include a redacted `rocklake support bundle` when the
  catalog can be opened. Review paths and identifiers before sharing it.

See the [v0.63.0 beta evidence ledger](beta-evidence-v0.63.0.md) for the
observation status and required exercises.

## Escalation route

1. For a production outage, data correctness, recovery, or isolation issue,
   preserve logs and the support bundle, then contact `support@trickle-labs.com`
   with the affected release, deployment type, workload, and severity.
2. Report suspected vulnerabilities only through the private process in
   [`SECURITY.md`](../../SECURITY.md); do not open a public issue.
3. Use a public [GitHub issue](https://github.com/trickle-labs/rocklake/issues)
   for non-sensitive documentation, installation, and reproducible defects.

## Severity

| Severity | Definition | Response |
|---|---|---|
| P0 | Active data corruption, security or catalog-isolation breach, or complete supported-path outage with no safe workaround. | Stop affected writes, preserve evidence, and page the maintainer immediately. |
| P1 | A supported correctness, recovery, upgrade, or availability failure with material impact and no safe workaround. | Escalate the same business day; the issue blocks the next release until closed or explicitly reclassified. |
| P2 | A non-blocking defect, documentation gap, false alert, or operational surprise with a safe workaround. | Record it in the beta ledger and schedule a fix or clarification. |
| Post-1.0 enhancement | A feature request or compatibility expansion that is not required to correct the beta contract. | Do not add it to the beta branch; track it for a later milestone. |

Any correctness, security, format, or isolation fix resets the affected
observation gate. Ordinary maintenance does not reset the 30-day window.
