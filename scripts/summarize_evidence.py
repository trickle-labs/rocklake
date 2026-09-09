#!/usr/bin/env python3
"""Render a compact Markdown summary from a v0.52 evidence report."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path)
    args = parser.parse_args()
    report = json.loads(args.report.read_text())

    print(f"# {report['release']} {report['backend']} evidence")
    print()
    print(f"Generated: `{report['generated_at']}`  ")
    print(f"Git SHA: `{report['machine']['git_sha']}`")
    print(f"Binary SHA-256: `{report['machine']['binary_sha256'] or 'n/a'}`")
    print()
    print("| Dataset | Operation | Rows | Operation µs | First row µs | Peak RSS | Object ops | Digest |")
    print("|---:|---|---:|---:|---:|---:|---:|---|")
    for result in report["results"]:
        baseline = result["baseline_rss_bytes"] or 0
        peak = result["peak_rss_bytes"]
        incremental = "n/a" if peak is None else str(max(0, peak - baseline))
        digest = "n/a" if result["matches_expected_digest"] is None else ("ok" if result["matches_expected_digest"] else "FAIL")
        first_row = result["first_row_us"] if result["first_row_us"] is not None else "n/a"
        print(
            f"| {result['dataset_size']} | `{result['operation']}` | {result['rows']} | "
            f"{result['operation_us']} | {first_row} | {incremental} | "
            f"{result['object_store_operations']} | {digest} |"
        )


if __name__ == "__main__":
    main()
