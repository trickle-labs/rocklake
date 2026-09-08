#!/usr/bin/env python3
"""Create the aggregate checksum and release manifest for published assets."""

import argparse
import hashlib
import json
from pathlib import Path


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--certified-sha", required=True)
    args = parser.parse_args()

    files = sorted(
        path
        for path in args.artifacts.iterdir()
        if path.is_file() and path.name not in {"SHA256SUMS", "release-manifest.json"}
    )
    if not files:
        raise SystemExit("no release artifacts found")

    checksums = {path.name: digest(path) for path in files}
    checksums_path = args.artifacts / "SHA256SUMS"
    checksums_path.write_text(
        "".join(f"{checksums[name]}  {name}\n" for name in sorted(checksums)),
        encoding="utf-8",
    )

    sbom = next(
        (path.name for path in files if path.name.endswith(".sbom.spdx.json")), None
    )
    assets = []
    provenance_subjects = []
    for path in files:
        asset = {
            "filename": path.name,
            "byte_length": path.stat().st_size,
            "sha256": checksums[path.name],
        }
        if path.name.endswith(".build-metadata.json"):
            metadata = json.loads(path.read_text(encoding="utf-8"))
            asset["target_triple"] = metadata["target"]
        binary = path.name.startswith("rocklake-") and not path.name.endswith(
            (".sha256", ".build-metadata.json", ".sbom.spdx.json")
        )
        if binary:
            metadata_name = f"{path.name}.build-metadata.json"
            if metadata_name in checksums:
                metadata = json.loads(
                    (args.artifacts / metadata_name).read_text(encoding="utf-8")
                )
                asset["target_triple"] = metadata["target"]
                asset["build_metadata_filename"] = metadata_name
            if sbom:
                asset["sbom_filename"] = sbom
            asset["provenance_subject"] = path.name
            provenance_subjects.append(path.name)
        assets.append(asset)

    manifest = {
        "schema_version": 1,
        "release_version": args.version,
        "certified_git_sha": args.certified_sha,
        "catalog_read_format": 1,
        "catalog_write_format": 1,
        "assets": assets,
        "checksum_file": {
            "filename": checksums_path.name,
            "sha256": digest(checksums_path),
        },
        "sbom_filename": sbom,
        "provenance_subjects": provenance_subjects,
    }
    (args.artifacts / "release-manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    main()
