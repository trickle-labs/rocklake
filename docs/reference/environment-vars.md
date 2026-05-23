# Environment Variables Reference

This page lists all environment variables recognized by SlateDuck, organized by category.

## Storage Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SLATEDUCK_STORAGE` | Yes (or --storage flag) | — | Object storage path for the catalog |
| `AWS_REGION` | For S3 | `us-east-1` | AWS region for S3 access |
| `AWS_ENDPOINT_URL` | No | AWS default | Custom S3-compatible endpoint URL |
| `AWS_ACCESS_KEY_ID` | For static creds | — | AWS access key (prefer IAM roles) |
| `AWS_SECRET_ACCESS_KEY` | For static creds | — | AWS secret key (prefer IAM roles) |
| `AWS_SESSION_TOKEN` | For temp creds | — | AWS session token for temporary credentials |
| `GOOGLE_APPLICATION_CREDENTIALS` | For GCS | — | Path to GCS service account JSON file |
| `AZURE_STORAGE_ACCOUNT` | For Azure | — | Azure storage account name |
| `AZURE_STORAGE_ACCESS_KEY` | For Azure | — | Azure storage account key |

## Server Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SLATEDUCK_BIND` | No | `127.0.0.1:5432` | Address and port to listen on |
| `SLATEDUCK_MAX_SESSIONS` | No | `64` | Maximum concurrent client sessions |
| `SLATEDUCK_PASSWORD` | No | — | Required password for client authentication |
| `SLATEDUCK_REQUIRE_TLS` | No | `false` | Reject non-TLS connections |
| `SLATEDUCK_READ_ONLY` | No | `false` | Disable write operations |

## TLS Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SLATEDUCK_TLS_CERT` | For TLS | — | Path to TLS certificate PEM file |
| `SLATEDUCK_TLS_KEY` | For TLS | — | Path to TLS private key PEM file |

## Logging

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `RUST_LOG` | No | `info` | Log level filter (supports per-crate filtering) |
| `SLATEDUCK_LOG_FORMAT` | No | `text` | Log format: `text` or `json` |

## Performance Tuning

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SLATEDUCK_HOT_KEY_CACHE` | No | `true` | Enable hot key caching |
| `SLATEDUCK_BATCH_SIZE` | No | `1000` | Maximum keys per write batch |
| `SLATEDUCK_PREFETCH_DEPTH` | No | `4` | SST block prefetch depth during scans |
| `SLATEDUCK_CACHE_SIZE_MB` | No | `64` | SlateDB block cache size in MB |
| `SLATEDUCK_INLINE_THRESHOLD_BYTES` | No | `4096` | Maximum file size for inlining |

## Precedence

When the same option is specified in multiple places:

1. Command-line flags (highest priority)
2. Environment variables
3. Default values (lowest priority)
