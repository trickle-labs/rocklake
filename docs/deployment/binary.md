# Binary Deployment

The simplest way to run SlateDuck is as a standalone binary on a VM or bare-metal server. This is appropriate for development, testing, small-scale production deployments, and situations where container infrastructure is unavailable or unnecessary.

## Obtaining the Binary

Download the pre-built binary for your platform from the releases page:

```bash
# Linux (x86_64)
curl -L https://github.com/slateduck/slateduck/releases/latest/download/slateduck-linux-x86_64 -o slateduck
chmod +x slateduck

# macOS (Apple Silicon)
curl -L https://github.com/slateduck/slateduck/releases/latest/download/slateduck-darwin-aarch64 -o slateduck
chmod +x slateduck
```

Or build from source:

```bash
git clone https://github.com/slateduck/slateduck.git
cd slateduck
cargo build --release
# Binary is at target/release/slateduck
```

## Running

```bash
# Start with local filesystem storage (development)
./slateduck --storage ./my-catalog --bind 0.0.0.0:5432

# Start with S3 storage (production)
AWS_REGION=us-east-1 ./slateduck --storage s3://my-bucket/catalog/ --bind 0.0.0.0:5432
```

The process runs in the foreground by default. For background operation, use your operating system's process manager.

## systemd Service

For production Linux deployments, run SlateDuck as a systemd service:

```ini
[Unit]
Description=SlateDuck Catalog Server
After=network.target

[Service]
Type=simple
User=slateduck
Group=slateduck
ExecStart=/usr/local/bin/slateduck --storage s3://my-bucket/catalog/ --bind 0.0.0.0:5432
Restart=always
RestartSec=5
Environment=AWS_REGION=us-east-1
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

This ensures SlateDuck restarts automatically on crash and starts on boot.

## Resource Requirements

SlateDuck is lightweight:

- **Memory:** 50-200 MB depending on catalog size (hot key cache, active sessions)
- **CPU:** Single core is sufficient for most workloads; scales to multiple cores for concurrent read sessions
- **Disk:** None required (all data in object storage). Local disk is used only for SlateDB's optional WAL buffer.
- **Network:** Reliable connectivity to object storage with < 100ms latency

## Cloud Credentials

SlateDuck uses the standard cloud SDK credential chain:

- **AWS:** Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`), IAM instance role, ECS task role, or `~/.aws/credentials`
- **GCS:** `GOOGLE_APPLICATION_CREDENTIALS` environment variable pointing to a service account JSON file, or GCE metadata service
- **Azure:** `AZURE_STORAGE_ACCOUNT` + `AZURE_STORAGE_ACCESS_KEY`, or managed identity
