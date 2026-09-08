# ADR: v0.51.5 release artifact contract

## Decision

RockLake releases publish one raw executable per supported target:

| Target | Filename |
| --- | --- |
| Linux x86-64 | `rocklake-linux-x86_64` |
| Linux aarch64 | `rocklake-linux-aarch64` |
| macOS arm64 | `rocklake-macos-arm64` |
| Windows x86-64 | `rocklake-windows-x86_64.exe` |

Each executable has a matching `.sha256` and `.build-metadata.json` file. The
release also publishes one `SHA256SUMS`, one `release-manifest.json`, one SPDX
SBOM, and provenance attestations. The manifest is authoritative for filenames,
digests, target triples, and the certified Git SHA.

## Rationale

The raw names match the files produced by the build workflow and keep install
commands portable across releases. Archives would add another naming and
extraction contract without reducing the binary installation steps.
