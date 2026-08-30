# TLS and authentication

RockLake supports server-side TLS and password authentication on the
PostgreSQL wire listener. It does not support mutual TLS or certificate
hot-reload.

```bash
rocklake serve \
  --catalog s3://bucket/catalog/ \
  --bind 0.0.0.0:5432 \
  --tls-cert /etc/rocklake/server.crt \
  --tls-key /etc/rocklake/server.key \
  --tls-required \
  --auth-user ducklake \
  --auth-password "$ROCKLAKE_AUTH_PASSWORD"
```

Use a process restart to replace certificate files. Keep the listener bound to
`127.0.0.1` for local development and expose it publicly only with a suitable
network boundary. Password authentication without TLS sends credentials over
the wire in plaintext.

The supported environment variables are `ROCKLAKE_CATALOG`,
`ROCKLAKE_AUTH_USER`, `ROCKLAKE_AUTH_PASSWORD`, `ROCKLAKE_METRICS_PATH`,
`ROCKLAKE_EXTENSION_SCHEMAS`, and `ROCKLAKE_OTLP_ENDPOINT`. Provider-specific
object-store credentials use the provider's normal environment variables.
