# Support bundles

Create a redacted bundle for incident reports. The destination must be a new
directory; the command never overwrites an existing directory.

```bash
rocklake support bundle --catalog ./lake --output ./rocklake-support
```

If `metrics_port` is configured, the command snapshots the local Prometheus
endpoint. Use `--metrics-url` when metrics are exposed elsewhere:

```bash
rocklake support bundle \
  --catalog ./lake \
  --metrics-url http://127.0.0.1:9090/metrics \
  --output ./rocklake-support
```

The bundle contains:

- `version.json` and `config.json` with credentials and catalog URL credentials
  redacted.
- `status.json` and `verification.json` when the catalog can be opened.
- `metrics.prom` when a metrics endpoint is reachable.
- `logs.txt`, which records that logs are emitted to stderr and are not copied
  automatically.
- `support-bundle.json`, the bundle schema and redaction declaration.

Attach the directory contents to a support request only after checking for
deployment-specific paths or identifiers that your organization treats as
sensitive.
