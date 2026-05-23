# Docker Deployment

Running SlateDuck in Docker provides process isolation, reproducible environments, and easy integration with container orchestration platforms. The official Docker image is minimal (based on `scratch` or `alpine`) and contains only the SlateDuck binary.

## Quick Start

```bash
docker run -p 5432:5432 \
  -e AWS_REGION=us-east-1 \
  -e AWS_ACCESS_KEY_ID=your-key \
  -e AWS_SECRET_ACCESS_KEY=your-secret \
  ghcr.io/slateduck/slateduck:latest \
  --storage s3://my-bucket/catalog/ --bind 0.0.0.0:5432
```

## Docker Compose

For development environments with SlateDuck + MinIO (S3-compatible local storage):

```yaml
version: "3.8"
services:
  minio:
    image: minio/minio:latest
    command: server /data --console-address ":9001"
    ports:
      - "9000:9000"
      - "9001:9001"
    environment:
      MINIO_ROOT_USER: minioadmin
      MINIO_ROOT_PASSWORD: minioadmin
    volumes:
      - minio-data:/data

  slateduck:
    image: ghcr.io/slateduck/slateduck:latest
    ports:
      - "5432:5432"
    environment:
      AWS_ACCESS_KEY_ID: minioadmin
      AWS_SECRET_ACCESS_KEY: minioadmin
      AWS_ENDPOINT_URL: http://minio:9000
      AWS_REGION: us-east-1
    command: --storage s3://slateduck-catalog/ --bind 0.0.0.0:5432
    depends_on:
      - minio

volumes:
  minio-data:
```

Start the stack:

```bash
docker compose up -d
```

Then connect DuckDB:

```sql
ATTACH 'ducklake:host=localhost;port=5432' AS my_lake;
```

## Building the Image

If you need a custom image (e.g., with additional CA certificates or custom configurations):

```dockerfile
FROM rust:1.80 AS builder
WORKDIR /src
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /src/target/release/slateduck /usr/local/bin/slateduck
ENTRYPOINT ["slateduck"]
```

## Health Checks

Add a health check to the Docker configuration:

```yaml
healthcheck:
  test: ["CMD", "pg_isready", "-h", "localhost", "-p", "5432"]
  interval: 10s
  timeout: 5s
  retries: 3
```

## Security Considerations

- Never bake credentials into the image. Use environment variables or mounted secrets.
- Run as a non-root user inside the container for defense in depth.
- Use read-only filesystem where possible (`--read-only` flag).
- Limit container resources to prevent runaway memory/CPU usage.
