# Environment Variables Reference

RockLake v0.50.0 reads the variables below through its typed CLI. Command-line
flags take precedence over environment variables. Run `rocklake serve --help`
for the authoritative option list.

## RockLake variables

| Variable | CLI flag | Purpose |
|---|---|---|
| `ROCKLAKE_CATALOG` | `--catalog` | Catalog URL |
| `ROCKLAKE_METRICS_PATH` | `--metrics-path` | HTTP metrics path |
| `ROCKLAKE_AUTH_USER` | `--auth-user` | Required connection username |
| `ROCKLAKE_AUTH_PASSWORD` | `--auth-password` | Required connection password |
| `ROCKLAKE_AUTH_PASSWORD_FILE` | `--auth-password-file` | Read connection password from a file |
| `ROCKLAKE_ENCRYPTION_KEY_FILE` | `--encryption-key-file` | Read the encryption key from a file |
| `ROCKLAKE_EXTENSION_SCHEMAS` | `--extension-schemas` | Comma-separated extension schemas |
| `ROCKLAKE_OTLP_ENDPOINT` | `--otlp-endpoint` | OpenTelemetry HTTP endpoint |

The catalog path and server options can also be supplied directly:

```bash
ROCKLAKE_CATALOG=file:///var/lib/rocklake/catalog \
ROCKLAKE_AUTH_USER=ducklake \
ROCKLAKE_AUTH_PASSWORD="$(< /run/secrets/rocklake-auth-password)" \
rocklake serve
```

Keep secrets out of command-line arguments. Inject `ROCKLAKE_AUTH_PASSWORD`
from a secret manager or a permission-restricted file, and do not commit the
file. RockLake does not read a TOML or YAML configuration file.

## Object-store variables

Cloud credentials use the provider SDK variables. For example, AWS uses
`AWS_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and
`AWS_SESSION_TOKEN`. GCS and Azure use their standard SDK credential variables.

The S3-compatible endpoint and path-style options are CLI-only in v0.50.0:
`--s3-endpoint` and `--s3-path-style`.

## DuckLake data inlining

Data inlining is controlled by DuckDB's `DATA_INLINING_ROW_LIMIT` attach option,
not by a RockLake environment variable. Use `DATA_INLINING_ROW_LIMIT 0` when
you need every data file written as an external object during testing.
