# TLS and authentication

RockLake v0.50.0 server supports server-side TLS, cleartext password
authentication, and SCRAM-SHA-256 authentication on the
PostgreSQL wire listener. It does not support mutual TLS or certificate
hot-reload.

```bash
rocklake serve \
  --catalog s3://bucket/catalog/ \
  --bind 0.0.0.0:5432 \
  --tls-cert /etc/rocklake/server.crt \
  --tls-key /etc/rocklake/server.key \
  --tls-required \
  --auth-user ducklake
```

Use a process restart to replace certificate files. Keep the listener bound to
`127.0.0.1` for local development and expose it publicly only with a suitable
network boundary. Password authentication without TLS sends credentials over
the wire in plaintext.

Set `ROCKLAKE_AUTH_PASSWORD` from a secret manager or a
permission-restricted file. Do not put passwords in command-line arguments.
The v0.50.0 release binary selects SCRAM-SHA-256 for authenticated
connections, so clients must support SCRAM-SHA-256. The cleartext password
path remains available to library users that configure it directly and must
use TLS outside trusted local use.

The supported environment variables are `ROCKLAKE_CATALOG`,
`ROCKLAKE_AUTH_USER`, `ROCKLAKE_AUTH_PASSWORD`,
`ROCKLAKE_AUTH_PASSWORD_FILE`, `ROCKLAKE_ENCRYPTION_KEY_FILE`,
`ROCKLAKE_METRICS_PATH`, `ROCKLAKE_EXTENSION_SCHEMAS`, and
`ROCKLAKE_OTLP_ENDPOINT`. Provider-specific object-store credentials use the
provider's normal environment variables.
