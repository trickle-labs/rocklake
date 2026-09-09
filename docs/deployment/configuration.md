# Configuration

RockLake v0.55.0 accepts typed `rocklake.toml` configuration alongside the
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

## Managed catalog registry

The v0.55.0 registry stores routing state separately from tenant catalogs. It
keeps aliases, lifecycle state, policy references, and credential-provider
names; it never stores raw credentials. `remove` detaches a route and retains a
tombstone. It does not delete catalog or data bytes.

```toml
[registry]
location = "file:///var/lib/rocklake/registry"
emergency_read_only = true
recovery_file = "/etc/rocklake/recovery.toml"
```

Initialize and manage routes locally:

```bash
rocklake registry init --registry file:///var/lib/rocklake/registry
rocklake catalogs create --registry file:///var/lib/rocklake/registry \
  --id 018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9 --alias analytics \
  --catalog s3://company-rocklake/catalogs/analytics \
  --data s3://company-data/analytics --credential-provider aws-default \
  --request-id create-analytics
rocklake registry backup --registry file:///var/lib/rocklake/registry --output ./registry-backup
rocklake registry verify --registry file:///var/lib/rocklake/registry
```

Use `rocklake registry migrate-static` to import the v0.54 static route table.
Registry and tenant prefixes must be disjoint. Registry management is local;
there is no remote management API in this release.

## Static multi-catalog routing

Put multiple independent catalogs in `rocklake.toml`. The PostgreSQL startup
`database` name selects an alias; aliases are not used as storage prefixes.

```toml
[router]
mode = "static"
default_catalog = "analytics"
max_open_catalogs = 64
catalog_idle_timeout = 300

[[catalogs]]
id = "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9"
aliases = ["analytics", "analytics_prod"]
catalog = "s3://company-rocklake/catalogs/analytics"
data = "s3://company-data/analytics"
mode = "read-write"
credential_provider = "aws-default"

[[catalogs]]
id = "018f4f4d-d520-7d91-b9f0-7018b7b50d13"
aliases = ["research"]
catalog = "s3://company-rocklake/catalogs/research"
data = "s3://company-data/research"
mode = "read-only"
credential_provider = "aws-default"
```

Validate routes without opening a catalog, or inspect the redacted route table:

```bash
rocklake catalogs validate
rocklake catalogs list --output json
rocklake catalogs status
```

Catalog and data locations must use provider credentials from the environment
or a named provider. Embedded URL credentials, traversal segments, and any
equal or ancestor/descendant prefix overlap are rejected.

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
| `--max-active-scans <n>` | `25` |
| `--stream-queue-depth <n>` | `64` |
| `--max-buffered-rows <n>` | `1024` |
| `--max-response-bytes <n>` | Unlimited (optional) |
| `--slow-operation-threshold-ms <n>` | `1000` |

`--stream-queue-depth` and `--max-buffered-rows` remain accepted for
compatibility. RockLake warns when either option is configured, but neither
option has an independent runtime effect. Leave both options out of new
configuration files.

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
