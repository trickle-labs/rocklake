# Upgrades

Install the v0.51.3 binary, stop the current process, and restart it with the
same catalog URL and flags. Catalog state remains in the configured object
store.

```bash
rocklake --version
rocklake serve --catalog s3://bucket/catalog/ --bind 127.0.0.1:5432
```

Before upgrading, run `rocklake inspect snapshot --catalog ...` and keep an
export if the catalog is operationally important. Review the relevant
`CHANGELOG.md` entry and rerun `scripts/quickstart.sh` for a local binary.

There is no published Docker image or RockLake-specific container upgrade path
in v0.51.3.
