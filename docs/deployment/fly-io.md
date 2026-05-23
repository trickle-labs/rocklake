# Deploying on Fly.io

Fly.io is a global application platform that runs containers close to users with automatic TLS and global anycast networking. SlateDuck can be deployed on Fly.io for a globally-distributed catalog with minimal configuration.

## Configuration

Create a `fly.toml`:

```toml
app = "my-slateduck"
primary_region = "iad"

[build]
  image = "ghcr.io/slateduck/slateduck:latest"

[env]
  AWS_REGION = "us-east-1"
  RUST_LOG = "info"
  SLATEDUCK_STORAGE = "s3://my-bucket/catalog/"

[http_service]
  internal_port = 5432
  force_https = false
  auto_stop_machines = false
  auto_start_machines = true
  min_machines_running = 1

[[services]]
  protocol = "tcp"
  internal_port = 5432

  [[services.ports]]
    port = 5432
    handlers = []

  [[services.tcp_checks]]
    grace_period = "10s"
    interval = "10s"
    timeout = "5s"
```

## Secrets

Set credentials as Fly secrets (never in fly.toml):

```bash
fly secrets set AWS_ACCESS_KEY_ID=your-key
fly secrets set AWS_SECRET_ACCESS_KEY=your-secret
```

## Deploy

```bash
fly deploy
```

## Connection

Connect from DuckDB using the Fly.io allocated address:

```sql
ATTACH 'ducklake:host=my-slateduck.fly.dev;port=5432' AS my_lake;
```

## Scaling

Fly.io can run machines in multiple regions, but remember that SlateDuck uses a single-writer model. For multi-region read replicas:

```bash
fly scale count 1 --region iad  # Primary writer
fly scale count 1 --region cdg  # Read replica (with --read-only flag)
```

## Cost Considerations

Fly.io's pricing is based on VM hours and bandwidth. A single shared-cpu-1x machine (256MB RAM) is sufficient for most SlateDuck workloads and costs approximately $2-5/month, making it one of the most cost-effective hosting options for light catalog workloads.
