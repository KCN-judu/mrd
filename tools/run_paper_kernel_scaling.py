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
SCHEMA_VERSION = 2
CHECKPOINT_SCHEMA_VERSION = 2
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
BOUNDARY_DISCOVERY_BACKENDS = (
    "reference-edge-toggle",
    "prepared-exposed-edges",
)
CANONICAL_BACKENDS = (
    "clone-canonical-reference",
    "borrowed-canonical",
)
RUNNER_FAILURE_STATES = frozenset(("runner-error", "runner-timeout"))
RESUME_ENVIRONMENT_FIELDS = (
    "clock_source",
    "cpu_model",
    "operating_system",
    "architecture",
    "logical_cpu_count",
    "rustc_version",
    "compiler_profile",
    "git_commit",
    "git_dirty",
    "binary_sha256",
    "power_source",
    "turbo_or_power_mode",
)
KERNEL_LEAF_PHASES = (
    "embedding_ns",
    "conflict_discovery_ns",
    "representation_construction_ns",
    "explicit_network_construction_ns",
    "compressed_network_construction_ns",
    "matching_or_flow_ns",
    "minimum_vertex_cover_recovery_ns",
)
SCOPE_B_PARENT_PHASES = KERNEL_LEAF_PHASES
SCOPE_A_PARENT_PHASES = (
    "canonical_component_clone_ns",
    "canonical_context_borrow_or_share_ns",
    "canonical_component_release_ns",
    "geometry_preprocessing_ns",
    "chord_generation_ns",
    "solver_workspace_prepare_ns",
    *KERNEL_LEAF_PHASES,
    "chord_selection_ns",
    "rectangle_completion_recovery_ns",
    "output_validation_ns",
)
NESTED_TIMING_GROUPS = (
    (
        "boundary_total_build_ns",
        (
            "boundary_edge_discovery_ns",
            "boundary_adjacency_build_ns",
            "boundary_loop_tracing_ns",
            "boundary_loop_normalization_ns",
            "reflex_detection_ns",
            "boundary_unit_edge_sort_ns",
        ),
        "boundary",
    ),
    (
        "geometry_preprocessing_ns",
        (
            "prepared_component_build_ns",
            "boundary_total_build_ns",
            "boundary_index_construction_ns",
            "reflex_grouping_ns",
        ),
        "geometry",
    ),
    (
        "chord_generation_ns",
        (
            "horizontal_chord_generation_ns",
            "vertical_chord_generation_ns",
            "chord_validation_filtering_ns",
            "endpoint_index_construction_ns",
        ),
        "chord",
    ),
    (
        "rectangle_completion_recovery_ns",
        (
            "selected_cut_materialization_ns",
            "horizontal_completion_ns",
            "vertical_completion_ns",
            "rectangle_reconstruction_ns",
            "completion_finalization_ns",
        ),
        "completion",
    ),
    (
        "output_validation_ns",
        ("internal_output_validation_ns", "final_output_validation_ns"),
        "output_validation",
    ),
)
TIMING_FIELDS = tuple(
    sorted(
        {
            "geometry_preprocessing_ns",
            "chord_generation_ns",
            "embedding_ns",
            "explicit_conflict_construction_ns",
            "biclique_construction_ns",
            "explicit_network_construction_ns",
            "compressed_network_construction_ns",
            "matching_ns",
            "max_flow_ns",
            "vertex_cover_recovery_ns",
            "chord_selection_ns",
            "rectangle_completion_recovery_ns",
            "verification_ns",
            "scope_a_total_ns",
            "scope_b_total_ns",
            "canonical_component_clone_ns",
            "canonical_context_borrow_or_share_ns",
            "canonical_component_release_ns",
            "solver_workspace_prepare_ns",
            "prepared_component_build_ns",
            "boundary_total_build_ns",
            "boundary_edge_discovery_ns",
            "boundary_adjacency_build_ns",
            "boundary_loop_tracing_ns",
            "boundary_loop_normalization_ns",
            "reflex_detection_ns",
            "boundary_unit_edge_sort_ns",
            "boundary_index_construction_ns",
            "reflex_grouping_ns",
            "horizontal_chord_generation_ns",
            "vertical_chord_generation_ns",
            "chord_validation_filtering_ns",
            "endpoint_index_construction_ns",
            "conflict_discovery_ns",
            "representation_construction_ns",
            "matching_or_flow_ns",
            "minimum_vertex_cover_recovery_ns",
            "selected_cut_materialization_ns",
            "horizontal_completion_ns",
            "vertical_completion_ns",
            "rectangle_reconstruction_ns",
            "output_validation_ns",
            "internal_output_validation_ns",
            "final_output_validation_ns",
            "completion_finalization_ns",
            "scope_a_leaf_sum_ns",
            "scope_a_unattributed_ns",
            "scope_a_accounting_ok",
            "scope_b_leaf_sum_ns",
            "scope_b_unattributed_ns",
            "scope_b_accounting_ok",
            "boundary_leaf_sum_ns",
            "boundary_unattributed_ns",
            "boundary_accounting_ok",
            "geometry_leaf_sum_ns",
            "geometry_unattributed_ns",
            "geometry_accounting_ok",
            "chord_leaf_sum_ns",
            "chord_unattributed_ns",
            "chord_accounting_ok",
            "completion_leaf_sum_ns",
            "completion_unattributed_ns",
            "completion_accounting_ok",
            "output_validation_leaf_sum_ns",
            "output_validation_unattributed_ns",
            "output_validation_accounting_ok",
        }
    )
)

OPTIONAL_OWNERSHIP_TIMING_FIELDS = frozenset(
    {
        "canonical_context_borrow_or_share_ns",
        "canonical_component_release_ns",
        "solver_workspace_prepare_ns",
    }
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    self_test_only = "--self-test" in sys.argv
    parser.add_argument("--config", type=Path, required=not self_test_only)
    parser.add_argument("--binary", type=Path, default=Path("target/release/mrd"))
    parser.add_argument("--output", type=Path, required=not self_test_only)
    parser.add_argument("--csv", type=Path, required=not self_test_only)
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--family", action="append", choices=FAMILIES)
    parser.add_argument("--size", action="append", type=int)
    parser.add_argument("--print-plan", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def root_path(path: Path) -> Path:
    return path if path.is_absolute() else ROOT / path


def relative_path(path: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(ROOT).as_posix()
    except ValueError:
        return str(resolved)


def canonical_json(value: Any) -> str:
    return json.dumps(value, allow_nan=False, sort_keys=True, separators=(",", ":"))


def strict_json_loads(value: str) -> Any:
    def reject_constant(constant: str) -> None:
        raise ValueError(f"non-finite JSON number {constant!r} is not permitted")

    return json.loads(value, parse_constant=reject_constant)


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
    source = (
        "AC"
        if "AC Power" in output
        else "battery"
        if "Battery Power" in output
        else "unknown"
    )
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
    actual_schema = config.get("schema_version")
    if actual_schema != SCHEMA_VERSION:
        raise ValueError(
            "incompatible paper-kernel-scaling config schema_version "
            f"{actual_schema!r}; expected {SCHEMA_VERSION}"
        )
    if config.get("campaign") != CAMPAIGN:
        raise ValueError(f"config campaign must be {CAMPAIGN!r}")
    if config.get("boundary_discovery_backend") not in BOUNDARY_DISCOVERY_BACKENDS:
        raise ValueError(
            "boundary_discovery_backend must be one of "
            + ", ".join(BOUNDARY_DISCOVERY_BACKENDS)
        )
    canonical_backend = config.get("canonical_backend", "clone-canonical-reference")
    if canonical_backend not in CANONICAL_BACKENDS:
        raise ValueError(
            "canonical_backend must be one of " + ", ".join(CANONICAL_BACKENDS)
        )
    if config.get("algorithms") != list(ALGORITHMS):
        raise ValueError(
            "config must contain exactly the three timed algorithms in declared order"
        )
    if config.get("scopes") != list(SCOPES):
        raise ValueError("config must contain exactly Scope A and Scope B")
    families = config.get("families")
    sizes = config.get("initial_size_levels")
    if (
        not families
        or len(set(families)) != len(families)
        or any(value not in FAMILIES for value in families)
    ):
        raise ValueError("families must be unique supported campaign families")
    if (
        not sizes
        or len(set(sizes)) != len(sizes)
        or any(not isinstance(value, int) or value <= 0 for value in sizes)
    ):
        raise ValueError("initial_size_levels must be unique positive integers")
    if sizes != sorted(sizes):
        raise ValueError("initial_size_levels must be in increasing order")
    if isinstance(config.get("seed"), bool) or not isinstance(config.get("seed"), int):
        raise ValueError("seed must be an integer")
    if (
        not isinstance(config.get("oracle_cell_limit"), int)
        or int(config["oracle_cell_limit"]) <= 0
    ):
        raise ValueError("oracle_cell_limit must be a positive integer")
    if config.get("family_parameter_rule") != {
        "comb-staircase": "ceil(sqrt(target_size))",
        "representation-crossover": "ceil(sqrt(target_size))",
        "all_other_families": "target_size",
    }:
        raise ValueError("family_parameter_rule does not match the Rust generators")
    warmup = config.get("warmup_rule", {})
    repetitions = config.get("repetition_rule", {})
    stop = config.get("stop_conditions", {})
    fit = config.get("fit_rule", {})
    if int(warmup.get("minimum", 0)) < 5 or int(warmup.get("maximum", 0)) > 50:
        raise ValueError("warmup rule must use at least 5 and at most 50 iterations")
    if int(warmup.get("maximum", 0)) < int(warmup.get("minimum", 0)):
        raise ValueError("warmup maximum must cover its minimum")
    if [
        repetitions.get(key)
        for key in ("fast_minimum", "medium_minimum", "slow_minimum")
    ] != [31, 15, 7]:
        raise ValueError("repetition minima must be 31/15/7")
    if not 31 <= int(repetitions.get("maximum", 0)) <= 10_000:
        raise ValueError("maximum repetitions must be between 31 and 10000")
    if int(stop.get("max_iteration_ns", 0)) != 5_000_000_000:
        raise ValueError("per-iteration stop must be five seconds")
    if int(stop.get("max_point_ns", 0)) != 120_000_000_000:
        raise ValueError("per-point stop must be 120 seconds")
    if int(stop.get("max_estimated_structural_bytes", 0)) <= 0:
        raise ValueError("estimated structural byte limit must be positive")
    if float(config.get("partition_timeout_seconds", 0)) <= 120:
        raise ValueError("partition timeout must exceed the internal point limit")
    if int(fit.get("minimum_valid_size_levels", 0)) < 6:
        raise ValueError("fit rule requires at least six valid size levels")
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
        "boundary_discovery_backend": config["boundary_discovery_backend"],
        "canonical_backend": config.get(
            "canonical_backend", "clone-canonical-reference"
        ),
        "algorithms": config["algorithms"],
        "scopes": config["scopes"],
        "oracle_cell_limit": config["oracle_cell_limit"],
        "warmup": config["warmup_rule"],
        "repetitions": config["repetition_rule"],
        "stop": config["stop_conditions"],
    }


def atomic_write_text(path: Path, value: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
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
    atomic_write_text(
        path, json.dumps(value, allow_nan=False, indent=2, sort_keys=True) + "\n"
    )


def validate_runtime_identity(expected: dict[str, Any], binary: Path) -> None:
    current_commit = command_or_unknown(["git", "rev-parse", "HEAD"])
    if current_commit != expected["source_commit"]:
        raise ValueError("source commit changed during the campaign")
    current_binary_sha256 = sha256_file(binary)
    if current_binary_sha256 != expected["binary_sha256"]:
        raise ValueError("release binary changed during the campaign")


def provenance(config: dict[str, Any], binary: Path) -> dict[str, Any]:
    config_sha256 = sha256_bytes(canonical_json(config).encode())
    captured = environment(binary)
    identity_fields = {
        "checkpoint_schema_version": CHECKPOINT_SCHEMA_VERSION,
        "sample_schema_version": SCHEMA_VERSION,
        "campaign": CAMPAIGN,
        "config_sha256": config_sha256,
        "source_commit": captured["git_commit"],
        "binary_sha256": captured["binary_sha256"],
    }
    return {
        **identity_fields,
        "campaign_identity": "sha256:"
        + sha256_bytes(canonical_json(identity_fields).encode()),
        "environment": captured,
    }


def scope_tag(scope: str) -> str:
    return {
        "solve-from-canonical-instance": "scope-a",
        "representation-and-solver-kernel": "scope-b",
    }[scope]


def sample_identity(
    point: dict[str, Any],
    canonical_instance_identity: str,
    boundary_discovery_backend: str,
    scope: str,
    algorithm: str,
    iteration: int,
) -> str:
    return ":".join(
        (
            CAMPAIGN,
            f"v{SCHEMA_VERSION}",
            point["family"],
            str(point["target_size"]),
            str(point["seed"]),
            canonical_instance_identity,
            "measured",
            scope_tag(scope),
            algorithm,
            str(iteration),
            boundary_discovery_backend,
        )
    )


def require_nonnegative_integer(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValueError(f"{field} must be a nonnegative integer")
    return value


def validate_correctness(records: Any) -> int:
    if not isinstance(records, list):
        raise ValueError("point correctness must be an array")
    algorithms = [
        record.get("algorithm") for record in records if isinstance(record, dict)
    ]
    if len(records) != len(ALGORITHMS) or set(algorithms) != set(ALGORITHMS):
        raise ValueError(
            "point correctness must contain each timed algorithm exactly once"
        )
    optima = set()
    for record in records:
        if record.get("outcome") != "success":
            raise ValueError("point correctness contains a failed production gate")
        optimum = require_nonnegative_integer(
            record.get("optimum_rectangle_count"),
            "correctness optimum_rectangle_count",
        )
        matching = require_nonnegative_integer(
            record.get("matching_size"), "correctness matching_size"
        )
        cover = require_nonnegative_integer(
            record.get("vertex_cover_size"), "correctness vertex_cover_size"
        )
        if matching != cover:
            raise ValueError("point correctness matching and cover sizes disagree")
        optima.add(optimum)
    if len(optima) != 1:
        raise ValueError("point correctness optimum counts disagree")
    return next(iter(optima))


def validate_sizes(result: dict[str, Any], *, require_complete: bool) -> int | None:
    sizes = result.get("sizes")
    if not isinstance(sizes, dict):
        raise ValueError("point sizes must be an object")
    required_fields = (
        "width",
        "height",
        "bounding_box_area_a",
        "foreground_cells_n",
        "component_count",
        "boundary_size_b",
        "boundary_unit_edge_count_u",
        "reflex_count",
        "horizontal_chord_count_h",
        "vertical_chord_count_v",
        "q",
        "explicit_conflict_edge_count_k",
        "biclique_count",
        "biclique_total_vertex_occurrences_sigma",
        "compressed_network_node_count",
        "compressed_network_arc_count",
        "compressed_representation_size_m",
        "optimum_rectangle_count",
        "output_rectangle_count",
    )
    if require_complete:
        values = {
            field: require_nonnegative_integer(sizes.get(field), f"sizes {field}")
            for field in required_fields
        }
    else:
        values = {
            field: require_nonnegative_integer(value, f"sizes {field}")
            for field in required_fields
            if (value := sizes.get(field)) is not None
        }
    if all(
        field in values
        for field in ("bounding_box_area_a", "foreground_cells_n", "width", "height")
    ) and not (
        values["foreground_cells_n"]
        <= values["bounding_box_area_a"]
        <= values["width"] * values["height"]
    ):
        raise ValueError("sizes bounding_box_area_a is outside [N, width * height]")
    if all(
        field in values
        for field in ("q", "horizontal_chord_count_h", "vertical_chord_count_v")
    ) and values["q"] != (
        values["horizontal_chord_count_h"] + values["vertical_chord_count_v"]
    ):
        raise ValueError("sizes q differs from H + V")
    if all(
        field in values
        for field in (
            "compressed_representation_size_m",
            "compressed_network_node_count",
            "compressed_network_arc_count",
        )
    ) and values["compressed_representation_size_m"] != (
        values["compressed_network_node_count"] + values["compressed_network_arc_count"]
    ):
        raise ValueError("sizes M differs from compact nodes + arcs")
    if (
        all(
            field in values
            for field in ("output_rectangle_count", "optimum_rectangle_count")
        )
        and values["output_rectangle_count"] != values["optimum_rectangle_count"]
    ):
        raise ValueError("sizes output rectangle count differs from optimum")
    return values.get("optimum_rectangle_count")


def validate_setup_and_structure(
    result: dict[str, Any], *, require_complete: bool
) -> None:
    setup = result.get("setup_timings")
    structure = result.get("structure")
    if not isinstance(setup, dict) or not isinstance(structure, dict):
        raise ValueError("point setup_timings and structure must be objects")
    setup_fields = (
        "instance_generation_ns",
        "input_normalization_ns",
        "connected_component_extraction_ns",
        "setup_total_ns",
    )
    if require_complete:
        setup_values = {
            field: require_nonnegative_integer(setup.get(field), f"setup {field}")
            for field in setup_fields
        }
        if (
            sum(setup_values[field] for field in setup_fields[:-1])
            > setup_values["setup_total_ns"]
        ):
            raise ValueError("setup leaf timing sum exceeds setup_total_ns")
        for field in (
            "boundary_candidate_edge_probes",
            "boundary_exposed_unit_edges",
            "boundary_trace_edge_visits",
            "explicit_graph_node_count",
            "explicit_graph_edge_count",
            "biclique_count",
            "biclique_incidence_sigma",
            "compact_node_count",
            "compact_arc_count",
            "explicit_c0_node_count",
            "explicit_c0_arc_count",
            "explicit_estimated_structural_bytes",
            "compact_estimated_structural_bytes",
            "explicit_c0_estimated_structural_bytes",
            "estimated_peak_structural_bytes",
            "completion_candidate_queries",
            "completion_candidate_revalidations",
            "completion_stale_candidates",
            "completion_ray_extension_unit_steps",
            "rectangle_recovery_cell_visits",
        ):
            require_nonnegative_integer(structure.get(field), f"structure {field}")
        sizes = result["sizes"]
        expected_equalities = (
            ("boundary_candidate_edge_probes", 4 * sizes["foreground_cells_n"]),
            ("boundary_exposed_unit_edges", sizes["boundary_unit_edge_count_u"]),
            ("boundary_trace_edge_visits", sizes["boundary_unit_edge_count_u"]),
            ("explicit_graph_node_count", sizes["q"]),
            ("explicit_graph_edge_count", sizes["explicit_conflict_edge_count_k"]),
            ("biclique_count", sizes["biclique_count"]),
            (
                "biclique_incidence_sigma",
                sizes["biclique_total_vertex_occurrences_sigma"],
            ),
            ("compact_node_count", sizes["compressed_network_node_count"]),
            ("compact_arc_count", sizes["compressed_network_arc_count"]),
        )
        for field, expected in expected_equalities:
            if structure[field] != expected:
                raise ValueError(f"structure {field} differs from its canonical size")
        estimates = (
            structure["explicit_estimated_structural_bytes"],
            structure["compact_estimated_structural_bytes"],
            structure["explicit_c0_estimated_structural_bytes"],
        )
        if structure["estimated_peak_structural_bytes"] != max(estimates):
            raise ValueError(
                "estimated peak structural bytes differs from backend estimates"
            )


def validate_nested_timing_accounting(timings: dict[str, Any], identity: str) -> None:
    for parent, leaves, prefix in NESTED_TIMING_GROUPS:
        for field in (parent, *leaves):
            if field not in timings:
                raise ValueError(f"{identity} timings omit {field}")
            value = timings[field]
            if value is not None:
                require_nonnegative_integer(value, f"{identity} {field}")
        accounting_fields = (
            f"{prefix}_leaf_sum_ns",
            f"{prefix}_unattributed_ns",
            f"{prefix}_accounting_ok",
        )
        if any(field not in timings for field in accounting_fields):
            raise ValueError(f"{identity} timings omit {prefix} accounting")
        parent_value = timings[parent]
        if parent_value is None:
            if any(timings[field] is not None for field in accounting_fields):
                raise ValueError(f"{identity} has {prefix} accounting without a parent")
            continue
        leaf_sum = sum(timings[field] or 0 for field in leaves)
        declared = require_nonnegative_integer(
            timings[accounting_fields[0]], f"{identity} {accounting_fields[0]}"
        )
        unattributed = require_nonnegative_integer(
            timings[accounting_fields[1]], f"{identity} {accounting_fields[1]}"
        )
        if timings[accounting_fields[2]] is not True:
            raise ValueError(f"{identity} {prefix} accounting is not valid")
        if declared != leaf_sum or declared + unattributed != parent_value:
            raise ValueError(f"{identity} {prefix} timing accounting mismatch")


def validate_run_timings(timings: dict[str, Any], scope: str, identity: str) -> None:
    for field in TIMING_FIELDS:
        if field not in timings:
            if field in OPTIONAL_OWNERSHIP_TIMING_FIELDS:
                continue
            raise ValueError(f"{identity} timings omit {field}")
        if field.endswith("_ok"):
            if timings[field] is not None and not isinstance(timings[field], bool):
                raise ValueError(f"{identity} {field} must be boolean or null")
        elif timings[field] is not None:
            require_nonnegative_integer(timings[field], f"{identity} {field}")
    validate_nested_timing_accounting(timings, identity)
    prefix = "scope_a" if scope == SCOPES[0] else "scope_b"
    total_field = f"{prefix}_total_ns"
    leaf_field = f"{prefix}_leaf_sum_ns"
    unattributed_field = f"{prefix}_unattributed_ns"
    ok_field = f"{prefix}_accounting_ok"
    phases = SCOPE_A_PARENT_PHASES if scope == SCOPES[0] else SCOPE_B_PARENT_PHASES
    total = require_nonnegative_integer(
        timings.get(total_field), f"{identity} {total_field}"
    )
    leaf_sum = sum(timings.get(field) or 0 for field in phases)
    declared = require_nonnegative_integer(
        timings.get(leaf_field), f"{identity} {leaf_field}"
    )
    unattributed = require_nonnegative_integer(
        timings.get(unattributed_field), f"{identity} {unattributed_field}"
    )
    if timings.get(ok_field) is not True:
        raise ValueError(f"{identity} scope accounting is not valid")
    if declared != leaf_sum or declared + unattributed != total:
        raise ValueError(f"{identity} scope timing accounting mismatch")


def validate_run_census(
    result: dict[str, Any],
    point: dict[str, Any],
    config: dict[str, Any],
    *,
    require_complete: bool,
) -> None:
    warmups = result.get("warmups")
    runs = result.get("runs")
    exact_order = result.get("exact_measured_order")
    if (
        not isinstance(warmups, list)
        or not isinstance(runs, list)
        or not isinstance(exact_order, list)
    ):
        raise ValueError("point warmups, runs, and exact_measured_order must be arrays")

    expected_pairs = {
        (scope, algorithm) for scope in SCOPES for algorithm in ALGORITHMS
    }
    expected_canonical_backend = config.get("canonical_backend")
    warmup_counts: dict[tuple[str, str], int] = {}
    for record in warmups:
        if not isinstance(record, dict):
            raise ValueError("warmup record must be an object")
        pair = (record.get("scope"), record.get("algorithm"))
        if pair not in expected_pairs or pair in warmup_counts:
            raise ValueError("point contains an invalid or duplicate warmup identity")
        if record.get("boundary_discovery_backend") != result.get(
            "boundary_discovery_backend"
        ):
            raise ValueError("warmup boundary discovery backend mismatch")
        if (
            expected_canonical_backend is not None
            and record.get("canonical_backend") != expected_canonical_backend
        ):
            raise ValueError("warmup canonical backend mismatch")
        count = require_nonnegative_integer(record.get("count"), "warmup count")
        if count < int(config["warmup_rule"]["minimum"]):
            raise ValueError("warmup count is below the configured minimum")
        preflight_ns = require_nonnegative_integer(
            record.get("preflight_ns"), "warmup preflight_ns"
        )
        measured = require_nonnegative_integer(
            record.get("measured_repetitions"), "warmup measured_repetitions"
        )
        repetition_rule = config["repetition_rule"]
        if preflight_ns < int(repetition_rule["fast_threshold_ns"]):
            minimum = int(repetition_rule["fast_minimum"])
        elif preflight_ns <= int(repetition_rule["medium_threshold_ns"]):
            minimum = int(repetition_rule["medium_minimum"])
        else:
            minimum = int(repetition_rule["slow_minimum"])
        target = int(repetition_rule["target_measured_ns"]) // max(preflight_ns, 1)
        expected_measured = min(max(target, minimum), int(repetition_rule["maximum"]))
        if measured != expected_measured:
            raise ValueError(
                "measured repetition count differs from the configured adaptive rule"
            )
        warmup_counts[pair] = measured
    if require_complete and set(warmup_counts) != expected_pairs:
        raise ValueError("complete point is missing warmup identities")

    canonical_identity = result.get("canonical_instance_identity")
    backend = result.get("boundary_discovery_backend")
    observed_keys: list[tuple[str, str, int]] = []
    observed_identities: list[str] = []
    for record in runs:
        if not isinstance(record, dict):
            raise ValueError("measured run must be an object")
        scope = record.get("scope")
        algorithm = record.get("algorithm")
        iteration = require_nonnegative_integer(
            record.get("iteration"), "run iteration"
        )
        pair = (scope, algorithm)
        if pair not in expected_pairs:
            raise ValueError("measured run has an unsupported scope or algorithm")
        if pair not in warmup_counts or iteration >= warmup_counts[pair]:
            raise ValueError(
                "measured run falls outside its declared repetition census"
            )
        if record.get("record_kind") != "measured":
            raise ValueError("measured run record_kind must be 'measured'")
        if record.get("seed") != point["seed"]:
            raise ValueError("measured run seed differs from its planned point")
        if record.get("canonical_instance_identity") != canonical_identity:
            raise ValueError("measured run canonical instance identity mismatch")
        if record.get("boundary_discovery_backend") != backend:
            raise ValueError("measured run boundary discovery backend mismatch")
        if (
            expected_canonical_backend is not None
            and record.get("canonical_backend") != expected_canonical_backend
        ):
            raise ValueError("measured run canonical backend mismatch")
        expected_identity = sample_identity(
            point,
            canonical_identity,
            backend,
            scope,
            algorithm,
            iteration,
        )
        if record.get("sample_identity") != expected_identity:
            raise ValueError(
                "measured run sample identity does not match its canonical fields"
            )
        elapsed_ns = require_nonnegative_integer(
            record.get("elapsed_ns"), "run elapsed_ns"
        )
        timings = record.get("timings")
        if not isinstance(timings, dict):
            raise ValueError("measured run timings must be an object")
        validate_run_timings(timings, scope, str(record.get("sample_identity")))
        if expected_canonical_backend is not None:
            allocations = record.get("allocations")
            if not isinstance(allocations, dict):
                raise ValueError("measured ownership run allocations must be an object")
            for field in (
                "canonical_cells_cloned",
                "canonical_clone_bytes_estimate",
                "solver_workspace_retained_bytes_estimate",
                "representation_retained_bytes_estimate",
                "ownership_vec_allocation_count_estimate",
            ):
                require_nonnegative_integer(
                    allocations.get(field),
                    f"{record.get('sample_identity')} allocations.{field}",
                )
            cloned_cells = allocations["canonical_cells_cloned"]
            cloned_bytes = allocations["canonical_clone_bytes_estimate"]
            if (
                scope == SCOPES[0]
                and expected_canonical_backend == "clone-canonical-reference"
            ):
                if (
                    cloned_cells != result["sizes"]["foreground_cells_n"]
                    or cloned_bytes <= 0
                ):
                    raise ValueError(
                        "clone-reference allocation diagnostics are inconsistent"
                    )
            elif cloned_cells != 0 or cloned_bytes != 0:
                raise ValueError("non-clone scope reports canonical clone allocations")
            workspace_bytes = allocations["solver_workspace_retained_bytes_estimate"]
            if scope == SCOPES[1] and workspace_bytes != 0:
                raise ValueError("Scope B reports a solver selection workspace")
        total_field = (
            "scope_a_total_ns"
            if scope == "solve-from-canonical-instance"
            else "scope_b_total_ns"
        )
        if timings.get(total_field) != elapsed_ns:
            raise ValueError(f"measured run {total_field} differs from elapsed_ns")
        matching = require_nonnegative_integer(
            record.get("matching_size"), "run matching_size"
        )
        cover = require_nonnegative_integer(
            record.get("vertex_cover_size"), "run vertex_cover_size"
        )
        if matching != cover:
            raise ValueError("measured run matching and cover sizes disagree")
        run_optimum = require_nonnegative_integer(
            record.get("optimum_rectangle_count"), "run optimum_rectangle_count"
        )
        if run_optimum != result["sizes"]["optimum_rectangle_count"]:
            raise ValueError("measured run optimum differs from point sizes")
        observed_keys.append((scope, algorithm, iteration))
        observed_identities.append(expected_identity)

    if len(observed_keys) != len(set(observed_keys)):
        raise ValueError("point contains duplicate measured run identities")
    if len(observed_identities) != len(set(observed_identities)):
        raise ValueError("point contains duplicate sample identities")
    if exact_order != observed_identities:
        raise ValueError("exact_measured_order differs from measured run order")
    if require_complete:
        expected_keys = {
            (scope, algorithm, iteration)
            for (scope, algorithm), count in warmup_counts.items()
            for iteration in range(count)
        }
        if set(observed_keys) != expected_keys:
            raise ValueError(
                "complete point does not contain its exact measured sample census"
            )


def validate_point_result(
    result: dict[str, Any], point: dict[str, Any], config: dict[str, Any]
) -> None:
    if not isinstance(result, dict):
        raise ValueError("point result must be an object")
    if result.get("point_identity") != point["point_identity"]:
        raise ValueError("point result identity differs from the plan")
    state = result.get("state")
    if state in RUNNER_FAILURE_STATES:
        return
    if result.get("schema_version") != SCHEMA_VERSION:
        raise ValueError(
            "point result schema_version mismatch: "
            f"expected {SCHEMA_VERSION}, found {result.get('schema_version')!r}"
        )
    expected_header = {
        "campaign": CAMPAIGN,
        "family": point["family"],
        "target_size": point["target_size"],
        "seed": point["seed"],
        "boundary_discovery_backend": config["boundary_discovery_backend"],
    }
    if "canonical_backend" in config:
        expected_header["canonical_backend"] = config["canonical_backend"]
    for field, expected_value in expected_header.items():
        if result.get(field) != expected_value:
            raise ValueError(f"point result {field} differs from its request")
    if state not in {"complete", "stopped", "invalid"}:
        raise ValueError(f"point result has unknown state {state!r}")

    propagated_from = result.get("stop_propagated_from_target_size")
    if propagated_from is not None:
        if state != "stopped" or not isinstance(propagated_from, int):
            raise ValueError("propagated stop has invalid state or source target")
        if propagated_from >= point["target_size"]:
            raise ValueError("propagated stop source must be a smaller target")
        if any(result.get(field) for field in ("runs", "warmups", "correctness")):
            raise ValueError("propagated stop must not contain measurements")
        return

    canonical_identity = result.get("canonical_instance_identity")
    if not isinstance(canonical_identity, str) or not canonical_identity:
        raise ValueError("point result must contain a canonical instance identity")
    if state == "invalid":
        return
    if not isinstance(result.get("message"), (str, type(None))):
        raise ValueError("point result message must be null or a string")
    require_complete = state == "complete"
    size_optimum = validate_sizes(result, require_complete=require_complete)
    validate_setup_and_structure(result, require_complete=require_complete)
    shared_preprocessing = result.get("shared_scope_b_preprocessing")
    if require_complete:
        if not isinstance(shared_preprocessing, dict):
            raise ValueError("complete point omits shared_scope_b_preprocessing")
        validate_nested_timing_accounting(
            shared_preprocessing, f"{point['point_identity']} shared preprocessing"
        )
        for field in ("geometry_preprocessing_ns", "chord_generation_ns"):
            require_nonnegative_integer(
                shared_preprocessing.get(field), f"shared preprocessing {field}"
            )
    if any(
        record.get("boundary_discovery_backend") != config["boundary_discovery_backend"]
        for record in result.get("correctness", [])
    ):
        raise ValueError("correctness boundary discovery backend mismatch")
    if "canonical_backend" in config and any(
        record.get("canonical_backend") != config["canonical_backend"]
        for record in result.get("correctness", [])
    ):
        raise ValueError("correctness canonical backend mismatch")
    correctness_optimum = validate_correctness(result.get("correctness"))
    if size_optimum is not None and correctness_optimum != size_optimum:
        raise ValueError("point correctness optimum differs from point sizes")
    oracle_optimum = result.get("oracle_optimum_rectangle_count")
    if oracle_optimum is not None and oracle_optimum != correctness_optimum:
        raise ValueError("exact-cover Oracle optimum differs from production optimum")
    validate_run_census(
        result,
        point,
        config,
        require_complete=state == "complete",
    )


def validate_checkpoint(
    checkpoint: dict[str, Any],
    expected: dict[str, Any],
    planned: list[dict[str, Any]],
    config: dict[str, Any],
) -> None:
    identity_keys = (
        "checkpoint_schema_version",
        "sample_schema_version",
        "campaign",
        "config_sha256",
        "source_commit",
        "binary_sha256",
        "campaign_identity",
    )
    for key in identity_keys:
        if checkpoint.get(key) != expected.get(key):
            raise ValueError(
                f"checkpoint {key} mismatch: expected {expected.get(key)!r}, "
                f"found {checkpoint.get(key)!r}"
            )
    checkpoint_environment = checkpoint.get("environment")
    expected_environment = expected.get("environment")
    if not isinstance(checkpoint_environment, dict) or not isinstance(
        expected_environment, dict
    ):
        raise ValueError("checkpoint environment is missing or malformed")
    for field in RESUME_ENVIRONMENT_FIELDS:
        if checkpoint_environment.get(field) != expected_environment.get(field):
            raise ValueError(
                f"checkpoint environment.{field} mismatch: expected "
                f"{expected_environment.get(field)!r}, found "
                f"{checkpoint_environment.get(field)!r}"
            )
    if checkpoint.get("protocol") != config:
        raise ValueError("checkpoint protocol differs from the normalized config")
    if checkpoint.get("planned_points") != planned:
        raise ValueError("checkpoint plan differs from config")
    point_results = checkpoint.get("point_results")
    if not isinstance(point_results, list):
        raise ValueError("checkpoint point_results must be an array")
    planned_by_identity = {row["point_identity"]: row for row in planned}
    observed = [
        row.get("point_identity") for row in point_results if isinstance(row, dict)
    ]
    if len(observed) != len(point_results):
        raise ValueError("checkpoint contains a malformed point result")
    if len(observed) != len(set(observed)):
        raise ValueError("checkpoint contains duplicate point identities")
    if any(identity not in planned_by_identity for identity in observed):
        raise ValueError("checkpoint contains an unplanned point")
    for result in point_results:
        validate_point_result(
            result, planned_by_identity[result["point_identity"]], config
        )
    retry_history = checkpoint.get("retry_history", [])
    if not isinstance(retry_history, list) or any(
        not isinstance(row, dict)
        or row.get("state") not in RUNNER_FAILURE_STATES
        or row.get("point_identity") not in planned_by_identity
        for row in retry_history
    ):
        raise ValueError("checkpoint retry_history contains a malformed runner failure")


def refresh(checkpoint: dict[str, Any]) -> None:
    planned = {row["point_identity"] for row in checkpoint["planned_points"]}
    accepted = {
        row["point_identity"]
        for row in checkpoint["point_results"]
        if row.get("state") in {"complete", "stopped"}
    }
    observed = {row["point_identity"] for row in checkpoint["point_results"]}
    states: dict[str, int] = {}
    for row in checkpoint["point_results"]:
        states[row.get("state", "runner-error")] = (
            states.get(row.get("state", "runner-error"), 0) + 1
        )
    correctness_failures = sum(
        record.get("outcome") != "success"
        for row in checkpoint["point_results"]
        for record in row.get("correctness", [])
    )
    checkpoint["completion"] = {
        "complete": planned == accepted and correctness_failures == 0,
        "planned_point_count": len(planned),
        "observed_point_count": len(observed),
        "completed_point_count": len(accepted),
        "missing_point_count": len(planned - accepted),
        "missing_point_identities": sorted(planned - accepted),
        "terminal_state_counts": states,
        "correctness_failure_count": correctness_failures,
        "retry_history_count": len(checkpoint.get("retry_history", [])),
    }
    checkpoint["updated_at_epoch_seconds"] = int(time.time())


def launch(
    binary: Path, config: dict[str, Any], point: dict[str, Any]
) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(
        prefix="paper-kernel-scaling-", dir=ROOT / "results"
    ) as directory:
        directory_path = Path(directory)
        request_path = directory_path / "request.json"
        output_path = directory_path / "result.json"
        request_path.write_text(
            json.dumps(rust_request(config, point), allow_nan=False, sort_keys=True)
            + "\n"
        )
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
            result = strict_json_loads(output_path.read_text())
        except (OSError, ValueError, json.JSONDecodeError) as error:
            return {
                **point,
                "state": "runner-error",
                "message": str(error),
                "partition_wall_time_ns": wall,
            }
        returned_point_identity = result.get("point_identity")
        if (
            returned_point_identity is not None
            and returned_point_identity != point["point_identity"]
        ):
            return {
                **point,
                "state": "runner-error",
                "message": "Rust payload returned a conflicting point identity",
                "partition_wall_time_ns": wall,
                "exit_status": completed.returncode,
            }
        result.update(
            {
                "point_identity": point["point_identity"],
                "partition_wall_time_ns": wall,
                "exit_status": completed.returncode,
                "stderr_tail": completed.stderr[-4000:],
            }
        )
        try:
            validate_point_result(result, point, config)
        except ValueError as error:
            return {
                **point,
                "state": "runner-error",
                "message": f"malformed Rust benchmark payload: {error}",
                "partition_wall_time_ns": wall,
                "exit_status": completed.returncode,
                "stderr_tail": completed.stderr[-4000:],
            }
        return result


def csv_rows(checkpoint: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    environment_values = checkpoint["environment"]
    for point in checkpoint["point_results"]:
        for run in point.get("runs", []):
            row = {
                "schema_version": SCHEMA_VERSION,
                "campaign": CAMPAIGN,
                "config_sha256": checkpoint["config_sha256"],
                "campaign_identity": checkpoint["campaign_identity"],
                "source_commit": checkpoint["source_commit"],
                "binary_sha256": checkpoint["binary_sha256"],
                "point_identity": point["point_identity"],
                "point_state": point["state"],
                "family": point["family"],
                "target_size": point["target_size"],
                "generator_parameter": point.get("generator_parameter"),
                "canonical_instance_identity": point.get("canonical_instance_identity"),
                "boundary_discovery_backend": point.get("boundary_discovery_backend"),
                **run,
            }
            for prefix, values in (
                ("setup_timing_", point.get("setup_timings", {})),
                (
                    "shared_preprocessing_",
                    point.get("shared_scope_b_preprocessing", {}),
                ),
                ("size_", point.get("sizes", {})),
                ("structure_", point.get("structure", {})),
                ("timing_", run.get("timings", {})),
                ("allocation_", run.get("allocations", {})),
                ("host_", environment_values),
            ):
                for key, value in values.items():
                    row[prefix + key] = value
            row.pop("timings", None)
            row.pop("allocations", None)
            rows.append(row)
    return rows


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    fields = sorted({key for row in rows for key in row}) or ["campaign"]
    buffer = io.StringIO(newline="")
    writer = csv.DictWriter(buffer, fieldnames=fields, lineterminator="\n")
    writer.writeheader()
    for row in rows:
        writer.writerow(
            {
                key: canonical_json(value) if isinstance(value, (dict, list)) else value
                for key, value in row.items()
            }
        )
    atomic_write_text(path, buffer.getvalue())


def output_payload(checkpoint: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": CAMPAIGN,
        "protocol": checkpoint["protocol"],
        "config_sha256": checkpoint["config_sha256"],
        "campaign_identity": checkpoint["campaign_identity"],
        "source_commit": checkpoint["source_commit"],
        "binary_sha256": checkpoint["binary_sha256"],
        "environment": checkpoint["environment"],
        "planned_points": checkpoint["planned_points"],
        "completion": checkpoint["completion"],
        "point_results": checkpoint["point_results"],
        "runner_wall_time_ns": checkpoint.get("runner_wall_time_ns", 0),
        "retry_history": checkpoint.get("retry_history", []),
    }


def propagated_stop(
    point: dict[str, Any], source: dict[str, Any], config: dict[str, Any]
) -> dict[str, Any]:
    return {
        **point,
        "schema_version": SCHEMA_VERSION,
        "campaign": CAMPAIGN,
        "boundary_discovery_backend": config["boundary_discovery_backend"],
        "canonical_backend": config.get(
            "canonical_backend", "clone-canonical-reference"
        ),
        "canonical_instance_identity": None,
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
        "setup_timings": {},
        "partition_wall_time_ns": 0,
    }


def run_campaign(
    config: dict[str, Any],
    binary: Path,
    checkpoint_path: Path,
    resume: bool = False,
    families: list[str] | None = None,
    sizes: list[int] | None = None,
    launch_partition=launch,
) -> dict[str, Any]:
    validate_config(config)
    binary = root_path(binary)
    if not binary.exists():
        raise FileNotFoundError(f"release binary not found: {relative_path(binary)}")
    expected = provenance(config, binary)
    planned = plan(config, expected["config_sha256"])
    checkpoint_path = root_path(checkpoint_path)
    if resume:
        checkpoint = strict_json_loads(checkpoint_path.read_text())
        validate_checkpoint(checkpoint, expected, planned, config)
        invalid = [
            row["point_identity"]
            for row in checkpoint["point_results"]
            if row.get("state") == "invalid"
        ]
        if invalid:
            raise ValueError(
                "checkpoint contains terminal invalid points; preserve the evidence and "
                f"start a new campaign: {', '.join(invalid)}"
            )
        retryable = [
            row
            for row in checkpoint["point_results"]
            if row.get("state") in RUNNER_FAILURE_STATES
        ]
        if retryable:
            checkpoint.setdefault("retry_history", []).extend(retryable)
            checkpoint["point_results"] = [
                row
                for row in checkpoint["point_results"]
                if row.get("state") not in RUNNER_FAILURE_STATES
            ]
    else:
        if checkpoint_path.exists():
            raise FileExistsError("checkpoint exists; use --resume")
        checkpoint = {
            **expected,
            "protocol": config,
            "planned_points": planned,
            "point_results": [],
            "retry_history": [],
            "created_at_epoch_seconds": int(time.time()),
            "runner_wall_time_ns": 0,
        }
    selected_families = set(families or config["families"])
    selected_sizes = set(sizes or config["initial_size_levels"])
    if not selected_families.issubset(
        config["families"]
    ) or not selected_sizes.issubset(config["initial_size_levels"]):
        raise ValueError("partition selection is outside the predeclared config")
    completed = {
        row["point_identity"]
        for row in checkpoint["point_results"]
        if row.get("state") in {"complete", "stopped"}
    }
    family_stops = {
        row["family"]: row
        for row in checkpoint["point_results"]
        if row.get("state") == "stopped"
        and row.get("stop_propagated_from_target_size") is None
    }
    started = time.perf_counter_ns()
    for point in planned:
        if (
            point["point_identity"] in completed
            or point["family"] not in selected_families
            or point["target_size"] not in selected_sizes
        ):
            continue
        prior_stop = family_stops.get(point["family"])
        if prior_stop is not None and point["target_size"] > prior_stop["target_size"]:
            result = propagated_stop(point, prior_stop, config)
        else:
            result = launch_partition(binary, config, point)
            validate_runtime_identity(expected, binary)
            validate_point_result(result, point, config)
            if result.get("state") == "stopped":
                family_stops[point["family"]] = result
        checkpoint["point_results"].append(result)
        if result.get("state") in {"complete", "stopped"}:
            completed.add(point["point_identity"])
        checkpoint["runner_wall_time_ns"] += time.perf_counter_ns() - started
        started = time.perf_counter_ns()
        refresh(checkpoint)
        atomic_write_json(checkpoint_path, checkpoint)
    checkpoint["runner_wall_time_ns"] += time.perf_counter_ns() - started
    refresh(checkpoint)
    atomic_write_json(checkpoint_path, checkpoint)
    validate_runtime_identity(expected, binary)
    validate_checkpoint(checkpoint, expected, planned, config)
    return output_payload(checkpoint)


def self_test() -> None:
    config = {
        "schema_version": SCHEMA_VERSION,
        "campaign": CAMPAIGN,
        "boundary_discovery_backend": "reference-edge-toggle",
        "families": ["random-connected"],
        "initial_size_levels": [1, 2],
        "family_parameter_rule": {
            "comb-staircase": "ceil(sqrt(target_size))",
            "representation-crossover": "ceil(sqrt(target_size))",
            "all_other_families": "target_size",
        },
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
            "max_estimated_structural_bytes": 1_000_000_000,
        },
        "partition_timeout_seconds": 130,
        "fit_rule": {"minimum_valid_size_levels": 8, "bootstrap_resamples": 10_000},
    }
    validate_config(config)
    rows = plan(config, sha256_bytes(canonical_json(config).encode()))
    assert len(rows) == 2
    assert len({row["point_identity"] for row in rows}) == 2
    stopped = {**rows[0], "state": "stopped", "message": "iteration limit"}
    propagated = propagated_stop(rows[1], stopped, config)
    assert propagated["state"] == "stopped"
    assert propagated["stop_propagated_from_target_size"] == 1
    assert propagated["runs"] == []


def main() -> int:
    arguments = parse_args()
    if arguments.self_test:
        self_test()
        print("paper-kernel-scaling runner self-test: ok")
        return 0
    config = strict_json_loads(root_path(arguments.config).read_text())
    validate_config(config)
    planned = plan(config, sha256_bytes(canonical_json(config).encode()))
    if arguments.print_plan:
        print(
            json.dumps(
                {
                    "planned_points": len(planned),
                    "families": len(config["families"]),
                    "sizes": len(config["initial_size_levels"]),
                },
                sort_keys=True,
            )
        )
        return 0
    checkpoint = arguments.checkpoint
    if checkpoint is None:
        checkpoint = arguments.output.with_name(
            f"{arguments.output.stem}-checkpoint.json"
        )
    payload = run_campaign(
        config,
        arguments.binary,
        checkpoint,
        arguments.resume,
        arguments.family,
        arguments.size,
    )
    atomic_write_json(root_path(arguments.output), payload)
    write_csv(root_path(arguments.csv), csv_rows(payload))
    print(
        json.dumps(
            {
                "completion": payload["completion"],
                "measured_iterations": len(csv_rows(payload)),
                "output": relative_path(root_path(arguments.output)),
            },
            sort_keys=True,
        )
    )
    return 0 if payload["completion"]["complete"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
