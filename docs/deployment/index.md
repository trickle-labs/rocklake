# Deployment

RockLake v0.48.0 is supported as a single `rocklake` binary backed by a local
filesystem or cloud object storage. Keep the listener on `127.0.0.1` for local
use and configure a network boundary before exposing it elsewhere.

## Supported paths

- [Binary](binary.md): build, install, and run the sidecar.
- [Configuration](configuration.md): typed CLI options and environment
  variables.
- [TLS and authentication](tls.md): server-side TLS and password auth.
- [AWS S3](aws-s3.md), [GCS](gcs.md), [Azure](azure.md), and [MinIO](minio.md):
  object-store configuration.

Docker, Kubernetes, Fly.io, Lambda, and multi-region deployment recipes are
not release-supported integrations. Their pages remain as explicit notices so
they are not mistaken for tested product paths.

## Operational model

- One writer owns a catalog; readers can open the same object-store catalog in
  read-only mode.
- Durable catalog state lives in the configured object store.
- The process runs in the foreground; use the host's process supervisor when
  background operation is required.
