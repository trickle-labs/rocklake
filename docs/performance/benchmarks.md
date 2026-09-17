# Benchmarks

v0.63.2 publishes only the small candidate report produced by the CI evidence
gate. It records the exact source revision, machine, workload, unit, and raw
Criterion result. The gate compares that report with
[`v0.63.2-baseline.json`](../../benchmarks/v0.63.2-baseline.json).

The older performance tables are not release claims. The v0.42 report has no
exact source revision, and the v0.52 directory contains a schema and README but
no raw scale reports. The v0.61 profiles are historical capacity inputs. Do not
use any of them as a capacity limit or cloud-performance promise.

## Run the current smoke benchmark

```bash
python scripts/run_benchmark_candidate.py \
  --output target/benchmarks/v0.63.2-candidate.json
python scripts/check_benchmark_regression.py \
  --candidate target/benchmarks/v0.63.2-candidate.json \
  --baseline benchmarks/v0.63.2-baseline.json
```

The smoke benchmark measures `get_current_snapshot_warm` on a LocalFS catalog
with 100 data files. It uses the current checkout and writes a candidate report
with its Git revision. CI archives that report for each run.

## Add a baseline

Baseline changes are reviewed data changes. A normal gate never rewrites the
baseline and never accepts a report with missing, empty, nonfinite, or
incompatible measurements. Add a metric only when the candidate runner records
its source revision, environment, measurement identity, value, and unit.

The checked-in reports under `benchmarks/` remain useful historical references,
but they need a fresh run before they can support a release or workload claim.
Remote-cloud results need a real provider run. Emulator output keeps its
emulator label and does not graduate a backend to measured scale support.
