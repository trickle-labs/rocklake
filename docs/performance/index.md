# Performance

v0.63.2 publishes no general latency, throughput, capacity, or cloud-tier
promise. Performance depends on the workload, storage backend, cache state,
and machine.

## Current evidence

The v0.63.7 release gate records candidate-specific LocalFS samples at 10,000
and 100,000 visible data-file records. See the [local release evidence](evidence-v0.63.7.md)
for the raw-report contract and validation command.

The CI gate runs one warm snapshot-read benchmark on a LocalFS catalog with 100
data files. The candidate report records the source revision, machine,
workload, value, and unit. CI archives the raw report and compares it with the
reviewed v0.63.2 baseline.

```bash
python scripts/run_benchmark_candidate.py \
  --output target/benchmarks/v0.63.2-candidate.json
python scripts/check_benchmark_regression.py \
  --candidate target/benchmarks/v0.63.2-candidate.json \
  --baseline benchmarks/v0.63.2-baseline.json
```

## Historical material

The linked pages contain older implementation notes, estimates, and examples.
They are not v0.63.2 release claims. The v0.42 report lacks an exact source
revision. The v0.52 directory has a schema and README but no raw scale reports.
The v0.61 profiles are historical capacity inputs.

- [Benchmarks](benchmarks.md)
- [Capacity report](capacity.md)
- [Latency model](latency-model.md)
- [Tuning](tuning.md)
- [When to use RockLake](when-to-use.md)
- [RockLake vs. alternatives](vs-alternatives.md)
- [Cost analysis](cost-analysis.md)
- [S3 Express validation](s3-express-validation.md)
- [SlateDB tuning](slatedb-tuning.md)
- [Zone-map readiness](pruning.md)
