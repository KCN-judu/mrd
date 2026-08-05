#!/usr/bin/env python3
"""Analyze raw paper-kernel-scaling observations without dropping outliers."""

from __future__ import annotations

import argparse
import copy
import csv
import html
import io
import json
import math
import random
import statistics
import sys
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import Path
from typing import Any, Callable


ROOT = Path(__file__).resolve().parents[1]
CAMPAIGN = "paper-kernel-scaling"
CURRENT_SCHEMA_VERSION = 2
LEGACY_SCHEMA_VERSION = 1
SUMMARY_SCHEMA_VERSION = 2
SCOPES = ("solve-from-canonical-instance", "representation-and-solver-kernel")
SETUP_SCOPE = "campaign-setup"
SHARED_PREPROCESSING_SCOPE = "shared-preprocessing"
SHARED_ALGORITHM = "shared"
ALGORITHMS = ("compact-mrd", "explicit-hopcroft-karp", "explicit-c0-flow")
EXPLICIT = ("explicit-hopcroft-karp", "explicit-c0-flow")
VARIABLES = {
    "N": "foreground_cells_n",
    "B": "boundary_size_b",
    "r": "reflex_count",
    "H": "horizontal_chord_count_h",
    "V": "vertical_chord_count_v",
    "q": "q",
    "K": "explicit_conflict_edge_count_k",
    "M": "compressed_representation_size_m",
}
COARSE_PHASES = (
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
)
SETUP_PHASES = (
    "instance_generation_ns",
    "input_normalization_ns",
    "connected_component_extraction_ns",
)
GEOMETRY_LEAF_PHASES = (
    "canonical_component_clone_ns",
    "boundary_total_build_ns",
    "prepared_component_build_ns",
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
RECOVERY_LEAF_PHASES = (
    "chord_selection_ns",
    "selected_cut_materialization_ns",
    "horizontal_completion_ns",
    "vertical_completion_ns",
    "rectangle_reconstruction_ns",
    "output_validation_ns",
    "internal_output_validation_ns",
    "final_output_validation_ns",
    "completion_finalization_ns",
)
LEAF_PHASES = GEOMETRY_LEAF_PHASES + KERNEL_LEAF_PHASES + RECOVERY_LEAF_PHASES
ALL_ANALYZED_PHASES = SETUP_PHASES + LEAF_PHASES
SCOPE_A_ACCOUNTING_PHASES = (
    "canonical_component_clone_ns",
    "geometry_preprocessing_ns",
    "chord_generation_ns",
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
SHARED_PREPROCESSING_PHASES = tuple(
    phase for phase in GEOMETRY_LEAF_PHASES if phase != "canonical_component_clone_ns"
)
SUMMARY_PHASES = {
    SCOPES[0]: (
        "canonical_component_clone_ns",
        "geometry_preprocessing_ns",
        "chord_generation_ns",
        *KERNEL_LEAF_PHASES,
        "chord_selection_ns",
        "rectangle_completion_recovery_ns",
        "verification_ns",
    ),
    SCOPES[1]: KERNEL_LEAF_PHASES,
    SHARED_PREPROCESSING_SCOPE: ("geometry_preprocessing_ns", "chord_generation_ns"),
    SETUP_SCOPE: SETUP_PHASES,
}
V2_REQUIRED_SIZE_FIELDS = (
    "width",
    "height",
    "foreground_cells_n",
    "component_count",
    "bounding_box_area_a",
    "boundary_size_b",
    "boundary_unit_edge_count_u",
    "reflex_count",
    "horizontal_chord_count_h",
    "vertical_chord_count_v",
    "q",
    "explicit_conflict_edge_count_k",
    "compressed_representation_size_m",
    "biclique_count",
    "biclique_total_vertex_occurrences_sigma",
    "compressed_network_node_count",
    "compressed_network_arc_count",
    "optimum_rectangle_count",
    "output_rectangle_count",
)
BACKENDS = ("reference-edge-toggle", "prepared-exposed-edges")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    self_test_only = "--self-test" in sys.argv
    parser.add_argument("--input", type=Path, required=not self_test_only)
    parser.add_argument("--compare-input", type=Path)
    parser.add_argument("--comparison-config", type=Path)
    parser.add_argument("--summary-json", type=Path, required=not self_test_only)
    parser.add_argument("--summary-csv", type=Path, required=not self_test_only)
    parser.add_argument("--report", type=Path, required=not self_test_only)
    parser.add_argument("--tables", type=Path, required=not self_test_only)
    parser.add_argument("--figure-dir", type=Path, required=not self_test_only)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def root_path(path: Path) -> Path:
    return path if path.is_absolute() else ROOT / path


def require_mapping(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    return value


def require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise ValueError(f"{label} must be an array")
    return value


def require_nonnegative_integer(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValueError(f"{label} must be a nonnegative integer")
    return value


def require_optional_nonnegative_integer(value: Any, label: str) -> int | None:
    if value is None:
        return None
    return require_nonnegative_integer(value, label)


def backend_for_point(point: dict[str, Any], schema_version: int, protocol: dict[str, Any]) -> str:
    if schema_version == LEGACY_SCHEMA_VERSION:
        return "legacy-unspecified"
    backend = point.get("boundary_discovery_backend", protocol.get("boundary_discovery_backend"))
    if backend not in BACKENDS:
        raise ValueError(
            f"v2 point {point.get('point_identity', '<unknown>')} has invalid boundary_discovery_backend"
        )
    return str(backend)


def scope_leaf_phases(scope: str) -> tuple[str, ...]:
    if scope == SCOPES[0]:
        return LEAF_PHASES
    if scope == SCOPES[1]:
        return KERNEL_LEAF_PHASES
    raise ValueError(f"unknown timing scope: {scope}")


def accounting_scope_phases(scope: str) -> tuple[str, ...]:
    if scope == SCOPES[0]:
        return SCOPE_A_ACCOUNTING_PHASES
    if scope == SCOPES[1]:
        return KERNEL_LEAF_PHASES
    raise ValueError(f"unknown timing scope: {scope}")


def canonical_sample_identity(
    point: dict[str, Any],
    canonical_instance_identity: str,
    backend: str,
    scope: str,
    algorithm: str,
    iteration: int,
) -> str:
    scope_name = "scope-a" if scope == SCOPES[0] else "scope-b"
    return ":".join(
        (
            CAMPAIGN,
            f"v{CURRENT_SCHEMA_VERSION}",
            str(point["family"]),
            str(point["target_size"]),
            str(point["seed"]),
            canonical_instance_identity,
            "measured",
            scope_name,
            algorithm,
            str(iteration),
            backend,
        )
    )


def validate_nested_timing_groups(
    values: dict[str, Any], identity: str, groups: tuple[tuple[str, tuple[str, ...], str], ...]
) -> None:
    for parent, leaves, prefix_name in groups:
        accounting_fields = (
            f"{prefix_name}_leaf_sum_ns",
            f"{prefix_name}_unattributed_ns",
            f"{prefix_name}_accounting_ok",
        )
        for field in (parent, *leaves, *accounting_fields):
            if field not in values:
                raise ValueError(f"v2 timings for {identity} omit {field}")
        parent_value = values[parent]
        if parent_value is None:
            if any(values[field] is not None for field in accounting_fields):
                raise ValueError(f"v2 {prefix_name} accounting has no parent for {identity}")
            continue
        declared_nested = require_nonnegative_integer(
            values[accounting_fields[0]], f"{identity}.{accounting_fields[0]}"
        )
        unattributed_nested = require_nonnegative_integer(
            values[accounting_fields[1]], f"{identity}.{accounting_fields[1]}"
        )
        if values[accounting_fields[2]] is not True:
            raise ValueError(f"v2 {prefix_name} accounting is not valid for {identity}")
        computed_nested = sum(values[phase] or 0 for phase in leaves)
        if declared_nested != computed_nested or declared_nested + unattributed_nested != parent_value:
            raise ValueError(f"v2 {prefix_name} timing accounting mismatch for {identity}")


def validate_v2_timings(timings: Any, scope: str, identity: str) -> None:
    values = require_mapping(timings, f"timings for {identity}")
    for phase in set(COARSE_PHASES + LEAF_PHASES):
        if phase not in values:
            raise ValueError(f"v2 timings for {identity} omit {phase}")
        require_optional_nonnegative_integer(values[phase], f"{identity}.{phase}")

    prefix = "scope_a" if scope == SCOPES[0] else "scope_b"
    required_accounting = (
        f"{prefix}_total_ns",
        f"{prefix}_leaf_sum_ns",
        f"{prefix}_unattributed_ns",
        f"{prefix}_accounting_ok",
    )
    for field in required_accounting:
        if field not in values:
            raise ValueError(f"v2 timings for {identity} omit {field}")
    total = require_nonnegative_integer(values[required_accounting[0]], f"{identity}.{required_accounting[0]}")
    declared_leaf_sum = require_nonnegative_integer(
        values[required_accounting[1]], f"{identity}.{required_accounting[1]}"
    )
    unattributed = require_nonnegative_integer(
        values[required_accounting[2]], f"{identity}.{required_accounting[2]}"
    )
    if values[required_accounting[3]] is not True:
        raise ValueError(f"v2 timing accounting is not marked valid for {identity}")
    validate_nested_timing_groups(values, identity, NESTED_TIMING_GROUPS)
    computed_leaf_sum = sum(values[phase] or 0 for phase in accounting_scope_phases(scope))
    if declared_leaf_sum != computed_leaf_sum:
        raise ValueError(
            f"v2 leaf timing sum mismatch for {identity}: declared {declared_leaf_sum}, computed {computed_leaf_sum}"
        )
    if declared_leaf_sum + unattributed != total:
        raise ValueError(
            f"v2 scope timing accounting mismatch for {identity}: leaf {declared_leaf_sum} + "
            f"unattributed {unattributed} != total {total}"
        )


def validate_v2_point(
    point: dict[str, Any],
    protocol: dict[str, Any],
    global_sample_identities: set[str],
    *,
    require_complete: bool,
) -> None:
    identity = str(point["point_identity"])
    if point.get("schema_version") != CURRENT_SCHEMA_VERSION:
        raise ValueError(f"v2 point {identity} has incompatible point schema")
    if point.get("campaign") != CAMPAIGN:
        raise ValueError(f"v2 point {identity} has incompatible campaign")
    backend = backend_for_point(point, CURRENT_SCHEMA_VERSION, protocol)

    sizes = require_mapping(point.get("sizes"), f"sizes for {identity}")
    size_fields = V2_REQUIRED_SIZE_FIELDS if require_complete else tuple(sizes)
    for field in size_fields:
        if field not in sizes:
            raise ValueError(f"v2 sizes for {identity} omit {field}")
        require_nonnegative_integer(sizes[field], f"{identity}.sizes.{field}")
    if all(field in sizes for field in ("q", "horizontal_chord_count_h", "vertical_chord_count_v")) and sizes["q"] != sizes["horizontal_chord_count_h"] + sizes["vertical_chord_count_v"]:
        raise ValueError(f"v2 q != H + V for {identity}")
    if all(field in sizes for field in ("bounding_box_area_a", "foreground_cells_n", "width", "height")) and not (
        sizes["foreground_cells_n"] <= sizes["bounding_box_area_a"] <= sizes["width"] * sizes["height"]
    ):
        raise ValueError(f"v2 A is outside [N, width * height] for {identity}")
    if all(field in sizes for field in ("compressed_representation_size_m", "compressed_network_node_count", "compressed_network_arc_count")) and sizes["compressed_representation_size_m"] != (
        sizes["compressed_network_node_count"] + sizes["compressed_network_arc_count"]
    ):
        raise ValueError(f"v2 M != compact nodes + arcs for {identity}")
    if all(field in sizes for field in ("output_rectangle_count", "optimum_rectangle_count")) and sizes["output_rectangle_count"] != sizes["optimum_rectangle_count"]:
        raise ValueError(f"v2 output rectangle count != optimum for {identity}")

    setup_value = point.get("setup_timings")
    setup = require_mapping(setup_value, f"setup_timings for {identity}") if setup_value is not None else {}
    if require_complete:
        for phase in SETUP_PHASES + ("setup_total_ns",):
            if phase not in setup:
                raise ValueError(f"v2 setup timings for {identity} omit {phase}")
    for phase in setup:
        require_nonnegative_integer(setup[phase], f"{identity}.setup_timings.{phase}")
    if "setup_total_ns" in setup and sum(setup.get(phase, 0) for phase in SETUP_PHASES) > setup["setup_total_ns"]:
        raise ValueError(f"v2 setup leaf timings exceed setup total for {identity}")

    structure_value = point.get("structure")
    structure = require_mapping(structure_value, f"structure for {identity}") if structure_value is not None else {}
    structure_fields = (
        "boundary_candidate_edge_probes", "boundary_exposed_unit_edges", "boundary_trace_edge_visits",
        "explicit_graph_node_count", "explicit_graph_edge_count", "biclique_count", "biclique_incidence_sigma",
        "compact_node_count", "compact_arc_count", "explicit_c0_node_count", "explicit_c0_arc_count",
        "explicit_estimated_structural_bytes", "compact_estimated_structural_bytes",
        "explicit_c0_estimated_structural_bytes", "estimated_peak_structural_bytes",
        "completion_candidate_queries", "completion_candidate_revalidations", "completion_stale_candidates",
        "completion_ray_extension_unit_steps", "rectangle_recovery_cell_visits",
    )
    if require_complete:
        for field in structure_fields:
            if field not in structure:
                raise ValueError(f"v2 structure for {identity} omits {field}")
    for field in structure_fields:
        if field in structure:
            require_nonnegative_integer(structure[field], f"{identity}.structure.{field}")
    if all(field in sizes for field in ("foreground_cells_n", "boundary_unit_edge_count_u", "q", "explicit_conflict_edge_count_k", "biclique_count", "biclique_total_vertex_occurrences_sigma", "compressed_network_node_count", "compressed_network_arc_count")) and all(field in structure for field in structure_fields[:9]):
        equalities = {
            "boundary_candidate_edge_probes": 4 * sizes["foreground_cells_n"],
            "boundary_exposed_unit_edges": sizes["boundary_unit_edge_count_u"],
            "boundary_trace_edge_visits": sizes["boundary_unit_edge_count_u"],
            "explicit_graph_node_count": sizes["q"],
            "explicit_graph_edge_count": sizes["explicit_conflict_edge_count_k"],
            "biclique_count": sizes["biclique_count"],
            "biclique_incidence_sigma": sizes["biclique_total_vertex_occurrences_sigma"],
            "compact_node_count": sizes["compressed_network_node_count"],
            "compact_arc_count": sizes["compressed_network_arc_count"],
        }
        for field, expected in equalities.items():
            if structure.get(field) != expected:
                raise ValueError(f"v2 structure {field} differs from canonical size for {identity}")
    estimates = [structure.get(field) for field in ("explicit_estimated_structural_bytes", "compact_estimated_structural_bytes", "explicit_c0_estimated_structural_bytes")]
    if all(value is not None for value in estimates) and structure.get("estimated_peak_structural_bytes") != max(estimates):
        raise ValueError(f"v2 estimated peak structural bytes mismatch for {identity}")

    shared = point.get("shared_scope_b_preprocessing")
    if require_complete:
        shared = require_mapping(shared, f"shared_scope_b_preprocessing for {identity}")
        for field in ("geometry_preprocessing_ns", "chord_generation_ns"):
            require_nonnegative_integer(shared.get(field), f"{identity}.shared.{field}")
    if isinstance(shared, dict):
        validate_nested_timing_groups(
            shared,
            f"shared preprocessing {identity}",
            NESTED_TIMING_GROUPS[:3],
        )

    correctness = require_list(point.get("correctness"), f"correctness for {identity}")
    correctness_algorithms = [row.get("algorithm") for row in correctness if isinstance(row, dict)]
    if sorted(correctness_algorithms) != sorted(ALGORITHMS):
        raise ValueError(f"v2 correctness census mismatch for {identity}")
    if any(row.get("outcome") != "success" for row in correctness):
        raise ValueError(f"v2 correctness failure for {identity}")
    if any(row.get("boundary_discovery_backend") != backend for row in correctness):
        raise ValueError(f"v2 correctness backend mismatch for {identity}")

    warmups = require_list(point.get("warmups"), f"warmups for {identity}")
    warmup_by_key: dict[tuple[str, str], dict[str, Any]] = {}
    for warmup in warmups:
        row = require_mapping(warmup, f"warmup for {identity}")
        key = (row.get("scope"), row.get("algorithm"))
        if key in warmup_by_key:
            raise ValueError(f"duplicate v2 warmup key {key} for {identity}")
        if row.get("boundary_discovery_backend") != backend:
            raise ValueError(f"v2 warmup backend mismatch for {identity}")
        warmup_by_key[key] = row
    expected_keys = {(scope, algorithm) for scope in SCOPES for algorithm in ALGORITHMS}
    if require_complete and set(warmup_by_key) != expected_keys:
        raise ValueError(f"v2 warmup census mismatch for {identity}")

    runs = require_list(point.get("runs"), f"runs for {identity}")
    runs_by_key: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    ordered_identities: list[str] = []
    canonical_instance_identity = point.get("canonical_instance_identity")
    if not isinstance(canonical_instance_identity, str) or not canonical_instance_identity:
        raise ValueError(f"v2 complete point {identity} has no canonical instance identity")
    for index, run_value in enumerate(runs):
        run = require_mapping(run_value, f"run {index} for {identity}")
        sample_identity = run.get("sample_identity")
        if not isinstance(sample_identity, str) or not sample_identity:
            raise ValueError(f"v2 run {index} for {identity} has no sample identity")
        if sample_identity in global_sample_identities:
            raise ValueError(f"duplicate v2 sample identity: {sample_identity}")
        global_sample_identities.add(sample_identity)
        ordered_identities.append(sample_identity)
        key = (run.get("scope"), run.get("algorithm"))
        if key not in expected_keys:
            raise ValueError(f"v2 run {sample_identity} has invalid scope/algorithm")
        if run.get("boundary_discovery_backend") != backend:
            raise ValueError(f"v2 run {sample_identity} has inconsistent boundary backend")
        if run.get("record_kind") != "measured":
            raise ValueError(f"v2 run {sample_identity} is not marked measured")
        if run.get("seed") != point.get("seed"):
            raise ValueError(f"v2 run {sample_identity} has inconsistent seed")
        if run.get("canonical_instance_identity") != canonical_instance_identity:
            raise ValueError(f"v2 run {sample_identity} has inconsistent canonical identity")
        iteration = require_nonnegative_integer(run.get("iteration"), f"{sample_identity}.iteration")
        elapsed_ns = require_nonnegative_integer(run.get("elapsed_ns"), f"{sample_identity}.elapsed_ns")
        expected_identity = canonical_sample_identity(
            point,
            canonical_instance_identity,
            backend,
            str(run["scope"]),
            str(run["algorithm"]),
            iteration,
        )
        if sample_identity != expected_identity:
            raise ValueError(f"v2 run {sample_identity} is not its canonical sample identity")
        validate_v2_timings(run.get("timings"), str(run["scope"]), sample_identity)
        total_field = "scope_a_total_ns" if run["scope"] == SCOPES[0] else "scope_b_total_ns"
        if run["timings"].get(total_field) != elapsed_ns:
            raise ValueError(f"v2 run {sample_identity} elapsed/total mismatch")
        runs_by_key[key].append(run)

    for key in sorted(warmup_by_key):
        warmup = warmup_by_key[key]
        count = require_nonnegative_integer(
            warmup.get("measured_repetitions"), f"{identity}.{key}.measured_repetitions"
        )
        if count <= 0:
            raise ValueError(f"v2 measured repetition count must be positive for {identity} {key}")
        iterations = sorted(run["iteration"] for run in runs_by_key.get(key, []))
        if require_complete and iterations != list(range(count)):
            raise ValueError(f"v2 measured sample census mismatch for {identity} {key}")

    exact_order = require_list(point.get("exact_measured_order"), f"exact_measured_order for {identity}")
    if exact_order != ordered_identities:
        raise ValueError(f"v2 exact measured order mismatch for {identity}")


def validate_v2(raw: dict[str, Any]) -> None:
    protocol = require_mapping(raw.get("protocol"), "protocol")
    if protocol.get("schema_version") != CURRENT_SCHEMA_VERSION or protocol.get("campaign") != CAMPAIGN:
        raise ValueError("v2 protocol schema/campaign mismatch")
    if protocol.get("algorithms") != list(ALGORITHMS) or protocol.get("scopes") != list(SCOPES):
        raise ValueError("v2 protocol algorithm/scope census mismatch")
    if protocol.get("boundary_discovery_backend") not in BACKENDS:
        raise ValueError("v2 protocol has invalid boundary_discovery_backend")

    planned = require_list(raw.get("planned_points"), "planned_points")
    points = require_list(raw.get("point_results"), "point_results")
    planned_ids = [row.get("point_identity") for row in planned if isinstance(row, dict)]
    observed_ids = [row.get("point_identity") for row in points if isinstance(row, dict)]
    if len(planned_ids) != len(planned) or any(not isinstance(value, str) or not value for value in planned_ids):
        raise ValueError("v2 plan contains a malformed point identity")
    if len(set(planned_ids)) != len(planned_ids):
        raise ValueError("v2 plan contains duplicate point identities")
    if len(observed_ids) != len(points) or len(set(observed_ids)) != len(observed_ids):
        raise ValueError("v2 result contains malformed or duplicate point identities")
    if set(observed_ids) != set(planned_ids):
        raise ValueError("v2 campaign is incomplete: planned and observed point sets differ")

    completion = require_mapping(raw.get("completion"), "completion")
    if completion.get("complete") is not True:
        raise ValueError("v2 campaign is incomplete")
    expected_count = len(planned_ids)
    if any(
        completion.get(field) != value
        for field, value in (
            ("planned_point_count", expected_count),
            ("observed_point_count", expected_count),
            ("completed_point_count", expected_count),
            ("missing_point_count", 0),
            ("correctness_failure_count", 0),
        )
    ) or completion.get("missing_point_identities") != []:
        raise ValueError("v2 completion census is inconsistent")

    planned_by_id = {row["point_identity"]: row for row in planned}
    state_counts: dict[str, int] = defaultdict(int)
    sample_identities: set[str] = set()
    for point_value in points:
        point = require_mapping(point_value, "point result")
        identity = point["point_identity"]
        state = point.get("state")
        if state not in {"complete", "stopped"}:
            raise ValueError(f"v2 campaign has non-terminal or invalid state {state!r} for {identity}")
        state_counts[str(state)] += 1
        planned_point = require_mapping(planned_by_id[identity], f"planned point {identity}")
        for field in ("family", "target_size", "seed"):
            if point.get(field) != planned_point.get(field):
                raise ValueError(f"v2 planned/result {field} mismatch for {identity}")
        if backend_for_point(point, CURRENT_SCHEMA_VERSION, protocol) != protocol["boundary_discovery_backend"]:
            raise ValueError(f"v2 point backend differs from protocol for {identity}")
        propagated = point.get("stop_propagated_from_target_size")
        if state == "complete":
            validate_v2_point(
                point, protocol, sample_identities, require_complete=True
            )
        else:
            if propagated is not None:
                if not isinstance(propagated, int) or propagated >= point["target_size"]:
                    raise ValueError(f"v2 propagated stop has invalid source for {identity}")
                if any(point.get(field) for field in ("runs", "warmups", "correctness")):
                    raise ValueError(f"v2 propagated stop contains measurements for {identity}")
            else:
                validate_v2_point(
                    point, protocol, sample_identities, require_complete=False
                )

    if completion.get("terminal_state_counts") != dict(state_counts):
        raise ValueError("v2 terminal-state census is inconsistent")


def validate_and_describe_input(raw_value: Any) -> dict[str, Any]:
    raw = require_mapping(raw_value, "benchmark input")
    schema_version = raw.get("schema_version")
    if schema_version not in {LEGACY_SCHEMA_VERSION, CURRENT_SCHEMA_VERSION}:
        raise ValueError(
            f"paper-kernel-scaling schema version must be 1 or {CURRENT_SCHEMA_VERSION}; found {schema_version!r}"
        )
    if raw.get("campaign") != CAMPAIGN:
        raise ValueError(f"campaign must be {CAMPAIGN}")
    require_mapping(raw.get("protocol"), "protocol")
    require_list(raw.get("point_results"), "point_results")
    if schema_version == CURRENT_SCHEMA_VERSION:
        validate_v2(raw)
        return {
            "input_schema_version": CURRENT_SCHEMA_VERSION,
            "normalization": "native-v2-fine-phases",
            "legacy_input": False,
            "fine_phase_status": "available",
            "legacy_m_definition": None,
        }
    return {
        "input_schema_version": LEGACY_SCHEMA_VERSION,
        "normalization": "legacy-v1-coarse",
        "legacy_input": True,
        "fine_phase_status": "unavailable-null",
        "legacy_m_definition": "structure.compact_node_count + structure.compact_arc_count",
    }


def quantile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    location = (len(ordered) - 1) * fraction
    lower = math.floor(location)
    upper = math.ceil(location)
    if lower == upper:
        return ordered[lower]
    weight = location - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight


def distribution(values: list[float]) -> dict[str, Any]:
    median = statistics.median(values)
    mad = statistics.median(abs(value - median) for value in values)
    mean = statistics.fmean(values)
    return {
        "n": len(values),
        "min": min(values),
        "q1": quantile(values, 0.25),
        "median": median,
        "q3": quantile(values, 0.75),
        "max": max(values),
        "mad": mad,
        "coefficient_of_variation": statistics.pstdev(values) / mean if mean else 0.0,
    }


def bootstrap_statistic(values: list[float], statistic: Callable[[list[float]], float], count: int, seed: int) -> list[float]:
    generator = random.Random(seed)
    return [
        statistic([values[generator.randrange(len(values))] for _ in values])
        for _ in range(count)
    ]


def bootstrap_ci(values: list[float], statistic: Callable[[list[float]], float], count: int, seed: int) -> list[float]:
    samples = bootstrap_statistic(values, statistic, count, seed)
    return [quantile(samples, 0.025), quantile(samples, 0.975)]


def bootstrap_median_ci(values: list[float], count: int, seed: int) -> list[float]:
    """Sample the exact empirical-bootstrap median order statistics in O(count)."""
    ordered = sorted(values)
    size = len(ordered)
    generator = random.Random(seed)
    samples = []
    if size % 2:
        order = size // 2 + 1
        for _ in range(count):
            probability = generator.betavariate(order, size + 1 - order)
            samples.append(ordered[min(size - 1, int(probability * size))])
    else:
        lower_order = size // 2
        for _ in range(count):
            lower_probability = generator.betavariate(
                lower_order, size + 1 - lower_order
            )
            upper_probability = lower_probability + (1 - lower_probability) * generator.betavariate(
                1, size - lower_order
            )
            lower = ordered[min(size - 1, int(lower_probability * size))]
            upper = ordered[min(size - 1, int(upper_probability * size))]
            samples.append((lower + upper) / 2)
    return [quantile(samples, 0.025), quantile(samples, 0.975)]


def geometric_mean(values: list[float]) -> float:
    return math.exp(statistics.fmean(math.log(value) for value in values))


def ols(points: list[tuple[float, float]]) -> dict[str, float]:
    x_mean = statistics.fmean(x for x, _ in points)
    y_mean = statistics.fmean(y for _, y in points)
    denominator = sum((x - x_mean) ** 2 for x, _ in points)
    slope = sum((x - x_mean) * (y - y_mean) for x, y in points) / denominator
    intercept = y_mean - slope * x_mean
    residual = sum((y - (intercept + slope * x)) ** 2 for x, y in points)
    total = sum((y - y_mean) ** 2 for _, y in points)
    return {"slope": slope, "intercept": intercept, "r_squared": 1 - residual / total if total else 1.0}


def theil_sen(points: list[tuple[float, float]]) -> float:
    slopes = [
        (right_y - left_y) / (right_x - left_x)
        for index, (left_x, left_y) in enumerate(points)
        for right_x, right_y in points[index + 1 :]
        if right_x != left_x
    ]
    return statistics.median(slopes)


def bootstrap_slope(points: list[tuple[float, float]], count: int, seed: int) -> list[float] | None:
    generator = random.Random(seed)
    slopes: list[float] = []
    for _ in range(count):
        sampled = [points[generator.randrange(len(points))] for _ in points]
        if len({x for x, _ in sampled}) < 2:
            continue
        slopes.append(ols(sampled)["slope"])
    return [quantile(slopes, 0.025), quantile(slopes, 0.975)] if slopes else None


def point_sizes(point: dict[str, Any], schema_version: int) -> dict[str, Any]:
    sizes = dict(point.get("sizes", {}))
    if schema_version == LEGACY_SCHEMA_VERSION:
        structure = point.get("structure", {})
        nodes = structure.get("compact_node_count")
        arcs = structure.get("compact_arc_count")
        sizes["compressed_representation_size_m"] = (
            nodes + arcs if nodes is not None and arcs is not None else None
        )
    return sizes


def normalized_run_timings(timings: Any, schema_version: int) -> dict[str, Any]:
    values = dict(timings or {})
    if schema_version == LEGACY_SCHEMA_VERSION:
        for phase in LEAF_PHASES:
            values[phase] = None
    return values


def flatten(raw: dict[str, Any], schema_version: int) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    protocol = raw["protocol"]
    for point in raw.get("point_results", []):
        sizes = point_sizes(point, schema_version)
        backend = backend_for_point(point, schema_version, protocol)
        warmups = {
            (row["scope"], row["algorithm"]): row
            for row in point.get("warmups", [])
        }
        for run in point.get("runs", []):
            row = {
                **run,
                "family": point["family"],
                "target_size": point["target_size"],
                "point_state": point["state"],
                "sizes": sizes,
                "structure": point.get("structure", {}),
                "warmup": warmups.get((run["scope"], run["algorithm"]), {}),
                "boundary_discovery_backend": backend,
                "timings": normalized_run_timings(run.get("timings"), schema_version),
                "legacy_input": schema_version == LEGACY_SCHEMA_VERSION,
            }
            rows.append(row)
    return rows


def distribution_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    groups: dict[tuple[str, str, int, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        if row["point_state"] != "complete":
            continue
        groups[(row["boundary_discovery_backend"], row["family"], row["target_size"], row["scope"], row["algorithm"])].append(row)
    result = []
    for (backend, family, size, scope, algorithm), group in sorted(groups.items()):
        result.append(
            {
                "boundary_discovery_backend": backend,
                "family": family,
                "target_size": size,
                "scope": scope,
                "algorithm": algorithm,
                "elapsed_ns": distribution([float(row["elapsed_ns"]) for row in group]),
                "warmup_count": group[0].get("warmup", {}).get("count"),
                "warmup_converged": group[0].get("warmup", {}).get("converged"),
                "state": group[0]["point_state"],
                "sizes": group[0]["sizes"],
            }
        )
    return result


def paired_rows(rows: list[dict[str, Any]], config: dict[str, Any]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str, int, str, str], dict[int, dict[str, Any]]] = defaultdict(dict)
    for row in rows:
        if row["point_state"] != "complete":
            continue
        grouped[(row["boundary_discovery_backend"], row["family"], row["target_size"], row["scope"], row["algorithm"])][row["iteration"]] = row
    bootstrap_count = int(config["fit_rule"]["bootstrap_resamples"])
    seed = int(config["fit_rule"]["bootstrap_seed"])
    result = []
    backend_family_sizes = sorted(
        {
            (row["boundary_discovery_backend"], row["family"], row["target_size"])
            for row in rows
            if row["point_state"] == "complete"
        }
    )
    for backend, family, size in backend_family_sizes:
        for scope in SCOPES:
            for explicit in EXPLICIT:
                    ratios = []
                    compact_rows = grouped.get((backend, family, size, scope, "compact-mrd"), {})
                    reference_rows = grouped.get((backend, family, size, scope, explicit), {})
                    iterations = sorted(set(compact_rows) & set(reference_rows))
                    for iteration in iterations:
                        compact = compact_rows.get(iteration)
                        reference = reference_rows.get(iteration)
                        if compact is not None and reference is not None:
                            ratios.append(float(compact["elapsed_ns"]) / float(reference["elapsed_ns"]))
                    if not ratios:
                        continue
                    result.append(
                        {
                            "boundary_discovery_backend": backend,
                            "family": family,
                            "target_size": size,
                            "scope": scope,
                            "explicit_algorithm": explicit,
                            "ratios": distribution(ratios),
                            "geometric_mean_ratio": geometric_mean(ratios),
                            "median_ratio_ci95": bootstrap_median_ci(ratios, bootstrap_count, seed ^ size),
                            "q": compact_rows[iterations[0]]["sizes"].get("q"),
                        }
                    )
    return result


def classifications(paired: list[dict[str, Any]], config: dict[str, Any]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in paired:
        grouped[(row["boundary_discovery_backend"], row["family"], row["scope"], row["explicit_algorithm"])].append(row)
    required = int(config["fit_rule"]["minimum_valid_size_levels"])
    result = []
    for (backend, family, scope, explicit), levels in sorted(grouped.items()):
        levels.sort(key=lambda row: row["target_size"])
        all_ratios = [value for level in levels for value in [level["ratios"]["median"]]]
        aggregate_ci = bootstrap_ci(
            all_ratios,
            statistics.median,
            int(config["fit_rule"]["bootstrap_resamples"]),
            int(config["fit_rule"]["bootstrap_seed"]),
        )
        if len(levels) < required:
            classification = "insufficient"
        elif aggregate_ci[1] < 1:
            classification = "compact-clearly-faster"
        elif aggregate_ci[0] > 1:
            classification = "compact-clearly-slower"
        else:
            classification = "unresolved"
        crossover = None
        if len(levels) >= required:
            for index in range(len(levels) - 2):
                suffix = levels[index:]
                first_three = suffix[:3]
                if all(level["ratios"]["median"] < 1 for level in first_three) and sum(
                    level["median_ratio_ci95"][1] < 1 for level in first_three
                ) >= 2:
                    crossover = first_three[0]["target_size"]
                    break
        result.append(
            {
                "boundary_discovery_backend": backend,
                "family": family,
                "scope": scope,
                "explicit_algorithm": explicit,
                "valid_size_levels": len(levels),
                "classification": classification,
                "aggregate_median_ratio": statistics.median(all_ratios),
                "aggregate_median_ratio_ci95": aggregate_ci,
                "stable_crossover_target_size": crossover,
            }
        )
    return result


def level_status_rows(raw: dict[str, Any], schema_version: int) -> list[dict[str, Any]]:
    protocol = raw["protocol"]
    observed = {
        point.get("point_identity"): point
        for point in raw.get("point_results", [])
        if point.get("point_identity") is not None
    }
    result = []
    for planned in raw.get("planned_points", []):
        point = observed.get(planned.get("point_identity"))
        source = point if point is not None else planned
        result.append(
            {
                "boundary_discovery_backend": backend_for_point(source, schema_version, protocol),
                "family": planned["family"],
                "target_size": planned["target_size"],
                "state": point.get("state", "missing") if point is not None else "missing",
                "message": point.get("message") if point is not None else "planned point was not observed",
                "stop_propagated_from_target_size": (
                    point.get("stop_propagated_from_target_size") if point is not None else None
                ),
            }
        )
    if result:
        return sorted(
            result,
            key=lambda row: (
                row["boundary_discovery_backend"],
                row["family"],
                row["target_size"],
            ),
        )
    for point in raw.get("point_results", []):
        result.append(
            {
                "boundary_discovery_backend": backend_for_point(point, schema_version, protocol),
                "family": point["family"],
                "target_size": point["target_size"],
                "state": point.get("state", "missing"),
                "message": point.get("message"),
                "stop_propagated_from_target_size": point.get("stop_propagated_from_target_size"),
            }
        )
    return sorted(
        result,
        key=lambda row: (
            row["boundary_discovery_backend"],
            row["family"],
            row["target_size"],
        ),
    )


def fit_one_group(
    *,
    backend: str,
    family: str,
    scope: str,
    algorithm: str,
    phase: str,
    levels: list[dict[str, Any]],
    statuses: list[dict[str, Any]],
    variable: str,
    field: str,
    dependent: Callable[[dict[str, Any]], float | None],
    minimum: int,
    count: int,
    seed: int,
) -> dict[str, Any]:
    by_target = {row["target_size"]: row for row in levels}
    relevant_statuses = [
        row
        for row in statuses
        if row["boundary_discovery_backend"] == backend and row["family"] == family
    ]
    excluded = []
    valid: list[dict[str, Any]] = []
    for status in sorted(relevant_statuses, key=lambda row: row["target_size"]):
        target = status["target_size"]
        level = by_target.get(target)
        reason = None
        if status["state"] == "stopped":
            reason = "censored-stopped"
        elif status["state"] != "complete":
            reason = f"ineligible-state-{status['state']}"
        elif level is None:
            reason = "missing-phase-or-algorithm-measurement"
        elif level["sizes"].get(field) in (None, 0):
            reason = f"nonpositive-or-missing-{variable}"
        elif dependent(level) in (None, 0):
            reason = "nonpositive-or-missing-dependent-measurement"
        if reason is None:
            valid.append(level)
        else:
            excluded.append(
                {
                    "target_size": target,
                    "state": status["state"],
                    "reason": reason,
                    "message": status.get("message"),
                    "stop_propagated_from_target_size": status.get(
                        "stop_propagated_from_target_size"
                    ),
                }
            )

    # Legacy files without an explicit plan still retain every observed complete level.
    known_targets = {row["target_size"] for row in relevant_statuses}
    for level in levels:
        if level["target_size"] in known_targets:
            continue
        value = level["sizes"].get(field)
        measured = dependent(level)
        if value not in (None, 0) and measured not in (None, 0):
            valid.append(level)

    valid.sort(key=lambda row: row["target_size"])
    distinct_variables = len({level["sizes"][field] for level in valid})
    base = {
        "boundary_discovery_backend": backend,
        "family": family,
        "scope": scope,
        "algorithm": algorithm,
        "phase": phase,
        "independent_variable": variable,
        "minimum_valid_size_levels": minimum,
        "candidate_size_levels": len(relevant_statuses) or len(levels),
        "complete_size_levels": sum(row["state"] == "complete" for row in relevant_statuses),
        "valid_size_levels": len(valid),
        "distinct_variable_levels": distinct_variables,
        "fit_target_sizes": [level["target_size"] for level in valid],
        "excluded_target_sizes": [row["target_size"] for row in excluded],
        "censored_target_sizes": [
            row["target_size"] for row in excluded if row["reason"] == "censored-stopped"
        ],
        "excluded_levels": excluded,
    }
    if len(valid) < minimum:
        return {
            **base,
            "ols_slope": None,
            "ols_slope_ci95": None,
            "r_squared": None,
            "theil_sen_slope": None,
            "status": "insufficient-valid-size-levels",
        }
    if distinct_variables < 2:
        return {
            **base,
            "ols_slope": None,
            "ols_slope_ci95": None,
            "r_squared": None,
            "theil_sen_slope": None,
            "status": "insufficient-distinct-variable-levels",
        }
    points = [
        (
            math.log(float(level["sizes"][field])),
            math.log(float(dependent(level))),
        )
        for level in valid
    ]
    fitted = ols(points)
    return {
        **base,
        "ols_slope": fitted["slope"],
        "ols_slope_ci95": bootstrap_slope(points, count, seed),
        "r_squared": fitted["r_squared"],
        "theil_sen_slope": theil_sen(points),
        "status": "estimated",
    }


def fit_rows(
    distributions: list[dict[str, Any]], config: dict[str, Any], statuses: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in distributions:
        grouped[(row["boundary_discovery_backend"], row["family"], row["scope"], row["algorithm"])].append(row)
    minimum = max(6, int(config["fit_rule"]["minimum_valid_size_levels"]))
    count = int(config["fit_rule"]["bootstrap_resamples"])
    seed = int(config["fit_rule"]["bootstrap_seed"])
    result = []
    for (backend, family, scope, algorithm), levels in sorted(grouped.items()):
        for variable, field in VARIABLES.items():
            result.append(
                fit_one_group(
                    backend=backend,
                    family=family,
                    scope=scope,
                    algorithm=algorithm,
                    phase="total_elapsed_ns",
                    levels=levels,
                    statuses=statuses,
                    variable=variable,
                    field=field,
                    dependent=lambda level: float(level["elapsed_ns"]["median"]),
                    minimum=minimum,
                    count=count,
                    seed=seed,
                )
            )
    return result


def phase_rows(
    raw: dict[str, Any], rows: list[dict[str, Any]], schema_version: int
) -> list[dict[str, Any]]:
    groups: dict[tuple[str, str, int, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        if row["point_state"] != "complete":
            continue
        groups[(row["boundary_discovery_backend"], row["family"], row["target_size"], row["scope"], row["algorithm"])].append(row)
    result = []
    for (backend, family, size, scope, algorithm), group in sorted(groups.items()):
        medians = {phase: None for phase in ALL_ANALYZED_PHASES}
        coarse_medians = {}
        for phase in LEAF_PHASES:
            values = [float(row["timings"][phase]) for row in group if row["timings"].get(phase) is not None]
            medians[phase] = statistics.median(values) if values else None
        for phase in COARSE_PHASES:
            values = [float(row["timings"][phase]) for row in group if row["timings"].get(phase) is not None]
            coarse_medians[phase] = statistics.median(values) if values else None
        accounting_prefix = "scope_a" if scope == SCOPES[0] else "scope_b"
        unattributed_values = [
            float(row["timings"][f"{accounting_prefix}_unattributed_ns"])
            for row in group
            if row["timings"].get(f"{accounting_prefix}_unattributed_ns") is not None
        ]
        result.append({
            "boundary_discovery_backend": backend,
            "family": family,
            "target_size": size,
            "scope": scope,
            "algorithm": algorithm,
            "sizes": group[0]["sizes"],
            "phase_medians_ns": medians,
            "coarse_phase_medians_ns": coarse_medians,
            "sample_count": len(group),
            "unattributed_median_ns": statistics.median(unattributed_values) if unattributed_values else None,
            "legacy_input": schema_version == LEGACY_SCHEMA_VERSION,
        })

    if schema_version == CURRENT_SCHEMA_VERSION:
        protocol = raw["protocol"]
        for point in raw.get("point_results", []):
            if point.get("state") != "complete":
                continue
            medians = {phase: None for phase in ALL_ANALYZED_PHASES}
            for phase in SETUP_PHASES:
                medians[phase] = float(point["setup_timings"][phase])
            result.append(
                {
                    "boundary_discovery_backend": backend_for_point(point, schema_version, protocol),
                    "family": point["family"],
                    "target_size": point["target_size"],
                    "scope": SETUP_SCOPE,
                    "algorithm": SHARED_ALGORITHM,
                    "sizes": point_sizes(point, schema_version),
                    "phase_medians_ns": medians,
                    "coarse_phase_medians_ns": {},
                    "sample_count": 1,
                    "unattributed_median_ns": float(
                        point["setup_timings"]["setup_total_ns"]
                        - sum(point["setup_timings"][phase] for phase in SETUP_PHASES)
                    ),
                    "legacy_input": False,
                }
            )
            shared = point.get("shared_scope_b_preprocessing", {})
            if isinstance(shared, dict) and shared:
                # Keep the enclosing geometry/chord totals in the normalized
                # phase map as well as in coarse_phase_medians_ns.  The
                # shared scope has no per-algorithm run timings, so these
                # point-level parent values are its only complete accounting
                # of the common preprocessing work.
                shared_medians = {phase: None for phase in ALL_ANALYZED_PHASES}
                for phase in (
                    *SHARED_PREPROCESSING_PHASES,
                    "geometry_preprocessing_ns",
                    "chord_generation_ns",
                ):
                    shared_medians[phase] = (
                        float(shared[phase]) if shared.get(phase) is not None else None
                    )
                result.append(
                    {
                        "boundary_discovery_backend": backend_for_point(
                            point, schema_version, protocol
                        ),
                        "family": point["family"],
                        "target_size": point["target_size"],
                        "scope": SHARED_PREPROCESSING_SCOPE,
                        "algorithm": SHARED_ALGORITHM,
                        "sizes": point_sizes(point, schema_version),
                        "phase_medians_ns": shared_medians,
                        "coarse_phase_medians_ns": {
                            phase: float(shared[phase])
                            for phase in ("geometry_preprocessing_ns", "chord_generation_ns")
                            if shared.get(phase) is not None
                        },
                        "sample_count": 1,
                        "unattributed_median_ns": None,
                        "legacy_input": False,
                    }
                )
    return result


def phase_fit_rows(
    phases: list[dict[str, Any]], config: dict[str, Any], statuses: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in phases:
        grouped[(row["boundary_discovery_backend"], row["family"], row["scope"], row["algorithm"])].append(row)
    minimum = max(6, int(config["fit_rule"]["minimum_valid_size_levels"]))
    count = int(config["fit_rule"]["bootstrap_resamples"])
    seed = int(config["fit_rule"]["bootstrap_seed"])
    result = []
    for (backend, family, scope, algorithm), levels in sorted(grouped.items()):
        if scope == SETUP_SCOPE:
            relevant_phases = SETUP_PHASES
        elif scope == SHARED_PREPROCESSING_SCOPE:
            relevant_phases = SHARED_PREPROCESSING_PHASES
        else:
            relevant_phases = scope_leaf_phases(scope)
        for phase in relevant_phases:
            if not any(level["phase_medians_ns"].get(phase) is not None for level in levels):
                continue
            for variable, field in VARIABLES.items():
                result.append(
                    fit_one_group(
                        backend=backend,
                        family=family,
                        scope=scope,
                        algorithm=algorithm,
                        phase=phase,
                        levels=levels,
                        statuses=statuses,
                        variable=variable,
                        field=field,
                        dependent=lambda level, phase=phase: level["phase_medians_ns"].get(phase),
                        minimum=minimum,
                        count=count,
                        seed=seed,
                    )
                )
    return result


def coverage(raw: dict[str, Any], rows: list[dict[str, Any]]) -> dict[str, Any]:
    points = raw.get("point_results", [])
    checks = [check for point in points for check in point.get("correctness", [])]
    mismatches = [point for point in points if point.get("state") == "invalid"]
    identities = [row["sample_identity"] for row in rows]
    censored = [
        {
            "family": point.get("family"),
            "target_size": point.get("target_size"),
            "message": point.get("message"),
            "stop_propagated_from_target_size": point.get("stop_propagated_from_target_size"),
        }
        for point in points
        if point.get("state") == "stopped"
    ]
    return {
        "planned_points": raw.get("completion", {}).get("planned_point_count"),
        "observed_points": len(points),
        "complete_points": sum(point.get("state") == "complete" for point in points),
        "stopped_points": sum(point.get("state") == "stopped" for point in points),
        "analysis_eligible_points": sum(point.get("state") == "complete" for point in points),
        "invalid_points": len(mismatches),
        "runner_failures": sum(point.get("state") in {"runner-error", "runner-timeout"} for point in points),
        "measured_iterations": len(rows),
        "correctness_checks": len(checks),
        "correctness_failures": sum(check.get("outcome") != "success" for check in checks),
        "duplicate_sample_identities": len(identities) - len(set(identities)),
        "missing_planned_points": raw.get("completion", {}).get("missing_point_count"),
        "censored_levels": censored,
    }


def structural_compression(distributions: list[dict[str, Any]]) -> list[dict[str, Any]]:
    result = []
    for backend, family in sorted(
        {(row["boundary_discovery_backend"], row["family"]) for row in distributions}
    ):
        levels = [
            row
            for row in distributions
            if row["boundary_discovery_backend"] == backend
            if row["family"] == family
            and row["algorithm"] == "compact-mrd"
            and row["scope"] == SCOPES[0]
        ]
        if not levels:
            continue
        level = max(levels, key=lambda row: row["target_size"])
        k_value = level["sizes"].get("explicit_conflict_edge_count_k")
        m_value = level["sizes"].get("compressed_representation_size_m")
        result.append(
            {
                "boundary_discovery_backend": backend,
                "family": family,
                "target_size": level["target_size"],
                "q": level["sizes"].get("q"),
                "K": k_value,
                "M": m_value,
                "K_over_M": k_value / m_value if k_value and m_value else None,
            }
        )
    return result


def phase_conclusions(phases: list[dict[str, Any]]) -> list[dict[str, Any]]:
    result = []
    grouped: dict[tuple[str, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in phases:
        grouped[(row["boundary_discovery_backend"], row["family"], row["scope"], row["algorithm"])].append(row)
    for (backend, family, scope, algorithm), group in sorted(grouped.items()):
        row = max(group, key=lambda item: item["target_size"])
        relevant = SUMMARY_PHASES[scope]
        values = {
            phase: row["phase_medians_ns"].get(phase)
            for phase in relevant
            if row["phase_medians_ns"].get(phase) is not None
        }
        total = sum(values.values())
        dominant = max(values, key=values.get) if values else None
        dominant_value = values.get(dominant) if dominant else None
        unattributed = row.get("unattributed_median_ns")
        result.append(
            {
                "boundary_discovery_backend": backend,
                "family": family,
                "target_size": row["target_size"],
                "scope": scope,
                "algorithm": algorithm,
                "dominant_phase": dominant,
                "dominant_phase_median_ns": dominant_value,
                "dominant_phase_share": dominant_value / total if dominant_value is not None and total else None,
                "accounted_leaf_median_sum_ns": total if values else None,
                "unattributed_median_ns": unattributed,
                "unattributed_exceeds_dominant_leaf": (
                    unattributed > dominant_value
                    if unattributed is not None and dominant_value is not None
                    else None
                ),
                "status": "available" if dominant is not None else "fine-phases-unavailable",
            }
        )
    return result


def phase_operation(phase: str | None) -> str | None:
    if phase is None:
        return None
    if phase in {
        "boundary_edge_discovery_ns",
        "boundary_adjacency_build_ns",
        "boundary_loop_tracing_ns",
        "boundary_loop_normalization_ns",
        "boundary_unit_edge_sort_ns",
        "boundary_index_construction_ns",
        "reflex_detection_ns",
        "reflex_grouping_ns",
    }:
        return "boundary-preparation"
    if phase in {"horizontal_chord_generation_ns", "vertical_chord_generation_ns", "chord_validation_filtering_ns"}:
        return "chord-generation-and-filtering"
    if phase in {"endpoint_index_construction_ns", "conflict_discovery_ns"}:
        return "indexed-or-pairwise-conflict-discovery"
    if phase in {
        "selected_cut_materialization_ns",
        "horizontal_completion_ns",
        "vertical_completion_ns",
        "rectangle_reconstruction_ns",
    }:
        return "rectangle-reconstruction"
    if phase == "output_validation_ns":
        return "output-validation"
    if phase in {"canonical_component_clone_ns", "prepared_component_build_ns"}:
        return "geometry-copying-or-index-preparation"
    if phase in KERNEL_LEAF_PHASES:
        return "representation-or-solver-kernel"
    if phase in SETUP_PHASES:
        return "campaign-input-setup"
    return "other-measured-operation"


def fixed_overhead_assessment(fit: dict[str, Any] | None) -> str:
    if fit is None or fit.get("status") != "estimated":
        return "insufficient-evidence"
    slope = float(fit["ols_slope"])
    r_squared = float(fit["r_squared"])
    if r_squared >= 0.8 and slope >= 0.5:
        return "size-associated-in-measured-range"
    if abs(slope) <= 0.25:
        return "weak-size-dependence-consistent-with-fixed-cost-over-measured-range"
    return "fixed-versus-size-dependent-unresolved"


def diagnostic_rows(
    conclusions: list[dict[str, Any]], phase_fits: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    result = []
    for conclusion in conclusions:
        phase = conclusion["dominant_phase"]
        candidates = [
            fit
            for fit in phase_fits
            if fit["boundary_discovery_backend"] == conclusion["boundary_discovery_backend"]
            and fit["family"] == conclusion["family"]
            and fit["scope"] == conclusion["scope"]
            and fit["algorithm"] == conclusion["algorithm"]
            and fit["phase"] == phase
            and fit["status"] == "estimated"
        ]
        best = max(candidates, key=lambda row: (row["r_squared"], row["valid_size_levels"])) if candidates else None
        result.append(
            {
                **conclusion,
                "operation_class": phase_operation(phase),
                "best_explanatory_variable": best["independent_variable"] if best else None,
                "best_variable_ols_slope": best["ols_slope"] if best else None,
                "best_variable_ols_slope_ci95": best["ols_slope_ci95"] if best else None,
                "best_variable_theil_sen_slope": best["theil_sen_slope"] if best else None,
                "best_variable_r_squared": best["r_squared"] if best else None,
                "best_variable_valid_size_levels": best["valid_size_levels"] if best else 0,
                "best_variable_censored_target_sizes": best["censored_target_sizes"] if best else [],
                "fixed_overhead_assessment": fixed_overhead_assessment(best),
                "causal_identification": "not-established-structural-variables-may-be-correlated",
            }
        )
    return result


def dominant_phase_variation(diagnosis: list[dict[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in diagnosis:
        if row["dominant_phase"] is not None:
            grouped[
                (
                    row["boundary_discovery_backend"],
                    row["scope"],
                    row["algorithm"],
                )
            ].append(row)
    return [
        {
            "boundary_discovery_backend": backend,
            "scope": scope,
            "algorithm": algorithm,
            "dominant_phase_by_family": {
                row["family"]: row["dominant_phase"] for row in sorted(rows, key=lambda item: item["family"])
            },
            "varies_by_family": len({row["dominant_phase"] for row in rows}) > 1,
        }
        for (backend, scope, algorithm), rows in sorted(grouped.items())
    ]


def p15_comparison(classification_rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    path = ROOT / "results/paper-scaling-full-summary.json"
    if not path.exists():
        return []
    previous = json.loads(path.read_text())
    p15 = {row["family"]: row for row in previous.get("paired_comparisons", [])}
    result = []
    backend_families = sorted(
        {
            (row["boundary_discovery_backend"], row["family"])
            for row in classification_rows
            if row["family"] in p15
        }
    )
    for backend, family in backend_families:
        scope_a = next(
            (
                row
                for row in classification_rows
                if row["boundary_discovery_backend"] == backend
                and row["family"] == family
                and row["scope"] == SCOPES[0]
                and row["explicit_algorithm"] == "explicit-hopcroft-karp"
            ),
            None,
        )
        scope_b = next(
            (
                row
                for row in classification_rows
                if row["boundary_discovery_backend"] == backend
                and row["family"] == family
                and row["scope"] == SCOPES[1]
                and row["explicit_algorithm"] == "explicit-hopcroft-karp"
            ),
            None,
        )
        if scope_a is None or scope_b is None:
            continue
        old_ratio = p15[family]["median_ratio"]
        kernel_ratio = scope_b["aggregate_median_ratio"]
        result.append(
            {
                "boundary_discovery_backend": backend,
                "family": family,
                "p15_fresh_process_ratio": old_ratio,
                "scope_a_ratio": scope_a["aggregate_median_ratio"],
                "scope_b_ratio": kernel_ratio,
                "fixed_process_cost_masked_kernel_difference": (
                    abs(old_ratio - 1) <= 0.05 and abs(kernel_ratio - 1) > 0.10
                ),
            }
        )
    return result


def summarize(raw: dict[str, Any]) -> dict[str, Any]:
    input_schema = validate_and_describe_input(raw)
    schema_version = input_schema["input_schema_version"]
    rows = flatten(raw, schema_version)
    distributions = distribution_rows(rows)
    paired = paired_rows(rows, raw["protocol"])
    classification_rows = classifications(paired, raw["protocol"])
    statuses = level_status_rows(raw, schema_version)
    phases = phase_rows(raw, rows, schema_version)
    phase_fits = phase_fit_rows(phases, raw["protocol"], statuses)
    conclusions = phase_conclusions(phases)
    diagnosis = diagnostic_rows(conclusions, phase_fits)
    return {
        "schema_version": SUMMARY_SCHEMA_VERSION,
        "campaign": raw["campaign"],
        "input_schema": input_schema,
        "source_commit": raw["source_commit"],
        "binary_sha256": raw["binary_sha256"],
        "config_sha256": raw["config_sha256"],
        "environment": raw["environment"],
        "protocol": raw["protocol"],
        "coverage": coverage(raw, rows),
        "distributions": distributions,
        "paired_comparisons": paired,
        "family_classifications": classification_rows,
        "fits": fit_rows(distributions, raw["protocol"], statuses),
        "phase_fits": phase_fits,
        "phase_decomposition": phases,
        "level_accounting": statuses,
        "structural_compression": structural_compression(distributions),
        "phase_conclusions": conclusions,
        "diagnosis": diagnosis,
        "dominant_phase_variation": dominant_phase_variation(diagnosis),
        "p15_comparison": p15_comparison(classification_rows),
        "structural_variable_definitions": {
            "N": "sizes.foreground_cells_n",
            "B": "sizes.boundary_size_b",
            "r": "sizes.reflex_count",
            "H": "sizes.horizontal_chord_count_h",
            "V": "sizes.vertical_chord_count_v",
            "q": "sizes.q; validated as H + V for schema v2",
            "K": "sizes.explicit_conflict_edge_count_k",
            "M": (
                "sizes.compressed_representation_size_m"
                if schema_version == CURRENT_SCHEMA_VERSION
                else "legacy derivation: structure.compact_node_count + structure.compact_arc_count"
            ),
        },
        "phase_accounting": {
            "setup_leaf_phases": list(SETUP_PHASES),
            "scope_a_leaf_phases": list(LEAF_PHASES),
            "scope_b_leaf_phases": list(KERNEL_LEAF_PHASES),
            "coarse_aliases_excluded_from_leaf_sums": list(COARSE_PHASES),
            "dominant_phase_rule": "largest complete level; disjoint leaf medians only",
        },
        "claim_boundaries": [
            "No exponent is estimated from fewer than six valid, distinct target-size levels.",
            "Log-log fits are empirical descriptions of the recorded families, host, compiler, and measured range; they are not complexity proofs.",
            "The variable with greatest R-squared is descriptive, not causal, because N, B, r, H, V, q, K, and M may be correlated.",
            "Stopped levels are censored and excluded from fits; they are never converted into timing observations.",
            "Coarse aliases and enclosing totals are excluded from dominant-phase sums to prevent double counting.",
        ],
        "limitations": [
            "In-process maximum RSS deltas were unavailable and remain null.",
            "Structural byte counts are declared estimates, not allocator measurements.",
            "Results are specific to the recorded host, compiler, families, and measured range.",
            "Empirical timing does not prove asymptotic complexity.",
            "A high R-squared does not distinguish correlated structural variables or establish a causal cost model.",
        ],
    }


def comparison_point_key(point: dict[str, Any]) -> tuple[str, int, int]:
    return (str(point["family"]), int(point["target_size"]), int(point["seed"]))


def comparison_protocol(protocol: dict[str, Any]) -> dict[str, Any]:
    normalized = copy.deepcopy(protocol)
    normalized.pop("boundary_discovery_backend", None)
    normalized.pop("evidence_status", None)
    return normalized


def validate_comparison_config(
    comparison_config: dict[str, Any] | None,
    reference_raw: dict[str, Any],
    optimized_raw: dict[str, Any],
) -> None:
    if comparison_config is None:
        return
    if comparison_config.get("schema_version") != 1:
        raise ValueError("before/after comparison config schema_version must be 1")
    if comparison_config.get("reference_backend") != "reference-edge-toggle" or comparison_config.get(
        "optimized_backend"
    ) != "prepared-exposed-edges":
        raise ValueError("before/after comparison config declares invalid backends")
    required_equal = comparison_config.get("required_equal_protocol_fields")
    if not isinstance(required_equal, list):
        raise ValueError("before/after comparison config omits required protocol fields")
    for field in required_equal:
        if reference_raw["protocol"].get(field) != optimized_raw["protocol"].get(field):
            raise ValueError(f"before/after protocol field {field} differs")
    if comparison_config.get("pairing_keys") != [
        "family", "target_size", "seed", "canonical_instance_identity", "scope", "algorithm", "iteration"
    ]:
        raise ValueError("before/after comparison pairing keys are not canonical")


def paired_fit_rows(
    reference_summary: dict[str, Any], optimized_summary: dict[str, Any]
) -> list[dict[str, Any]]:
    fields = (
        "status",
        "valid_size_levels",
        "censored_target_sizes",
        "ols_slope",
        "ols_slope_ci95",
        "theil_sen_slope",
        "r_squared",
    )
    key_fields = (
        "family",
        "scope",
        "algorithm",
        "phase",
        "independent_variable",
    )
    reference = {
        tuple(row[field] for field in key_fields): row
        for row in reference_summary["fits"] + reference_summary["phase_fits"]
    }
    optimized = {
        tuple(row[field] for field in key_fields): row
        for row in optimized_summary["fits"] + optimized_summary["phase_fits"]
    }
    result = []
    for key in sorted(set(reference) | set(optimized)):
        before = reference.get(key)
        after = optimized.get(key)
        result.append(
            {
                **dict(zip(key_fields, key, strict=True)),
                "reference": (
                    {field: before.get(field) for field in fields} if before is not None else None
                ),
                "optimized": (
                    {field: after.get(field) for field in fields} if after is not None else None
                ),
            }
        )
    return result


def compare_campaigns(
    left_raw: dict[str, Any],
    right_raw: dict[str, Any],
    left_summary: dict[str, Any] | None = None,
    comparison_config: dict[str, Any] | None = None,
) -> dict[str, Any]:
    left_schema = validate_and_describe_input(left_raw)
    right_schema = validate_and_describe_input(right_raw)
    if left_schema["input_schema_version"] != CURRENT_SCHEMA_VERSION or right_schema[
        "input_schema_version"
    ] != CURRENT_SCHEMA_VERSION:
        raise ValueError("before/after comparison requires two schema-v2 inputs")
    left_backend = left_raw["protocol"]["boundary_discovery_backend"]
    right_backend = right_raw["protocol"]["boundary_discovery_backend"]
    if left_backend == right_backend:
        raise ValueError("before/after comparison requires distinct boundary discovery backends")
    if comparison_protocol(left_raw["protocol"]) != comparison_protocol(right_raw["protocol"]):
        raise ValueError("before/after protocols differ beyond boundary_discovery_backend")

    raw_by_backend = {left_backend: left_raw, right_backend: right_raw}
    if set(raw_by_backend) != set(BACKENDS):
        raise ValueError("before/after comparison requires reference and prepared boundary backends")
    reference_raw = raw_by_backend["reference-edge-toggle"]
    optimized_raw = raw_by_backend["prepared-exposed-edges"]
    validate_comparison_config(comparison_config, reference_raw, optimized_raw)
    if reference_raw.get("source_commit") != optimized_raw.get("source_commit"):
        raise ValueError("before/after campaigns use different source commits")
    if reference_raw.get("binary_sha256") != optimized_raw.get("binary_sha256"):
        raise ValueError("before/after campaigns use different release binaries")
    summary_by_backend = {
        left_backend: left_summary if left_summary is not None else summarize(left_raw),
        right_backend: summarize(right_raw),
    }
    reference_summary = summary_by_backend["reference-edge-toggle"]
    optimized_summary = summary_by_backend["prepared-exposed-edges"]

    reference_points = {
        comparison_point_key(point): point for point in reference_raw["point_results"]
    }
    optimized_points = {
        comparison_point_key(point): point for point in optimized_raw["point_results"]
    }
    if set(reference_points) != set(optimized_points):
        raise ValueError("before/after campaigns have different family/size/seed point sets")

    bootstrap_count = int(reference_raw["protocol"]["fit_rule"]["bootstrap_resamples"])
    bootstrap_seed = int(reference_raw["protocol"]["fit_rule"]["bootstrap_seed"])
    phase_speedups = []
    state_comparisons = []
    paired_point_count = 0
    for point_key in sorted(reference_points):
        reference_point = reference_points[point_key]
        optimized_point = optimized_points[point_key]
        if reference_point.get("generator_version") != optimized_point.get("generator_version"):
            raise ValueError(f"before/after generator version mismatch at {point_key}")
        state_comparisons.append(
            {
                "family": point_key[0],
                "target_size": point_key[1],
                "seed": point_key[2],
                "reference_state": reference_point["state"],
                "optimized_state": optimized_point["state"],
                "changed": reference_point["state"] != optimized_point["state"],
                "reference_message": reference_point.get("message"),
                "optimized_message": optimized_point.get("message"),
            }
        )
        if reference_point["state"] != "complete" or optimized_point["state"] != "complete":
            continue
        paired_point_count += 1
        if reference_point["canonical_instance_identity"] != optimized_point[
            "canonical_instance_identity"
        ]:
            raise ValueError(f"before/after canonical instance mismatch at {point_key}")
        for field in V2_REQUIRED_SIZE_FIELDS:
            if reference_point["sizes"][field] != optimized_point["sizes"][field]:
                raise ValueError(f"before/after structural field {field} mismatch at {point_key}")

        reference_runs = {
            (run["scope"], run["algorithm"], run["iteration"]): run
            for run in reference_point["runs"]
        }
        optimized_runs = {
            (run["scope"], run["algorithm"], run["iteration"]): run
            for run in optimized_point["runs"]
        }
        for scope in SCOPES:
            for algorithm in ALGORITHMS:
                phases = ("total_elapsed_ns",) + scope_leaf_phases(scope)
                reference_keys = {
                    key for key in reference_runs if key[0] == scope and key[1] == algorithm
                }
                optimized_keys = {
                    key for key in optimized_runs if key[0] == scope and key[1] == algorithm
                }
                paired_keys = sorted(reference_keys & optimized_keys)
                for phase in phases:
                    ratios = []
                    for key in paired_keys:
                        before = (
                            reference_runs[key]["elapsed_ns"]
                            if phase == "total_elapsed_ns"
                            else reference_runs[key]["timings"].get(phase)
                        )
                        after = (
                            optimized_runs[key]["elapsed_ns"]
                            if phase == "total_elapsed_ns"
                            else optimized_runs[key]["timings"].get(phase)
                        )
                        if before not in (None, 0) and after not in (None, 0):
                            ratios.append(float(before) / float(after))
                    if not ratios:
                        continue
                    phase_speedups.append(
                        {
                            "family": point_key[0],
                            "target_size": point_key[1],
                            "seed": point_key[2],
                            "canonical_instance_identity": reference_point[
                                "canonical_instance_identity"
                            ],
                            "scope": scope,
                            "algorithm": algorithm,
                            "phase": phase,
                            "speedup_reference_over_optimized": distribution(ratios),
                            "paired_iterations": len(ratios),
                            "reference_iterations": len(reference_keys),
                            "optimized_iterations": len(optimized_keys),
                        }
                    )

    grouped_speedups: dict[tuple[str, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in phase_speedups:
        grouped_speedups[
            (row["family"], row["scope"], row["algorithm"], row["phase"])
        ].append(row)
    aggregate_speedups = []
    for (family, scope, algorithm, phase), levels in sorted(grouped_speedups.items()):
        values = [
            float(level["speedup_reference_over_optimized"]["median"]) for level in levels
        ]
        aggregate_speedups.append(
            {
                "family": family,
                "scope": scope,
                "algorithm": algorithm,
                "phase": phase,
                "valid_size_levels": len(levels),
                "target_sizes": sorted(level["target_size"] for level in levels),
                "median_speedup_reference_over_optimized": statistics.median(values),
                "median_speedup_ci95": bootstrap_median_ci(
                    values, bootstrap_count, bootstrap_seed
                ),
                "status": "estimated" if len(levels) >= 6 else "descriptive-fewer-than-six-levels",
            }
        )

    return {
        "reference_backend": "reference-edge-toggle",
        "optimized_backend": "prepared-exposed-edges",
        "reference_source_commit": reference_raw["source_commit"],
        "optimized_source_commit": optimized_raw["source_commit"],
        "reference_binary_sha256": reference_raw["binary_sha256"],
        "optimized_binary_sha256": optimized_raw["binary_sha256"],
        "paired_point_count": paired_point_count,
        "structural_mismatch_count": 0,
        "objective_mismatch_count": 0,
        "state_comparisons": state_comparisons,
        "stop_or_censoring_changes": [row for row in state_comparisons if row["changed"]],
        "phase_speedups": phase_speedups,
        "aggregate_speedups": aggregate_speedups,
        "scope_speedups": [
            row for row in aggregate_speedups if row["phase"] == "total_elapsed_ns"
        ],
        "fits_before_after": paired_fit_rows(reference_summary, optimized_summary),
        "claim_boundary": (
            "Speedups are paired on family, target, seed, canonical instance, scope, "
            "algorithm, and common iteration. They are empirical and host-specific."
        ),
    }


def format_number(value: Any) -> str:
    if value is None:
        return "NA"
    if isinstance(value, float):
        return f"{value:.4g}"
    return str(value)


def comparison_report_rows(summary: dict[str, Any]) -> list[dict[str, Any]]:
    comparison = summary.get("before_after_comparison")
    if comparison is None:
        return []
    dominant_phases = {
        row["dominant_phase"] for row in summary["diagnosis"] if row["dominant_phase"] is not None
    }
    return [
        row
        for row in comparison["aggregate_speedups"]
        if row["phase"] == "total_elapsed_ns" or row["phase"] in dominant_phases
    ]


def report_markdown(summary: dict[str, Any]) -> str:
    coverage_row = summary["coverage"]
    environment_row = summary["environment"]
    schema_row = summary["input_schema"]
    lines = [
        "# Paper Kernel Scaling Phase Diagnosis",
        "",
        "## Scope and protocol",
        "",
        "This campaign measures three exact implementations in one release process per family/size partition. Scope A starts from the canonical component and includes geometry, solving, completion, and verification. Scope B starts after shared geometry and chord generation and measures representation construction, matching or flow, and cover recovery only.",
        "",
        f"Input schema: {schema_row['input_schema_version']} (`{schema_row['normalization']}`). Fine-phase status: `{schema_row['fine_phase_status']}`.",
        "",
        f"Source commit: `{summary['source_commit']}`. Binary SHA-256: `{summary['binary_sha256']}`. Config SHA-256: `{summary['config_sha256']}`.",
        "",
        f"Host: {environment_row.get('cpu_model')} on {environment_row.get('operating_system')}; compiler {environment_row.get('rustc_version')}; power source {environment_row.get('power_source')}.",
        "",
        "## Correctness and coverage",
        "",
        f"The campaign contains {coverage_row['measured_iterations']} retained measured iterations across {coverage_row['complete_points']} analysis-eligible complete points. It has {coverage_row['stopped_points']} censored stopped points, {coverage_row['invalid_points']} invalid points, {coverage_row['correctness_failures']} failed production gates, {coverage_row['duplicate_sample_identities']} duplicate identities, and {coverage_row['missing_planned_points']} missing planned points.",
        "",
        "Stopped levels remain in `level_accounting` and every fit's `excluded_levels`; no stopped point contributes a timing median or exponent.",
        "",
        "## Family-level paired results",
        "",
        "| Backend | Family | Scope | Explicit reference | Median ratio | 95% CI | Classification | Stable crossover target |",
        "| --- | --- | --- | --- | ---: | ---: | --- | ---: |",
    ]
    for row in summary["family_classifications"]:
        lines.append(
            f"| {row['boundary_discovery_backend']} | {row['family']} | {row['scope']} | {row['explicit_algorithm']} | {format_number(row['aggregate_median_ratio'])} | [{format_number(row['aggregate_median_ratio_ci95'][0])}, {format_number(row['aggregate_median_ratio_ci95'][1])}] | {row['classification']} | {format_number(row['stable_crossover_target_size'])} |"
        )
    lines += [
        "",
        "Ratios are compact divided by the named explicit implementation; values below one favor compact. A crossover is emitted only after three consecutive larger measured levels favor compact and at least two corresponding confidence intervals lie wholly below one.",
        "",
        "## Scaling and phases",
        "",
        "Empirical exponents use one median per predeclared size level. The JSON and CSV retain total-time fits and per-leaf-phase fits against N, B, r, H, V, q, K, and M, including OLS, fixed-seed bootstrap intervals, R-squared, Theil-Sen, and explicit exclusions. A fit is not estimated with fewer than six valid target-size levels.",
        "",
        "## Structural compression",
        "",
        "| Backend | Family | Largest complete target | q | K | M | K/M |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for row in summary["structural_compression"]:
        lines.append(
            f"| {row['boundary_discovery_backend']} | {row['family']} | {row['target_size']} | {format_number(row['q'])} | {format_number(row['K'])} | {format_number(row['M'])} | {format_number(row['K_over_M'])} |"
        )
    lines += [
        "",
        "K/M is descriptive structural evidence. Zero-conflict families have K=0 and therefore no positive compression ratio; dense and crossover families show whether explicit conflict materialization grows faster than the measured compressed topology.",
        "",
        "## Phase diagnosis",
        "",
        "Dominance is computed only from mutually disjoint leaf medians. Coarse aliases, Scope totals, and unattributed measurement overhead are never candidates.",
        "",
        "| Backend | Family | Scope | Algorithm | Target | Dominant leaf | Operation | Share | Best variable | OLS slope (95% CI) | Theil-Sen | R2 | Levels | Cost assessment | Unattributed larger? |",
        "| --- | --- | --- | --- | ---: | --- | --- | ---: | --- | --- | ---: | ---: | ---: | --- | --- |",
    ]
    for row in summary["diagnosis"]:
        ci = row["best_variable_ols_slope_ci95"]
        ci_text = (
            f"{format_number(row['best_variable_ols_slope'])} "
            f"[{format_number(ci[0])}, {format_number(ci[1])}]"
            if ci is not None
            else "NA"
        )
        lines.append(
            f"| {row['boundary_discovery_backend']} | {row['family']} | {row['scope']} | {row['algorithm']} | {row['target_size']} | {format_number(row['dominant_phase'])} | {format_number(row['operation_class'])} | {format_number(row['dominant_phase_share'])} | {format_number(row['best_explanatory_variable'])} | {ci_text} | {format_number(row['best_variable_theil_sen_slope'])} | {format_number(row['best_variable_r_squared'])} | {row['best_variable_valid_size_levels']} | {row['fixed_overhead_assessment']} | {format_number(row['unattributed_exceeds_dominant_leaf'])} |"
        )
    lines += [
        "",
        "The highest-R2 variable is a descriptive ranking among correlated structural measures, not a causal identification. The fixed-overhead assessment in JSON and CSV is likewise limited to the measured range.",
        "",
        "The dominant_phase_variation records show "
        + (
            "that at least one backend/scope/algorithm group changes dominant leaf across families."
            if any(row["varies_by_family"] for row in summary["dominant_phase_variation"])
            else "no dominant-leaf change across families in any measured backend/scope/algorithm group."
        ),
        "",
    ]
    comparison = summary.get("before_after_comparison")
    if comparison is not None:
        lines += [
            "## Paired before/after comparison",
            "",
            f"{comparison['paired_point_count']} complete point pairs passed canonical-instance, structural, and optimum equality gates. Structural mismatches: {comparison['structural_mismatch_count']}; objective mismatches: {comparison['objective_mismatch_count']}; stop/censoring changes: {len(comparison['stop_or_censoring_changes'])}.",
            "",
            "| Family | Scope | Algorithm | Phase | Levels | Median speedup | 95% CI | Status |",
            "| --- | --- | --- | --- | ---: | ---: | ---: | --- |",
        ]
        for row in comparison_report_rows(summary):
            interval = row["median_speedup_ci95"]
            lines.append(
                f"| {row['family']} | {row['scope']} | {row['algorithm']} | {row['phase']} | "
                f"{row['valid_size_levels']} | {format_number(row['median_speedup_reference_over_optimized'])} | "
                f"[{format_number(interval[0])}, {format_number(interval[1])}] | {row['status']} |"
            )
        lines += ["", comparison["claim_boundary"], ""]
    lines += [
        "## Relationship to P15",
        "",
        "P15 measures fresh-process wall time and remains valid for reproducibility at its measured sizes. Scope A removes process creation and CLI/config/serialization overhead while retaining the solve pipeline. Scope B additionally removes common geometry and final completion/verification.",
        "",
        "| Backend | Family | P15 fresh-process ratio | Scope A ratio | Scope B ratio | Fixed process cost masked kernel difference |",
        "| --- | --- | ---: | ---: | ---: | --- |",
    ]
    for row in summary["p15_comparison"]:
        lines.append(
            f"| {row['boundary_discovery_backend']} | {row['family']} | {format_number(row['p15_fresh_process_ratio'])} | {format_number(row['scope_a_ratio'])} | {format_number(row['scope_b_ratio'])} | {str(row['fixed_process_cost_masked_kernel_difference']).lower()} |"
        )
    lines += [
        "",
        "The masking indicator is a predeclared descriptive comparison: P15 lies within 5% of parity while Scope B differs from parity by more than 10%. It does not assert hardware-independent causality and does not invalidate P15.",
        "",
        "## Claim boundary",
        "",
    ]
    for boundary in summary["claim_boundaries"]:
        lines.append(f"- {boundary}")
    lines += [
        "",
        "These measurements do not prove asymptotic complexity, universal speedup, AN19 runtime, or a crossover outside the measured families and host. Scope B is not end-to-end runtime.",
        "",
    ]
    return "\n".join(lines)


def latex(value: Any) -> str:
    return str(value).replace("_", "\\_").replace("%", "\\%")


def latex_tables(summary: dict[str, Any]) -> str:
    coverage_row = summary["coverage"]
    tables = [
        "% Generated by tools/analyze_paper_kernel_scaling.py",
        "% Requires booktabs.",
        "\\begin{table}[t]",
        "\\caption{Kernel campaign correctness and coverage.}",
        "\\label{tab:kernel-coverage}",
        "\\begin{tabular}{rrrrrr}",
        "\\toprule",
        r"Points & Complete & Stopped & Invalid & Iterations & Mismatches \\",
        "\\midrule",
        f"{coverage_row['observed_points']} & {coverage_row['complete_points']} & "
        f"{coverage_row['stopped_points']} & {coverage_row['invalid_points']} & "
        f"{coverage_row['measured_iterations']} & {coverage_row['correctness_failures']} \\\\",
        "\\bottomrule",
        "\\end{tabular}",
        "\\end{table}",
        "",
    ]
    for scope, letter in zip(SCOPES, ("a", "b"), strict=True):
        tables += [
            "\\begin{table}[t]",
            f"\\caption{{Scope {letter.upper()} representative median timings.}}",
            f"\\label{{tab:kernel-scope-{letter}}}",
            "\\begin{tabular}{lllrrr}",
            "\\toprule",
            r"Backend & Family & Algorithm & Target & Median (ns) & $q$ \\",
            "\\midrule",
        ]
        for row in summary["distributions"]:
            peers = [
                level
                for level in summary["distributions"]
                if level["boundary_discovery_backend"] == row["boundary_discovery_backend"]
                and level["family"] == row["family"]
                and level["scope"] == scope
            ]
            if row["scope"] != scope or row["target_size"] != max(level["target_size"] for level in peers):
                continue
            tables.append(
                f"{latex(row['boundary_discovery_backend'])} & {latex(row['family'])} & "
                f"{latex(row['algorithm'])} & {row['target_size']} & "
                f"{row['elapsed_ns']['median']:.4g} & {format_number(row['sizes'].get('q'))} \\\\"
            )
        tables += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]

    tables += [
        "\\begin{table}[t]",
        "\\caption{Empirical total-time log--log exponents.}",
        "\\label{tab:kernel-exponents}",
        "\\begin{tabular}{lllllrrr}",
        "\\toprule",
        r"Backend & Family & Scope & Algorithm & Variable & OLS & Theil--Sen & $R^2$ \\",
        "\\midrule",
    ]
    for row in summary["fits"]:
        if row["status"] == "estimated":
            tables.append(
                f"{latex(row['boundary_discovery_backend'])} & {latex(row['family'])} & "
                f"{latex(row['scope'])} & {latex(row['algorithm'])} & "
                f"{row['independent_variable']} & {row['ols_slope']:.3g} & "
                f"{row['theil_sen_slope']:.3g} & {row['r_squared']:.3g} \\\\"
            )
    tables += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]

    tables += [
        "\\begin{table}[t]",
        "\\caption{Predeclared crossover classifications.}",
        "\\label{tab:kernel-crossover}",
        "\\begin{tabular}{lllllrr}",
        "\\toprule",
        r"Backend & Family & Scope & Reference & Class & Ratio & Crossover \\",
        "\\midrule",
    ]
    for row in summary["family_classifications"]:
        tables.append(
            f"{latex(row['boundary_discovery_backend'])} & {latex(row['family'])} & "
            f"{latex(row['scope'])} & {latex(row['explicit_algorithm'])} & "
            f"{latex(row['classification'])} & {row['aggregate_median_ratio']:.3g} & "
            f"{format_number(row['stable_crossover_target_size'])} \\\\"
        )
    tables += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]

    tables += [
        "\\begin{table}[t]",
        "\\caption{Structural compression at the largest complete level.}",
        "\\label{tab:kernel-compression}",
        "\\begin{tabular}{llrrrrr}",
        "\\toprule",
        r"Backend & Family & Target & $q$ & $K$ & $M$ & $K/M$ \\",
        "\\midrule",
    ]
    for row in summary["structural_compression"]:
        tables.append(
            f"{latex(row['boundary_discovery_backend'])} & {latex(row['family'])} & "
            f"{row['target_size']} & {format_number(row['q'])} & {format_number(row['K'])} & "
            f"{format_number(row['M'])} & {format_number(row['K_over_M'])} \\\\"
        )
    tables += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]

    tables += [
        "\\begin{table}[t]",
        "\\caption{Dominant disjoint leaf phases and their best descriptive fits.}",
        "\\label{tab:kernel-phases}",
        "\\begin{tabular}{lllllrrrr}",
        "\\toprule",
        r"Backend & Family & Scope & Algorithm & Leaf & Share & Variable & OLS & $R^2$ \\",
        "\\midrule",
    ]
    for row in summary["diagnosis"]:
        if row["dominant_phase"] is None:
            continue
        tables.append(
            f"{latex(row['boundary_discovery_backend'])} & {latex(row['family'])} & "
            f"{latex(row['scope'])} & {latex(row['algorithm'])} & "
            f"{latex(row['dominant_phase'])} & {format_number(row['dominant_phase_share'])} & "
            f"{format_number(row['best_explanatory_variable'])} & "
            f"{format_number(row['best_variable_ols_slope'])} & "
            f"{format_number(row['best_variable_r_squared'])} \\\\"
        )
    tables += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]
    comparison = summary.get("before_after_comparison")
    if comparison is not None:
        tables += [
            "\\begin{table}[t]",
            "\\caption{Paired reference-to-optimized speedups.}",
            "\\label{tab:kernel-before-after}",
            "\\begin{tabular}{llllrrrr}",
            "\\toprule",
            r"Family & Scope & Algorithm & Phase & Levels & Speedup & CI low & CI high \\",
            "\\midrule",
        ]
        for row in comparison_report_rows(summary):
            interval = row["median_speedup_ci95"]
            tables.append(
                f"{latex(row['family'])} & {latex(row['scope'])} & "
                f"{latex(row['algorithm'])} & {latex(row['phase'])} & "
                f"{row['valid_size_levels']} & "
                f"{format_number(row['median_speedup_reference_over_optimized'])} & "
                f"{format_number(interval[0])} & {format_number(interval[1])} \\\\"
            )
        tables += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]
    return "\n".join(tables)


def svg_chart(path: Path, title: str, x_label: str, y_label: str, series: list[tuple[str, list[tuple[float, float]]]], horizontal_one: bool = False) -> None:
    width, height = 960, 560
    left, top, right, bottom = 90, 55, 220, 70
    points = [point for _, values in series for point in values]
    if not points:
        points = [(0, 0), (1, 1)]
    x_min, x_max = min(x for x, _ in points), max(x for x, _ in points)
    y_min, y_max = min(y for _, y in points), max(y for _, y in points)
    if x_min == x_max:
        x_max += 1
    if y_min == y_max:
        y_max += 1
    def locate(x: float, y: float) -> tuple[float, float]:
        return (
            left + (x - x_min) / (x_max - x_min) * (width - left - right),
            height - bottom - (y - y_min) / (y_max - y_min) * (height - top - bottom),
        )
    colors = ("#007f73", "#c65d00", "#4c5f7a", "#a33f5d", "#6b5ca5", "#2e7d32", "#8d6e63")
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">', '<rect width="100%" height="100%" fill="white"/>', f'<text x="{width/2}" y="28" text-anchor="middle" font-family="sans-serif" font-size="18">{html.escape(title)}</text>', f'<line x1="{left}" y1="{height-bottom}" x2="{width-right}" y2="{height-bottom}" stroke="#263238"/>', f'<line x1="{left}" y1="{top}" x2="{left}" y2="{height-bottom}" stroke="#263238"/>', f'<text x="{(left+width-right)/2}" y="{height-18}" text-anchor="middle" font-family="sans-serif" font-size="13">{html.escape(x_label)}</text>', f'<text x="20" y="{height/2}" text-anchor="middle" transform="rotate(-90 20 {height/2})" font-family="sans-serif" font-size="13">{html.escape(y_label)}</text>']
    if horizontal_one and y_min <= 1 <= y_max:
        _, y = locate(x_min, 1)
        parts.append(f'<line x1="{left}" y1="{y}" x2="{width-right}" y2="{y}" stroke="#666" stroke-dasharray="5 4"/>')
    for index, (label, values) in enumerate(series):
        color = colors[index % len(colors)]
        coordinates = [locate(x, y) for x, y in values]
        if coordinates:
            path_data = " ".join(("M" if point_index == 0 else "L") + f" {x:.2f},{y:.2f}" for point_index, (x, y) in enumerate(coordinates))
            parts.append(f'<path d="{path_data}" fill="none" stroke="{color}" stroke-width="2"/>')
            parts.extend(f'<circle cx="{x:.2f}" cy="{y:.2f}" r="3" fill="{color}"/>' for x, y in coordinates)
        parts.append(f'<text x="{width-right+12}" y="{top+16*index}" font-family="sans-serif" font-size="10" fill="{color}">{html.escape(label)}</text>')
    parts.append("</svg>")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(parts) + "\n")


def figures(summary: dict[str, Any], output: Path) -> None:
    distributions = summary["distributions"]
    def time_series(scope: str, x_field: str, algorithm_x: bool = False) -> list[tuple[str, list[tuple[float, float]]]]:
        result = []
        for backend, family in sorted(
            {(row["boundary_discovery_backend"], row["family"]) for row in distributions}
        ):
            for algorithm in ALGORITHMS:
                values = []
                for row in distributions:
                    if (
                        row["boundary_discovery_backend"] != backend
                        or row["family"] != family
                        or row["scope"] != scope
                        or row["algorithm"] != algorithm
                    ):
                        continue
                    field = "compressed_representation_size_m" if algorithm_x and algorithm == "compact-mrd" else x_field
                    x = row["sizes"].get(field)
                    if x and row["elapsed_ns"]["median"]:
                        values.append((math.log10(x), math.log10(row["elapsed_ns"]["median"])))
                if values:
                    result.append((f"{backend}:{family}:{algorithm}", sorted(values)))
        return result
    svg_chart(output / "scope-a-time-vs-q.svg", "Scope A time versus q", "log10 q", "log10 median time (ns)", time_series(SCOPES[0], "q"))
    svg_chart(output / "scope-b-time-vs-structure.svg", "Scope B time versus K or M", "log10 K (explicit) or M (compact)", "log10 median time (ns)", time_series(SCOPES[1], "explicit_conflict_edge_count_k", True))
    ratio_series = []
    for backend, family in sorted(
        {
            (row["boundary_discovery_backend"], row["family"])
            for row in summary["paired_comparisons"]
        }
    ):
        for scope in SCOPES:
            for explicit in EXPLICIT:
                values = [(row["q"], row["ratios"]["median"]) for row in summary["paired_comparisons"] if row["boundary_discovery_backend"] == backend and row["family"] == family and row["scope"] == scope and row["explicit_algorithm"] == explicit and row["q"]]
                if values:
                    ratio_series.append((f"{backend}:{family}:{scope}:{explicit}", sorted(values)))
    svg_chart(output / "paired-ratio-vs-q.svg", "Compact / explicit paired ratio", "q (chords)", "median paired ratio", ratio_series, True)
    structure_series = []
    for backend, family in sorted(
        {(row["boundary_discovery_backend"], row["family"]) for row in distributions}
    ):
        values = []
        for row in distributions:
            if row["boundary_discovery_backend"] == backend and row["family"] == family and row["algorithm"] == "compact-mrd" and row["scope"] == SCOPES[0]:
                k_value, m_value = row["sizes"].get("explicit_conflict_edge_count_k"), row["sizes"].get("compressed_representation_size_m")
                if k_value is not None and m_value is not None:
                    values.append((k_value, m_value))
        if values:
            structure_series.append((f"{backend}:{family}", sorted(values)))
    svg_chart(output / "k-vs-m.svg", "Explicit K versus compressed M", "explicit conflict edges K", "compressed nodes + arcs M", structure_series)
    representative = []
    for backend, family in sorted(
        {
            (row["boundary_discovery_backend"], row["family"])
            for row in summary["phase_decomposition"]
        }
    ):
        family_rows = [row for row in summary["phase_decomposition"] if row["boundary_discovery_backend"] == backend and row["family"] == family and row["scope"] == SCOPES[1]]
        if family_rows:
            largest = max(row["target_size"] for row in family_rows)
            representative.extend(row for row in family_rows if row["target_size"] == largest)
    phase_series = []
    for index, row in enumerate(representative, start=1):
        for phase in KERNEL_LEAF_PHASES:
            value = row["phase_medians_ns"].get(phase)
            if value is not None:
                phase_series.append((f"{row['boundary_discovery_backend']}:{row['family']}:{row['algorithm']}:{phase}", [(index, value)]))
    svg_chart(output / "phase-decomposition.svg", "Scope B phase decomposition at largest levels", "representative family/solver index", "median phase time (ns)", phase_series)
    construction_series = []
    for backend, family in sorted(
        {
            (row["boundary_discovery_backend"], row["family"])
            for row in summary["phase_decomposition"]
        }
    ):
        for algorithm in ALGORITHMS:
            values = []
            for row in summary["phase_decomposition"]:
                if row["boundary_discovery_backend"] != backend or row["family"] != family or row["algorithm"] != algorithm or row["scope"] != SCOPES[1]:
                    continue
                construction = sum(row["phase_medians_ns"].get(key) or 0 for key in ("embedding_ns", "conflict_discovery_ns", "representation_construction_ns", "explicit_network_construction_ns", "compressed_network_construction_ns"))
                structure_field = "compressed_representation_size_m" if algorithm == "compact-mrd" else "explicit_conflict_edge_count_k"
                structural_size = row["sizes"].get(structure_field)
                if structural_size and construction:
                    values.append((math.log10(structural_size), math.log10(construction)))
            if values:
                construction_series.append((f"{backend}:{family}:{algorithm}", sorted(values)))
    svg_chart(output / "construction-time-vs-structure.svg", "Construction time versus K or M", "log10 K (explicit) or M (compact)", "log10 construction time (ns)", construction_series)
    memory_series = []
    for backend, family in sorted(
        {(row["boundary_discovery_backend"], row["family"]) for row in distributions}
    ):
        explicit_values, compact_values = [], []
        for row in distributions:
            if row["boundary_discovery_backend"] == backend and row["family"] == family and row["algorithm"] == "compact-mrd" and row["scope"] == SCOPES[0]:
                structure = next((item for item in summary.get("_point_structures", []) if item[0] == backend and item[1] == family and item[2] == row["target_size"]), None)
                if structure:
                    explicit_values.append((row["target_size"], structure[3].get("explicit_estimated_structural_bytes", 0)))
                    compact_values.append((row["target_size"], structure[3].get("compact_estimated_structural_bytes", 0)))
        if explicit_values:
            memory_series.extend(((f"{backend}:{family}:explicit", explicit_values), (f"{backend}:{family}:compact", compact_values)))
    svg_chart(output / "structural-memory.svg", "Estimated structural storage", "target size", "estimated structural bytes", memory_series)


def summary_csv(path: Path, summary: dict[str, Any]) -> None:
    rows = []
    for key in (
        "distributions",
        "paired_comparisons",
        "family_classifications",
        "fits",
        "phase_fits",
        "phase_decomposition",
        "level_accounting",
        "structural_compression",
        "phase_conclusions",
        "diagnosis",
        "dominant_phase_variation",
        "p15_comparison",
    ):
        rows.extend({"record_type": key, **row} for row in summary[key])
    comparison = summary.get("before_after_comparison")
    if comparison is not None:
        for key in (
            "state_comparisons",
            "phase_speedups",
            "aggregate_speedups",
            "fits_before_after",
        ):
            rows.extend(
                {"record_type": f"before_after_{key}", **row}
                for row in comparison[key]
            )
    fields = sorted({key for row in rows for key in row}) or ["record_type"]
    buffer = io.StringIO(newline="")
    writer = csv.DictWriter(buffer, fieldnames=fields, lineterminator="\n")
    writer.writeheader()
    for row in rows:
        writer.writerow({key: json.dumps(value, sort_keys=True, separators=(",", ":")) if isinstance(value, (dict, list)) else value for key, value in row.items()})
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(buffer.getvalue())


def validate_artifacts(figure_dir: Path, tables: str, expected_tables: int = 7) -> None:
    figures = sorted(figure_dir.glob("*.svg"))
    if len(figures) != 7:
        raise ValueError(f"expected seven SVG figures, found {len(figures)}")
    for figure in figures:
        if figure.stat().st_size == 0:
            raise ValueError(f"empty SVG: {figure}")
        ET.parse(figure)
    if tables.count("\\begin{table}") != expected_tables or tables.count("\\end{table}") != expected_tables:
        raise ValueError(f"LaTeX output must contain {expected_tables} balanced tables")
    if tables.count("\\toprule") != expected_tables or tables.count("\\bottomrule") != expected_tables:
        raise ValueError("every LaTeX table must have booktabs rules")


def synthetic_v2_input() -> dict[str, Any]:
    backend = "reference-edge-toggle"
    protocol = {
        "schema_version": 2,
        "campaign": CAMPAIGN,
        "boundary_discovery_backend": backend,
        "families": ["random-connected"],
        "initial_size_levels": [8],
        "algorithms": list(ALGORITHMS),
        "scopes": list(SCOPES),
        "fit_rule": {
            "minimum_valid_size_levels": 6,
            "bootstrap_resamples": 100,
            "bootstrap_seed": 17,
        },
    }
    planned = {
        "point_identity": "point-8",
        "family": "random-connected",
        "target_size": 8,
        "seed": 42,
    }
    canonical = "canonical-8"
    warmups = [
        {
            "boundary_discovery_backend": backend,
            "scope": scope,
            "algorithm": algorithm,
            "measured_repetitions": 1,
        }
        for scope in SCOPES
        for algorithm in ALGORITHMS
    ]
    runs = []
    for scope in SCOPES:
        for algorithm in ALGORITHMS:
            timings = {phase: None for phase in set(COARSE_PHASES + LEAF_PHASES)}
            for _, leaves, prefix_name in NESTED_TIMING_GROUPS:
                for field in leaves:
                    timings.setdefault(field, None)
                timings[f"{prefix_name}_leaf_sum_ns"] = None
                timings[f"{prefix_name}_unattributed_ns"] = None
                timings[f"{prefix_name}_accounting_ok"] = None
            if scope == SCOPES[0]:
                for phase in GEOMETRY_LEAF_PHASES + RECOVERY_LEAF_PHASES:
                    timings[phase] = 1
                timings["boundary_total_build_ns"] = 6
                timings["geometry_preprocessing_ns"] = 9
                timings["chord_generation_ns"] = 4
                timings["rectangle_completion_recovery_ns"] = 5
                timings["output_validation_ns"] = 2
                timings["internal_output_validation_ns"] = 1
                timings["final_output_validation_ns"] = 1
                timings["completion_finalization_ns"] = 1
                for prefix_name, parent, leaves in (
                    ("boundary", "boundary_total_build_ns", NESTED_TIMING_GROUPS[0][1]),
                    ("geometry", "geometry_preprocessing_ns", NESTED_TIMING_GROUPS[1][1]),
                    ("chord", "chord_generation_ns", NESTED_TIMING_GROUPS[2][1]),
                    ("completion", "rectangle_completion_recovery_ns", NESTED_TIMING_GROUPS[3][1]),
                    ("output_validation", "output_validation_ns", NESTED_TIMING_GROUPS[4][1]),
                ):
                    leaf_sum = sum(timings[phase] or 0 for phase in leaves)
                    timings[f"{prefix_name}_leaf_sum_ns"] = leaf_sum
                    timings[f"{prefix_name}_unattributed_ns"] = timings[parent] - leaf_sum
                    timings[f"{prefix_name}_accounting_ok"] = True
            if algorithm != "explicit-hopcroft-karp":
                timings["embedding_ns"] = 1
                timings["representation_construction_ns"] = 1
            if algorithm != "compact-mrd":
                timings["conflict_discovery_ns"] = 1
            if algorithm == "compact-mrd":
                timings["compressed_network_construction_ns"] = 1
            elif algorithm == "explicit-c0-flow":
                timings["explicit_network_construction_ns"] = 1
            timings["matching_or_flow_ns"] = 1
            timings["minimum_vertex_cover_recovery_ns"] = 1
            leaf_sum = sum(timings[phase] or 0 for phase in accounting_scope_phases(scope))
            total = leaf_sum + 3
            prefix = "scope_a" if scope == SCOPES[0] else "scope_b"
            timings[f"{prefix}_total_ns"] = total
            timings[f"{prefix}_leaf_sum_ns"] = leaf_sum
            timings[f"{prefix}_unattributed_ns"] = 3
            timings[f"{prefix}_accounting_ok"] = True
            sample = canonical_sample_identity(
                planned, canonical, backend, scope, algorithm, 0
            )
            runs.append(
                {
                    "sample_identity": sample,
                    "record_kind": "measured",
                    "seed": 42,
                    "canonical_instance_identity": canonical,
                    "boundary_discovery_backend": backend,
                    "scope": scope,
                    "algorithm": algorithm,
                    "iteration": 0,
                    "elapsed_ns": total,
                    "timings": timings,
                }
            )
    sizes = {
        "width": 4,
        "height": 4,
        "foreground_cells_n": 8,
        "component_count": 1,
        "bounding_box_area_a": 16,
        "boundary_size_b": 8,
        "boundary_unit_edge_count_u": 12,
        "reflex_count": 4,
        "horizontal_chord_count_h": 2,
        "vertical_chord_count_v": 3,
        "q": 5,
        "explicit_conflict_edge_count_k": 4,
        "compressed_representation_size_m": 13,
        "biclique_count": 1,
        "biclique_total_vertex_occurrences_sigma": 5,
        "compressed_network_node_count": 6,
        "compressed_network_arc_count": 7,
        "optimum_rectangle_count": 3,
        "output_rectangle_count": 3,
    }
    structure = {
        "boundary_candidate_edge_probes": 32,
        "boundary_exposed_unit_edges": 12,
        "boundary_trace_edge_visits": 12,
        "explicit_graph_node_count": 5,
        "explicit_graph_edge_count": 4,
        "biclique_count": 1,
        "biclique_incidence_sigma": 5,
        "compact_node_count": 6,
        "compact_arc_count": 7,
        "explicit_c0_node_count": 6,
        "explicit_c0_arc_count": 7,
        "explicit_estimated_structural_bytes": 104,
        "compact_estimated_structural_bytes": 104,
        "explicit_c0_estimated_structural_bytes": 104,
        "estimated_peak_structural_bytes": 104,
        "completion_candidate_queries": 1,
        "completion_candidate_revalidations": 1,
        "completion_stale_candidates": 0,
        "completion_ray_extension_unit_steps": 1,
        "rectangle_recovery_cell_visits": 8,
    }
    shared = {phase: None for phase in set(COARSE_PHASES + LEAF_PHASES)}
    for _, leaves, prefix_name in NESTED_TIMING_GROUPS[:3]:
        for field in leaves:
            shared.setdefault(field, None)
        shared[f"{prefix_name}_leaf_sum_ns"] = None
        shared[f"{prefix_name}_unattributed_ns"] = None
        shared[f"{prefix_name}_accounting_ok"] = None
    shared.update(
        {
            "boundary_total_build_ns": 6,
            "boundary_edge_discovery_ns": 1,
            "boundary_adjacency_build_ns": 1,
            "boundary_loop_tracing_ns": 1,
            "boundary_loop_normalization_ns": 1,
            "reflex_detection_ns": 1,
            "boundary_unit_edge_sort_ns": 1,
            "prepared_component_build_ns": 1,
            "boundary_index_construction_ns": 1,
            "reflex_grouping_ns": 1,
            "geometry_preprocessing_ns": 9,
            "horizontal_chord_generation_ns": 1,
            "vertical_chord_generation_ns": 1,
            "chord_validation_filtering_ns": 1,
            "endpoint_index_construction_ns": 1,
            "chord_generation_ns": 4,
        }
    )
    for prefix_name, parent, leaves in (
        ("boundary", "boundary_total_build_ns", NESTED_TIMING_GROUPS[0][1]),
        ("geometry", "geometry_preprocessing_ns", NESTED_TIMING_GROUPS[1][1]),
        ("chord", "chord_generation_ns", NESTED_TIMING_GROUPS[2][1]),
    ):
        leaf_sum = sum(shared[phase] or 0 for phase in leaves)
        shared[f"{prefix_name}_leaf_sum_ns"] = leaf_sum
        shared[f"{prefix_name}_unattributed_ns"] = shared[parent] - leaf_sum
        shared[f"{prefix_name}_accounting_ok"] = True
    point = {
        **planned,
        "schema_version": 2,
        "campaign": CAMPAIGN,
        "generator_version": "synthetic-v2",
        "boundary_discovery_backend": backend,
        "canonical_instance_identity": canonical,
        "state": "complete",
        "message": None,
        "sizes": sizes,
        "structure": structure,
        "setup_timings": {
            "instance_generation_ns": 5,
            "input_normalization_ns": 7,
            "connected_component_extraction_ns": 11,
            "setup_total_ns": 25,
        },
        "correctness": [
            {
                "boundary_discovery_backend": backend,
                "algorithm": algorithm,
                "outcome": "success",
                "optimum_rectangle_count": 3,
                "matching_size": 1,
                "vertex_cover_size": 1,
            }
            for algorithm in ALGORITHMS
        ],
        "warmups": warmups,
        "shared_scope_b_preprocessing": shared,
        "runs": runs,
        "exact_measured_order": [run["sample_identity"] for run in runs],
    }
    return {
        "schema_version": 2,
        "campaign": CAMPAIGN,
        "protocol": protocol,
        "config_sha256": "config",
        "source_commit": "commit",
        "binary_sha256": "binary",
        "environment": {},
        "planned_points": [planned],
        "point_results": [point],
        "completion": {
            "complete": True,
            "planned_point_count": 1,
            "observed_point_count": 1,
            "completed_point_count": 1,
            "missing_point_count": 0,
            "missing_point_identities": [],
            "terminal_state_counts": {"complete": 1},
            "correctness_failure_count": 0,
        },
    }


def self_test() -> None:
    assert distribution([1, 2, 3])["median"] == 2
    points = [(math.log(1), math.log(2)), (math.log(2), math.log(4)), (math.log(4), math.log(8))]
    assert abs(ols(points)["slope"] - 1) < 1e-12
    assert abs(theil_sen(points) - 1) < 1e-12
    assert bootstrap_ci([1, 2, 3], statistics.median, 100, 42) == bootstrap_ci([1, 2, 3], statistics.median, 100, 42)
    assert bootstrap_median_ci([1, 2, 3, 4], 100, 42) == bootstrap_median_ci(
        [1, 2, 3, 4], 100, 42
    )
    legacy = {
        "schema_version": 1,
        "campaign": CAMPAIGN,
        "protocol": {},
        "point_results": [],
    }
    legacy_description = validate_and_describe_input(legacy)
    assert legacy_description["normalization"] == "legacy-v1-coarse"
    assert all(
        normalized_run_timings({}, LEGACY_SCHEMA_VERSION)[phase] is None
        for phase in LEAF_PHASES
    )

    v2 = synthetic_v2_input()
    summary = summarize(v2)
    assert summary["input_schema"]["normalization"] == "native-v2-fine-phases"
    assert summary["phase_fits"]
    assert all(row["legacy_input"] is False for row in summary["phase_decomposition"])
    shared_conclusions = [
        row
        for row in summary["phase_conclusions"]
        if row["scope"] == SHARED_PREPROCESSING_SCOPE
    ]
    assert shared_conclusions and all(
        row["status"] == "available" for row in shared_conclusions
    )
    assert "Claim boundary" in report_markdown(summary)
    rendered_tables = latex_tables(summary)
    assert rendered_tables.count("\\begin{table}") == 7
    assert rendered_tables.count("\\end{table}") == 7
    optimized = copy.deepcopy(v2)
    optimized_backend = "prepared-exposed-edges"
    optimized["protocol"]["boundary_discovery_backend"] = optimized_backend
    optimized_point = optimized["point_results"][0]
    optimized_point["boundary_discovery_backend"] = optimized_backend
    for record in optimized_point["correctness"] + optimized_point["warmups"]:
        record["boundary_discovery_backend"] = optimized_backend
    for run in optimized_point["runs"]:
        run["boundary_discovery_backend"] = optimized_backend
        run["sample_identity"] = canonical_sample_identity(
            optimized_point,
            optimized_point["canonical_instance_identity"],
            optimized_backend,
            run["scope"],
            run["algorithm"],
            run["iteration"],
        )
    optimized_point["exact_measured_order"] = [
        run["sample_identity"] for run in optimized_point["runs"]
    ]
    comparison = compare_campaigns(v2, optimized, summary)
    assert comparison["paired_point_count"] == 1
    assert comparison["structural_mismatch_count"] == 0
    summary["before_after_comparison"] = comparison
    assert latex_tables(summary).count("\\begin{table}") == 8
    malformed = copy.deepcopy(v2)
    malformed["completion"]["complete"] = False
    try:
        summarize(malformed)
    except ValueError as error:
        assert "incomplete" in str(error)
    else:
        raise AssertionError("incomplete v2 campaign was accepted")

    statuses = [
        {
            "boundary_discovery_backend": backend,
            "family": "synthetic",
            "target_size": target,
            "state": "complete",
            "message": None,
            "stop_propagated_from_target_size": None,
        }
        for target in range(1, 7)
        for backend in ("reference-edge-toggle",)
    ]
    levels = [
        {
            "boundary_discovery_backend": "reference-edge-toggle",
            "family": "synthetic",
            "target_size": target,
            "scope": SCOPES[0],
            "algorithm": "compact-mrd",
            "sizes": {field: target for field in VARIABLES.values()},
            "elapsed_ns": {"median": float(target * target)},
        }
        for target in range(1, 7)
    ]
    config = {
        "fit_rule": {
            "minimum_valid_size_levels": 6,
            "bootstrap_resamples": 100,
            "bootstrap_seed": 19,
        }
    }
    fitted = next(
        row
        for row in fit_rows(levels, config, statuses)
        if row["independent_variable"] == "N"
    )
    assert fitted["status"] == "estimated"
    assert abs(fitted["ols_slope"] - 2) < 1e-12


def main() -> int:
    arguments = parse_args()
    if arguments.self_test:
        self_test()
        print("paper-kernel-scaling analyzer self-test: ok")
        return 0
    raw = json.loads(root_path(arguments.input).read_text())
    summary = summarize(raw)
    if arguments.compare_input is not None:
        comparison_raw = json.loads(root_path(arguments.compare_input).read_text())
        comparison_config = None
        if arguments.comparison_config is not None:
            comparison_config = json.loads(root_path(arguments.comparison_config).read_text())
        summary["before_after_comparison"] = compare_campaigns(
            raw, comparison_raw, summary, comparison_config
        )
    input_schema_version = summary["input_schema"]["input_schema_version"]
    summary["_point_structures"] = [
        (
            backend_for_point(point, input_schema_version, raw["protocol"]),
            point["family"],
            point["target_size"],
            point.get("structure", {}),
        )
        for point in raw.get("point_results", [])
    ]
    figure_dir = root_path(arguments.figure_dir)
    figures(summary, figure_dir)
    summary.pop("_point_structures", None)
    tables = latex_tables(summary)
    expected_tables = 8 if "before_after_comparison" in summary else 7
    validate_artifacts(figure_dir, tables, expected_tables)
    root_path(arguments.summary_json).write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    summary_csv(root_path(arguments.summary_csv), summary)
    root_path(arguments.report).write_text(report_markdown(summary))
    root_path(arguments.tables).write_text(tables + "\n")
    print(json.dumps({"coverage": summary["coverage"], "figures": 7, "tables": expected_tables}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
