#!/usr/bin/env python3
"""Validate the committed v0.61 capacity inputs."""

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PRICING = ROOT / "benchmarks/pricing/us-east-1-2026-09-11.json"
PROFILES = ROOT / "benchmarks/evidence/v0.61.0/profiles.json"


def main() -> None:
    pricing = json.loads(PRICING.read_text())
    assert pricing["schema_version"] == 1
    assert pricing["effective_date"] == "2026-09-11"
    assert pricing["currency"] == "USD"
    assert pricing["storage_usd_per_gb_month"] >= 0
    assert set(pricing["request_cost_per_1000"]) == {"get", "put", "list", "delete"}
    assert all(value >= 0 for value in pricing["request_cost_per_1000"].values())

    evidence = json.loads(PROFILES.read_text())
    assert evidence == {
        "release": "v0.61.0",
        "schema_version": 1,
        "source_evidence": "v0.52.0 LocalFS and MinIO scale classes",
        "profiles": [
            {"name": "small", "max_data_files": 10000, "max_read_ops_per_second": 50, "max_write_ops_per_second": 5},
            {"name": "medium", "max_data_files": 100000, "max_read_ops_per_second": 250, "max_write_ops_per_second": 25},
            {"name": "large", "max_data_files": 1000000, "max_read_ops_per_second": 1000, "max_write_ops_per_second": 100},
        ],
    }
    print("v0.61.0 capacity contract is valid")


if __name__ == "__main__":
    main()
