#!/usr/bin/env python3
"""Run the reproducible paper-scaling campaign in fresh release processes.

The runner owns process-level timing, deterministic counterbalancing, timeout
censoring, paired correctness checks, and raw-result persistence. It does not
fit models or discard observations; use ``analyze_paper_scaling.py`` for that.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import platform
import random
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = 1
ALGORITHMS = (
    "compact-mrd",
    "explicit-hopcroft-karp",
    "explicit-c0-flow",
    "exact-cover-oracle",
)
FAMILIES = (
    "random-connected",
    "dense-conflict",
    "sparse-conflict",
    "comb-staircase",
    "supported-holes",
    "polyomino",
    "representation-crossover",
)
TIMING_FIELDS = (
    "input_loading_ns",
    "instance_generation_ns",
    "geometry_preprocessing_ns",
    "chord_generation_ns",
    "embedding_ns",
    "explicit_conflict_graph_ns",
    "biclique_construction_ns",
    "network_construction_ns",
    "matching_or_flow_ns",
    "vertex_cover_recovery_ns",
    "chord_selection_ns",
    "geometric_completion_ns",
    "rectangle_recovery_ns",
    "verification_ns",
    "total_in_process_solve_ns",
)
SIZE_FIELDS = (
    "width",
    "height",
    "foreground_cells_n",
    "component_count",
    "boundary_size_b",
    "reflex_count",
    "horizontal_chord_count_h",
    "vertical_chord_count_v",
    "q",
    "explicit_conflict_edge_count_k",
    "biclique_count",
    "biclique_total_vertex_occurrences_sigma",
    "compressed_network_node_count",
    "compressed_network_arc_count",
    "optimum_rectangle_count",
)
STRUCTURE_FIELDS = (
    "rank_sort_count",
    "rank_map_entry_count",
    "rank_map_owned_bytes",
    "matching_size",
    "vertex_cover_size",
    "c0_network_node_count",
    "c0_network_arc_count",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, required=False)
    parser.add_argument("--binary", type=Path, default=Path("target/release/mrd"))
    parser.add_argument("--output", type=Path, default=Path("results/paper-scaling.json"))
    parser.add_argument(
        "--csv", type=Path, default=Path("results/paper-scaling-runs.csv")
    )
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def root_path(path: Path) -> Path:
    return path if path.is_absolute() else ROOT / path


def relative_path(path: Path) -> str:
    resolved = path.resolve()
    return resolved.relative_to(ROOT).as_posix()


def command_output(command: list[str]) -> str:
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def command_or_unknown(command: list[str]) -> str:
    try:
        return command_output(command)
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def binary_hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def environment(binary: Path) -> dict[str, Any]:
    return {
        "operating_system": platform.platform(),
        "architecture": platform.machine() or "unknown",
        "cpu_model": command_or_unknown(
            ["sysctl", "-n", "machdep.cpu.brand_string"]
        )
        if sys.platform == "darwin"
        else platform.processor() or "unknown",
        "logical_cpu_count": os.cpu_count() or 0,
        "python_version": platform.python_version(),
        "rustc_version": command_or_unknown(["rustc", "--version"]),
        "binary_sha256": binary_hash(binary),
        "git_commit": command_or_unknown(["git", "rev-parse", "HEAD"]),
        "git_dirty": bool(command_or_unknown(["git", "status", "--porcelain"])),
        "maximum_rss_bytes": None,
        "maximum_rss_method": "unavailable-per-child-portable-probe",
    }


def splitmix64(value: int) -> int:
    value = (value + 0x9E3779B97F4A7C15) & ((1 << 64) - 1)
    value = ((value ^ (value >> 30)) * 0xBF58476D1CE4E5B9) & ((1 << 64) - 1)
    value = ((value ^ (value >> 27)) * 0x94D049BB133111EB) & ((1 << 64) - 1)
    return value ^ (value >> 31)


def execution_order(algorithms: list[str], seed: int, pair_key: str) -> list[str]:
    """Return a deterministic Fisher-Yates permutation for one pair."""
    state = splitmix64(seed ^ int.from_bytes(pair_key.encode(), "little", signed=False))
    order = list(algorithms)
    for index in range(len(order) - 1, 0, -1):
        state = splitmix64(state)
        swap = state % (index + 1)
        order[index], order[swap] = order[swap], order[index]
    return order


def validate_config(config: dict[str, Any]) -> None:
    if config.get("schema_version") != SCHEMA_VERSION:
        raise ValueError("paper-scaling config schema_version must be 1")
    families = config.get("families")
    algorithms = config.get("algorithms", list(ALGORITHMS))
    sizes = config.get("sizes")
    if not families or any(family not in FAMILIES for family in families):
        raise ValueError(f"families must be nonempty members of {FAMILIES}")
    if not algorithms or any(algorithm not in ALGORITHMS for algorithm in algorithms):
        raise ValueError(f"algorithms must be nonempty members of {ALGORITHMS}")
    if not sizes or any(not isinstance(size, int) or size <= 0 for size in sizes):
        raise ValueError("sizes must be positive integers")
    warmups = int(config.get("warmups", 0))
    repetitions = int(config.get("repetitions", 0))
    if warmups < 1 or repetitions < 3:
        raise ValueError("campaign requires at least one warm-up and three measured repetitions")
    small_repetitions = int(config.get("small_medium_repetitions", repetitions))
    if small_repetitions < repetitions:
        raise ValueError("small_medium_repetitions cannot be below repetitions")
    if float(config.get("timeout_seconds", 0)) <= 0:
        raise ValueError("timeout_seconds must be positive")
    fit = config.get("fit", {})
    if int(fit.get("bootstrap_resamples", 0)) < 10_000:
        raise ValueError("fit.bootstrap_resamples must be at least 10000")
    if int(fit.get("minimum_target_size", 0)) <= 0:
        raise ValueError("fit.minimum_target_size must be predeclared and positive")
    if int(fit.get("minimum_size_levels", 0)) < 6:
        raise ValueError("fit.minimum_size_levels must be at least 6")


def request_for(
    family: str, size: int, seed: int, algorithm: str, oracle_limit: int
) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "family": family,
        "target_size": size,
        "seed": seed,
        "algorithm": algorithm,
        "oracle_cell_limit": oracle_limit,
    }


def scalar(value: Any) -> Any:
    if isinstance(value, (dict, list)):
        return json.dumps(value, sort_keys=True, separators=(",", ":"))
    return value


def launch_sample(
    binary: Path,
    request: dict[str, Any],
    pair_id: str,
    repetition: int,
    warmup: bool,
    execution_rank: int,
    timeout_seconds: float,
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="paper-scaling-", dir=ROOT / "results") as directory:
        directory_path = Path(directory)
        request_path = directory_path / "request.json"
        output_path = directory_path / "sample.json"
        request_path.write_text(json.dumps(request, sort_keys=True) + "\n")
        command = [
            relative_path(binary),
            "benchmark",
            "--suite",
            "paper-scaling",
            "--paper-scaling-request",
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
                timeout=timeout_seconds,
            )
            process_wall_time_ns = time.perf_counter_ns() - started
        except subprocess.TimeoutExpired as error:
            process_wall_time_ns = time.perf_counter_ns() - started
            return base_record(
                request,
                pair_id,
                repetition,
                warmup,
                execution_rank,
                timeout_seconds,
                "timeout",
                None,
                process_wall_time_ns,
                f"timeout after {timeout_seconds:g}s: {error}",
            )
        if not output_path.exists():
            return base_record(
                request,
                pair_id,
                repetition,
                warmup,
                execution_rank,
                timeout_seconds,
                "error",
                completed.returncode,
                process_wall_time_ns,
                "sample output missing; stderr=" + completed.stderr[-2_000:],
            )
        try:
            sample = json.loads(output_path.read_text())
        except (OSError, json.JSONDecodeError) as error:
            return base_record(
                request,
                pair_id,
                repetition,
                warmup,
                execution_rank,
                timeout_seconds,
                "error",
                completed.returncode,
                process_wall_time_ns,
                f"invalid sample JSON: {error}",
            )
        outcome = sample.get("outcome", "error")
        state = "success" if outcome == "success" else outcome
        total = sample.get("timings", {}).get("total_in_process_solve_ns")
        record = base_record(
            request,
            pair_id,
            repetition,
            warmup,
            execution_rank,
            timeout_seconds,
            state,
            completed.returncode,
            process_wall_time_ns,
            sample.get("message"),
        )
        record.update(sample)
        record["state"] = state
        record["exit_status"] = completed.returncode
        record["process_wall_time_ns"] = process_wall_time_ns
        record["process_startup_overhead_ns"] = (
            max(0, process_wall_time_ns - int(total)) if total is not None else None
        )
        record["timeout_seconds"] = timeout_seconds
        record["warmup"] = warmup
        record["measured"] = not warmup
        record["repetition"] = repetition
        record["execution_order"] = execution_rank
        record["pair_id"] = pair_id
        record["request"] = request
        record["stderr_tail"] = completed.stderr[-2_000:]
        return record


def base_record(
    request: dict[str, Any],
    pair_id: str,
    repetition: int,
    warmup: bool,
    execution_rank: int,
    timeout_seconds: float,
    state: str,
    exit_status: int | None,
    process_wall_time_ns: int,
    message: str | None,
) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": "paper-scaling",
        "family": request["family"],
        "algorithm": request["algorithm"],
        "seed": request["seed"],
        "target_size": request["target_size"],
        "pair_id": pair_id,
        "repetition": repetition,
        "warmup": warmup,
        "measured": not warmup,
        "execution_order": execution_rank,
        "timeout_seconds": timeout_seconds,
        "state": state,
        "exit_status": exit_status,
        "process_wall_time_ns": process_wall_time_ns,
        "process_startup_overhead_ns": None,
        "correctness": "not-run",
        "message": message,
        "stderr_tail": None,
    }


def paired_structural(records: list[dict[str, Any]]) -> None:
    """Attach explicit structural measurements without timing another solver."""
    groups: dict[str, list[dict[str, Any]]] = {}
    for record in records:
        if record.get("measured"):
            groups.setdefault(record["pair_id"], []).append(record)
    for rows in groups.values():
        source = next(
            (
                row
                for row in rows
                if row.get("state") == "success"
                and row.get("algorithm") == "explicit-hopcroft-karp"
            ),
            None,
        )
        compact = next(
            (row for row in rows if row.get("algorithm") == "compact-mrd"), None
        )
        if source is None:
            continue
        sizes = source.get("sizes", {})
        compact_sizes = compact.get("sizes", {}) if compact else {}
        shared = {
            "q": sizes.get("q"),
            "explicit_conflict_edge_count_k": sizes.get("explicit_conflict_edge_count_k"),
            "boundary_size_b": sizes.get("boundary_size_b"),
            "reflex_count": sizes.get("reflex_count"),
            "biclique_count": compact_sizes.get("biclique_count"),
            "biclique_total_vertex_occurrences_sigma": compact_sizes.get(
                "biclique_total_vertex_occurrences_sigma"
            ),
            "compressed_network_node_count": compact_sizes.get(
                "compressed_network_node_count"
            ),
            "compressed_network_arc_count": compact_sizes.get(
                "compressed_network_arc_count"
            ),
        }
        for row in rows:
            row["paired_structural"] = shared


def validate_pairs(records: list[dict[str, Any]]) -> list[str]:
    """Mark mismatches while retaining every original row."""
    errors: list[str] = []
    by_pair: dict[str, list[dict[str, Any]]] = {}
    for row in records:
        if row.get("measured"):
            by_pair.setdefault(row["pair_id"], []).append(row)
    for pair_id, rows in by_pair.items():
        valid = [
            row
            for row in rows
            if row.get("state") == "success" and row.get("correctness") == "valid"
        ]
        objectives = {row.get("optimum_rectangle_count") for row in valid}
        if len(objectives) > 1:
            message = f"paired optimum mismatch in {pair_id}: {sorted(objectives)}"
            errors.append(message)
            for row in valid:
                row["correctness"] = "invalid-cross-algorithm-mismatch"
                row["invalid_reason"] = message
    deterministic: dict[tuple[str, str, int], str] = {}
    for row in records:
        if row.get("state") != "success" or not row.get("measured"):
            continue
        rectangles = json.dumps(row.get("canonical_rectangles"), sort_keys=True)
        key = (row["family"], row["algorithm"], row["target_size"])
        previous = deterministic.get(key)
        if previous is not None and previous != rectangles:
            message = f"nondeterministic mathematical result in {key}"
            row["correctness"] = "invalid-nondeterministic-result"
            row["invalid_reason"] = message
            errors.append(message)
        deterministic[key] = rectangles
    return errors


def csv_fields() -> list[str]:
    fields = [
        "schema_version",
        "campaign",
        "family",
        "algorithm",
        "solver_provenance",
        "seed",
        "target_size",
        "generation_attempts",
        "pair_id",
        "repetition",
        "warmup",
        "measured",
        "execution_order",
        "timeout_seconds",
        "state",
        "outcome",
        "exit_status",
        "correctness",
        "message",
        "invalid_reason",
        "process_wall_time_ns",
        "process_startup_overhead_ns",
    ]
    fields.extend(f"size_{field}" for field in SIZE_FIELDS)
    fields.extend(f"structure_{field}" for field in STRUCTURE_FIELDS)
    fields.extend(f"timing_{field}" for field in TIMING_FIELDS)
    fields.extend(f"paired_{field}" for field in SIZE_FIELDS)
    return fields


def csv_row(record: dict[str, Any]) -> dict[str, Any]:
    row = {field: record.get(field) for field in csv_fields()}
    for prefix, key, fields in (
        ("size_", "sizes", SIZE_FIELDS),
        ("structure_", "structure", STRUCTURE_FIELDS),
        ("timing_", "timings", TIMING_FIELDS),
        ("paired_", "paired_structural", SIZE_FIELDS),
    ):
        values = record.get(key, {}) or {}
        for field in fields:
            row[prefix + field] = scalar(values.get(field))
    return {key: scalar(value) for key, value in row.items()}


def write_csv(path: Path, records: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=csv_fields(), lineterminator="\n")
        writer.writeheader()
        writer.writerows(csv_row(record) for record in records)


def run_campaign(config: dict[str, Any], binary: Path) -> tuple[dict[str, Any], list[str]]:
    validate_config(config)
    binary = root_path(binary)
    if not binary.exists():
        raise FileNotFoundError(f"release binary does not exist: {relative_path(binary)}")
    seed = int(config.get("seed", 42))
    oracle_limit = int(config.get("oracle_cell_limit", 40))
    algorithms = list(config.get("algorithms", ALGORITHMS))
    small_medium_max = config.get("small_medium_max_target_size")
    small_medium_repetitions = int(
        config.get("small_medium_repetitions", config["repetitions"])
    )
    records: list[dict[str, Any]] = []
    for family in config["families"]:
        for size in config["sizes"]:
            measured_repetitions = (
                small_medium_repetitions
                if small_medium_max is not None and size <= int(small_medium_max)
                else int(config["repetitions"])
            )
            for warmup, count in ((True, config["warmups"]), (False, measured_repetitions)):
                for repetition in range(count):
                    pair_id = f"{family}:{seed}:{size}:{repetition}"
                    order = execution_order(algorithms, seed, pair_id)
                    for execution_rank, algorithm in enumerate(order):
                        request = request_for(family, size, seed, algorithm, oracle_limit)
                        records.append(
                            launch_sample(
                                binary,
                                request,
                                pair_id,
                                repetition,
                                warmup,
                                execution_rank,
                                float(config["timeout_seconds"]),
                            )
                        )
    paired_structural(records)
    errors = validate_pairs(records)
    payload = {
        "schema_version": SCHEMA_VERSION,
        "campaign": "paper-scaling",
        "protocol": config,
        "environment": environment(binary),
        "records": records,
        "paired_validation_errors": errors,
        "generated_at_epoch_seconds": int(time.time()),
    }
    return payload, errors


def main() -> int:
    arguments = parse_args()
    if arguments.self_test:
        self_test()
        print("paper-scaling runner self-test: ok")
        return 0
    config_path = root_path(arguments.config or Path("results/paper-scaling-smoke-config.json"))
    config = json.loads(config_path.read_text())
    payload, errors = run_campaign(config, arguments.binary)
    output = root_path(arguments.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    write_csv(root_path(arguments.csv), payload["records"])
    print(
        json.dumps(
            {
                "records": len(payload["records"]),
                "paired_validation_errors": len(errors),
                "output": relative_path(output),
                "csv": relative_path(root_path(arguments.csv)),
            },
            sort_keys=True,
        )
    )
    return 1 if errors else 0


def self_test() -> None:
    algorithms = ["a", "b", "c"]
    first = execution_order(algorithms, 42, "pair")
    assert first == execution_order(algorithms, 42, "pair")
    assert sorted(first) == algorithms
    rows = [
        {
            "pair_id": "p",
            "family": "f",
            "target_size": 1,
            "measured": True,
            "state": "timeout",
            "algorithm": "compact-mrd",
            "correctness": "not-run",
        },
        {
            "pair_id": "p",
            "family": "f",
            "target_size": 1,
            "measured": True,
            "state": "success",
            "algorithm": "explicit-hopcroft-karp",
            "correctness": "valid",
            "optimum_rectangle_count": 2,
        },
    ]
    assert validate_pairs(rows) == []
    assert rows[0]["state"] == "timeout"
    rows.append(
        {
            "pair_id": "p",
            "family": "f",
            "target_size": 1,
            "measured": True,
            "state": "success",
            "algorithm": "compact-mrd",
            "correctness": "valid",
            "optimum_rectangle_count": 3,
        }
    )
    assert validate_pairs(rows)
    assert rows[0]["state"] == "timeout"
    try:
        validate_config(
            {
                "schema_version": 1,
                "families": ["random-connected"],
                "algorithms": ["compact-mrd"],
                "sizes": [1],
                "warmups": 1,
                "repetitions": 3,
                "timeout_seconds": 1,
                "fit": {"minimum_target_size": 1, "minimum_size_levels": 5, "bootstrap_resamples": 10_000},
            }
        )
    except ValueError:
        pass
    else:
        raise AssertionError("fit minimum-size gate was not enforced")


if __name__ == "__main__":
    raise SystemExit(main())
