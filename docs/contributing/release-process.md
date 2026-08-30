# Release process

Releases are prepared from the tagged source commit. For v0.48.0, the
release-blocking documentation gates are the strict MkDocs build, compatibility
manifest validation, the executable quickstart, and the preserved v0.47.17
production-failure certification.

## Before the release PR

Run the checks that apply locally:

```bash
cargo fmt --all -- --check
cargo test --workspace
mkdocs build --strict
python scripts/validate_compatibility_manifest.py
bash scripts/quickstart.sh
```

Update `CHANGELOG.md` and current version references. Keep claims tied to
tests: v0.48.0 supports the binary, DuckLake 1.0 targets covered by CI, local
and cloud object storage, server-side TLS, and password authentication. It
does not publish Docker images or support configuration files, mTLS, or
certificate hot-reload.

## Tagging

After review and green CI, merge the release PR and tag the merge commit:

```bash
git tag v0.48.0
git push origin v0.48.0
```

Release artifacts must be built from that tag. The v0.47.17 certification
remains a required regression gate for later releases.
