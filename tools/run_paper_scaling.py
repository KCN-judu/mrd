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
import io
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
CHECKPOINT_SCHEMA_VERSION = 1
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
    parser.add_argument(
        "--checkpoint",
        type=Path,
        help="atomically updated raw checkpoint for interruption-safe resume",
    )
    parser.add_argument(
        "--resume",
        action="store_true",
        help="resume only identities absent from --checkpoint",
    )
    parser.add_argument(
        "--family",
        action="append",
        choices=FAMILIES,
        help="run one or more predeclared families while retaining the global plan",
    )
    parser.add_argument(
        "--size",
        type=int,
        action="append",
        help="run one or more predeclared target sizes while retaining the global plan",
    )
    parser.add_argument(
        "--print-plan",
        action="store_true",
        help="print predeclared process counts without launching samples",
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


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def config_hash(config: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_json(config).encode()).hexdigest()


def atomic_write_text(path: Path, text: str) -> None:
    """Replace a file only after its complete contents reach a sibling temp file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w") as handle:
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def atomic_write_json(path: Path, value: dict[str, Any]) -> None:
    atomic_write_text(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


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
    campaign = config.get("campaign", "paper-scaling")
    if not isinstance(campaign, str) or not campaign:
        raise ValueError("campaign must be a nonempty string")


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


def planned_identity(
    config_sha256: str,
    campaign: str,
    family: str,
    size: int,
    algorithm: str,
    warmup: bool,
    repetition: int,
    pair_id: str,
    execution_rank: int,
) -> dict[str, Any]:
    return {
        "config_sha256": config_sha256,
        "campaign": campaign,
        "family": family,
        "target_size": size,
        "algorithm": algorithm,
        "warmup": warmup,
        "repetition": repetition,
        "pair_id": pair_id,
        "execution_order": execution_rank,
    }


def identity_key(identity: dict[str, Any]) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(identity).encode()).hexdigest()


def planned_samples(config: dict[str, Any], config_sha256: str) -> list[dict[str, Any]]:
    """Expand the immutable protocol into independently resumable process rows."""
    validate_config(config)
    campaign = str(config.get("campaign", "paper-scaling"))
    seed = int(config.get("seed", 42))
    oracle_limit = int(config.get("oracle_cell_limit", 40))
    algorithms = list(config.get("algorithms", ALGORITHMS))
    small_medium_max = config.get("small_medium_max_target_size")
    small_medium_repetitions = int(
        config.get("small_medium_repetitions", config["repetitions"])
    )
    plan: list[dict[str, Any]] = []
    for family in config["families"]:
        for size in config["sizes"]:
            measured_repetitions = (
                small_medium_repetitions
                if small_medium_max is not None and size <= int(small_medium_max)
                else int(config["repetitions"])
            )
            for warmup, count in ((True, int(config["warmups"])), (False, measured_repetitions)):
                for repetition in range(count):
                    pair_id = f"{family}:{seed}:{size}:{repetition}"
                    for execution_rank, algorithm in enumerate(
                        execution_order(algorithms, seed, pair_id)
                    ):
                        identity = planned_identity(
                            config_sha256,
                            campaign,
                            family,
                            size,
                            algorithm,
                            warmup,
                            repetition,
                            pair_id,
                            execution_rank,
                        )
                        plan.append(
                            {
                                "sample_identity": identity_key(identity),
                                "identity": identity,
                                "request": request_for(
                                    family, size, seed, algorithm, oracle_limit
                                ),
                            }
                        )
    validate_planned_samples(plan)
    return plan


def validate_planned_samples(plan: list[dict[str, Any]]) -> None:
    identities = [sample.get("sample_identity") for sample in plan]
    if any(not isinstance(identity, str) for identity in identities):
        raise ValueError("planned samples must have stable identities")
    if len(set(identities)) != len(identities):
        raise ValueError("planned sample identities are not unique")
    for sample in plan:
        identity = sample["identity"]
        if identity_key(identity) != sample["sample_identity"]:
            raise ValueError("planned sample identity hash does not match its fields")


def workload_counts(plan: list[dict[str, Any]], timeout_seconds: float) -> dict[str, Any]:
    warmups = sum(sample["identity"]["warmup"] for sample in plan)
    measured = len(plan) - warmups
    return {
        "family_count": len({sample["identity"]["family"] for sample in plan}),
        "size_count": len({sample["identity"]["target_size"] for sample in plan}),
        "algorithm_count": len({sample["identity"]["algorithm"] for sample in plan}),
        "combination_count": len(
            {
                (
                    sample["identity"]["family"],
                    sample["identity"]["target_size"],
                    sample["identity"]["algorithm"],
                )
                for sample in plan
            }
        ),
        "warmup_process_count": warmups,
        "measured_process_count": measured,
        "total_process_count": len(plan),
        "timeout_upper_bound_seconds": len(plan) * timeout_seconds,
    }


def select_planned_samples(
    plan: list[dict[str, Any]], families: list[str] | None, sizes: list[int] | None
) -> list[dict[str, Any]]:
    available_families = {sample["identity"]["family"] for sample in plan}
    available_sizes = {sample["identity"]["target_size"] for sample in plan}
    requested_families = set(families or available_families)
    requested_sizes = set(sizes or available_sizes)
    unknown_families = requested_families - available_families
    unknown_sizes = requested_sizes - available_sizes
    if unknown_families:
        raise ValueError(f"families are not in the predeclared config: {sorted(unknown_families)}")
    if unknown_sizes:
        raise ValueError(f"sizes are not in the predeclared config: {sorted(unknown_sizes)}")
    selected = [
        sample
        for sample in plan
        if sample["identity"]["family"] in requested_families
        and sample["identity"]["target_size"] in requested_sizes
    ]
    if not selected:
        raise ValueError("partition selection produced no planned samples")
    return selected


def checkpoint_provenance(
    config: dict[str, Any], binary: Path, config_sha256: str
) -> dict[str, Any]:
    captured = environment(binary)
    return {
        "checkpoint_schema_version": CHECKPOINT_SCHEMA_VERSION,
        "sample_schema_version": SCHEMA_VERSION,
        "campaign": str(config.get("campaign", "paper-scaling")),
        "config_sha256": config_sha256,
        "source_commit": captured["git_commit"],
        "binary_sha256": captured["binary_sha256"],
        "environment": captured,
    }


def new_checkpoint(
    config: dict[str, Any], binary: Path, plan: list[dict[str, Any]], config_sha256: str
) -> dict[str, Any]:
    provenance = checkpoint_provenance(config, binary, config_sha256)
    checkpoint = {
        **provenance,
        "protocol": config,
        "planned_samples": plan,
        "records": [],
        "paired_validation_errors": [],
        "completed_runner_wall_time_ns": 0,
        "runner_invocations": [],
        "created_at_epoch_seconds": int(time.time()),
        "updated_at_epoch_seconds": int(time.time()),
    }
    refresh_checkpoint(checkpoint)
    return checkpoint


def terminal_record(record: dict[str, Any]) -> bool:
    return record.get("state") in {"success", "unsupported", "timeout", "error"}


def record_matches_plan(record: dict[str, Any], sample: dict[str, Any]) -> bool:
    identity = sample["identity"]
    return all(
        record.get(key) == value
        for key, value in identity.items()
        if key != "config_sha256" and key != "campaign"
    )


def validate_checkpoint(
    checkpoint: dict[str, Any], expected: dict[str, Any]
) -> None:
    for key in (
        "checkpoint_schema_version",
        "sample_schema_version",
        "config_sha256",
        "source_commit",
        "binary_sha256",
    ):
        if checkpoint.get(key) != expected.get(key):
            raise ValueError(
                f"checkpoint {key} mismatch: expected {expected.get(key)!r}, "
                f"found {checkpoint.get(key)!r}"
            )
    plan = checkpoint.get("planned_samples")
    records = checkpoint.get("records")
    if not isinstance(plan, list) or not isinstance(records, list):
        raise ValueError("checkpoint must contain planned_samples and records arrays")
    validate_planned_samples(plan)
    planned = {sample["sample_identity"]: sample for sample in plan}
    seen: set[str] = set()
    for record in records:
        identity = record.get("sample_identity")
        if not isinstance(identity, str) or identity not in planned:
            raise ValueError("checkpoint record has an unknown sample identity")
        if identity in seen:
            raise ValueError(f"duplicate completed sample identity: {identity}")
        if not terminal_record(record):
            raise ValueError(f"checkpoint record is not terminal: {identity}")
        if not record_matches_plan(record, planned[identity]):
            raise ValueError(f"checkpoint record fields do not match its plan: {identity}")
        seen.add(identity)


def completion_state(checkpoint: dict[str, Any]) -> dict[str, Any]:
    plan = checkpoint["planned_samples"]
    records = checkpoint["records"]
    planned_ids = {sample["sample_identity"] for sample in plan}
    observed_ids = {record["sample_identity"] for record in records}
    missing = sorted(planned_ids - observed_ids)
    state_counts = {
        state: sum(record.get("state") == state for record in records)
        for state in ("success", "unsupported", "timeout", "error")
    }
    return {
        "complete": not missing,
        "planned_record_count": len(plan),
        "completed_record_count": len(records),
        "missing_record_count": len(missing),
        "missing_sample_identities": missing,
        "terminal_state_counts": state_counts,
    }


def refresh_checkpoint(checkpoint: dict[str, Any]) -> None:
    validate_planned_samples(checkpoint["planned_samples"])
    paired_structural(checkpoint["records"])
    checkpoint["paired_validation_errors"] = sorted(
        set(validate_pairs(checkpoint["records"]))
    )
    checkpoint["completion"] = completion_state(checkpoint)
    checkpoint["updated_at_epoch_seconds"] = int(time.time())


def begin_runner_invocation(checkpoint: dict[str, Any], resume: bool) -> int:
    """Start a wall-clock segment, preserving the last durable interrupted segment."""
    previous = checkpoint.pop("active_runner_invocation", None)
    if previous is not None:
        elapsed = int(previous.get("last_elapsed_ns", 0))
        checkpoint["completed_runner_wall_time_ns"] = int(
            checkpoint.get("completed_runner_wall_time_ns", 0)
        ) + elapsed
        checkpoint.setdefault("runner_invocations", []).append(
            {**previous, "elapsed_ns": elapsed, "interrupted": True}
        )
    checkpoint["active_runner_invocation"] = {
        "started_at_epoch_ns": time.time_ns(),
        "last_elapsed_ns": 0,
        "resume": resume,
    }
    return time.perf_counter_ns()


def refresh_runner_elapsed(checkpoint: dict[str, Any], started_ns: int) -> None:
    active = checkpoint["active_runner_invocation"]
    active["last_elapsed_ns"] = time.perf_counter_ns() - started_ns
    checkpoint["runner_wall_time_ns"] = int(
        checkpoint.get("completed_runner_wall_time_ns", 0)
    ) + int(active["last_elapsed_ns"])


def finish_runner_invocation(checkpoint: dict[str, Any], started_ns: int) -> None:
    refresh_runner_elapsed(checkpoint, started_ns)
    active = checkpoint.pop("active_runner_invocation")
    elapsed = int(active["last_elapsed_ns"])
    checkpoint["completed_runner_wall_time_ns"] = int(
        checkpoint.get("completed_runner_wall_time_ns", 0)
    ) + elapsed
    checkpoint.setdefault("runner_invocations", []).append(
        {**active, "elapsed_ns": elapsed, "interrupted": False}
    )
    checkpoint["runner_wall_time_ns"] = checkpoint["completed_runner_wall_time_ns"]


def load_checkpoint(path: Path, expected: dict[str, Any]) -> dict[str, Any]:
    checkpoint = json.loads(path.read_text())
    validate_checkpoint(checkpoint, expected)
    refresh_checkpoint(checkpoint)
    return checkpoint


def append_record(
    checkpoint: dict[str, Any], sample: dict[str, Any], record: dict[str, Any]
) -> None:
    identity = sample["sample_identity"]
    if any(existing.get("sample_identity") == identity for existing in checkpoint["records"]):
        raise ValueError(f"refusing to overwrite completed sample identity: {identity}")
    record["sample_identity"] = identity
    record["sample_identity_fields"] = sample["identity"]
    record["config_sha256"] = checkpoint["config_sha256"]
    record["campaign"] = checkpoint["campaign"]
    if not terminal_record(record):
        raise ValueError(f"runner produced a nonterminal record: {identity}")
    if not record_matches_plan(record, sample):
        raise ValueError(f"runner record does not match the requested plan: {identity}")
    checkpoint["records"].append(record)
    refresh_checkpoint(checkpoint)


def output_payload(checkpoint: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": checkpoint["campaign"],
        "protocol": checkpoint["protocol"],
        "environment": checkpoint["environment"],
        "config_sha256": checkpoint["config_sha256"],
        "source_commit": checkpoint["source_commit"],
        "binary_sha256": checkpoint["binary_sha256"],
        "runner_wall_time_ns": checkpoint.get("runner_wall_time_ns"),
        "runner_invocations": checkpoint.get("runner_invocations", []),
        "planned_samples": checkpoint["planned_samples"],
        "completion": checkpoint["completion"],
        "records": checkpoint["records"],
        "paired_validation_errors": checkpoint["paired_validation_errors"],
        "generated_at_epoch_seconds": int(time.time()),
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
    buffer = io.StringIO(newline="")
    writer = csv.DictWriter(buffer, fieldnames=csv_fields(), lineterminator="\n")
    writer.writeheader()
    writer.writerows(csv_row(record) for record in records)
    atomic_write_text(path, buffer.getvalue())


def run_campaign(
    config: dict[str, Any],
    binary: Path,
    checkpoint_path: Path | None = None,
    resume: bool = False,
    families: list[str] | None = None,
    sizes: list[int] | None = None,
    launch=launch_sample,
) -> tuple[dict[str, Any], list[str]]:
    """Run missing rows of a fixed campaign plan and persist every terminal row."""
    validate_config(config)
    binary = root_path(binary)
    if not binary.exists():
        raise FileNotFoundError(f"release binary does not exist: {relative_path(binary)}")
    checksum = config_hash(config)
    plan = planned_samples(config, checksum)
    expected = checkpoint_provenance(config, binary, checksum)
    resolved_checkpoint = root_path(checkpoint_path) if checkpoint_path else None
    if resume:
        if resolved_checkpoint is None:
            raise ValueError("--resume requires --checkpoint")
        if not resolved_checkpoint.exists():
            raise FileNotFoundError(
                f"cannot resume because checkpoint does not exist: {relative_path(resolved_checkpoint)}"
            )
        checkpoint = load_checkpoint(resolved_checkpoint, expected)
        if checkpoint["planned_samples"] != plan:
            raise ValueError("checkpoint plan does not match the predeclared config")
    else:
        if resolved_checkpoint is not None and resolved_checkpoint.exists():
            raise FileExistsError(
                f"checkpoint already exists; use --resume: {relative_path(resolved_checkpoint)}"
            )
        checkpoint = new_checkpoint(config, binary, plan, checksum)

    invocation_started = begin_runner_invocation(checkpoint, resume)

    def persist_checkpoint() -> None:
        refresh_runner_elapsed(checkpoint, invocation_started)
        if resolved_checkpoint is not None:
            atomic_write_json(resolved_checkpoint, checkpoint)

    persist_checkpoint()

    selected = select_planned_samples(plan, families, sizes)
    completed = {record["sample_identity"] for record in checkpoint["records"]}
    for sample in selected:
        if sample["sample_identity"] in completed:
            continue
        identity = sample["identity"]
        record = launch(
            binary,
            sample["request"],
            str(identity["pair_id"]),
            int(identity["repetition"]),
            bool(identity["warmup"]),
            int(identity["execution_order"]),
            float(config["timeout_seconds"]),
        )
        append_record(checkpoint, sample, record)
        completed.add(sample["sample_identity"])
        persist_checkpoint()

    refresh_checkpoint(checkpoint)
    finish_runner_invocation(checkpoint, invocation_started)
    if resolved_checkpoint is not None:
        atomic_write_json(resolved_checkpoint, checkpoint)
    payload = output_payload(checkpoint)
    return payload, list(checkpoint["paired_validation_errors"])


def main() -> int:
    arguments = parse_args()
    if arguments.self_test:
        self_test()
        print("paper-scaling runner self-test: ok")
        return 0
    config_path = root_path(arguments.config or Path("results/paper-scaling-smoke-config.json"))
    config = json.loads(config_path.read_text())
    binary = root_path(arguments.binary)
    validate_config(config)
    checksum = config_hash(config)
    plan = planned_samples(config, checksum)
    if arguments.print_plan:
        print(json.dumps(workload_counts(plan, float(config["timeout_seconds"])), sort_keys=True))
        return 0
    if arguments.resume and arguments.checkpoint is None:
        raise ValueError("--resume requires --checkpoint")
    payload, errors = run_campaign(
        config,
        binary,
        arguments.checkpoint,
        arguments.resume,
        arguments.family,
        arguments.size,
    )
    output = root_path(arguments.output)
    atomic_write_json(output, payload)
    write_csv(root_path(arguments.csv), payload["records"])
    print(
        json.dumps(
            {
                "records": len(payload["records"]),
                "complete": payload["completion"]["complete"],
                "missing_records": payload["completion"]["missing_record_count"],
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
