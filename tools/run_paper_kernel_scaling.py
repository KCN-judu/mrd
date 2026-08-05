#!/usr/bin/env python3
"""Run resumable in-process paper-kernel benchmark partitions."""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
import os
import platform
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = 1
CHECKPOINT_SCHEMA_VERSION = 1
CAMPAIGN = "paper-kernel-scaling"
ALGORITHMS = (
    "compact-mrd",
    "explicit-hopcroft-karp",
    "explicit-c0-flow",
)
SCOPES = (
    "solve-from-canonical-instance",
    "representation-and-solver-kernel",
)
FAMILIES = (
    "random-connected",
    "dense-conflict",
    "sparse-conflict",
    "comb-staircase",
    "supported-holes",
    "representation-crossover",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=Path("target/release/mrd"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--csv", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--family", action="append", choices=FAMILIES)
    parser.add_argument("--size", action="append", type=int)
    parser.add_argument("--print-plan", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def root_path(path: Path) -> Path:
    return path if path.is_absolute() else ROOT / path


def relative_path(path: Path) -> str:
    return path.resolve().relative_to(ROOT).as_posix()


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def command_or_unknown(command: list[str]) -> str:
    try:
        return subprocess.check_output(command, cwd=ROOT, text=True).strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def power_state() -> dict[str, Any]:
    if sys.platform != "darwin":
        return {"power_source": "unknown", "turbo_or_power_mode": "unknown"}
    output = command_or_unknown(["pmset", "-g", "batt"])
    source = "AC" if "AC Power" in output else "battery" if "Battery Power" in output else "unknown"
    return {"power_source": source, "turbo_or_power_mode": "unknown"}


def memory_pressure() -> str:
    if sys.platform != "darwin":
        return "unavailable"
    output = command_or_unknown(["memory_pressure"])
    for line in output.splitlines():
        if "System-wide memory free percentage" in line:
            return line.strip()
    return "available-but-unparsed" if output != "unknown" else "unavailable"


def environment(binary: Path) -> dict[str, Any]:
    cpu = (
        command_or_unknown(["sysctl", "-n", "machdep.cpu.brand_string"])
        if sys.platform == "darwin"
        else platform.processor() or "unknown"
    )
    return {
        "clock_source": "std::time::Instant (platform monotonic high-resolution clock)",
        "clock_resolution_ns": None,
        "cpu_model": cpu,
        "operating_system": platform.platform(),
        "architecture": platform.machine() or "unknown",
        "logical_cpu_count": os.cpu_count() or 0,
        "rustc_version": command_or_unknown(["rustc", "--version"]),
        "compiler_profile": "workspace release profile",
        "git_commit": command_or_unknown(["git", "rev-parse", "HEAD"]),
        "git_dirty": bool(command_or_unknown(["git", "status", "--porcelain"])),
        "binary_sha256": sha256_file(binary),
        "memory_pressure_note": memory_pressure(),
        **power_state(),
    }


def validate_config(config: dict[str, Any]) -> None:
    if config.get("schema_version") != SCHEMA_VERSION or config.get("campaign") != CAMPAIGN:
        raise ValueError("config must use paper-kernel-scaling schema version 1")
    if config.get("algorithms") != list(ALGORITHMS):
        raise ValueError("config must contain exactly the three timed algorithms in declared order")
    if config.get("scopes") != list(SCOPES):
        raise ValueError("config must contain exactly Scope A and Scope B")
    families = config.get("families")
    sizes = config.get("initial_size_levels")
    if not families or len(set(families)) != len(families) or any(value not in FAMILIES for value in families):
        raise ValueError("families must be unique supported campaign families")
    if not sizes or len(set(sizes)) != len(sizes) or any(not isinstance(value, int) or value <= 0 for value in sizes):
        raise ValueError("initial_size_levels must be unique positive integers")
    warmup = config.get("warmup_rule", {})
    repetitions = config.get("repetition_rule", {})
    stop = config.get("stop_conditions", {})
    fit = config.get("fit_rule", {})
    if int(warmup.get("minimum", 0)) < 5 or int(warmup.get("maximum", 0)) > 50:
        raise ValueError("warmup rule must use at least 5 and at most 50 iterations")
    if int(warmup.get("maximum", 0)) < int(warmup.get("minimum", 0)):
        raise ValueError("warmup maximum must cover its minimum")
    if [repetitions.get(key) for key in ("fast_minimum", "medium_minimum", "slow_minimum")] != [31, 15, 7]:
        raise ValueError("repetition minima must be 31/15/7")
    if int(repetitions.get("maximum", 0)) > 10_000:
        raise ValueError("maximum repetitions cannot exceed 10000")
    if int(stop.get("max_iteration_ns", 0)) != 5_000_000_000:
        raise ValueError("per-iteration stop must be five seconds")
    if int(stop.get("max_point_ns", 0)) != 120_000_000_000:
        raise ValueError("per-point stop must be 120 seconds")
    if float(config.get("partition_timeout_seconds", 0)) <= 120:
        raise ValueError("partition timeout must exceed the internal point limit")
    if int(fit.get("minimum_valid_size_levels", 0)) < 8:
        raise ValueError("fit rule requires at least eight valid size levels")
    if int(fit.get("bootstrap_resamples", 0)) < 10_000:
        raise ValueError("bootstrap resamples must be at least 10000")


def point_identity(config_sha256: str, family: str, size: int, seed: int) -> str:
    fields = {
        "campaign": CAMPAIGN,
        "config_sha256": config_sha256,
        "family": family,
        "target_size": size,
        "seed": seed,
    }
    return "sha256:" + sha256_bytes(canonical_json(fields).encode())


def plan(config: dict[str, Any], config_sha256: str) -> list[dict[str, Any]]:
    seed = int(config["seed"])
    rows = [
        {
            "point_identity": point_identity(config_sha256, family, size, seed),
            "family": family,
            "target_size": size,
            "seed": seed,
        }
        for family in config["families"]
        for size in config["initial_size_levels"]
    ]
    if len({row["point_identity"] for row in rows}) != len(rows):
        raise ValueError("duplicate planned point identity")
    return rows


def rust_request(config: dict[str, Any], point: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": CAMPAIGN,
        "family": point["family"],
        "target_size": point["target_size"],
        "seed": point["seed"],
        "algorithms": config["algorithms"],
        "scopes": config["scopes"],
        "oracle_cell_limit": config["oracle_cell_limit"],
        "warmup": config["warmup_rule"],
        "repetitions": config["repetition_rule"],
        "stop": config["stop_conditions"],
    }


def atomic_write_text(path: Path, value: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w") as handle:
            handle.write(value)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def atomic_write_json(path: Path, value: dict[str, Any]) -> None:
    atomic_write_text(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def provenance(config: dict[str, Any], binary: Path) -> dict[str, Any]:
    config_sha256 = sha256_bytes(canonical_json(config).encode())
    captured = environment(binary)
    return {
        "checkpoint_schema_version": CHECKPOINT_SCHEMA_VERSION,
        "sample_schema_version": SCHEMA_VERSION,
        "campaign": CAMPAIGN,
        "config_sha256": config_sha256,
        "source_commit": captured["git_commit"],
        "binary_sha256": captured["binary_sha256"],
        "environment": captured,
    }


def validate_checkpoint(checkpoint: dict[str, Any], expected: dict[str, Any], planned: list[dict[str, Any]]) -> None:
    for key in ("checkpoint_schema_version", "sample_schema_version", "campaign", "config_sha256", "source_commit", "binary_sha256"):
        if checkpoint.get(key) != expected.get(key):
            raise ValueError(f"checkpoint {key} mismatch")
    if checkpoint.get("planned_points") != planned:
        raise ValueError("checkpoint plan differs from config")
    known = {row["point_identity"] for row in planned}
    observed = [row.get("point_identity") for row in checkpoint.get("point_results", [])]
    if len(observed) != len(set(observed)):
        raise ValueError("checkpoint contains duplicate point identities")
    if any(identity not in known for identity in observed):
        raise ValueError("checkpoint contains an unplanned point")


def refresh(checkpoint: dict[str, Any]) -> None:
    planned = {row["point_identity"] for row in checkpoint["planned_points"]}
    observed = {row["point_identity"] for row in checkpoint["point_results"]}
    states: dict[str, int] = {}
    for row in checkpoint["point_results"]:
        states[row.get("state", "runner-error")] = states.get(row.get("state", "runner-error"), 0) + 1
    checkpoint["completion"] = {
        "complete": planned == observed,
        "planned_point_count": len(planned),
        "completed_point_count": len(observed),
        "missing_point_count": len(planned - observed),
        "missing_point_identities": sorted(planned - observed),
        "terminal_state_counts": states,
    }
    checkpoint["updated_at_epoch_seconds"] = int(time.time())


def launch(binary: Path, config: dict[str, Any], point: dict[str, Any]) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="paper-kernel-scaling-", dir=ROOT / "results") as directory:
        directory_path = Path(directory)
        request_path = directory_path / "request.json"
        output_path = directory_path / "result.json"
        request_path.write_text(json.dumps(rust_request(config, point), sort_keys=True) + "\n")
        command = [
            relative_path(binary),
            "benchmark",
            "--suite",
            CAMPAIGN,
            "--paper-kernel-scaling-request",
            relative_path(request_path),
            "--output",
            relative_path(output_path),
        ]
        started = time.perf_counter_ns()
        try:
            completed = subprocess.run(
                command,
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
                timeout=float(config["partition_timeout_seconds"]),
            )
        except subprocess.TimeoutExpired as error:
            return {
                **point,
                "state": "runner-timeout",
                "message": str(error),
                "partition_wall_time_ns": time.perf_counter_ns() - started,
            }
        wall = time.perf_counter_ns() - started
        if completed.returncode != 0 or not output_path.exists():
            return {
                **point,
                "state": "runner-error",
                "message": completed.stderr[-4000:],
                "exit_status": completed.returncode,
                "partition_wall_time_ns": wall,
            }
        try:
            result = json.loads(output_path.read_text())
        except (OSError, json.JSONDecodeError) as error:
            return {**point, "state": "runner-error", "message": str(error), "partition_wall_time_ns": wall}
        result.update(
            {
                "point_identity": point["point_identity"],
                "partition_wall_time_ns": wall,
                "exit_status": completed.returncode,
                "stderr_tail": completed.stderr[-4000:],
            }
        )
        identities = [row.get("sample_identity") for row in result.get("runs", [])]
        if len(identities) != len(set(identities)):
            result["state"] = "invalid"
            result["message"] = "duplicate sample identities returned by Rust harness"
        return result


def csv_rows(checkpoint: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    environment_values = checkpoint["environment"]
    for point in checkpoint["point_results"]:
        for run in point.get("runs", []):
            row = {
                "campaign": CAMPAIGN,
                "config_sha256": checkpoint["config_sha256"],
                "source_commit": checkpoint["source_commit"],
                "binary_sha256": checkpoint["binary_sha256"],
                "point_identity": point["point_identity"],
                "point_state": point["state"],
                "family": point["family"],
                "target_size": point["target_size"],
                "generator_parameter": point.get("generator_parameter"),
                "canonical_instance_identity": point.get("canonical_instance_identity"),
                **run,
            }
            for prefix, values in (
                ("size_", point.get("sizes", {})),
                ("structure_", point.get("structure", {})),
                ("timing_", run.get("timings", {})),
                ("host_", environment_values),
            ):
                for key, value in values.items():
                    row[prefix + key] = value
            row.pop("timings", None)
            rows.append(row)
    return rows


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    fields = sorted({key for row in rows for key in row}) or ["campaign"]
    buffer = io.StringIO(newline="")
    writer = csv.DictWriter(buffer, fieldnames=fields, lineterminator="\n")
    writer.writeheader()
    for row in rows:
        writer.writerow({key: canonical_json(value) if isinstance(value, (dict, list)) else value for key, value in row.items()})
    atomic_write_text(path, buffer.getvalue())


def output_payload(checkpoint: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": CAMPAIGN,
        "protocol": checkpoint["protocol"],
        "config_sha256": checkpoint["config_sha256"],
        "source_commit": checkpoint["source_commit"],
        "binary_sha256": checkpoint["binary_sha256"],
        "environment": checkpoint["environment"],
        "planned_points": checkpoint["planned_points"],
        "completion": checkpoint["completion"],
        "point_results": checkpoint["point_results"],
        "runner_wall_time_ns": checkpoint.get("runner_wall_time_ns", 0),
    }


def propagated_stop(point: dict[str, Any], source: dict[str, Any]) -> dict[str, Any]:
    return {
        **point,
        "state": "stopped",
        "message": (
            f"larger level omitted after predeclared stop at target "
            f"{source['target_size']}: {source.get('message', 'unspecified stop')}"
        ),
        "stop_propagated_from_target_size": source["target_size"],
        "runs": [],
        "warmups": [],
        "correctness": [],
        "sizes": {},
        "structure": {},
        "partition_wall_time_ns": 0,
    }


def run_campaign(config: dict[str, Any], binary: Path, checkpoint_path: Path, resume: bool, families: list[str] | None, sizes: list[int] | None) -> dict[str, Any]:
    validate_config(config)
    binary = root_path(binary)
    if not binary.exists():
        raise FileNotFoundError(f"release binary not found: {relative_path(binary)}")
    expected = provenance(config, binary)
    planned = plan(config, expected["config_sha256"])
    checkpoint_path = root_path(checkpoint_path)
    if resume:
        checkpoint = json.loads(checkpoint_path.read_text())
        validate_checkpoint(checkpoint, expected, planned)
    else:
        if checkpoint_path.exists():
            raise FileExistsError("checkpoint exists; use --resume")
        checkpoint = {
            **expected,
            "protocol": config,
            "planned_points": planned,
            "point_results": [],
            "created_at_epoch_seconds": int(time.time()),
            "runner_wall_time_ns": 0,
        }
    selected_families = set(families or config["families"])
    selected_sizes = set(sizes or config["initial_size_levels"])
    if not selected_families.issubset(config["families"]) or not selected_sizes.issubset(config["initial_size_levels"]):
        raise ValueError("partition selection is outside the predeclared config")
    completed = {row["point_identity"] for row in checkpoint["point_results"]}
    family_stops = {
        row["family"]: row
        for row in checkpoint["point_results"]
        if row.get("state") == "stopped"
        and row.get("stop_propagated_from_target_size") is None
    }
    started = time.perf_counter_ns()
    for point in planned:
        if point["point_identity"] in completed or point["family"] not in selected_families or point["target_size"] not in selected_sizes:
            continue
        prior_stop = family_stops.get(point["family"])
        if prior_stop is not None and point["target_size"] > prior_stop["target_size"]:
            result = propagated_stop(point, prior_stop)
        else:
            result = launch(binary, config, point)
            if result.get("state") == "stopped":
                family_stops[point["family"]] = result
        checkpoint["point_results"].append(result)
        completed.add(point["point_identity"])
        checkpoint["runner_wall_time_ns"] += time.perf_counter_ns() - started
        started = time.perf_counter_ns()
        refresh(checkpoint)
        atomic_write_json(checkpoint_path, checkpoint)
    checkpoint["runner_wall_time_ns"] += time.perf_counter_ns() - started
    refresh(checkpoint)
    atomic_write_json(checkpoint_path, checkpoint)
    return output_payload(checkpoint)


def self_test() -> None:
    config = {
        "schema_version": 1,
        "campaign": CAMPAIGN,
        "families": ["random-connected"],
        "initial_size_levels": [1, 2],
        "algorithms": list(ALGORITHMS),
        "scopes": list(SCOPES),
        "seed": 42,
        "oracle_cell_limit": 40,
        "warmup_rule": {"minimum": 5, "maximum": 50, "cv_threshold_ppm": 50_000},
        "repetition_rule": {
            "target_measured_ns": 500_000_000,
            "fast_threshold_ns": 10_000_000,
            "medium_threshold_ns": 100_000_000,
            "fast_minimum": 31,
            "medium_minimum": 15,
            "slow_minimum": 7,
            "maximum": 10_000,
        },
        "stop_conditions": {
            "max_explicit_edges": 1_000_000,
            "max_iteration_ns": 5_000_000_000,
            "max_point_ns": 120_000_000_000,
            "host_memory_budget_bytes": 1_000_000_000,
        },
        "partition_timeout_seconds": 130,
        "fit_rule": {"minimum_valid_size_levels": 8, "bootstrap_resamples": 10_000},
    }
    validate_config(config)
    rows = plan(config, sha256_bytes(canonical_json(config).encode()))
    assert len(rows) == 2
    assert len({row["point_identity"] for row in rows}) == 2
    stopped = {**rows[0], "state": "stopped", "message": "iteration limit"}
    propagated = propagated_stop(rows[1], stopped)
    assert propagated["state"] == "stopped"
    assert propagated["stop_propagated_from_target_size"] == 1
    assert propagated["runs"] == []


def main() -> int:
    arguments = parse_args()
    if arguments.self_test:
        self_test()
        print("paper-kernel-scaling runner self-test: ok")
        return 0
    config = json.loads(root_path(arguments.config).read_text())
    validate_config(config)
    planned = plan(config, sha256_bytes(canonical_json(config).encode()))
    if arguments.print_plan:
        print(json.dumps({"planned_points": len(planned), "families": len(config["families"]), "sizes": len(config["initial_size_levels"])}, sort_keys=True))
        return 0
    payload = run_campaign(config, arguments.binary, arguments.checkpoint, arguments.resume, arguments.family, arguments.size)
    atomic_write_json(root_path(arguments.output), payload)
    write_csv(root_path(arguments.csv), csv_rows(payload))
    print(json.dumps({"completion": payload["completion"], "measured_iterations": len(csv_rows(payload)), "output": relative_path(root_path(arguments.output))}, sort_keys=True))
    return 0 if all(point.get("state") in {"complete", "stopped"} for point in payload["point_results"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
