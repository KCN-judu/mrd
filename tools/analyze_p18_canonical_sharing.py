#!/usr/bin/env python3
"""Analyze paired P18 canonical clone-versus-borrowed measurements."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import random
import statistics
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = 1
BACKENDS = ("clone-canonical-reference", "borrowed-canonical")
ALGORITHMS = ("compact-mrd", "explicit-hopcroft-karp", "explicit-c0-flow")
SCOPES = ("solve-from-canonical-instance", "representation-and-solver-kernel")
FAMILIES = (
    "comb-staircase",
    "representation-crossover",
    "dense-conflict",
    "random-connected",
    "sparse-conflict",
    "supported-holes",
)
PHASES = (
    "canonical_component_clone_ns",
    "canonical_context_borrow_or_share_ns",
    "canonical_component_release_ns",
    "solver_workspace_prepare_ns",
    "geometry_preprocessing_ns",
    "chord_generation_ns",
    "embedding_ns",
    "conflict_discovery_ns",
    "representation_construction_ns",
    "explicit_network_construction_ns",
    "compressed_network_construction_ns",
    "matching_or_flow_ns",
    "minimum_vertex_cover_recovery_ns",
    "chord_selection_ns",
    "rectangle_completion_recovery_ns",
    "output_validation_ns",
)
REQUIRED_TIMING_FIELDS = frozenset(
    {
        *PHASES,
        "scope_a_total_ns",
        "scope_b_total_ns",
        "scope_a_leaf_sum_ns",
        "scope_a_unattributed_ns",
        "scope_a_accounting_ok",
        "scope_b_leaf_sum_ns",
        "scope_b_unattributed_ns",
        "scope_b_accounting_ok",
    }
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
SCOPE_A_PHASES = (
    "canonical_component_clone_ns",
    "canonical_context_borrow_or_share_ns",
    "canonical_component_release_ns",
    "geometry_preprocessing_ns",
    "chord_generation_ns",
    "solver_workspace_prepare_ns",
    "embedding_ns",
    "conflict_discovery_ns",
    "representation_construction_ns",
    "explicit_network_construction_ns",
    "compressed_network_construction_ns",
    "matching_or_flow_ns",
    "minimum_vertex_cover_recovery_ns",
    "chord_selection_ns",
    "rectangle_completion_recovery_ns",
    "output_validation_ns",
)
SCOPE_B_PHASES = (
    "embedding_ns",
    "conflict_discovery_ns",
    "representation_construction_ns",
    "explicit_network_construction_ns",
    "compressed_network_construction_ns",
    "matching_or_flow_ns",
    "minimum_vertex_cover_recovery_ns",
)
RUNNER_FAILURE_STATES = frozenset(("runner-error", "runner-timeout"))
PROVENANCE_ENVIRONMENT_FIELDS = (
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


def canonical_json(value: Any) -> str:
    return json.dumps(value, allow_nan=False, sort_keys=True, separators=(",", ":"))


def strict_json_loads(value: str) -> Any:
    def reject_constant(constant: str) -> None:
        raise ValueError(f"non-finite JSON number {constant!r} is not permitted")

    return json.loads(value, parse_constant=reject_constant)


def sha256_json(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode()).hexdigest()


def expected_backend_config(protocol: dict[str, Any], backend: str) -> dict[str, Any]:
    config = {**protocol, "canonical_backend": backend}
    config.pop("canonical_backends", None)
    return config


def expected_campaign_identity(
    config_sha256: str, source_commit: str, binary_sha256: str
) -> str:
    fields = {
        "checkpoint_schema_version": 2,
        "sample_schema_version": 2,
        "campaign": "paper-kernel-scaling",
        "config_sha256": config_sha256,
        "source_commit": source_commit,
        "binary_sha256": binary_sha256,
    }
    return "sha256:" + sha256_json(fields)


def expected_point_identity(
    config_sha256: str, family: str, target_size: int, seed: int
) -> str:
    fields = {
        "campaign": "paper-kernel-scaling",
        "config_sha256": config_sha256,
        "family": family,
        "target_size": target_size,
        "seed": seed,
    }
    return "sha256:" + sha256_json(fields)


def expected_backend_plan(
    protocol: dict[str, Any], backend: str
) -> list[dict[str, Any]]:
    families = protocol.get("families")
    sizes = protocol.get("initial_size_levels")
    seed = protocol.get("seed")
    if (
        not isinstance(families, list)
        or not families
        or len(families) != len(set(families))
        or any(family not in FAMILIES for family in families)
    ):
        raise ValueError("P18 protocol has an invalid family census")
    if (
        not isinstance(sizes, list)
        or not sizes
        or sizes != sorted(sizes)
        or len(sizes) != len(set(sizes))
        or any(
            isinstance(size, bool) or not isinstance(size, int) or size <= 0
            for size in sizes
        )
    ):
        raise ValueError("P18 protocol has an invalid target-size census")
    if isinstance(seed, bool) or not isinstance(seed, int):
        raise ValueError("P18 protocol seed must be an integer")
    backend_sha = sha256_json(expected_backend_config(protocol, backend))
    return [
        {
            "point_identity": expected_point_identity(
                backend_sha, family, target_size, seed
            ),
            "family": family,
            "target_size": target_size,
            "seed": seed,
        }
        for family in families
        for target_size in sizes
    ]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    self_test_only = "--self-test" in __import__("sys").argv
    parser.add_argument("--input", type=Path, required=not self_test_only)
    parser.add_argument("--summary-json", type=Path, required=not self_test_only)
    parser.add_argument("--summary-csv", type=Path, required=not self_test_only)
    parser.add_argument("--report", type=Path, required=not self_test_only)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def median(values: list[float | int]) -> float | None:
    return float(statistics.median(values)) if values else None


def bootstrap_ci(
    values: list[float], seed: int, resamples: int = 10_000
) -> tuple[float, float] | None:
    if not values:
        return None
    rng = random.Random(seed)
    samples = [
        statistics.median(rng.choices(values, k=len(values))) for _ in range(resamples)
    ]
    samples.sort()
    return samples[int(0.025 * (len(samples) - 1))], samples[
        int(0.975 * (len(samples) - 1))
    ]


def slope(points: list[tuple[int, float]]) -> float | None:
    points = [(x, y) for x, y in points if x > 0 and y > 0]
    if len(points) < 6:
        return None
    xs = [math.log(x) for x, _ in points]
    ys = [math.log(y) for _, y in points]
    x_bar = statistics.mean(xs)
    y_bar = statistics.mean(ys)
    denominator = sum((x - x_bar) ** 2 for x in xs)
    return (
        None
        if denominator == 0
        else sum((x - x_bar) * (y - y_bar) for x, y in zip(xs, ys)) / denominator
    )


def require_nonnegative_integer(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValueError(f"{label} must be a nonnegative integer")
    return value


def validate_run_timing_shape(
    timings: dict[str, Any], scope: str, identity: str
) -> None:
    missing = sorted(REQUIRED_TIMING_FIELDS - set(timings))
    if missing:
        raise ValueError(
            f"{identity} timings omit required fields: {', '.join(missing)}"
        )
    for field, value in timings.items():
        if field.endswith("_ok"):
            if value is not None and not isinstance(value, bool):
                raise ValueError(f"{identity} {field} must be boolean or null")
        elif value is not None:
            require_nonnegative_integer(value, f"{identity} timing {field}")

    for parent, leaves, prefix in NESTED_TIMING_GROUPS:
        accounting = (
            f"{prefix}_leaf_sum_ns",
            f"{prefix}_unattributed_ns",
            f"{prefix}_accounting_ok",
        )
        parent_value = timings[parent]
        if parent_value is None:
            if any(timings.get(field) is not None for field in (*leaves, *accounting)):
                raise ValueError(
                    f"{identity} {prefix} accounting exists without its parent"
                )
            continue
        missing = [field for field in (*leaves, *accounting) if field not in timings]
        if missing:
            raise ValueError(
                f"{identity} {prefix} accounting omits: {', '.join(missing)}"
            )
        leaf_sum = sum(timings[field] or 0 for field in leaves)
        declared = require_nonnegative_integer(
            timings[accounting[0]], f"{identity} {accounting[0]}"
        )
        unattributed = require_nonnegative_integer(
            timings[accounting[1]], f"{identity} {accounting[1]}"
        )
        if timings[accounting[2]] is not True:
            raise ValueError(f"{identity} {prefix} accounting is not valid")
        if declared != leaf_sum or declared + unattributed != parent_value:
            raise ValueError(f"{identity} {prefix} timing accounting mismatch")

    total_field = "scope_a_total_ns" if scope == SCOPES[0] else "scope_b_total_ns"
    leaf_field = "scope_a_leaf_sum_ns" if scope == SCOPES[0] else "scope_b_leaf_sum_ns"
    unattributed_field = (
        "scope_a_unattributed_ns" if scope == SCOPES[0] else "scope_b_unattributed_ns"
    )
    ok_field = (
        "scope_a_accounting_ok" if scope == SCOPES[0] else "scope_b_accounting_ok"
    )
    phase_fields = SCOPE_A_PHASES if scope == SCOPES[0] else SCOPE_B_PHASES
    total = require_nonnegative_integer(
        timings[total_field], f"{identity} {total_field}"
    )
    declared = require_nonnegative_integer(
        timings[leaf_field], f"{identity} {leaf_field}"
    )
    unattributed = require_nonnegative_integer(
        timings[unattributed_field], f"{identity} {unattributed_field}"
    )
    if timings[ok_field] is not True:
        raise ValueError(f"{identity} scope accounting is not valid")
    leaf_sum = sum(timings[field] or 0 for field in phase_fields)
    if declared != leaf_sum or declared + unattributed != total:
        raise ValueError(f"{identity} scope timing accounting mismatch")


def validate_allocation_semantics(
    allocations: dict[str, Any],
    point: dict[str, Any],
    backend: str,
    scope: str,
    identity: str,
) -> None:
    fields = (
        "canonical_cells_cloned",
        "canonical_clone_bytes_estimate",
        "solver_workspace_retained_bytes_estimate",
        "representation_retained_bytes_estimate",
        "ownership_vec_allocation_count_estimate",
    )
    for field in fields:
        require_nonnegative_integer(
            allocations.get(field), f"{identity} allocations.{field}"
        )
    clone_cells = allocations["canonical_cells_cloned"]
    clone_bytes = allocations["canonical_clone_bytes_estimate"]
    workspace_bytes = allocations["solver_workspace_retained_bytes_estimate"]
    ownership_allocations = allocations["ownership_vec_allocation_count_estimate"]
    if scope == SCOPES[1]:
        if clone_cells != 0 or clone_bytes != 0:
            raise ValueError(f"{identity} Scope B reports canonical clone payload")
        if workspace_bytes != 0 or ownership_allocations != 0:
            raise ValueError(
                f"{identity} Scope B reports Scope-A ownership allocations"
            )
        return
    foreground_cells = (point.get("sizes") or {}).get("foreground_cells_n")
    if not isinstance(foreground_cells, int) or isinstance(foreground_cells, bool):
        return
    if backend == BACKENDS[0]:
        if clone_cells != foreground_cells:
            raise ValueError(
                f"{identity} clone-reference cells differ from foreground N"
            )
        if foreground_cells > 0 and clone_bytes == 0:
            raise ValueError(f"{identity} clone-reference payload bytes are zero")
    elif clone_cells != 0 or clone_bytes != 0:
        raise ValueError(f"{identity} borrowed backend reports canonical clone payload")


def validate_sample_census(
    point: dict[str, Any], backend: str, protocol: dict[str, Any]
) -> None:
    warmups = point.get("warmups")
    runs = point.get("runs")
    exact_order = point.get("exact_measured_order")
    if not isinstance(warmups, list) or not isinstance(runs, list):
        raise ValueError(f"{backend} point sample census is malformed")
    if point.get("stop_propagated_from_target_size") is not None:
        if exact_order not in (None, []) or warmups or runs:
            raise ValueError(f"{backend} propagated stop contains samples")
        return
    if not isinstance(exact_order, list):
        raise ValueError(f"{backend} point sample census is malformed")

    expected_pairs = {
        (scope, algorithm) for scope in SCOPES for algorithm in ALGORITHMS
    }
    warmup_rule = protocol.get("warmup_rule")
    repetition_rule = protocol.get("repetition_rule")
    if not isinstance(warmup_rule, dict) or not isinstance(repetition_rule, dict):
        raise ValueError("P18 protocol omits warmup or repetition rules")
    repetition_counts: dict[tuple[str, str], int] = {}
    for record in warmups:
        if not isinstance(record, dict):
            raise ValueError(f"{backend} warmup record is malformed")
        pair = (record.get("scope"), record.get("algorithm"))
        if pair not in expected_pairs or pair in repetition_counts:
            raise ValueError(f"{backend} warmup census has an invalid identity")
        if record.get("canonical_backend") != backend:
            raise ValueError(f"{backend} warmup ownership identity mismatch")
        warmup_count = require_nonnegative_integer(record.get("count"), "warmup count")
        if not (
            int(warmup_rule.get("minimum", -1))
            <= warmup_count
            <= int(warmup_rule.get("maximum", -1))
        ):
            raise ValueError(f"{backend} warmup count violates the protocol")
        preflight_ns = require_nonnegative_integer(
            record.get("preflight_ns"), "preflight duration"
        )
        repetitions = require_nonnegative_integer(
            record.get("measured_repetitions"), "measured repetition count"
        )
        if preflight_ns < int(repetition_rule.get("fast_threshold_ns", -1)):
            minimum = int(repetition_rule.get("fast_minimum", -1))
        elif preflight_ns <= int(repetition_rule.get("medium_threshold_ns", -1)):
            minimum = int(repetition_rule.get("medium_minimum", -1))
        else:
            minimum = int(repetition_rule.get("slow_minimum", -1))
        target = int(repetition_rule.get("target_measured_ns", -1)) // max(
            preflight_ns, 1
        )
        expected_repetitions = min(
            max(target, minimum), int(repetition_rule.get("maximum", -1))
        )
        if repetitions != expected_repetitions:
            raise ValueError(
                f"{backend} measured repetition count violates the adaptive rule"
            )
        repetition_counts[pair] = repetitions

    if point.get("state") == "complete" and set(repetition_counts) != expected_pairs:
        raise ValueError(f"{backend} complete point has an incomplete warmup census")

    canonical_identity = point.get("canonical_instance_identity")
    boundary_backend = point.get("boundary_discovery_backend")
    observed_keys: list[tuple[str, str, int]] = []
    observed_identities: list[str] = []
    for record in runs:
        if not isinstance(record, dict):
            raise ValueError(f"{backend} measured record is malformed")
        scope = record.get("scope")
        algorithm = record.get("algorithm")
        pair = (scope, algorithm)
        iteration = require_nonnegative_integer(
            record.get("iteration"), "measured iteration"
        )
        if pair not in repetition_counts or iteration >= repetition_counts[pair]:
            raise ValueError(
                f"{backend} measured record is outside its declared census"
            )
        if record.get("record_kind") != "measured":
            raise ValueError(f"{backend} measured record has the wrong kind")
        if record.get("canonical_backend") != backend:
            raise ValueError(f"{backend} measured run ownership mismatch")
        if record.get("boundary_discovery_backend") != boundary_backend:
            raise ValueError(f"{backend} measured run boundary backend mismatch")
        if record.get("canonical_instance_identity") != canonical_identity:
            raise ValueError(f"{backend} measured run canonical identity mismatch")
        if record.get("seed") != point.get("seed"):
            raise ValueError(f"{backend} measured run seed mismatch")
        expected_identity = (
            "paper-kernel-scaling:v2:"
            f"{point.get('family')}:{point.get('target_size')}:{point.get('seed')}:"
            f"{canonical_identity}:measured:"
            f"{'scope-a' if scope == SCOPES[0] else 'scope-b'}:"
            f"{algorithm}:{iteration}:{boundary_backend}"
        )
        if record.get("sample_identity") != expected_identity:
            raise ValueError(f"{backend} measured sample identity mismatch")
        timings = record.get("timings")
        if not isinstance(timings, dict):
            raise ValueError(f"{backend} measured run timings are malformed")
        validate_run_timing_shape(timings, str(scope), expected_identity)
        elapsed_field = "scope_a_total_ns" if scope == SCOPES[0] else "scope_b_total_ns"
        elapsed = require_nonnegative_integer(
            record.get("elapsed_ns"), "measured elapsed duration"
        )
        if timings.get(elapsed_field) != elapsed:
            raise ValueError(f"{backend} measured elapsed duration mismatch")
        order_position = require_nonnegative_integer(
            record.get("order_position"), "measured order position"
        )
        if order_position >= len(ALGORITHMS):
            raise ValueError(f"{backend} measured order position is invalid")
        observed_keys.append((str(scope), str(algorithm), iteration))
        observed_identities.append(expected_identity)

    if len(observed_keys) != len(set(observed_keys)):
        raise ValueError(f"{backend} measured census contains duplicate identities")
    if exact_order != observed_identities:
        raise ValueError(f"{backend} exact measured order mismatch")
    if point.get("state") == "complete":
        expected_keys = {
            (scope, algorithm, iteration)
            for (scope, algorithm), repetitions in repetition_counts.items()
            for iteration in range(repetitions)
        }
        if set(observed_keys) != expected_keys:
            raise ValueError(
                f"{backend} complete point has an incomplete sample census"
            )


def validate_input(raw: dict[str, Any]) -> dict[str, Any]:
    if (
        raw.get("schema_version") != SCHEMA_VERSION
        or raw.get("campaign") != "p18-canonical-sharing"
    ):
        raise ValueError("incompatible P18 report schema or campaign")
    if raw.get("canonical_backends") != list(BACKENDS):
        raise ValueError(
            "P18 backend order must be clone reference followed by borrowed"
        )
    protocol = raw.get("protocol")
    if not isinstance(protocol, dict):
        raise ValueError("P18 protocol is missing")
    if protocol.get("canonical_backends") != list(BACKENDS):
        raise ValueError("P18 protocol backend census is inconsistent")
    if protocol.get("algorithms") != list(ALGORITHMS):
        raise ValueError("P18 protocol algorithm census is inconsistent")
    if protocol.get("scopes") != list(SCOPES):
        raise ValueError("P18 protocol scope census is inconsistent")
    fit_rule = protocol.get("fit_rule")
    if not isinstance(fit_rule, dict):
        raise ValueError("P18 protocol fit rule is missing")
    if int(fit_rule.get("minimum_valid_size_levels", 0)) < 6:
        raise ValueError("P18 fit rule permits fewer than six valid levels")
    if int(fit_rule.get("bootstrap_resamples", 0)) < 10_000:
        raise ValueError("P18 fit rule permits fewer than 10000 bootstrap resamples")
    if isinstance(fit_rule.get("bootstrap_seed"), bool) or not isinstance(
        fit_rule.get("bootstrap_seed"), int
    ):
        raise ValueError("P18 fit rule bootstrap seed is invalid")
    expected_config_sha = sha256_json(protocol)
    if raw.get("config_sha256") != expected_config_sha:
        raise ValueError("P18 top-level config SHA-256 mismatch")
    source_commit = raw.get("source_commit")
    binary_sha256 = raw.get("binary_sha256")
    if not isinstance(source_commit, str) or not source_commit:
        raise ValueError("P18 source commit is missing")
    if not isinstance(binary_sha256, str) or not binary_sha256:
        raise ValueError("P18 binary SHA-256 is missing")
    top_environment = raw.get("environment")
    if (
        not isinstance(top_environment, dict)
        or top_environment.get("git_dirty") is not False
    ):
        raise ValueError("P18 top-level environment does not have clean provenance")
    missing_environment_fields = [
        field for field in PROVENANCE_ENVIRONMENT_FIELDS if field not in top_environment
    ]
    if missing_environment_fields:
        raise ValueError(
            "P18 top-level environment omits provenance fields: "
            + ", ".join(missing_environment_fields)
        )
    payloads = raw.get("backends")
    if not isinstance(payloads, dict) or set(payloads) != set(BACKENDS):
        raise ValueError("P18 report must contain both backend payloads")
    plans: list[tuple[str, int, int]] | None = None
    backend_stats: dict[str, dict[str, Any]] = {}
    paired_rows: dict[str, dict[tuple[str, int], dict[str, Any]]] = {}
    for backend in BACKENDS:
        payload = payloads[backend]
        if payload.get("campaign") != "paper-kernel-scaling":
            raise ValueError(f"{backend} payload is not paper-kernel-scaling")
        if payload.get("canonical_backends") is not None:
            raise ValueError("nested payload has malformed ownership protocol")
        backend_protocol = expected_backend_config(protocol, backend)
        if payload.get("protocol") != backend_protocol:
            raise ValueError(f"{backend} nested protocol mismatch")
        backend_config_sha = sha256_json(backend_protocol)
        if payload.get("config_sha256") != backend_config_sha:
            raise ValueError(f"{backend} config SHA-256 mismatch")
        if payload.get("campaign_identity") != expected_campaign_identity(
            backend_config_sha, source_commit, binary_sha256
        ):
            raise ValueError(f"{backend} campaign identity mismatch")
        if payload.get("source_commit") != source_commit:
            raise ValueError("source commits differ across P18 backends")
        if payload.get("binary_sha256") != binary_sha256:
            raise ValueError("release binary hashes differ across P18 backends")
        environment = payload.get("environment")
        if (
            not isinstance(environment, dict)
            or environment.get("git_dirty") is not False
        ):
            raise ValueError(f"{backend} does not have clean source provenance")
        if environment.get("git_commit") != source_commit:
            raise ValueError(f"{backend} environment source commit mismatch")
        if environment.get("binary_sha256") != binary_sha256:
            raise ValueError(f"{backend} environment binary SHA-256 mismatch")
        for field in PROVENANCE_ENVIRONMENT_FIELDS:
            if field not in environment:
                raise ValueError(f"{backend} environment.{field} is missing")
            if environment.get(field) != top_environment.get(field):
                raise ValueError(
                    f"{backend} environment.{field} differs across backends"
                )
        completion = payload.get("completion")
        if not isinstance(completion, dict) or not completion.get("complete"):
            raise ValueError(f"{backend} payload is incomplete")
        planned_points = payload.get("planned_points")
        point_results = payload.get("point_results")
        retry_history = payload.get("retry_history")
        if not isinstance(planned_points, list) or not isinstance(point_results, list):
            raise ValueError(f"{backend} point census is malformed")
        if any(not isinstance(point, dict) for point in point_results):
            raise ValueError(f"{backend} point census contains a malformed record")
        if not isinstance(retry_history, list):
            raise ValueError(f"{backend} retry history is malformed")
        expected_plan = expected_backend_plan(protocol, backend)
        if planned_points != expected_plan:
            raise ValueError(f"{backend} plan differs from the predeclared protocol")
        planned_ids = [row.get("point_identity") for row in planned_points]
        observed_ids = [row.get("point_identity") for row in point_results]
        if any(not isinstance(value, str) or not value for value in planned_ids):
            raise ValueError(f"{backend} plan has malformed point identities")
        if len(planned_ids) != len(set(planned_ids)):
            raise ValueError(f"{backend} plan has duplicate point identities")
        if len(observed_ids) != len(set(observed_ids)):
            raise ValueError(f"{backend} output has duplicate point identities")
        if set(planned_ids) != set(observed_ids):
            raise ValueError(f"{backend} planned/observed point census differs")
        planned_by_identity = {row["point_identity"]: row for row in planned_points}
        planned_id_set = set(planned_ids)
        if any(
            not isinstance(record, dict)
            or record.get("state") not in RUNNER_FAILURE_STATES
            or record.get("point_identity") not in planned_id_set
            for record in retry_history
        ):
            raise ValueError(f"{backend} retry history contains an invalid record")
        retry_fingerprints = [canonical_json(record) for record in retry_history]
        if len(retry_fingerprints) != len(set(retry_fingerprints)):
            raise ValueError(f"{backend} retry history contains an exact duplicate")
        state_counts: dict[str, int] = {}
        sample_identities: set[str] = set()
        correctness_count = 0
        for point in point_results:
            expected_point = planned_by_identity[point["point_identity"]]
            for field in ("family", "target_size", "seed"):
                if point.get(field) != expected_point[field]:
                    raise ValueError(
                        f"{backend} point {field} differs from its planned identity"
                    )
            state = point.get("state")
            if state not in ("complete", "stopped"):
                raise ValueError("P18 point is not terminal")
            state_counts[state] = state_counts.get(state, 0) + 1
            if point.get("canonical_backend") != backend:
                raise ValueError(
                    f"point has wrong canonical backend: {point.get('point_identity')}"
                )
            propagated = point.get("stop_propagated_from_target_size") is not None
            canonical_identity = point.get("canonical_instance_identity")
            if not propagated and canonical_identity in (None, ""):
                raise ValueError("measured point lacks canonical instance identity")
            correctness = point.get("correctness", [])
            if not propagated:
                if not isinstance(correctness, list) or len(correctness) != len(
                    ALGORITHMS
                ):
                    raise ValueError(f"{backend} correctness census mismatch")
                correctness_algorithms = [
                    record.get("algorithm") for record in correctness
                ]
                if len(correctness_algorithms) != len(
                    set(correctness_algorithms)
                ) or set(correctness_algorithms) != set(ALGORITHMS):
                    raise ValueError(f"{backend} correctness algorithm census mismatch")
                if any(
                    not isinstance(record, dict)
                    or record.get("outcome") != "success"
                    or not isinstance(record.get("witness_checksum"), str)
                    or not record.get("witness_checksum")
                    for record in correctness
                ):
                    raise ValueError(f"{backend} correctness failure")
                if any(
                    record.get("canonical_backend") != backend for record in correctness
                ):
                    raise ValueError(f"{backend} correctness ownership mismatch")
                correctness_count += len(correctness)
            validate_sample_census(point, backend, protocol)
            runs = point.get("runs", [])
            exact_order = point.get("exact_measured_order", [])
            if exact_order is None:
                exact_order = []
            if not isinstance(runs, list) or not isinstance(exact_order, list):
                raise ValueError(f"{backend} measured sample census is malformed")
            identities = [run.get("sample_identity") for run in runs]
            if identities != exact_order:
                raise ValueError(f"{backend} exact measured order mismatch")
            if any(not isinstance(value, str) or not value for value in identities):
                raise ValueError(f"{backend} has malformed sample identities")
            if len(identities) != len(set(identities)):
                raise ValueError(
                    f"{backend} has duplicate sample identities within a point"
                )
            if sample_identities.intersection(identities):
                raise ValueError(
                    f"{backend} has duplicate sample identities across points"
                )
            sample_identities.update(identities)
            for run in runs:
                if run.get("canonical_backend") != backend:
                    raise ValueError(f"{backend} measured run ownership mismatch")
                if run.get("canonical_instance_identity") != canonical_identity:
                    raise ValueError(
                        f"{backend} measured run canonical identity mismatch"
                    )
                allocations = run.get("allocations")
                if not isinstance(allocations, dict):
                    raise ValueError(
                        f"{backend} measured run lacks allocation diagnostics"
                    )
                validate_allocation_semantics(
                    allocations,
                    point,
                    backend,
                    str(run.get("scope", "")),
                    f"{backend} run {run.get('sample_identity')}",
                )
        if completion.get("planned_point_count") != len(planned_ids):
            raise ValueError(f"{backend} planned completion census mismatch")
        if completion.get("observed_point_count") != len(observed_ids):
            raise ValueError(f"{backend} observed completion census mismatch")
        if completion.get("completed_point_count") != len(observed_ids):
            raise ValueError(f"{backend} terminal completion census mismatch")
        if completion.get("terminal_state_counts") != state_counts:
            raise ValueError(f"{backend} terminal state census mismatch")
        if (
            completion.get("missing_point_count") != 0
            or completion.get("missing_point_identities") != []
        ):
            raise ValueError(f"{backend} reports missing planned points")
        if completion.get("correctness_failure_count") != 0:
            raise ValueError(f"{backend} reports correctness failures")
        if completion.get("retry_history_count") != len(retry_history):
            raise ValueError(f"{backend} retry history census mismatch")
        normalized_plan = sorted(
            (row["family"], row["target_size"], row["seed"]) for row in planned_points
        )
        if plans is None:
            plans = normalized_plan
        elif plans != normalized_plan:
            raise ValueError("P18 plans differ; paired instances are not comparable")
        paired_rows[backend] = {
            (point["family"], point["target_size"]): point for point in point_results
        }
        backend_stats[backend] = {
            "planned_points": len(planned_ids),
            "terminal_points": len(observed_ids),
            "state_counts": state_counts,
            "measured_runs": len(sample_identities),
            "correctness_records": correctness_count,
            "retry_history": len(retry_history),
            "config_sha256": backend_config_sha,
            "campaign_identity": payload["campaign_identity"],
        }
    structural_mismatches = 0
    objective_mismatches = 0
    witness_mismatches = 0
    canonical_identity_mismatches = 0
    sample_identity_mismatches = 0
    sample_order_mismatches = 0
    adaptive_count_mismatches = 0
    paired_keys = set(paired_rows[BACKENDS[0]]) & set(paired_rows[BACKENDS[1]])
    if len(paired_keys) != len(plans or []):
        raise ValueError("P18 paired point census is incomplete")
    for key in paired_keys:
        left = paired_rows[BACKENDS[0]][key]
        right = paired_rows[BACKENDS[1]][key]
        if left.get("canonical_instance_identity") != right.get(
            "canonical_instance_identity"
        ):
            canonical_identity_mismatches += 1
        if left.get("sizes") != right.get("sizes") or left.get(
            "structure"
        ) != right.get("structure"):
            structural_mismatches += 1
        for pair in {
            (scope, algorithm) for scope in SCOPES for algorithm in ALGORITHMS
        }:
            left_runs = sorted(
                (
                    run
                    for run in left.get("runs", [])
                    if (run.get("scope"), run.get("algorithm")) == pair
                ),
                key=lambda run: run.get("iteration", -1),
            )
            right_runs = sorted(
                (
                    run
                    for run in right.get("runs", [])
                    if (run.get("scope"), run.get("algorithm")) == pair
                ),
                key=lambda run: run.get("iteration", -1),
            )
            if len(left_runs) != len(right_runs):
                adaptive_count_mismatches += 1
            for left_run, right_run in zip(left_runs, right_runs):
                if left_run.get("sample_identity") != right_run.get("sample_identity"):
                    sample_identity_mismatches += 1
                if left_run.get("order_position") != right_run.get("order_position"):
                    sample_order_mismatches += 1
        left_correctness = {
            row.get("algorithm"): row for row in left.get("correctness", [])
        }
        right_correctness = {
            row.get("algorithm"): row for row in right.get("correctness", [])
        }
        for algorithm in set(left_correctness) & set(right_correctness):
            if left_correctness[algorithm].get(
                "optimum_rectangle_count"
            ) != right_correctness[algorithm].get("optimum_rectangle_count"):
                objective_mismatches += 1
            if left_correctness[algorithm].get("witness_checksum") != right_correctness[
                algorithm
            ].get("witness_checksum"):
                witness_mismatches += 1
    mismatch_counts = {
        "canonical_identity": canonical_identity_mismatches,
        "structural": structural_mismatches,
        "objective": objective_mismatches,
        "witness": witness_mismatches,
        "sample_identity": sample_identity_mismatches,
        "sample_order": sample_order_mismatches,
        "adaptive_count": adaptive_count_mismatches,
    }
    if any(
        mismatch_counts[field]
        for field in (
            "canonical_identity",
            "structural",
            "objective",
            "witness",
            "sample_identity",
            "sample_order",
        )
    ):
        raise ValueError(f"P18 paired semantic mismatches: {mismatch_counts}")
    top_completion = raw.get("completion")
    if (
        not isinstance(top_completion, dict)
        or top_completion.get("complete") is not True
    ):
        raise ValueError("P18 top-level completion is inconsistent")
    if top_completion.get("backend_completion") != {
        backend: payloads[backend]["completion"] for backend in BACKENDS
    }:
        raise ValueError("P18 top-level backend completion census differs")
    return {
        "config_sha256": expected_config_sha,
        "backends": backend_stats,
        "paired_points": len(paired_keys),
        "mismatches": mismatch_counts,
    }


def rows_by_backend(
    raw: dict[str, Any],
) -> dict[str, dict[tuple[str, int], dict[str, Any]]]:
    return {
        backend: {
            (point["family"], point["target_size"]): point
            for point in raw["backends"][backend]["point_results"]
        }
        for backend in BACKENDS
    }


def point_rows(raw: dict[str, Any]) -> dict[str, dict[tuple[str, int], dict[str, Any]]]:
    rows: dict[str, dict[tuple[str, int], dict[str, Any]]] = {
        backend: {} for backend in BACKENDS
    }
    for backend in BACKENDS:
        for point in raw["backends"][backend]["point_results"]:
            rows[backend][(point["family"], point["target_size"])] = point
    return rows


def run_medians(
    point: dict[str, Any], scope: str, algorithm: str, field: str
) -> float | None:
    values = [
        run.get("timings", {}).get(field)
        for run in point.get("runs", [])
        if run.get("scope") == scope and run.get("algorithm") == algorithm
    ]
    values = [value for value in values if isinstance(value, int) and value >= 0]
    return median(values) if values else None


def allocation_medians(
    point: dict[str, Any], scope: str, algorithm: str, field: str
) -> float | None:
    values = [
        run.get("allocations", {}).get(field)
        for run in point.get("runs", [])
        if run.get("scope") == scope and run.get("algorithm") == algorithm
    ]
    values = [value for value in values if isinstance(value, int) and value >= 0]
    return median(values) if values else None


def analyze(raw: dict[str, Any]) -> dict[str, Any]:
    validation = validate_input(raw)
    fit_rule = raw["protocol"]["fit_rule"]
    bootstrap_resamples = int(fit_rule["bootstrap_resamples"])
    bootstrap_seed = int(fit_rule["bootstrap_seed"])
    rows = point_rows(raw)
    families = sorted({family for family, _ in rows[BACKENDS[0]]})
    levels = sorted({size for family, size in rows[BACKENDS[0]]})
    summary: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "campaign": "p18-canonical-sharing-analysis",
        "source_commit": raw["source_commit"],
        "binary_sha256": raw["binary_sha256"],
        "config_sha256": raw["config_sha256"],
        "environment": raw["environment"],
        "validation": validation,
        "families": {},
        "stopped_points": {
            backend: [
                {"family": family, "target_size": size, "message": point.get("message")}
                for (family, size), point in rows[backend].items()
                if point.get("state") == "stopped"
            ]
            for backend in BACKENDS
        },
        "claim_boundary": "Finite paired measurements on the recorded host, families, and target levels; no asymptotic claim.",
    }
    csv_rows: list[dict[str, Any]] = []
    for family in families:
        family_summary: dict[str, Any] = {"algorithms": {}, "largest_level": None}
        complete_levels = [
            size
            for size in levels
            if rows[BACKENDS[0]].get((family, size), {}).get("state") == "complete"
            and rows[BACKENDS[1]].get((family, size), {}).get("state") == "complete"
        ]
        if complete_levels:
            largest = max(complete_levels)
            family_summary["largest_level"] = largest
        for algorithm in ALGORITHMS:
            algorithm_summary: dict[str, Any] = {}
            for scope in SCOPES:
                key = "scope_a" if scope == SCOPES[0] else "scope_b"
                ratios: list[float] = []
                clone_phase_ratios: list[float] = []
                clone_phase_before: list[float] = []
                clone_phase_after: list[float] = []
                borrow_share_before: list[float] = []
                borrow_share_after: list[float] = []
                release_before: list[float] = []
                release_after: list[float] = []
                ownership_lifecycle_ratios: list[float] = []
                ownership_removed_fractions: list[float] = []
                workspace_overheads: list[float] = []
                workspace_before_ns: list[float] = []
                workspace_after_ns: list[float] = []
                representation_changes: list[float] = []
                byte_reductions: list[float] = []
                clone_bytes_before: list[float] = []
                clone_bytes_after: list[float] = []
                workspace_bytes_before: list[float] = []
                workspace_bytes_after: list[float] = []
                representation_bytes_before: list[float] = []
                representation_bytes_after: list[float] = []
                ownership_allocations_before: list[float] = []
                ownership_allocations_after: list[float] = []
                before_points: list[tuple[int, float]] = []
                after_points: list[tuple[int, float]] = []
                phase_by_level: dict[str, dict[str, float]] = {}
                for size in complete_levels:
                    clone = rows[BACKENDS[0]][(family, size)]
                    borrowed = rows[BACKENDS[1]][(family, size)]
                    before = run_medians(
                        clone,
                        scope,
                        algorithm,
                        "scope_a_total_ns"
                        if scope == SCOPES[0]
                        else "scope_b_total_ns",
                    )
                    after = run_medians(
                        borrowed,
                        scope,
                        algorithm,
                        "scope_a_total_ns"
                        if scope == SCOPES[0]
                        else "scope_b_total_ns",
                    )
                    if before is None or after is None or after == 0:
                        continue
                    ratios.append(before / after)
                    before_points.append((size, before))
                    after_points.append((size, after))
                    clone_phase = (
                        run_medians(
                            clone, scope, algorithm, "canonical_component_clone_ns"
                        )
                        or 0
                    )
                    borrowed_phase = (
                        run_medians(
                            borrowed, scope, algorithm, "canonical_component_clone_ns"
                        )
                        or 0
                    )
                    clone_phase_before.append(clone_phase)
                    clone_phase_after.append(borrowed_phase)
                    if borrowed_phase == 0 and clone_phase > 0:
                        clone_phase_ratios.append(float("inf"))
                    elif borrowed_phase > 0:
                        clone_phase_ratios.append(clone_phase / borrowed_phase)
                    clone_borrow_share = (
                        run_medians(
                            clone,
                            scope,
                            algorithm,
                            "canonical_context_borrow_or_share_ns",
                        )
                        or 0
                    )
                    borrowed_borrow_share = (
                        run_medians(
                            borrowed,
                            scope,
                            algorithm,
                            "canonical_context_borrow_or_share_ns",
                        )
                        or 0
                    )
                    clone_release = (
                        run_medians(
                            clone,
                            scope,
                            algorithm,
                            "canonical_component_release_ns",
                        )
                        or 0
                    )
                    borrowed_release = (
                        run_medians(
                            borrowed,
                            scope,
                            algorithm,
                            "canonical_component_release_ns",
                        )
                        or 0
                    )
                    borrow_share_before.append(clone_borrow_share)
                    borrow_share_after.append(borrowed_borrow_share)
                    release_before.append(clone_release)
                    release_after.append(borrowed_release)
                    ownership_before = clone_phase + clone_borrow_share + clone_release
                    ownership_after = (
                        borrowed_phase + borrowed_borrow_share + borrowed_release
                    )
                    if ownership_after > 0:
                        ownership_lifecycle_ratios.append(
                            ownership_before / ownership_after
                        )
                    if before > 0:
                        ownership_removed_fractions.append(
                            max(ownership_before - ownership_after, 0) / before
                        )
                    clone_workspace = (
                        run_medians(
                            clone, scope, algorithm, "solver_workspace_prepare_ns"
                        )
                        or 0
                    )
                    borrowed_workspace = (
                        run_medians(
                            borrowed, scope, algorithm, "solver_workspace_prepare_ns"
                        )
                        or 0
                    )
                    workspace_before_ns.append(clone_workspace)
                    workspace_after_ns.append(borrowed_workspace)
                    workspace_overheads.append(borrowed_workspace - clone_workspace)
                    clone_representation = (
                        run_medians(
                            clone, scope, algorithm, "representation_construction_ns"
                        )
                        or 0
                    )
                    borrowed_representation = (
                        run_medians(
                            borrowed, scope, algorithm, "representation_construction_ns"
                        )
                        or 0
                    )
                    if clone_representation:
                        representation_changes.append(
                            borrowed_representation / clone_representation
                        )
                    clone_bytes = (
                        allocation_medians(
                            clone, scope, algorithm, "canonical_clone_bytes_estimate"
                        )
                        or 0
                    )
                    borrowed_bytes = (
                        allocation_medians(
                            borrowed, scope, algorithm, "canonical_clone_bytes_estimate"
                        )
                        or 0
                    )
                    clone_bytes_before.append(clone_bytes)
                    clone_bytes_after.append(borrowed_bytes)
                    clone_workspace_bytes = (
                        allocation_medians(
                            clone,
                            scope,
                            algorithm,
                            "solver_workspace_retained_bytes_estimate",
                        )
                        or 0
                    )
                    borrowed_workspace_bytes = (
                        allocation_medians(
                            borrowed,
                            scope,
                            algorithm,
                            "solver_workspace_retained_bytes_estimate",
                        )
                        or 0
                    )
                    workspace_bytes_before.append(clone_workspace_bytes)
                    workspace_bytes_after.append(borrowed_workspace_bytes)
                    clone_representation_bytes = (
                        allocation_medians(
                            clone,
                            scope,
                            algorithm,
                            "representation_retained_bytes_estimate",
                        )
                        or 0
                    )
                    borrowed_representation_bytes = (
                        allocation_medians(
                            borrowed,
                            scope,
                            algorithm,
                            "representation_retained_bytes_estimate",
                        )
                        or 0
                    )
                    representation_bytes_before.append(clone_representation_bytes)
                    representation_bytes_after.append(borrowed_representation_bytes)
                    clone_ownership_allocations = (
                        allocation_medians(
                            clone,
                            scope,
                            algorithm,
                            "ownership_vec_allocation_count_estimate",
                        )
                        or 0
                    )
                    borrowed_ownership_allocations = (
                        allocation_medians(
                            borrowed,
                            scope,
                            algorithm,
                            "ownership_vec_allocation_count_estimate",
                        )
                        or 0
                    )
                    ownership_allocations_before.append(clone_ownership_allocations)
                    ownership_allocations_after.append(borrowed_ownership_allocations)
                    if clone_bytes:
                        byte_reductions.append(1 - borrowed_bytes / clone_bytes)
                    if size == family_summary["largest_level"]:
                        phase_values = {
                            f"{backend}:{phase}": run_medians(
                                point, scope, algorithm, phase
                            )
                            or 0
                            for backend, point in (
                                ("clone", clone),
                                ("borrowed", borrowed),
                            )
                            for phase in PHASES
                        }
                        phase_by_level[key] = phase_values
                    csv_rows.append(
                        {
                            "family": family,
                            "target_size": size,
                            "algorithm": algorithm,
                            "scope": scope,
                            "clone_median_ns": before,
                            "borrowed_median_ns": after,
                            "scope_speedup": before / after,
                            "clone_phase_ns": clone_phase,
                            "borrowed_phase_ns": borrowed_phase,
                            "clone_borrow_or_share_ns": clone_borrow_share,
                            "borrowed_borrow_or_share_ns": borrowed_borrow_share,
                            "clone_release_ns": clone_release,
                            "borrowed_release_ns": borrowed_release,
                            "ownership_lifecycle_ratio": (
                                ownership_before / ownership_after
                            )
                            if ownership_after > 0
                            else None,
                            "workspace_prepare_delta_ns": borrowed_workspace
                            - clone_workspace,
                            "clone_workspace_prepare_ns": clone_workspace,
                            "borrowed_workspace_prepare_ns": borrowed_workspace,
                            "representation_ratio": (
                                borrowed_representation / clone_representation
                            )
                            if clone_representation
                            else None,
                            "canonical_clone_byte_reduction": (
                                1 - borrowed_bytes / clone_bytes
                            )
                            if clone_bytes
                            else None,
                            "clone_workspace_retained_bytes": clone_workspace_bytes,
                            "borrowed_workspace_retained_bytes": borrowed_workspace_bytes,
                            "clone_representation_retained_bytes": clone_representation_bytes,
                            "borrowed_representation_retained_bytes": borrowed_representation_bytes,
                            "clone_ownership_vec_allocations": clone_ownership_allocations,
                            "borrowed_ownership_vec_allocations": borrowed_ownership_allocations,
                        }
                    )
                stable_seed = sum(
                    (index + 1) * ord(character)
                    for index, character in enumerate(f"{family}:{algorithm}:{scope}")
                )
                ci = bootstrap_ci(
                    [value for value in ratios if math.isfinite(value)],
                    bootstrap_seed + stable_seed % 1000,
                    resamples=bootstrap_resamples,
                )
                algorithm_summary[key] = {
                    "complete_levels": complete_levels,
                    "scope_speedup_median": median(ratios),
                    "scope_speedup_bootstrap_ci_95": ci,
                    "material_scope_improvement": bool(ci and ci[0] > 1.0),
                    "clone_phase_speedup_median": median(
                        [value for value in clone_phase_ratios if math.isfinite(value)]
                    ),
                    "clone_phase_speedup_status": (
                        "eliminated-denominator-zero"
                        if clone_phase_before
                        and max(clone_phase_before) > 0
                        and max(clone_phase_after) == 0
                        else "finite"
                    ),
                    "clone_phase_before_median_ns": median(clone_phase_before),
                    "clone_phase_after_median_ns": median(clone_phase_after),
                    "canonical_borrow_or_share_before_median_ns": median(
                        borrow_share_before
                    ),
                    "canonical_borrow_or_share_after_median_ns": median(
                        borrow_share_after
                    ),
                    "canonical_release_before_median_ns": median(release_before),
                    "canonical_release_after_median_ns": median(release_after),
                    "ownership_lifecycle_speedup_median": median(
                        ownership_lifecycle_ratios
                    ),
                    "deep_clone_eliminated": bool(
                        clone_phase_before
                        and max(clone_phase_before) > 0
                        and max(clone_phase_after) == 0
                    ),
                    "workspace_prepare_delta_median_ns": median(workspace_overheads),
                    "workspace_prepare_before_median_ns": median(workspace_before_ns),
                    "workspace_prepare_after_median_ns": median(workspace_after_ns),
                    "representation_phase_ratio_median": median(representation_changes),
                    "canonical_clone_byte_reduction_median": median(byte_reductions),
                    "canonical_clone_bytes_before_median": median(clone_bytes_before),
                    "canonical_clone_bytes_after_median": median(clone_bytes_after),
                    "solver_workspace_retained_bytes_before_median": median(
                        workspace_bytes_before
                    ),
                    "solver_workspace_retained_bytes_after_median": median(
                        workspace_bytes_after
                    ),
                    "representation_retained_bytes_before_median": median(
                        representation_bytes_before
                    ),
                    "representation_retained_bytes_after_median": median(
                        representation_bytes_after
                    ),
                    "ownership_vec_allocations_before_median": median(
                        ownership_allocations_before
                    ),
                    "ownership_vec_allocations_after_median": median(
                        ownership_allocations_after
                    ),
                    "amdahl_ownership_removed_fraction_median": median(
                        ownership_removed_fractions
                    ),
                    "amdahl_ideal_speedup_median": (
                        median(
                            [
                                1 / (1 - fraction)
                                for fraction in ownership_removed_fractions
                                if fraction < 1
                            ]
                        )
                        if ownership_removed_fractions
                        else None
                    ),
                    "empirical_slope_before": slope(before_points),
                    "empirical_slope_after": slope(after_points),
                    "largest_level_phases_ns": phase_by_level.get(key, {}),
                    "largest_level_dominant_phase_before": dominant_phase(
                        phase_by_level.get(key, {}), "clone"
                    ),
                    "largest_level_dominant_phase_after": dominant_phase(
                        phase_by_level.get(key, {}), "borrowed"
                    ),
                    "largest_level_dominant_phase_before_share": dominant_phase_share(
                        phase_by_level.get(key, {}), "clone"
                    ),
                    "largest_level_dominant_phase_after_share": dominant_phase_share(
                        phase_by_level.get(key, {}), "borrowed"
                    ),
                }
            family_summary["algorithms"][algorithm] = algorithm_summary
        summary["families"][family] = family_summary
    summary["csv_rows"] = csv_rows
    return summary


def dominant_phase(values: dict[str, float], backend: str) -> str | None:
    candidates = {
        phase: value
        for phase, value in values.items()
        if phase.startswith(f"{backend}:") and value > 0
    }
    if not candidates:
        return None
    return max(candidates, key=candidates.get).split(":", 1)[1]


def dominant_phase_share(values: dict[str, float], backend: str) -> float | None:
    candidates = {
        phase: value
        for phase, value in values.items()
        if phase.startswith(f"{backend}:") and value > 0
    }
    total = sum(candidates.values())
    return None if not candidates or total <= 0 else max(candidates.values()) / total


def render_report(summary: dict[str, Any]) -> str:
    validation = summary["validation"]
    lines = [
        "# P18 Canonical Sharing Report",
        "",
        "This report separates measured facts from implementation explanations and empirical inferences. It does not claim a new asymptotic algorithm.",
        "",
        f"- Source commit: `{summary['source_commit']}`",
        f"- Release binary SHA-256: `{summary['binary_sha256']}`",
        f"- P18 config SHA-256: `{summary['config_sha256']}`",
        "- Backends: `clone-canonical-reference` and `borrowed-canonical`",
        f"- Paired point census: {validation['paired_points']}; semantic mismatches: {sum(value for key, value in validation['mismatches'].items() if key != 'adaptive_count')}; adaptive-count differences: {validation['mismatches'].get('adaptive_count', 0)}",
        "- P9.5e.3g.3 remains blocked; P9.6a remains deferred; P18 does not establish AN19 runtime or automatic target decision.",
        "",
        "## Census and outcome",
        "",
    ]
    for backend in BACKENDS:
        stats = validation["backends"][backend]
        lines.append(
            f"- `{backend}`: {stats['planned_points']} planned, "
            f"{stats['terminal_points']} terminal, "
            f"{stats['state_counts'].get('complete', 0)} complete, "
            f"{stats['state_counts'].get('stopped', 0)} stopped, "
            f"{stats['measured_runs']} measured rows, "
            f"{stats['correctness_records']} correctness records, "
            f"{stats['retry_history']} retained retries."
        )
    lines.extend(
        [
            "",
            "Deep canonical cloning was eliminated on the borrowed path, but the finite paired campaign does not by itself establish a general material Scope A speedup. A predeclared confidence interval crossing 1.0 is reported as a negative result.",
            "",
            "## Results",
            "",
        ]
    )
    for family, family_summary in summary["families"].items():
        lines.append(f"### {family}")
        lines.append("")
        for algorithm, algorithm_summary in family_summary["algorithms"].items():
            scope_a = algorithm_summary.get("scope_a", {})
            scope_b = algorithm_summary.get("scope_b", {})
            lines.append(
                f"- `{algorithm}`: Scope A speedup median {fmt(scope_a.get('scope_speedup_median'))}, "
                f"95% bootstrap CI {fmt_ci(scope_a.get('scope_speedup_bootstrap_ci_95'))}; "
                f"deep clone eliminated={scope_a.get('deep_clone_eliminated')}, "
                f"clone acquisition {fmt(scope_a.get('clone_phase_before_median_ns'))}->{fmt(scope_a.get('clone_phase_after_median_ns'))} ns, "
                f"borrow/share {fmt(scope_a.get('canonical_borrow_or_share_before_median_ns'))}->{fmt(scope_a.get('canonical_borrow_or_share_after_median_ns'))} ns, "
                f"release {fmt(scope_a.get('canonical_release_before_median_ns'))}->{fmt(scope_a.get('canonical_release_after_median_ns'))} ns; "
                f"workspace {fmt(scope_a.get('workspace_prepare_before_median_ns'))}->{fmt(scope_a.get('workspace_prepare_after_median_ns'))} ns; "
                f"largest-level dominant phase `{scope_a.get('largest_level_dominant_phase_before')}` -> `{scope_a.get('largest_level_dominant_phase_after')}`; "
                f"Amdahl removed fraction {fmt(scope_a.get('amdahl_ownership_removed_fraction_median'))}, "
                f"ideal speedup {fmt(scope_a.get('amdahl_ideal_speedup_median'))}."
            )
            lines.append(
                f"  Scope B speedup median {fmt(scope_b.get('scope_speedup_median'))}, "
                f"95% bootstrap CI {fmt_ci(scope_b.get('scope_speedup_bootstrap_ci_95'))}; "
                f"representation ratio {fmt(scope_a.get('representation_phase_ratio_median'))}; "
                f"clone-byte reduction {fmt(scope_a.get('canonical_clone_byte_reduction_median'))} "
                f"({fmt(scope_a.get('canonical_clone_bytes_before_median'))}->{fmt(scope_a.get('canonical_clone_bytes_after_median'))} bytes); "
                f"workspace bytes {fmt(scope_a.get('solver_workspace_retained_bytes_before_median'))}->{fmt(scope_a.get('solver_workspace_retained_bytes_after_median'))}, "
                f"representation bytes {fmt(scope_a.get('representation_retained_bytes_before_median'))}->{fmt(scope_a.get('representation_retained_bytes_after_median'))}; "
                f"ownership Vec allocation estimate {fmt(scope_a.get('ownership_vec_allocations_before_median'))}->{fmt(scope_a.get('ownership_vec_allocations_after_median'))}; "
                f"empirical slope before/after {fmt(scope_a.get('empirical_slope_before'))}/{fmt(scope_a.get('empirical_slope_after'))}."
            )
        lines.append("")
    lines.extend(
        [
            "## Claim classification",
            "",
            "- **Measured fact:** paired medians, phase timings, structural-byte estimates, and terminal point states are taken directly from the versioned records.",
            "- **Implementation explanation:** the borrowed path removes the deep `Vec<Cell>` copy; solver workspace preparation remains separately timed.",
            "- **Empirical inference:** a Scope A CI wholly above 1.0 supports a host- and family-specific improvement for that pair only.",
            "- **Rejected performance hypothesis:** where the predeclared Scope A bootstrap interval crosses 1.0, clone removal is not called a material speedup.",
            "- **Rejected theoretical hypothesis:** clone removal is not evidence for a new asymptotic complexity class.",
            "- **Unresolved optimization:** representation construction is audited after clone closeout; no selector, hybrid policy, or zero-conflict shortcut is implemented here.",
            "- **Theoretical claim:** none is made about AN19 runtime, automatic target choice, or unmeasured hosts and families.",
            "",
            "The removed payload is exactly proportional to `N`: one `GridComponent.cells` buffer containing `N` `Cell` values. `B`, `U`, and `q` explain downstream geometry and representation work, not the canonical copy itself. Structural-byte fields are estimates; RSS was not measured.",
        ]
    )
    return "\n".join(lines) + "\n"


def fmt(value: Any) -> str:
    return "n/a" if value is None else f"{value:.4g}"


def fmt_ci(value: Any) -> str:
    if not value:
        return "n/a"
    return f"[{value[0]:.4g}, {value[1]:.4g}]"


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fields = sorted({key for row in rows for key in row})
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def self_test() -> None:
    protocol = {
        "p18_schema_version": 1,
        "schema_version": 2,
        "campaign": "paper-kernel-scaling",
        "canonical_backends": list(BACKENDS),
        "boundary_discovery_backend": "prepared-exposed-edges",
        "families": ["comb-staircase"],
        "initial_size_levels": [16, 32, 64, 128, 256, 512],
        "algorithms": list(ALGORITHMS),
        "scopes": list(SCOPES),
        "seed": 42,
        "warmup_rule": {
            "minimum": 5,
            "maximum": 5,
            "cv_threshold_ppm": 50_000,
        },
        "repetition_rule": {
            "target_measured_ns": 1,
            "fast_threshold_ns": 10_000_000,
            "medium_threshold_ns": 100_000_000,
            "fast_minimum": 2,
            "medium_minimum": 2,
            "slow_minimum": 2,
            "maximum": 2,
        },
        "fit_rule": {
            "minimum_valid_size_levels": 6,
            "bootstrap_resamples": 10_000,
            "bootstrap_seed": 604_019,
        },
    }
    environment = {
        field: False if field == "git_dirty" else "a"
        for field in PROVENANCE_ENVIRONMENT_FIELDS
    }
    environment["git_commit"] = "a"
    environment["binary_sha256"] = "b"

    def point(backend: str, planned: dict[str, Any]) -> dict[str, Any]:
        size = planned["target_size"]
        clone_path = backend == BACKENDS[0]
        runs = []
        warmups = []
        for scope in SCOPES:
            for algorithm_index, algorithm in enumerate(ALGORITHMS):
                warmups.append(
                    {
                        "canonical_backend": backend,
                        "scope": scope,
                        "algorithm": algorithm,
                        "count": 5,
                        "preflight_ns": 1,
                        "measured_repetitions": 2,
                    }
                )
                for iteration in range(2):
                    identity = (
                        "paper-kernel-scaling:v2:comb-staircase:"
                        f"{size}:42:same-{size}:measured:"
                        f"{'scope-a' if scope == SCOPES[0] else 'scope-b'}:"
                        f"{algorithm}:{iteration}:prepared-exposed-edges"
                    )
                    scope_a = scope == SCOPES[0]
                    timings = {
                        "scope_a_total_ns": size * (10 if clone_path else 8)
                        if scope_a
                        else None,
                        "scope_b_total_ns": size * 3 if not scope_a else None,
                        "canonical_component_clone_ns": 20
                        if clone_path and scope_a
                        else 0,
                        "canonical_context_borrow_or_share_ns": 1
                        if not clone_path and scope_a
                        else 0,
                        "canonical_component_release_ns": 2
                        if clone_path and scope_a
                        else 1
                        if scope_a
                        else 0,
                        "solver_workspace_prepare_ns": 5 if scope_a else 0,
                        "representation_construction_ns": 10,
                    }
                    for field in REQUIRED_TIMING_FIELDS:
                        timings.setdefault(field, None)
                    for parent, leaves, prefix in NESTED_TIMING_GROUPS:
                        if timings.get(parent) is None:
                            for field in (
                                *leaves,
                                f"{prefix}_leaf_sum_ns",
                                f"{prefix}_unattributed_ns",
                                f"{prefix}_accounting_ok",
                            ):
                                timings[field] = None
                    scope_phases = SCOPE_A_PHASES if scope_a else SCOPE_B_PHASES
                    scope_prefix = "scope_a" if scope_a else "scope_b"
                    scope_total = timings[f"{scope_prefix}_total_ns"]
                    scope_leaf_sum = sum(timings[field] or 0 for field in scope_phases)
                    timings[f"{scope_prefix}_leaf_sum_ns"] = scope_leaf_sum
                    timings[f"{scope_prefix}_unattributed_ns"] = (
                        scope_total - scope_leaf_sum
                    )
                    timings[f"{scope_prefix}_accounting_ok"] = True
                    runs.append(
                        {
                            "sample_identity": identity,
                            "record_kind": "measured",
                            "seed": 42,
                            "boundary_discovery_backend": "prepared-exposed-edges",
                            "canonical_backend": backend,
                            "canonical_instance_identity": f"same-{size}",
                            "scope": scope,
                            "algorithm": algorithm,
                            "iteration": iteration,
                            "order_position": algorithm_index,
                            "elapsed_ns": size * (10 if clone_path else 8)
                            if scope_a
                            else size * 3,
                            "timings": timings,
                            "allocations": {
                                "canonical_cells_cloned": 4
                                if clone_path and scope_a
                                else 0,
                                "canonical_clone_bytes_estimate": 64
                                if clone_path and scope_a
                                else 0,
                                "solver_workspace_retained_bytes_estimate": 4
                                if scope_a
                                else 0,
                                "representation_retained_bytes_estimate": 16,
                                "ownership_vec_allocation_count_estimate": 3
                                if clone_path and scope_a
                                else 2
                                if scope_a
                                else 0,
                            },
                        }
                    )
        return {
            **planned,
            "family": "comb-staircase",
            "target_size": size,
            "state": "complete",
            "boundary_discovery_backend": "prepared-exposed-edges",
            "canonical_backend": backend,
            "canonical_instance_identity": f"same-{size}",
            "sizes": {"foreground_cells_n": 4, "optimum_rectangle_count": 1},
            "structure": {"explicit_graph_edge_count": 0},
            "correctness": [
                {
                    "algorithm": algorithm,
                    "outcome": "success",
                    "canonical_backend": backend,
                    "optimum_rectangle_count": 1,
                    "witness_checksum": "same-witness",
                }
                for algorithm in ALGORITHMS
            ],
            "warmups": warmups,
            "runs": runs,
            "exact_measured_order": [run["sample_identity"] for run in runs],
        }

    backends: dict[str, Any] = {}
    sizes = [16, 32, 64, 128, 256, 512]
    for backend in BACKENDS:
        backend_protocol = expected_backend_config(protocol, backend)
        config_sha = sha256_json(backend_protocol)
        planned = expected_backend_plan(protocol, backend)
        points = [point(backend, row) for row in planned]
        completion = {
            "complete": True,
            "planned_point_count": len(sizes),
            "observed_point_count": len(sizes),
            "completed_point_count": len(sizes),
            "missing_point_count": 0,
            "missing_point_identities": [],
            "terminal_state_counts": {"complete": len(sizes)},
            "correctness_failure_count": 0,
            "retry_history_count": 0,
        }
        backends[backend] = {
            "campaign": "paper-kernel-scaling",
            "protocol": backend_protocol,
            "config_sha256": config_sha,
            "campaign_identity": expected_campaign_identity(config_sha, "a", "b"),
            "source_commit": "a",
            "binary_sha256": "b",
            "environment": environment,
            "completion": completion,
            "planned_points": planned,
            "point_results": points,
            "retry_history": [],
        }

    payload = {
        "schema_version": SCHEMA_VERSION,
        "campaign": "p18-canonical-sharing",
        "protocol": protocol,
        "config_sha256": sha256_json(protocol),
        "canonical_backends": list(BACKENDS),
        "source_commit": "a",
        "binary_sha256": "b",
        "environment": environment,
        "backends": backends,
        "completion": {
            "complete": True,
            "backend_completion": {
                backend: backends[backend]["completion"] for backend in BACKENDS
            },
        },
    }
    summary = analyze(payload)
    assert summary["validation"]["paired_points"] == len(sizes)
    assert summary["validation"]["mismatches"]["sample_order"] == 0
    assert summary["validation"]["mismatches"]["adaptive_count"] == 0
    assert summary["families"]["comb-staircase"]["algorithms"][ALGORITHMS[0]][
        "scope_a"
    ]["deep_clone_eliminated"]

    malformed = strict_json_loads(json.dumps(payload, allow_nan=False))
    malformed["config_sha256"] = "wrong"
    try:
        validate_input(malformed)
    except ValueError as error:
        assert "config SHA-256" in str(error)
    else:
        raise AssertionError("malformed config provenance was accepted")

    missing_point = strict_json_loads(json.dumps(payload, allow_nan=False))
    backend_payload = missing_point["backends"][BACKENDS[0]]
    backend_payload["planned_points"].pop()
    backend_payload["point_results"].pop()
    try:
        validate_input(missing_point)
    except ValueError as error:
        assert "predeclared protocol" in str(error)
    else:
        raise AssertionError("incomplete predeclared point census was accepted")

    missing_sample = strict_json_loads(json.dumps(payload, allow_nan=False))
    sample_point = missing_sample["backends"][BACKENDS[0]]["point_results"][0]
    sample_point["runs"].pop()
    sample_point["exact_measured_order"].pop()
    try:
        validate_input(missing_sample)
    except ValueError as error:
        assert "incomplete sample census" in str(error)
    else:
        raise AssertionError("incomplete measured sample census was accepted")

    swapped_identity = strict_json_loads(json.dumps(payload, allow_nan=False))
    swapped_points = swapped_identity["backends"][BACKENDS[0]]["point_results"]
    swapped_points[0]["point_identity"], swapped_points[1]["point_identity"] = (
        swapped_points[1]["point_identity"],
        swapped_points[0]["point_identity"],
    )
    try:
        validate_input(swapped_identity)
    except ValueError as error:
        assert "differs from its planned identity" in str(error)
    else:
        raise AssertionError("swapped point identities were accepted")

    weakened_repetitions = strict_json_loads(json.dumps(payload, allow_nan=False))
    weakened_point = weakened_repetitions["backends"][BACKENDS[0]]["point_results"][0]
    for warmup in weakened_point["warmups"]:
        warmup["measured_repetitions"] = 1
    weakened_point["runs"] = [
        run for run in weakened_point["runs"] if run["iteration"] == 0
    ]
    weakened_point["exact_measured_order"] = [
        run["sample_identity"] for run in weakened_point["runs"]
    ]
    try:
        validate_input(weakened_repetitions)
    except ValueError as error:
        assert "adaptive rule" in str(error)
    else:
        raise AssertionError("weakened adaptive repetition census was accepted")

    negative_timing = strict_json_loads(json.dumps(payload, allow_nan=False))
    negative_run = negative_timing["backends"][BACKENDS[0]]["point_results"][0]["runs"][
        0
    ]
    negative_run["timings"]["scope_a_total_ns"] = -1
    negative_run["elapsed_ns"] = -1
    try:
        validate_input(negative_timing)
    except ValueError as error:
        assert "nonnegative integer" in str(error)
    else:
        raise AssertionError("negative timing was accepted")

    bad_allocation = strict_json_loads(json.dumps(payload, allow_nan=False))
    bad_allocation["backends"][BACKENDS[0]]["point_results"][0]["runs"][0][
        "allocations"
    ]["canonical_cells_cloned"] = 0
    try:
        validate_input(bad_allocation)
    except ValueError as error:
        assert "clone-reference cells" in str(error)
    else:
        raise AssertionError("inconsistent ownership allocation was accepted")

    bad_scope_b_allocation = strict_json_loads(json.dumps(payload, allow_nan=False))
    bad_scope_b_allocation["backends"][BACKENDS[0]]["point_results"][0]["runs"][6][
        "allocations"
    ]["solver_workspace_retained_bytes_estimate"] = 1
    try:
        validate_input(bad_scope_b_allocation)
    except ValueError as error:
        assert "Scope B" in str(error)
    else:
        raise AssertionError("Scope B ownership allocation was accepted")

    missing_timing = strict_json_loads(json.dumps(payload, allow_nan=False))
    missing_timing["backends"][BACKENDS[0]]["point_results"][0]["runs"][0][
        "timings"
    ].pop("representation_construction_ns")
    try:
        validate_input(missing_timing)
    except ValueError as error:
        assert "required fields" in str(error)
    else:
        raise AssertionError("missing timing field was accepted")

    missing_witness = strict_json_loads(json.dumps(payload, allow_nan=False))
    missing_witness["backends"][BACKENDS[0]]["point_results"][0]["correctness"][0][
        "witness_checksum"
    ] = None
    try:
        validate_input(missing_witness)
    except ValueError as error:
        assert "correctness failure" in str(error)
    else:
        raise AssertionError("missing witness checksum was accepted")

    mismatched_order = strict_json_loads(json.dumps(payload, allow_nan=False))
    second_point = mismatched_order["backends"][BACKENDS[1]]["point_results"][0]
    second_point["exact_measured_order"] = list(
        reversed(second_point["exact_measured_order"])
    )
    second_point["runs"] = list(reversed(second_point["runs"]))
    second_point["runs"][0]["order_position"] = 0
    try:
        validate_input(mismatched_order)
    except ValueError as error:
        assert "sample_order" in str(error) or "exact measured order" in str(error)
    else:
        raise AssertionError("cross-backend sample order mismatch was accepted")

    wrong_boundary = strict_json_loads(json.dumps(payload, allow_nan=False))
    wrong_boundary["backends"][BACKENDS[0]]["point_results"][0]["runs"][0][
        "boundary_discovery_backend"
    ] = "reference-edge-toggle"
    try:
        validate_input(wrong_boundary)
    except ValueError as error:
        assert "boundary backend" in str(error)
    else:
        raise AssertionError("wrong run boundary backend was accepted")

    missing_environment = strict_json_loads(json.dumps(payload, allow_nan=False))
    missing_environment["environment"].pop("cpu_model")
    try:
        validate_input(missing_environment)
    except ValueError as error:
        assert "omits provenance fields" in str(error)
    else:
        raise AssertionError("missing environment field was accepted")

    changed_power_source = strict_json_loads(json.dumps(payload, allow_nan=False))
    changed_power_source["backends"][BACKENDS[1]]["environment"]["power_source"] = (
        "battery"
    )
    try:
        validate_input(changed_power_source)
    except ValueError as error:
        assert "power_source" in str(error)
    else:
        raise AssertionError("changed power source was accepted")

    duplicate_retry = strict_json_loads(json.dumps(payload, allow_nan=False))
    duplicate_backend = duplicate_retry["backends"][BACKENDS[0]]
    retry = {
        **duplicate_backend["planned_points"][0],
        "state": "runner-error",
        "message": "same failure",
    }
    duplicate_backend["retry_history"] = [retry, dict(retry)]
    duplicate_backend["completion"]["retry_history_count"] = 2
    duplicate_retry["completion"]["backend_completion"][BACKENDS[0]][
        "retry_history_count"
    ] = 2
    try:
        validate_input(duplicate_retry)
    except ValueError as error:
        assert "exact duplicate" in str(error)
    else:
        raise AssertionError("duplicate retry record was accepted")

    try:
        strict_json_loads('{"value": NaN}')
    except ValueError as error:
        assert "non-finite JSON number" in str(error)
    else:
        raise AssertionError("non-finite JSON was accepted")
    print("p18 canonical-sharing analyzer self-test: ok")


def main() -> int:
    arguments = parse_args()
    if arguments.self_test:
        self_test()
        return 0
    raw = strict_json_loads(arguments.input.read_text())
    summary = analyze(raw)
    arguments.summary_json.parent.mkdir(parents=True, exist_ok=True)
    arguments.summary_json.write_text(
        json.dumps(
            {key: value for key, value in summary.items() if key != "csv_rows"},
            allow_nan=False,
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    write_csv(arguments.summary_csv, summary["csv_rows"])
    arguments.report.parent.mkdir(parents=True, exist_ok=True)
    arguments.report.write_text(render_report(summary))
    print(
        json.dumps(
            {"families": len(summary["families"]), "rows": len(summary["csv_rows"])},
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
