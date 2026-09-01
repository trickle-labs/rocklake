# MinIO

MinIO-backed RockLake testing is covered by CI when the MinIO test feature is
enabled. v0.51.2 does not publish a RockLake Docker image or a Docker Compose
deployment. Run RockLake as a binary and point it at an S3-compatible MinIO
endpoint:

```bash
AWS_ACCESS_KEY_ID=minioadmin AWS_SECRET_ACCESS_KEY=minioadmin \
rocklake serve --catalog s3://rocklake-catalog/ \
  --s3-endpoint http://127.0.0.1:9000 --s3-path-style
```

Use the MinIO project's own documentation to start and provision MinIO.
