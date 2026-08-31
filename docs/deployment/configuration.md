# Configuration

RockLake v0.50.0 accepts typed `rocklake.toml` configuration alongside the
environment variables and command-line flags exposed by `rocklake serve`.
Precedence is built-in defaults, TOML, environment, then command-line flags.

## Minimal local setup

```bash
rocklake serve ./lake
```

Generate a complete example with `rocklake config example` and validate a file
with `rocklake config check --file rocklake.toml`.

## Cloud catalog

```bash
rocklake serve --catalog <file://...,s3://...,gs://...,az://...>
```

## Common options

| Option | Default |
|---|---|
| `--bind <address:port>` | `127.0.0.1:5432` |
| `--max-sessions <n>` | `50` |
| `--mode <writer\|reader>` | `writer` |
| `--read-only` | off; alias for reader mode |
| `--cost-mode <conservative\|balanced\|latency>` | `balanced` |
| `--metrics-port <port>` | disabled |
| `--metrics-path <path>` | `/metrics` |
| `--tls-cert <path>` / `--tls-key <path>` | disabled |
| `--tls-required` | off |
| `--auth-user <name>` / `--auth-password <secret>` | disabled; use environment or a mounted secret file for the password |
| `--s3-endpoint <url>` / `--s3-path-style` | disabled |
| `--encryption-key <64 hex digits>` / `--encryption-key-file <path>` | disabled |
| `--idle-connection-timeout <seconds>` | `60` |
| `--drain-timeout <seconds>` | `30` |
| `--datafusion-bridge-queue-depth <n>` | `256` |

Run `rocklake serve --help` for the authoritative list and validation rules.
The server uses the standard AWS, GCS, and Azure credential environment
variables for cloud storage.

The default listener is loopback at `127.0.0.1:5432`. Bind to a private or
public address only when the network boundary, TLS, and authentication are
ready. Do not pass passwords as command-line arguments. Set
`ROCKLAKE_AUTH_PASSWORD` from a secret manager or a permission-restricted file.
Alternatively, set `ROCKLAKE_AUTH_PASSWORD_FILE` or pass
`--auth-password-file`. Use `ROCKLAKE_ENCRYPTION_KEY_FILE` or
`--encryption-key-file` for the encryption key.
