# v0.63.7 local release evidence

The v0.63.7 gate records candidate-specific LocalFS evidence from fresh child
processes. It is release evidence, not a universal performance promise.

The required profile uses 10,000 and 100,000 visible data-file records and
three repetitions per operation. Each raw result includes the checked-out Git
SHA, binary digest, first-row and completion latency, RSS samples, close state,
and object-store operation and byte counters. Digest mismatches fail the run.

```bash
cargo run --release -p rocklake-evidence -- run \
  --backend localfs \
  --path benchmarks/evidence/v0.63.7/localfs \
  --sizes 10000,100000 \
  --repetitions 3 \
  --output benchmarks/evidence/v0.63.7/localfs.json \
  --events benchmarks/evidence/v0.63.7/localfs.jsonl
python3 scripts/validate_evidence.py \
  benchmarks/evidence/v0.63.7/localfs.json \
  --release v0.63.7 \
  --repetitions 3
```

The release workflow also runs the populated DuckDB quickstart against the
built artifact. It does not rebuild when `ROCKLAKE_BINARY` is set. Previous
release upgrade and restore behavior remains covered by the compatibility and
backup tests; optional million-file, MinIO, and emulator runs are not implied
when their raw reports are absent.
