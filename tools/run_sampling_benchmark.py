#!/usr/bin/env python3
"""Run a controlled local sample of the direct-grid parity benchmark.

The workload is a deterministic finite census.  This driver adds repeated
process-level timing observations while retaining the full correctness result
from every run.  It intentionally makes no cross-machine or asymptotic claim.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import platform
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
EXPECTED = {
    "masks_examined": 511,
    "components_examined": 897,
    "pipeline_comparisons": 1_794,
    "direct_rank_sort_count": 0,
    "direct_rank_map_entry_count": 0,
    "direct_rank_map_owned_bytes": 0,
    "ranked_rank_sort_count": 3_588,
    "ranked_rank_map_entry_count": 624,
    "ranked_rank_map_owned_bytes": 18_240,
}


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary",
        type=Path,
        default=Path("target/release/mrd"),
        help="release CLI binary relative to the repository root",
    )
    parser.add_argument("--repetitions", type=int, default=31)
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("results/benchmark-sampling.json"),
    )
    parser.add_argument(
        "--csv",
        type=Path,
        default=Path("results/benchmark-sampling-runs.csv"),
    )
    return parser.parse_args()


def root_relative(path: Path) -> Path:
    return path if path.is_absolute() else ROOT / path


def public_relative(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def command_output(command: list[str]) -> str:
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def command_or_unknown(command: list[str]) -> str:
    try:
        return command_output(command)
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def cpu_model() -> str:
    if sys.platform == "darwin":
        return command_or_unknown(["sysctl", "-n", "machdep.cpu.brand_string"])
    return platform.processor() or "unknown"


def percentile(values: list[float], proportion: float) -> float:
    """Use the inclusive linear-interpolation definition (R type 7)."""
    ordered = sorted(values)
    index = (len(ordered) - 1) * proportion
    lower = math.floor(index)
    upper = math.ceil(index)
    if lower == upper:
        return ordered[lower]
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (index - lower)


def summary(values: list[int | float]) -> dict[str, float | int]:
    if not values:
        raise ValueError("cannot summarize an empty sample")
    numeric = [float(value) for value in values]
    result: dict[str, float | int] = {
        "count": len(numeric),
        "min": min(numeric),
        "q1": percentile(numeric, 0.25),
        "median": statistics.median(numeric),
        "q3": percentile(numeric, 0.75),
        "max": max(numeric),
    }
    for key, value in tuple(result.items()):
        if isinstance(value, float) and value.is_integer():
            result[key] = int(value)
    return result


def total_phase_microseconds(report: dict[str, Any], backend: str) -> int:
    suffix = f"{backend}_phase_microseconds"
    return sum(
        sum(mode[suffix].values())
        for mode in report["mode_baselines"].values()
    )


def verify_report(report: dict[str, Any]) -> None:
    for key, expected in EXPECTED.items():
        actual = report.get(key)
        if actual != expected:
            raise RuntimeError(f"direct-grid parity {key}={actual!r}, expected {expected!r}")
    if report.get("mismatches"):
        raise RuntimeError(f"direct-grid parity mismatches: {report['mismatches']!r}")
    if report.get("solver_errors"):
        raise RuntimeError(f"direct-grid parity solver errors: {report['solver_errors']!r}")
    expected_modes = {"fully-audited", "compact-only"}
    if set(report.get("mode_baselines", {})) != expected_modes:
        raise RuntimeError("direct-grid parity mode baseline set changed")
    if any(
        mode.get("comparisons") != 897
        for mode in report["mode_baselines"].values()
    ):
        raise RuntimeError("direct-grid parity mode comparison count changed")


def run_once(binary: Path, output: Path) -> tuple[dict[str, Any], int]:
    command = [
        public_relative(binary),
        "benchmark",
        "--suite",
        "direct-grid-parity",
        "--output",
        public_relative(output),
    ]
    start = time.perf_counter_ns()
    completed = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    elapsed = (time.perf_counter_ns() - start) // 1_000
    if completed.returncode:
        raise RuntimeError(
            "benchmark command failed:\n"
            f"command: {' '.join(command)}\n"
            f"stdout: {completed.stdout}\n"
            f"stderr: {completed.stderr}"
        )
    report = json.loads(output.read_text(encoding="utf-8"))
    verify_report(report)
    return report, elapsed


def observed_run(sample: int, report: dict[str, Any], elapsed: int) -> dict[str, Any]:
    direct_embedding = report["direct_embedding_microseconds"]
    ranked_embedding = report["ranked_embedding_microseconds"]
    direct_total = total_phase_microseconds(report, "direct")
    ranked_total = total_phase_microseconds(report, "ranked")
    if ranked_embedding == 0 or ranked_total == 0:
        raise RuntimeError("timing resolution is insufficient for a paired ratio")
    return {
        "sample": sample,
        "process_wall_microseconds": elapsed,
        "direct_embedding_microseconds": direct_embedding,
        "ranked_embedding_microseconds": ranked_embedding,
        "direct_total_phase_microseconds": direct_total,
        "ranked_total_phase_microseconds": ranked_total,
        "direct_over_ranked_embedding_ratio": direct_embedding / ranked_embedding,
        "direct_over_ranked_total_phase_ratio": direct_total / ranked_total,
        "correctness": {
            key: report[key]
            for key in (
                "masks_examined",
                "components_examined",
                "pipeline_comparisons",
                "direct_rank_sort_count",
                "direct_rank_map_entry_count",
                "direct_rank_map_owned_bytes",
                "ranked_rank_sort_count",
                "ranked_rank_map_entry_count",
                "ranked_rank_map_owned_bytes",
            )
        },
        "benchmark_report": report,
    }


def csv_row(run: dict[str, Any]) -> dict[str, Any]:
    return {
        key: run[key]
        for key in (
            "sample",
            "process_wall_microseconds",
            "direct_embedding_microseconds",
            "ranked_embedding_microseconds",
            "direct_total_phase_microseconds",
            "ranked_total_phase_microseconds",
            "direct_over_ranked_embedding_ratio",
            "direct_over_ranked_total_phase_ratio",
        )
    }


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(f"{path.suffix}.tmp")
    temporary.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(path)


def write_csv(path: Path, runs: list[dict[str, Any]]) -> None:
    fields = list(csv_row(runs[0]))
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(f"{path.suffix}.tmp")
    with temporary.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(csv_row(run) for run in runs)
    temporary.replace(path)


def main() -> None:
    args = arguments()
    if args.repetitions < 1:
        raise SystemExit("--repetitions must be positive")
    if args.warmups < 0:
        raise SystemExit("--warmups cannot be negative")
    binary = root_relative(args.binary)
    output = root_relative(args.output)
    csv_output = root_relative(args.csv)
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise SystemExit(f"executable benchmark binary not found: {args.binary}")
    if output == csv_output:
        raise SystemExit("--output and --csv must name different files")

    temp_parent = ROOT / "results"
    temp_parent.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".benchmark-sampling-", dir=temp_parent) as temporary:
        temporary_path = Path(temporary)
        run_output = temporary_path / "direct-grid-parity.json"
        for _ in range(args.warmups):
            run_once(binary, run_output)
        runs = []
        for sample in range(1, args.repetitions + 1):
            report, elapsed = run_once(binary, run_output)
            runs.append(observed_run(sample, report, elapsed))

    summary_fields = (
        "process_wall_microseconds",
        "direct_embedding_microseconds",
        "ranked_embedding_microseconds",
        "direct_total_phase_microseconds",
        "ranked_total_phase_microseconds",
        "direct_over_ranked_embedding_ratio",
        "direct_over_ranked_total_phase_ratio",
    )
    source_status = command_output(["git", "status", "--porcelain"])
    payload = {
        "schema_version": 1,
        "claim_boundary": "Local process-level observations on one fixed finite workload; no cross-machine, throughput, asymptotic, or causal performance claim.",
        "source": {
            "git_commit": command_output(["git", "rev-parse", "HEAD"]),
            "working_tree_clean": not bool(source_status),
            "binary": public_relative(binary),
            "binary_sha256": file_sha256(binary),
            "rustc_version": command_or_unknown(["rustc", "--version"]),
        },
        "environment": {
            "operating_system": platform.platform(),
            "machine": platform.machine() or "unknown",
            "cpu_model": cpu_model(),
            "logical_cpu_count": os.cpu_count() or 0,
            "python_version": platform.python_version(),
        },
        "protocol": {
            "workload_command": "target/release/mrd benchmark --suite direct-grid-parity --output <temporary-relative-path>",
            "population": {
                "nonzero_three_by_three_masks": 511,
                "foreground_components": 897,
                "paired_pipeline_comparisons": 1_794,
                "verification_modes": ["fully-audited", "compact-only"],
            },
            "warmup_runs_excluded": args.warmups,
            "measured_process_repetitions": args.repetitions,
            "measurement_unit": "one fresh CLI process executing the complete finite workload",
            "pairing": "Every measured process runs both backends for every component and mode.",
            "backend_order": "Within each mode the workload executes ranked-coordinates before direct-grid-parity.",
            "randomization": "None. The input census and backend order are fixed; this controls the workload but leaves possible order and host-load bias explicit.",
            "statistics": "Minimum, inclusive-linear-interpolation quartiles (R type 7), median, and maximum; no significance test or confidence interval is reported.",
            "exclusion_rule": "Only the configured warm-up processes are excluded. A nonzero exit, mismatch, solver error, or changed structural counter aborts the campaign rather than being dropped.",
        },
        "correctness_gate": EXPECTED,
        "summary": {field: summary([run[field] for run in runs]) for field in summary_fields},
        "runs": runs,
    }
    write_json(output, payload)
    write_csv(csv_output, runs)
    print(f"wrote {public_relative(output)} and {public_relative(csv_output)}")


if __name__ == "__main__":
    main()
