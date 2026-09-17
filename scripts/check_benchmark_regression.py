#!/usr/bin/env python3
"""Compare a fresh benchmark report with a reviewed baseline."""

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any


class ReportError(ValueError):
    """The report does not satisfy the evidence contract."""


def _load(path: Path, label: str) -> dict[str, Any]:
    if not path.is_file():
        raise ReportError(f"{label} file not found: {path}")
    try:
        data = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ReportError(f"cannot read {label} report {path}: {error}") from error
    if not isinstance(data, dict) or not data:
        raise ReportError(f"{label} report must be a non-empty JSON object")
    if data.get("schema_version") != 1:
        raise ReportError(f"{label} report must use schema_version 1")
    return data


def _number(value: Any, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ReportError(f"{name} must be a number")
    if not math.isfinite(value):
        raise ReportError(f"{name} must be finite")
    return float(value)


def _identity(report: dict[str, Any], label: str) -> dict[str, Any]:
    identity = report.get("measurement_identity")
    if not isinstance(identity, dict) or not identity:
        raise ReportError(f"{label} report is missing measurement_identity")
    for key in ("benchmark", "workload", "storage"):
        if not isinstance(identity.get(key), str) or not identity[key].strip():
            raise ReportError(f"{label} measurement_identity.{key} is required")
    if report.get("benchmark") != identity["benchmark"]:
        raise ReportError(f"{label} benchmark does not match measurement_identity")
    return identity


def _results(report: dict[str, Any], label: str) -> dict[str, tuple[float, str]]:
    raw_results = report.get("results")
    if not isinstance(raw_results, dict) or not raw_results:
        raise ReportError(f"{label} report has no results")

    units = report.get("units", {})
    if not isinstance(units, dict):
        raise ReportError(f"{label} units must be an object")
    default_unit = report.get("unit")
    if default_unit is not None and (not isinstance(default_unit, str) or not default_unit.strip()):
        raise ReportError(f"{label} unit must be a non-empty string")

    parsed: dict[str, tuple[float, str]] = {}
    for metric, raw_value in raw_results.items():
        if not isinstance(metric, str) or not metric:
            raise ReportError(f"{label} result names must be non-empty strings")
        result_unit = units.get(metric, default_unit)
        value = raw_value
        if isinstance(raw_value, dict):
            if "value" not in raw_value:
                raise ReportError(f"{label} result {metric} is missing value")
            value = raw_value["value"]
            result_unit = raw_value.get("unit", result_unit)
        if not isinstance(result_unit, str) or not result_unit.strip():
            raise ReportError(f"{label} result {metric} is missing a unit")
        parsed[metric] = (_number(value, f"{label} result {metric}"), result_unit)
    return parsed


def _source_revision(report: dict[str, Any], label: str) -> None:
    value = report.get("source_revision")
    if not isinstance(value, str) or not value.strip() or value == "unknown":
        raise ReportError(f"{label} source_revision is required")


def _threshold_name(name: str) -> tuple[str, bool]:
    if name.endswith("_min"):
        metric = name[:-4]
        is_minimum = True
    elif name.endswith("_max"):
        metric = name[:-4]
        is_minimum = False
    else:
        metric = name
        is_minimum = False
    if not metric:
        raise ReportError(f"invalid threshold name: {name}")
    return metric, is_minimum


def check_regression(candidate_path: str | Path, baseline_path: str | Path) -> bool:
    """Return whether the candidate satisfies every baseline threshold."""
    try:
        candidate = _load(Path(candidate_path), "candidate")
        baseline = _load(Path(baseline_path), "baseline")
        candidate_identity = _identity(candidate, "candidate")
        baseline_identity = _identity(baseline, "baseline")
        if candidate_identity != baseline_identity:
            raise ReportError("candidate and baseline measurement identities differ")
        _source_revision(candidate, "candidate")
        _source_revision(baseline, "baseline")
        candidate_results = _results(candidate, "candidate")
        baseline_results = _results(baseline, "baseline")
        thresholds = baseline.get("regression_thresholds")
        if not isinstance(thresholds, dict) or not thresholds:
            raise ReportError("baseline report has no regression_thresholds")
    except ReportError as error:
        print(f"ERROR: {error}")
        return False

    passed = True
    for threshold_name, raw_threshold in thresholds.items():
        try:
            metric, is_minimum = _threshold_name(threshold_name)
            threshold = _number(raw_threshold, f"baseline threshold {threshold_name}")
        except ReportError as error:
            print(f"ERROR: {error}")
            passed = False
            continue

        baseline_value = baseline_results.get(metric)
        candidate_value = candidate_results.get(metric)
        if baseline_value is None:
            print(f"FAIL: baseline is missing required metric {metric}")
            passed = False
            continue
        if candidate_value is None:
            print(f"FAIL: candidate is missing required metric {metric}")
            passed = False
            continue
        if baseline_value[1] != candidate_value[1]:
            print(
                f"FAIL: {metric} unit mismatch: candidate {candidate_value[1]}, "
                f"baseline {baseline_value[1]}"
            )
            passed = False
            continue

        baseline_number = baseline_value[0]
        if is_minimum and threshold > baseline_number:
            print(f"ERROR: minimum threshold {threshold_name} is above its baseline result")
            passed = False
            continue
        if not is_minimum and threshold < baseline_number:
            print(f"ERROR: maximum threshold {threshold_name} is below its baseline result")
            passed = False
            continue

        candidate_number = candidate_value[0]
        if is_minimum:
            passed &= candidate_number >= threshold
            verdict = "PASS" if candidate_number >= threshold else "FAIL"
            print(f"{verdict}: {metric} = {candidate_number:g} >= {threshold:g} (minimum)")
        else:
            passed &= candidate_number <= threshold
            verdict = "PASS" if candidate_number <= threshold else "FAIL"
            print(f"{verdict}: {metric} = {candidate_number:g} <= {threshold:g} (maximum)")
    return passed


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--baseline", required=True, type=Path)
    args = parser.parse_args()
    if not check_regression(args.candidate, args.baseline):
        print("Benchmark regression gate failed.")
        sys.exit(1)
    print("Benchmark regression gate passed.")


if __name__ == "__main__":
    main()
