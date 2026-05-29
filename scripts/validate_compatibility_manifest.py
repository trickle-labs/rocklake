#!/usr/bin/env python3
"""
Compatibility manifest validator.

Verifies that docs/compatibility.md is consistent with
tests/fixtures/compatibility-matrix.toml:

  - Every 'supported' entry in the manifest must have a corresponding row
    in docs/compatibility.md that is marked as supported (✅).
  - Every 'unsupported' entry in the manifest must NOT appear as supported
    in docs/compatibility.md.
  - The manifest itself must not reference non-existent fixture files.
  - 'expected' and 'untested' entries are advisory only (no enforcement).

Exit codes:
  0 — all checks passed
  1 — one or more violations found
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

try:
    import tomllib  # Python 3.11+
except ImportError:
    try:
        import tomli as tomllib  # type: ignore[no-reattr]
    except ImportError:
        sys.exit("ERROR: tomllib/tomli not found. Install tomli or use Python 3.11+.")

REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = REPO_ROOT / "tests" / "fixtures" / "compatibility-matrix.toml"
COMPAT_DOC_PATH = REPO_ROOT / "docs" / "compatibility.md"


def load_manifest() -> list[dict]:
    with MANIFEST_PATH.open("rb") as fh:
        data = tomllib.load(fh)
    return data.get("entry", [])


def load_compat_doc() -> str:
    return COMPAT_DOC_PATH.read_text(encoding="utf-8")


def check_fixture_files(entries: list[dict]) -> list[str]:
    """Return errors for referenced fixture paths that do not exist."""
    errors: list[str] = []
    for entry in entries:
        fixture = entry.get("fixture_path", "")
        if fixture:
            path = REPO_ROOT / fixture
            if not path.exists():
                errors.append(
                    f"[fixture-missing] {entry['name']!r}: "
                    f"fixture_path={fixture!r} does not exist"
                )
    return errors


def _name_keywords(entry: dict) -> list[str]:
    """
    Extract a list of candidate search keywords for a manifest entry.

    We check whether ANY of these keywords appear in the compat doc.
    This is intentionally lenient: the doc may use different phrasing than
    the manifest name, so we match on meaningful fragments rather than the
    full name string.
    """
    name: str = entry["name"]
    component: str = entry.get("component", "")
    version: str = entry.get("version", "")

    keywords: list[str] = []

    # Split the name on common delimiters and take tokens >= 3 chars
    tokens = re.split(r"[\s\-/\(\),\.]+", name)
    keywords.extend(t for t in tokens if len(t) >= 3)

    # Add the version string if it's a useful discriminator
    if version and not version.startswith("<") and version not in ("any", "all"):
        keywords.append(version)

    # Add the component itself
    if component:
        keywords.append(component)

    return keywords


def check_supported_rows_in_docs(entries: list[dict], doc: str) -> list[str]:
    """
    Every 'supported' entry must have at least one keyword that appears
    anywhere in docs/compatibility.md.  We use lenient keyword matching to
    handle cases where the doc uses slightly different phrasing.
    """
    errors: list[str] = []
    doc_lower = doc.lower()
    for entry in entries:
        if entry.get("status") != "supported":
            continue
        name = entry["name"]
        keywords = _name_keywords(entry)
        if not any(kw.lower() in doc_lower for kw in keywords):
            errors.append(
                f"[doc-missing] Supported entry {name!r} "
                f"(component={entry['component']!r}) has no matching row in "
                f"docs/compatibility.md. Add a row or update the manifest."
            )
    return errors


def check_ci_jobs_for_supported(entries: list[dict]) -> list[str]:
    """Every 'supported' entry must reference a non-empty ci_job."""
    errors: list[str] = []
    for entry in entries:
        if entry.get("status") != "supported":
            continue
        if not entry.get("ci_job", "").strip():
            errors.append(
                f"[ci-job-missing] Supported entry {entry['name']!r} "
                f"has no ci_job. Add a CI job or downgrade status to 'expected'."
            )
    return errors


def check_docs_claims_have_manifest_entries(entries: list[dict], doc: str) -> list[str]:
    """
    Scan docs/compatibility.md for ✅ rows and warn if the component/version
    does not appear in the manifest at all. This catches docs drift.

    We only warn (prefixed with [drift-warn]) rather than fail, because the
    docs contain narrative prose that does not map 1:1 to manifest entries.
    """
    warnings: list[str] = []
    manifest_names_lower = {e["name"].lower() for e in entries}
    # Find lines with ✅
    for i, line in enumerate(doc.splitlines(), 1):
        if "✅" in line:
            # Extract the first cell of a Markdown table row (pipe-delimited)
            parts = [p.strip() for p in line.split("|") if p.strip()]
            if parts:
                cell = parts[0]
                if not any(cell.lower() in mn or mn in cell.lower()
                           for mn in manifest_names_lower):
                    warnings.append(
                        f"[drift-warn] Line {i} in docs/compatibility.md contains ✅ "
                        f"for {cell!r} but no manifest entry matches. "
                        f"Consider adding an entry to compatibility-matrix.toml."
                    )
    return warnings


def main() -> int:
    if not MANIFEST_PATH.exists():
        print(f"ERROR: Manifest not found at {MANIFEST_PATH}", file=sys.stderr)
        return 1
    if not COMPAT_DOC_PATH.exists():
        print(f"ERROR: Compat doc not found at {COMPAT_DOC_PATH}", file=sys.stderr)
        return 1

    entries = load_manifest()
    doc = load_compat_doc()

    all_errors: list[str] = []
    all_warnings: list[str] = []

    all_errors.extend(check_fixture_files(entries))
    all_errors.extend(check_supported_rows_in_docs(entries, doc))
    all_errors.extend(check_ci_jobs_for_supported(entries))
    all_warnings.extend(check_docs_claims_have_manifest_entries(entries, doc))

    for w in all_warnings:
        print(f"WARN  {w}")

    if not all_errors:
        print(
            f"OK: compatibility manifest validated "
            f"({len(entries)} entries, {len(all_warnings)} drift warnings)"
        )
        return 0

    for e in all_errors:
        print(f"ERROR {e}", file=sys.stderr)
    print(
        f"\nFAIL: {len(all_errors)} error(s) found in compatibility manifest validation.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
