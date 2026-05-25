#!/usr/bin/env python3
"""Check benchmark results against regression thresholds.

Usage:
    python scripts/check_benchmark_regression.py benchmarks/v0.15-ivm-hardening.json

Exits with code 1 if any metric exceeds its regression threshold.
"""

import json
import sys
from pathlib import Path


def check_regression(benchmark_path: str) -> bool:
    """Check if any benchmark metric exceeds its regression threshold.

    Returns True if all metrics are within bounds, False otherwise.
    """
    path = Path(benchmark_path)
    if not path.exists():
        print(f"ERROR: Benchmark file not found: {path}")
        return False

    with open(path) as f:
        data = json.load(f)

    results = data.get("results", {})
    thresholds = data.get("regression_thresholds", {})

    if not thresholds:
        print("WARNING: No regression thresholds defined")
        return True

    passed = True
    for metric, threshold in thresholds.items():
        if metric not in results:
            print(f"  SKIP: {metric} (not in results)")
            continue

        actual = results[metric]

        # Handle "min" thresholds (metric name ends with _min).
        if metric.endswith("_min"):
            base_metric = metric[:-4]  # Remove _min suffix.
            actual = results.get(base_metric, actual)
            if actual < threshold:
                print(f"  FAIL: {base_metric} = {actual} < {threshold} (minimum)")
                passed = False
            else:
                print(f"  PASS: {base_metric} = {actual} >= {threshold} (minimum)")
        else:
            if actual > threshold:
                print(f"  FAIL: {metric} = {actual} > {threshold}")
                passed = False
            else:
                print(f"  PASS: {metric} = {actual} <= {threshold}")

    return passed


def main():
    if len(sys.argv) < 2:
        # Default: check all benchmark files.
        benchmark_dir = Path("benchmarks")
        files = sorted(benchmark_dir.glob("*.json"))
        if not files:
            print("No benchmark files found in benchmarks/")
            sys.exit(0)
    else:
        files = [Path(arg) for arg in sys.argv[1:]]

    all_passed = True
    for path in files:
        print(f"\nChecking {path.name}:")
        if not check_regression(str(path)):
            all_passed = False

    if not all_passed:
        print("\nBenchmark regression detected!")
        sys.exit(1)
    else:
        print("\nAll benchmarks within thresholds.")
        sys.exit(0)


if __name__ == "__main__":
    main()
