# v0.52.0 scale evidence

v0.52.0 measures LocalFS and MinIO catalog behavior in fresh child processes.
The evidence runner is internal tooling; it is not part of the supported
RockLake binary API.

## Run the matrix

Use a dedicated machine for the full matrix. The default sizes are 10,000,
100,000, and 1,000,000 visible data-file records.

```bash
cargo run --release -p rocklake-evidence -- run \
  --backend localfs \
  --path benchmarks/evidence/v0.52.0/localfs \
  --sizes 10000,100000,1000000 \
  --output benchmarks/evidence/v0.52.0/localfs.json
python scripts/summarize_evidence.py benchmarks/evidence/v0.52.0/localfs.json
```

For MinIO, set the endpoint and standard AWS credential variables first:

```bash
export ROCKLAKE_EVIDENCE_ENDPOINT=http://127.0.0.1:9000
export AWS_ACCESS_KEY_ID=minioadmin
export AWS_SECRET_ACCESS_KEY=minioadmin
cargo run --release -p rocklake-evidence -- run \
  --backend minio \
  --path s3://rocklake-evidence/v0.52.0 \
  --output benchmarks/evidence/v0.52.0/minio.json
```

Each dataset is generated deterministically from the seed, and every measured
operation runs in a new process. Page, stream, and materialized reads share a
digest so correctness is checked independently of latency. The runner records
open, first-row, operation, close, wall-clock, RSS, object-store operation,
and object-store byte measurements. `schema.json` is the machine-readable
contract for the report and JSONL event files.

Do not convert a run into a compatibility claim until LocalFS and MinIO have
completed the required recovery, cancellation, failure-injection, and bounded
memory review described in the [v0.52.0 roadmap](../../ROADMAP.md#v0520--reproducible-localfs-and-minio-scale-evidence).
