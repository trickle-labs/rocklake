# Configuration

RockLake is configured with typed `rocklake serve` flags and the environment
variables exposed by those flags. There is no TOML configuration-file format.

## Required option

```bash
rocklake serve --catalog <file://...,s3://...,gs://...,az://...>
```

## Common options

| Option | Default |
|---|---|
| `--bind <address:port>` | `0.0.0.0:5432` |
| `--max-sessions <n>` | `50` |
| `--mode <writer\|reader>` | `writer` |
| `--read-only` | off; alias for reader mode |
| `--cost-mode <conservative\|balanced\|latency>` | `balanced` |
| `--metrics-port <port>` | disabled |
| `--metrics-path <path>` | `/metrics` |
| `--tls-cert <path>` / `--tls-key <path>` | disabled |
| `--tls-required` | off |
| `--auth-user <name>` / `--auth-password <secret>` | disabled |
| `--s3-endpoint <url>` / `--s3-path-style` | disabled |
| `--encryption-key <64 hex digits>` | disabled |
| `--datafusion-pg-wire <port>` | disabled |
| `--idle-connection-timeout <seconds>` | `60` |
| `--drain-timeout <seconds>` | `30` |
| `--datafusion-bridge-queue-depth <n>` | `256` |

Run `rocklake serve --help` for the authoritative list and validation rules.
The server uses the standard AWS, GCS, and Azure credential environment
variables for cloud storage.
