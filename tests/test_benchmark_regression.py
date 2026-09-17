import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))

from scripts.check_benchmark_regression import check_regression


IDENTITY = {
    "benchmark": "catalog-smoke",
    "workload": "latency and throughput",
    "storage": "local",
}


def report(results, units=None, source_revision="abc123"):
    data = {
        "schema_version": 1,
        "benchmark": "catalog-smoke",
        "source_revision": source_revision,
        "measurement_identity": IDENTITY,
        "results": results,
    }
    if units is not None:
        data["units"] = units
    return data


def run_check(candidate, baseline):
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        candidate_path = root / "candidate.json"
        baseline_path = root / "baseline.json"
        candidate_path.write_text(json.dumps(candidate))
        baseline_path.write_text(json.dumps(baseline))
        return check_regression(candidate_path, baseline_path)


def test_latency_maximum_and_throughput_minimum_pass():
    baseline = report(
        {"latency": 10, "throughput": 100}, {"latency": "us", "throughput": "rows/s"}
    )
    baseline["regression_thresholds"] = {"latency_max": 12, "throughput_min": 90}
    candidate = report(
        {"latency": 11, "throughput": 95}, {"latency": "us", "throughput": "rows/s"}
    )
    assert run_check(candidate, baseline)


def test_missing_metric_fails():
    baseline = report({"latency": 10}, {"latency": "us"})
    baseline["regression_thresholds"] = {"latency_max": 12}
    candidate = report({}, {})
    assert not run_check(candidate, baseline)


def test_empty_baseline_fails():
    baseline = report({}, {})
    baseline["regression_thresholds"] = {"latency_max": 12}
    candidate = report({"latency": 10}, {"latency": "us"})
    assert not run_check(candidate, baseline)


def test_nonfinite_candidate_fails():
    baseline = report({"latency": 10}, {"latency": "us"})
    baseline["regression_thresholds"] = {"latency_max": 12}
    candidate = report({"latency": float("nan")}, {"latency": "us"})
    assert not run_check(candidate, baseline)


def test_incompatible_identity_and_units_fail():
    baseline = report({"latency": 10}, {"latency": "us"})
    baseline["regression_thresholds"] = {"latency_max": 12}
    candidate = report({"latency": 10}, {"latency": "us"})
    candidate["measurement_identity"] = {**IDENTITY, "storage": "minio"}
    assert not run_check(candidate, baseline)

    candidate = report({"latency": 10}, {"latency": "ms"})
    assert not run_check(candidate, baseline)


if __name__ == "__main__":
    for name, test in sorted(globals().items()):
        if name.startswith("test_"):
            test()
    print("benchmark regression tests passed")
