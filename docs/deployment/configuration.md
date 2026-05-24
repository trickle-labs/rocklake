# Configuration

SlateDuck is configured through a combination of command-line flags, environment variables, and (optionally) a TOML configuration file. The design follows the twelve-factor app methodology: configuration that varies between environments lives in the environment, not in code. Sensible defaults mean that most deployments need only a storage path and a bind address to get started.

This page documents every available configuration option, explains the precedence rules, and provides guidance on which options matter for which deployment scenarios.

## Configuration Precedence

When the same option is specified in multiple places, higher-precedence sources override lower ones:

1. **Command-line flags** — highest priority, ideal for one-off overrides and debugging
2. **Environment variables** — standard for container deployments and CI/CD
3. **Configuration file** (`slateduck.toml`) — structured, version-controllable settings
4. **Compiled defaults** — lowest priority, documented below

This means you can set baseline configuration in a TOML file, override per-environment values with environment variables, and further override for testing with command-line flags. There are no surprises — the most specific source always wins.

## Command-Line Flags

The full set of command-line flags:

```bash
slateduck [FLAGS] [OPTIONS]
```

### Required

| Flag | Description |
|------|-------------|
| `--storage PATH` | Object storage path for the catalog. See [path formats](#object-storage-path-format) below. |

### Server Options

| Flag | Default | Description |
|------|---------|-------------|
| `--bind ADDR:PORT` | `127.0.0.1:5432` | Network address and port to listen on. Use `0.0.0.0:5432` to listen on all interfaces. |
| `--max-sessions N` | `64` | Maximum number of concurrent client sessions. Each session consumes ~1 MB. |
| `--read-only` | `false` | Disable all write operations. The server refuses DDL/DML and acts as a read replica. |
| `--log-level LEVEL` | `info` | Logging verbosity: `error`, `warn`, `info`, `debug`, `trace`. |
| `--log-format FORMAT` | `text` | Log output format: `text` (human-friendly) or `json` (machine-parseable). |

### TLS Options

| Flag | Default | Description |
|------|---------|-------------|
| `--tls-cert PATH` | (none) | Path to PEM-encoded TLS certificate file. If specified, `--tls-key` must also be provided. |
| `--tls-key PATH` | (none) | Path to PEM-encoded TLS private key file. |
| `--tls-ca PATH` | (none) | Path to PEM-encoded CA certificate for mutual TLS (client certificate verification). |

When TLS is configured, the server requires all connections to use TLS. There is no mixed-mode listener — either all connections are encrypted or none are. If you need both, run two SlateDuck instances on different ports.

### Authentication Options

| Flag | Default | Description |
|------|---------|-------------|
| `--auth-user NAME` | (none) | If set, require this username during PostgreSQL authentication. |
| `--auth-password SECRET` | (none) | If set, require this password. Prefer `SLATEDUCK_PASSWORD` env var to avoid shell history exposure. |

### Performance Tuning

| Flag | Default | Description |
|------|---------|-------------|
| `--hot-key-cache BOOL` | `true` | Enable caching of frequently-read keys in memory. Reduces object storage reads for catalog metadata. |
| `--batch-size N` | `1000` | Maximum number of key-value pairs in a single write batch. Larger batches reduce round trips but increase commit latency. |
| `--prefetch-depth N` | `4` | Number of SST data blocks to prefetch during sequential scans. Higher values improve scan throughput at the cost of memory. |
| `--compaction-interval SECS` | `300` | Seconds between background compaction checks. Set to `0` to disable automatic compaction (manual only). |

## Environment Variables

Environment variables provide the same configuration surface as command-line flags, plus additional provider-specific settings.

### Core SlateDuck Variables

| Variable | Equivalent Flag | Description |
|----------|----------------|-------------|
| `SLATEDUCK_STORAGE` | `--storage` | Object storage path |
| `SLATEDUCK_BIND` | `--bind` | Listen address and port |
| `SLATEDUCK_MAX_SESSIONS` | `--max-sessions` | Concurrent session limit |
| `SLATEDUCK_READ_ONLY` | `--read-only` | Read-only mode (`true`/`false`) |
| `SLATEDUCK_TLS_CERT` | `--tls-cert` | TLS certificate path |
| `SLATEDUCK_TLS_KEY` | `--tls-key` | TLS private key path |
| `SLATEDUCK_TLS_CA` | `--tls-ca` | Mutual TLS CA certificate |
| `SLATEDUCK_AUTH_USER` | `--auth-user` | Required username |
| `SLATEDUCK_PASSWORD` | `--auth-password` | Required password (preferred over flag) |
| `SLATEDUCK_LOG_LEVEL` | `--log-level` | Log verbosity |
| `SLATEDUCK_LOG_FORMAT` | `--log-format` | Log format |
| `SLATEDUCK_HOT_KEY_CACHE` | `--hot-key-cache` | Hot key cache toggle |
| `SLATEDUCK_BATCH_SIZE` | `--batch-size` | Write batch size |
| `SLATEDUCK_PREFETCH_DEPTH` | `--prefetch-depth` | Scan prefetch depth |
| `SLATEDUCK_COMPACTION_INTERVAL` | `--compaction-interval` | Compaction check interval |

### AWS / S3 Variables

| Variable | Description |
|----------|-------------|
| `AWS_REGION` | AWS region for S3 access (e.g., `us-east-1`) |
| `AWS_ACCESS_KEY_ID` | Static access key (prefer IAM roles in production) |
| `AWS_SECRET_ACCESS_KEY` | Static secret key |
| `AWS_SESSION_TOKEN` | Temporary session token (for assumed roles) |
| `AWS_ENDPOINT_URL` | Custom S3-compatible endpoint URL (MinIO, R2, Tigris, LocalStack) |
| `AWS_S3_EXPRESS` | Set to `true` to enable S3 Express One Zone optimizations |
| `AWS_PROFILE` | Named profile from `~/.aws/config` |

### Google Cloud Storage Variables

| Variable | Description |
|----------|-------------|
| `GOOGLE_APPLICATION_CREDENTIALS` | Path to service account JSON key file |
| `GOOGLE_CLOUD_PROJECT` | GCP project ID (for billing/quota) |

### Azure Blob Storage Variables

| Variable | Description |
|----------|-------------|
| `AZURE_STORAGE_ACCOUNT` | Storage account name |
| `AZURE_STORAGE_KEY` | Storage account access key |
| `AZURE_TENANT_ID` | Azure AD tenant for service principal auth |
| `AZURE_CLIENT_ID` | Service principal client ID |
| `AZURE_CLIENT_SECRET` | Service principal client secret |
| `AZURE_STORAGE_CONNECTION_STRING` | Full connection string (alternative to individual variables) |

### Logging Variables

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Fine-grained log filter (e.g., `slateduck_catalog=debug,slateduck_pgwire=info`) |
| `RUST_LOG_STYLE` | Terminal color support: `auto`, `always`, `never` |

## Configuration File (TOML)

For complex deployments, you can use a TOML configuration file. By default, SlateDuck looks for `slateduck.toml` in the current directory. Override with the `--config` flag or `SLATEDUCK_CONFIG` environment variable:

```bash
slateduck --config /etc/slateduck/slateduck.toml
```

Example configuration file:

```toml
# /etc/slateduck/slateduck.toml

[server]
storage = "s3://my-lakehouse-bucket/catalog/"
bind = "0.0.0.0:5432"
max_sessions = 100
read_only = false

[tls]
cert = "/etc/slateduck/tls/cert.pem"
key = "/etc/slateduck/tls/key.pem"
# ca = "/etc/slateduck/tls/ca.pem"  # Uncomment for mutual TLS

[auth]
user = "ducklake"
# Password should come from SLATEDUCK_PASSWORD env var

[logging]
level = "info"
format = "json"

[performance]
hot_key_cache = true
batch_size = 1000
prefetch_depth = 4
compaction_interval = 300
```

The TOML file uses the same names as environment variables but with dots replaced by section headers. Boolean values use `true`/`false` (not quoted strings).

## Object Storage Path Format

The `--storage` flag (or `SLATEDUCK_STORAGE` variable) accepts several path formats:

| Format | Example | Provider |
|--------|---------|----------|
| `s3://bucket/prefix/` | `s3://my-data/catalog/` | AWS S3, S3 Express One Zone |
| `s3://bucket/prefix/` | `s3://my-data/catalog/` | S3-compatible (MinIO, R2, Tigris) with `AWS_ENDPOINT_URL` |
| `gs://bucket/prefix/` | `gs://my-data/catalog/` | Google Cloud Storage |
| `az://container/prefix/` | `az://data/catalog/` | Azure Blob Storage |
| `./relative/path/` | `./my-catalog/` | Local filesystem (relative) |
| `/absolute/path/` | `/var/data/catalog/` | Local filesystem (absolute) |

The trailing slash is optional but recommended for clarity. The specified path becomes the root of the SlateDB instance — all WAL segments, sorted string tables (SSTs), manifests, and compacted files live under this prefix.

### Path Layout Within Storage

Once SlateDuck starts writing to a storage path, the internal layout is:

```
s3://my-bucket/catalog/
├── manifest/           # SlateDB manifest files
├── wal/               # Write-ahead log segments  
├── compacted/         # Compacted SST files
└── sst/               # Sorted string table files
```

Do not manually modify files under this prefix. SlateDuck manages this layout exclusively through SlateDB's compaction and garbage collection.

## Deployment-Specific Recipes

### Local Development (Minimal)

```bash
slateduck --storage ./dev-catalog --bind 127.0.0.1:5432
```

No environment variables needed. Data stored as local files.

### Docker / Kubernetes (Environment-Driven)

```bash
# All configuration via environment
export SLATEDUCK_STORAGE=s3://production-bucket/catalog/
export SLATEDUCK_BIND=0.0.0.0:5432
export SLATEDUCK_PASSWORD=secure-random-password
export SLATEDUCK_LOG_FORMAT=json
export AWS_REGION=us-east-1
slateduck
```

### Read Replica (Read-Only)

```bash
slateduck --storage s3://production-bucket/catalog/ --read-only --bind 0.0.0.0:5432
```

The server refuses any DDL or DML statements. Multiple read-only instances can connect to the same storage path concurrently.

### High-Security (Mutual TLS + Auth)

```bash
slateduck \
    --storage s3://sensitive-bucket/catalog/ \
    --bind 0.0.0.0:5432 \
    --tls-cert /etc/slateduck/tls/server-cert.pem \
    --tls-key /etc/slateduck/tls/server-key.pem \
    --tls-ca /etc/slateduck/tls/client-ca.pem \
    --auth-user ducklake \
    --max-sessions 20
```

### S3-Compatible (MinIO)

```bash
export AWS_ENDPOINT_URL=http://minio.internal:9000
export AWS_ACCESS_KEY_ID=minioadmin
export AWS_SECRET_ACCESS_KEY=minioadmin
slateduck --storage s3://my-bucket/catalog/ --bind 0.0.0.0:5432
```

## Validation and Diagnostics

On startup, SlateDuck validates all configuration and reports errors clearly:

```
ERROR: --storage is required but not set
ERROR: --tls-cert specified without --tls-key
ERROR: Cannot access storage path s3://bucket/path/ — Access Denied
```

Use `--log-level debug` to see the full resolved configuration (with secrets masked) at startup:

```
INFO  Configuration resolved:
INFO    storage: s3://my-bucket/catalog/
INFO    bind: 0.0.0.0:5432
INFO    max_sessions: 100
INFO    read_only: false
INFO    tls: enabled (cert: /etc/slateduck/tls/cert.pem)
INFO    auth: enabled (user: ducklake)
INFO    hot_key_cache: true
INFO    batch_size: 1000
```

## Further Reading

- **[Binary Deployment](binary.md)** — Running as a standalone process with systemd
- **[Docker Deployment](docker.md)** — Container configuration patterns
- **[TLS](tls.md)** — Certificate management details
- **[Networking](networking.md)** — Firewall and load balancer configuration
