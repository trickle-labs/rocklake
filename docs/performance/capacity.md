# Capacity report

`rocklake capacity report` turns a catalog inspection and measured workload
rates into a bounded capacity report. It reports catalog counts, snapshot
history, request rates, byte rates, cache budget, configured limits, and the
selected evidence envelope independently.

The report does not claim that a catalog can sustain an unmeasured workload.
The built-in profiles correspond to the v0.52 LocalFS/MinIO scale classes:

| Profile | Data files | Reads/s | Writes/s |
|---|---:|---:|---:|
| `small` | 10,000 | 50 | 5 |
| `medium` | 100,000 | 250 | 25 |
| `large` | 1,000,000 | 1,000 | 100 |

Choose a profile with `--evidence-profile`. A report outside its profile is a
recommendation to split catalogs or service instances and re-measure, not an
automatic rejection of the workload.

## Example

```bash
rocklake capacity report --catalog ./lake \
  --evidence-profile medium \
  --read-ops-per-second 20 \
  --write-ops-per-second 2 \
  --read-bytes-per-second 1048576 \
  --pricing-file benchmarks/pricing/us-east-1-2026-09-11.json \
  --output json
```

Request and byte projections are separate. Currency estimates are optional and
only appear when `--pricing-file` supplies a dated JSON input. The report uses
request rates for request fees and measured `--catalog-bytes` for storage fees;
it never converts a guessed metadata size into a currency claim.

Pricing files are inputs, not product defaults. Update them when a provider,
region, or pricing date changes, and retain the file with the evidence result.
