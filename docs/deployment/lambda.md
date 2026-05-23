# Lambda / Serverless Deployment

SlateDuck can run as a serverless function for workloads with infrequent catalog access where keeping a persistent process running is wasteful. This deployment model trades latency (cold start time) for cost efficiency (pay only for actual catalog operations).

## How It Works

In serverless mode, each function invocation:
1. Initializes a SlateDuck instance connected to the configured storage
2. Processes one or more catalog operations (received as the function payload or via a short-lived PG-wire listener)
3. Returns results and terminates (or waits for the next invocation if the runtime supports keep-alive)

The startup time is dominated by the initial read of the SlateDB manifest (one S3 GET, typically 20-50ms). Subsequent operations within the same invocation reuse the open catalog handle.

## AWS Lambda

Package SlateDuck as a custom runtime Lambda:

```bash
# Build for Amazon Linux 2
cargo build --release --target x86_64-unknown-linux-gnu

# Package
cp target/x86_64-unknown-linux-gnu/release/slateduck bootstrap
zip lambda.zip bootstrap
```

The Lambda handler receives catalog operation requests as JSON events and returns results as JSON responses. This is different from the PG-wire mode — it uses a request/response model rather than a persistent connection.

## Use Cases

**Infrequent access patterns:** If your DuckLake catalog is queried only a few times per hour (e.g., batch ETL jobs that run daily), a persistent SlateDuck process is wasteful. Lambda invocations cost only for the actual milliseconds of execution.

**Burst workloads:** If you have occasional spikes of catalog operations followed by long idle periods, serverless scaling handles this naturally without provisioning for peak.

**Multi-catalog management:** If you manage hundreds of independent catalogs (multi-tenant SaaS), running a persistent process per catalog is expensive. Lambda functions that open the appropriate catalog on demand are more cost-effective.

## Limitations

**Cold start latency:** The first invocation after an idle period requires reading the SlateDB manifest from object storage (20-100ms). For latency-sensitive workloads, use provisioned concurrency or keep the function warm.

**Connection model:** DuckDB's `ducklake` extension expects a persistent PG-wire connection. Serverless mode requires either a proxy that translates PG-wire to Lambda invocations, or using the FFI/DataFusion integration instead of PG-wire.

**Single writer semantics:** Only one concurrent Lambda invocation should write to a given catalog. Use concurrency limits or a queue to serialize writes.

## Cost Comparison

For a catalog accessed 100 times per day with 100ms average operation time:
- **Lambda:** ~$0.001/day (negligible)
- **Persistent EC2 t3.micro:** ~$0.25/day
- **Persistent Fargate (0.25 vCPU):** ~$0.30/day

For a catalog accessed 10,000 times per hour:
- **Lambda:** ~$1.50/day
- **Persistent EC2 t3.small:** ~$0.50/day

The crossover point is typically around 1,000-5,000 operations per hour — above that, persistent instances are more cost-effective.
