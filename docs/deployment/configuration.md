# Configuration

SlateDuck is configured through a combination of command-line flags, environment variables, and (optionally) a TOML configuration file. This page documents all available configuration options and their defaults.

## Command-Line Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--storage PATH` | (required) | Object storage path for the catalog |
| `--bind ADDR:PORT` | `127.0.0.1:5432` | Address and port to listen on |
| `--tls-cert PATH` | (none) | Path to TLS certificate PEM file |
| `--tls-key PATH` | (none) | Path to TLS private key PEM file |
| `--max-sessions N` | `64` | Maximum concurrent client sessions |
| `--read-only` | false | Disable write operations (read replica mode) |

## Environment Variables

### Storage Configuration

| Variable | Description |
|----------|-------------|
| `SLATEDUCK_STORAGE` | Equivalent to `--storage` flag |
| `AWS_REGION` | AWS region for S3 access |
| `AWS_ENDPOINT_URL` | Custom S3-compatible endpoint (MinIO, R2, etc.) |
| `AWS_ACCESS_KEY_ID` | Static AWS credentials (prefer IAM roles) |
| `AWS_SECRET_ACCESS_KEY` | Static AWS credentials (prefer IAM roles) |
| `GOOGLE_APPLICATION_CREDENTIALS` | Path to GCS service account JSON |
| `AZURE_STORAGE_ACCOUNT` | Azure storage account name |
| `AZURE_STORAGE_ACCESS_KEY` | Azure storage account key |

### Server Configuration

| Variable | Description |
|----------|-------------|
| `SLATEDUCK_BIND` | Equivalent to `--bind` flag |
| `SLATEDUCK_MAX_SESSIONS` | Equivalent to `--max-sessions` flag |
| `SLATEDUCK_TLS_CERT` | Equivalent to `--tls-cert` flag |
| `SLATEDUCK_TLS_KEY` | Equivalent to `--tls-key` flag |
| `SLATEDUCK_PASSWORD` | If set, require this password for client authentication |
| `SLATEDUCK_LOG_FORMAT` | Log format: `text` (default) or `json` |
| `RUST_LOG` | Log level filter (e.g., `info`, `debug`, `slateduck_catalog=trace`) |

### Performance Tuning

| Variable | Default | Description |
|----------|---------|-------------|
| `SLATEDUCK_HOT_KEY_CACHE` | `true` | Enable hot key caching for repeated reads |
| `SLATEDUCK_BATCH_SIZE` | `1000` | Maximum keys per write batch |
| `SLATEDUCK_PREFETCH_DEPTH` | `4` | Number of SST blocks to prefetch during scans |

## Configuration Precedence

When the same option is specified in multiple places, the precedence is:

1. Command-line flags (highest priority)
2. Environment variables
3. Configuration file
4. Default values (lowest priority)

## Object Storage Path Format

The `--storage` flag accepts these path formats:

| Format | Example | Provider |
|--------|---------|----------|
| `s3://bucket/prefix/` | `s3://my-data/catalog/` | AWS S3, MinIO, R2 |
| `gs://bucket/prefix/` | `gs://my-data/catalog/` | Google Cloud Storage |
| `az://container/prefix/` | `az://data/catalog/` | Azure Blob Storage |
| `./local/path/` | `./my-catalog/` | Local filesystem |
| `/absolute/path/` | `/var/data/catalog/` | Local filesystem |

Trailing slash is recommended but not required. The specified path becomes the root of the SlateDB instance — all WAL segments, SSTs, and manifests are stored under this prefix.
