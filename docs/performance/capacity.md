# Capacity report

`rocklake capacity report` turns catalog facts and operator-supplied workload
rates into a bounded report. It labels catalog counts as measured, rates as
inputs, monthly totals as projections, working-set values as estimates, and
the selected evidence envelope as a historical assumption.

The report does not claim that a catalog can sustain an unmeasured workload.
The built-in profiles are historical v0.61 inputs based on the v0.52 LocalFS
and MinIO plan. The raw v0.52 scale reports are absent from this checkout, so
the profiles are guidance only and are not certified capacity limits:

| Profile | Data files | Reads/s | Writes/s |
|---|---:|---:|---:|
| `small` | 10,000 | 50 | 5 |
| `medium` | 100,000 | 250 | 25 |
| `large` | 1,000,000 | 1,000 | 100 |

Choose a profile with `--evidence-profile`. A report outside its profile is a
recommendation to split catalogs or service instances and run a fresh local
measurement. It is not an automatic rejection of the workload.

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
request rates for request fees and supplied `--catalog-bytes` for storage fees;
it never converts a guessed metadata size into a currency claim. The cache
budget is an input to the report only; this command does not configure
SlateDB's serving cache or recommend a cache size.

Pricing files are inputs, not product defaults. Update them when a provider,
region, or pricing date changes, and retain the file with the evidence result.
