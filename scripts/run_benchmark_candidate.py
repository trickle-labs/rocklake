#!/usr/bin/env python3
"""Run the small v0.63.2 benchmark and write its candidate report."""

import argparse
import json
import os
import platform
import signal
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKLOAD = "get_current_snapshot_warm"


def command_text(command: list[str]) -> str:
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def stop_process(process, sig) -> None:
    try:
        os.killpg(process.pid, sig)
    except ProcessLookupError:
        pass


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target")
    args = parser.parse_args()

    estimates_path = args.target_dir / "criterion" / WORKLOAD / "new" / "estimates.json"
    estimates_path.unlink(missing_ok=True)
    process = subprocess.Popen(
        [
            "cargo",
            "bench",
            "-p",
            "rocklake-catalog",
            "--bench",
            "catalog_bench",
            "--target-dir",
            str(args.target_dir),
            "--",
            WORKLOAD,
            "--noplot",
            "--warm-up-time",
            "1",
            "--measurement-time",
            "1",
            "--sample-size",
            "20",
        ],
        cwd=ROOT,
        start_new_session=True,
    )
    deadline = time.monotonic() + 600
    while process.poll() is None and not estimates_path.exists():
        if time.monotonic() >= deadline:
            stop_process(process, signal.SIGKILL)
            process.wait()
            raise TimeoutError("benchmark did not produce an estimate within 10 minutes")
        time.sleep(0.1)
    if estimates_path.exists() and process.poll() is None:
        stop_process(process, signal.SIGTERM)
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            stop_process(process, signal.SIGKILL)
            process.wait()
    elif process.returncode:
        raise subprocess.CalledProcessError(process.returncode, process.args)

    estimates = json.loads(estimates_path.read_text())
    nanoseconds = estimates["median"]["point_estimate"]
    output = {
        "schema_version": 1,
        "benchmark": "rocklake-catalog-smoke",
        "release": "v0.63.2",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "source_revision": command_text(["git", "rev-parse", "HEAD"]),
        "environment": {
            "os": platform.platform(),
            "arch": platform.machine(),
            "rust_version": command_text(["rustc", "--version"]),
        },
        "measurement_identity": {
            "benchmark": "rocklake-catalog-smoke",
            "workload": WORKLOAD,
            "storage": "LocalFileSystem (tmpdir)",
            "catalog": "100 data files",
        },
        "results": {WORKLOAD: nanoseconds / 1_000},
        "units": {WORKLOAD: "us"},
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2) + "\n")
    print(f"Wrote {args.output}")


if __name__ == "__main__":
    main()
