#!/usr/bin/env python3
"""
Compatibility manifest validator.

Verifies manifest evidence against docs and GitHub Actions:

  - fixture paths exist;
  - supported claims appear in the compatibility matrix;
  - referenced workflow jobs and test targets exist; and
  - supported claims have validation commands.

Exit codes:
  0 — all checks passed
  1 — one or more violations found
"""

from __future__ import annotations

import re
import shlex
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
    in docs/compatibility.md. We use lenient keyword matching for wording
    differences between the manifest and the documentation.
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


def load_ci_jobs() -> dict[str, str]:
    """Return GitHub Actions job IDs and their workflow text."""
    jobs: dict[str, str] = {}
    for workflow in sorted((REPO_ROOT / ".github" / "workflows").glob("*.yml")):
        lines = workflow.read_text(encoding="utf-8").splitlines()
        try:
            start = next(i for i, line in enumerate(lines) if line.strip() == "jobs:")
        except StopIteration:
            continue

        job_id: str | None = None
        job_lines: list[str] = []
        for line in lines[start + 1 :]:
            if line and not line[0].isspace():
                break
            match = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
            if match:
                if job_id:
                    jobs[job_id] = "\n".join(job_lines)
                job_id = match.group(1)
                job_lines = []
            elif job_id:
                job_lines.append(line)
        if job_id:
            jobs[job_id] = "\n".join(job_lines)
    return jobs


def check_ci_jobs_for_supported(entries: list[dict], jobs: dict[str, str]) -> list[str]:
    """Ensure claimed CI jobs exist; supported entries must name one."""
    errors: list[str] = []
    for entry in entries:
        ci_job = entry.get("ci_job", "").strip()
        if entry.get("status") == "supported" and not ci_job:
            errors.append(
                f"[ci-job-missing] Supported entry {entry['name']!r} "
                f"has no ci_job. Add a CI job or downgrade status to 'expected'."
            )
        elif ci_job and ci_job not in jobs:
            errors.append(
                f"[ci-job-unknown] {entry['name']!r} references missing "
                f"GitHub Actions job {ci_job!r}."
            )
    return errors


def check_ci_commands(entries: list[dict], jobs: dict[str, str]) -> list[str]:
    """Check manifest commands name real packages/tests present in their job."""
    errors: list[str] = []
    for entry in entries:
        command = entry.get("test_command", "").strip()
        if not command:
            if entry.get("status") == "supported":
                errors.append(
                    f"[test-command-missing] Supported entry {entry['name']!r} "
                    "has no test command."
                )
            continue
        try:
            tokens = shlex.split(command)
        except ValueError as error:
            errors.append(f"[test-command-invalid] {entry['name']!r}: {error}.")
            continue
        if len(tokens) < 2 or tokens[0] not in {"cargo", "cross"}:
            errors.append(
                f"[test-command-invalid] {entry['name']!r}: "
                "expected a cargo/cross command."
            )
            continue

        job_text = jobs.get(entry.get("ci_job", ""), "")
        package = next(
            (
                tokens[i + 1]
                for i, token in enumerate(tokens[:-1])
                if token in {"-p", "--package"}
            ),
            None,
        )
        target = next(
            (tokens[i + 1] for i, token in enumerate(tokens[:-1]) if token == "--test"),
            None,
        )
        if package and not (REPO_ROOT / "crates" / package / "Cargo.toml").is_file():
            errors.append(
                f"[test-package-missing] {entry['name']!r} "
                f"names unknown package {package!r}."
            )
        if target and (
            not package
            or not (REPO_ROOT / "crates" / package / "tests" / f"{target}.rs").is_file()
        ):
            errors.append(
                f"[test-target-missing] {entry['name']!r} "
                f"names missing test target {target!r}."
            )

        for option in ("--workspace", "--all-targets", "--all-features"):
            if option in tokens and option not in job_text:
                errors.append(
                    f"[test-command-drift] {entry['name']!r}: {option} "
                    f"is absent from job {entry.get('ci_job')!r}."
                )
        for value in (package, target):
            if value and value not in job_text:
                errors.append(
                    f"[test-command-drift] {entry['name']!r}: {value!r} "
                    f"is absent from job {entry.get('ci_job')!r}."
                )
        for i, token in enumerate(tokens[:-1]):
            if token in {"--features", "--target"} and tokens[i + 1] not in job_text:
                errors.append(
                    f"[test-command-drift] {entry['name']!r}: {token} value "
                    f"{tokens[i + 1]!r} is absent from job {entry.get('ci_job')!r}."
                )
    return errors


def check_docs_claims_have_manifest_entries(entries: list[dict], doc: str) -> list[str]:
    """
    Warn when a compatibility matrix row has no manifest entry. Detailed
    DuckLake corpus and feature checklists share the catalog-format entry.
    """
    warnings: list[str] = []
    sections = {
        "DuckDB Client Versions",
        "SQL Clients",
        "Apache Spark",
        "Trino / Presto",
        "Apache DataFusion",
        "Object Storage Backends",
        "SlateDB",
        "TLS",
        "Rust",
        "Platform",
    }
    section = ""
    for i, line in enumerate(doc.splitlines(), 1):
        if line.startswith("## "):
            section = line[3:].strip()
        elif section in sections and "✅" in line and line.lstrip().startswith("|"):
            row = " ".join(part.strip() for part in line.split("|")).lower()
            matched = any(
                entry.get("status") == "supported"
                and (
                    entry["name"].lower() in row
                    or (
                        entry.get("version", "").lower() not in {"", "any", "all"}
                        and entry["version"].lower() in row
                    )
                )
                for entry in entries
            )
            if not matched:
                warnings.append(
                    f"[drift-warn] Line {i} in docs/compatibility.md has no "
                    "matching supported manifest entry."
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
    ci_jobs = load_ci_jobs()

    all_errors: list[str] = []
    all_warnings: list[str] = []

    all_errors.extend(check_fixture_files(entries))
    all_errors.extend(check_supported_rows_in_docs(entries, doc))
    all_errors.extend(check_ci_jobs_for_supported(entries, ci_jobs))
    all_errors.extend(check_ci_commands(entries, ci_jobs))
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
