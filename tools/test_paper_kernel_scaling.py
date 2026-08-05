#!/usr/bin/env python3
"""Protocol tests for the schema-v2 paper-kernel runner."""

from __future__ import annotations

import copy
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]


def load_runner():
    specification = importlib.util.spec_from_file_location(
        "run_paper_kernel_scaling",
        ROOT / "tools/run_paper_kernel_scaling.py",
    )
    assert specification is not None and specification.loader is not None
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


runner = load_runner()


def protocol(sizes: list[int] | None = None) -> dict[str, Any]:
    return {
        "schema_version": 2,
        "campaign": runner.CAMPAIGN,
        "boundary_discovery_backend": "reference-edge-toggle",
        "families": ["random-connected"],
        "initial_size_levels": sizes or [64],
        "algorithms": list(runner.ALGORITHMS),
        "scopes": list(runner.SCOPES),
        "seed": 42,
        "oracle_cell_limit": 40,
        "family_parameter_rule": {
            "comb-staircase": "ceil(sqrt(target_size))",
            "representation-crossover": "ceil(sqrt(target_size))",
            "all_other_families": "target_size",
        },
        "warmup_rule": {
            "minimum": 5,
            "maximum": 5,
            "cv_threshold_ppm": 50_000,
        },
        "repetition_rule": {
            "target_measured_ns": 1,
            "fast_threshold_ns": 10_000_000,
            "medium_threshold_ns": 100_000_000,
            "fast_minimum": 31,
            "medium_minimum": 15,
            "slow_minimum": 7,
            "maximum": 31,
        },
        "stop_conditions": {
            "max_explicit_edges": 1_000_000,
            "max_iteration_ns": 5_000_000_000,
            "max_point_ns": 120_000_000_000,
            "max_estimated_structural_bytes": 1_000_000_000,
        },
        "partition_timeout_seconds": 130,
        "fit_rule": {
            "minimum_valid_size_levels": 6,
            "bootstrap_resamples": 10_000,
            "bootstrap_seed": 604_019,
        },
    }


def point_for(config: dict[str, Any], size: int = 64) -> dict[str, Any]:
    config_hash = runner.sha256_bytes(runner.canonical_json(config).encode())
    return next(row for row in runner.plan(config, config_hash) if row["target_size"] == size)


def valid_point(config: dict[str, Any], point: dict[str, Any]) -> dict[str, Any]:
    canonical = "abc123"
    backend = config["boundary_discovery_backend"]
    sizes = {
        "width": 4,
        "height": 4,
        "bounding_box_area_a": 16,
        "foreground_cells_n": 8,
        "component_count": 1,
        "boundary_size_b": 8,
        "boundary_unit_edge_count_u": 12,
        "reflex_count": 4,
        "horizontal_chord_count_h": 2,
        "vertical_chord_count_v": 3,
        "q": 5,
        "explicit_conflict_edge_count_k": 4,
        "biclique_count": 1,
        "biclique_total_vertex_occurrences_sigma": 5,
        "compressed_network_node_count": 6,
        "compressed_network_arc_count": 7,
        "compressed_representation_size_m": 13,
        "optimum_rectangle_count": 3,
        "output_rectangle_count": 3,
    }
    correctness = [
        {
            "boundary_discovery_backend": backend,
            "algorithm": algorithm,
            "outcome": "success",
            "optimum_rectangle_count": 3,
            "matching_size": 2,
            "vertex_cover_size": 2,
            "witness_checksum": "feedface",
            "message": None,
        }
        for algorithm in runner.ALGORITHMS
    ]
    warmups = [
        {
            "boundary_discovery_backend": backend,
            "scope": scope,
            "algorithm": algorithm,
            "count": 5,
            "converged": True,
            "last_five_cv_ppm": 10,
            "preflight_ns": 1_000,
            "measured_repetitions": 31,
        }
        for scope in runner.SCOPES
        for algorithm in runner.ALGORITHMS
    ]

    def timings_for(scope: str, elapsed_ns: int) -> dict[str, Any]:
        timings = {field: None for field in runner.TIMING_FIELDS}
        prefix = "scope_a" if scope == runner.SCOPES[0] else "scope_b"
        if scope == runner.SCOPES[0]:
            parents = {
                "canonical_component_clone_ns": 1,
                "geometry_preprocessing_ns": 1,
                "chord_generation_ns": 1,
                "chord_selection_ns": 1,
                "rectangle_completion_recovery_ns": 1,
                "output_validation_ns": 1,
            }
            timings.update(parents)
            timings["boundary_leaf_sum_ns"] = None
            timings["boundary_unattributed_ns"] = None
            timings["boundary_accounting_ok"] = None
            for nested in ("geometry", "chord", "completion", "output_validation"):
                timings[f"{nested}_leaf_sum_ns"] = 0
                timings[f"{nested}_unattributed_ns"] = 1
                timings[f"{nested}_accounting_ok"] = True
        timings[f"{prefix}_total_ns"] = elapsed_ns
        timings[f"{prefix}_leaf_sum_ns"] = 6 if scope == runner.SCOPES[0] else 0
        timings[f"{prefix}_unattributed_ns"] = elapsed_ns - timings[f"{prefix}_leaf_sum_ns"]
        timings[f"{prefix}_accounting_ok"] = True
        return timings

    shared = {field: None for field in runner.TIMING_FIELDS}
    shared["boundary_leaf_sum_ns"] = None
    shared["boundary_unattributed_ns"] = None
    shared["boundary_accounting_ok"] = None
    shared["chord_leaf_sum_ns"] = 0
    shared["chord_unattributed_ns"] = 1
    shared["chord_accounting_ok"] = True
    shared["geometry_preprocessing_ns"] = 1
    shared["geometry_leaf_sum_ns"] = 0
    shared["geometry_unattributed_ns"] = 1
    shared["geometry_accounting_ok"] = True
    shared["chord_generation_ns"] = 1
    runs = []
    for iteration in range(31):
        for scope in runner.SCOPES:
            for order_position, algorithm in enumerate(runner.ALGORITHMS):
                elapsed_ns = 1_000 + iteration
                total_field = (
                    "scope_a_total_ns"
                    if scope == "solve-from-canonical-instance"
                    else "scope_b_total_ns"
                )
                runs.append(
                    {
                        "sample_identity": runner.sample_identity(
                            point,
                            canonical,
                            backend,
                            scope,
                            algorithm,
                            iteration,
                        ),
                        "record_kind": "measured",
                        "seed": point["seed"],
                        "canonical_instance_identity": canonical,
                        "boundary_discovery_backend": backend,
                        "scope": scope,
                        "algorithm": algorithm,
                        "iteration": iteration,
                        "order_position": order_position,
                        "elapsed_ns": elapsed_ns,
                        "timings": timings_for(scope, elapsed_ns),
                        "optimum_rectangle_count": 3,
                        "matching_size": 2,
                        "vertex_cover_size": 2,
                        "witness_checksum": "feedface",
                        "consumed_checksum": "deadbeef",
                    }
                )
    return {
        **point,
        "schema_version": 2,
        "campaign": runner.CAMPAIGN,
        "generator_version": "test-v1",
        "generator_parameter": point["target_size"],
        "boundary_discovery_backend": backend,
        "canonical_instance_identity": canonical,
        "state": "complete",
        "message": None,
        "sizes": sizes,
        "structure": {
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
        },
        "setup_timings": {
            "instance_generation_ns": 5,
            "input_normalization_ns": 10,
            "connected_component_extraction_ns": 15,
            "setup_total_ns": 35,
        },
        "correctness": correctness,
        "shared_scope_b_preprocessing": shared,
        "warmups": warmups,
        "runs": runs,
        "exact_measured_order": [row["sample_identity"] for row in runs],
    }


class KernelRunnerProtocolTests(unittest.TestCase):
    def test_schema_one_config_is_rejected_clearly(self) -> None:
        config = protocol()
        config["schema_version"] = 1
        with self.assertRaisesRegex(ValueError, "incompatible.*schema_version 1.*expected 2"):
            runner.validate_config(config)

    def test_complete_point_requires_exact_canonical_sample_census(self) -> None:
        config = protocol()
        point = point_for(config)
        result = valid_point(config, point)
        runner.validate_point_result(result, point, config)

        missing = copy.deepcopy(result)
        missing["runs"].pop()
        missing["exact_measured_order"].pop()
        with self.assertRaisesRegex(ValueError, "exact measured sample census"):
            runner.validate_point_result(missing, point, config)

        malformed = copy.deepcopy(result)
        malformed["runs"][0]["sample_identity"] += ":wrong"
        malformed["exact_measured_order"][0] = malformed["runs"][0]["sample_identity"]
        with self.assertRaisesRegex(ValueError, "canonical fields"):
            runner.validate_point_result(malformed, point, config)

    def test_point_header_and_structural_identities_are_validated(self) -> None:
        config = protocol()
        point = point_for(config)
        wrong_seed = valid_point(config, point)
        wrong_seed["seed"] += 1
        with self.assertRaisesRegex(ValueError, "seed differs"):
            runner.validate_point_result(wrong_seed, point, config)

        wrong_q = valid_point(config, point)
        wrong_q["sizes"]["q"] += 1
        with self.assertRaisesRegex(ValueError, "q differs"):
            runner.validate_point_result(wrong_q, point, config)

        wrong_backend = valid_point(config, point)
        wrong_backend["boundary_discovery_backend"] = "prepared-exposed-edges"
        with self.assertRaisesRegex(ValueError, "backend differs"):
            runner.validate_point_result(wrong_backend, point, config)

    def test_nonterminal_runner_failure_is_retried_on_resume(self) -> None:
        config = protocol()
        with tempfile.TemporaryDirectory(prefix="kernel-runner-test-") as directory:
            root = Path(directory)
            binary = root / "mrd"
            binary.write_bytes(b"test binary")
            checkpoint = root / "checkpoint.json"

            def fail(_binary: Path, _config: dict[str, Any], point: dict[str, Any]):
                return {
                    **point,
                    "state": "runner-error",
                    "message": "interrupted",
                    "partition_wall_time_ns": 1,
                }

            first = runner.run_campaign(
                config,
                binary,
                checkpoint,
                launch_partition=fail,
            )
            self.assertFalse(first["completion"]["complete"])
            self.assertEqual(first["completion"]["missing_point_count"], 1)

            calls: list[str] = []

            def succeed(_binary: Path, _config: dict[str, Any], point: dict[str, Any]):
                calls.append(point["point_identity"])
                return valid_point(config, point)

            resumed = runner.run_campaign(
                config,
                binary,
                checkpoint,
                resume=True,
                launch_partition=succeed,
            )
            self.assertTrue(resumed["completion"]["complete"])
            self.assertEqual(len(calls), 1)
            self.assertEqual(len(resumed["retry_history"]), 1)
            self.assertEqual(resumed["completion"]["retry_history_count"], 1)

    def test_checkpoint_campaign_identity_mismatch_is_rejected(self) -> None:
        config = protocol()
        with tempfile.TemporaryDirectory(prefix="kernel-runner-test-") as directory:
            root = Path(directory)
            binary = root / "mrd"
            binary.write_bytes(b"test binary")
            checkpoint_path = root / "checkpoint.json"

            runner.run_campaign(
                config,
                binary,
                checkpoint_path,
                launch_partition=lambda _binary, _config, point: valid_point(
                    config, point
                ),
            )
            checkpoint = json.loads(checkpoint_path.read_text())
            expected = runner.provenance(config, binary)
            planned = runner.plan(config, expected["config_sha256"])
            checkpoint["campaign_identity"] = "sha256:different"
            with self.assertRaisesRegex(ValueError, "campaign_identity mismatch"):
                runner.validate_checkpoint(checkpoint, expected, planned, config)

    def test_stopped_point_preserves_partial_measured_rows(self) -> None:
        config = protocol()
        point = point_for(config)
        partial = valid_point(config, point)
        partial["state"] = "stopped"
        partial["message"] = "predeclared point limit"
        partial["runs"] = partial["runs"][:1]
        partial["exact_measured_order"] = [row["sample_identity"] for row in partial["runs"]]

        runner.validate_point_result(partial, point, config)
        self.assertEqual(len(partial["runs"]), 1)
        payload = {
            "config_sha256": "config",
            "campaign_identity": "campaign",
            "source_commit": "commit",
            "binary_sha256": "binary",
            "environment": {},
            "point_results": [partial],
        }
        self.assertEqual(len(runner.csv_rows(payload)), 1)

    def test_shared_preprocessing_parent_accounting_is_required(self) -> None:
        config = protocol()
        point = point_for(config)
        malformed = valid_point(config, point)
        malformed["shared_scope_b_preprocessing"]["geometry_preprocessing_ns"] += 1
        with self.assertRaisesRegex(ValueError, "shared preprocessing geometry timing accounting mismatch"):
            runner.validate_point_result(malformed, point, config)

    def test_checkpoint_rejects_each_reproducibility_identity_mismatch(self) -> None:
        config = protocol()
        point = point_for(config)
        with tempfile.TemporaryDirectory(prefix="kernel-runner-test-") as directory:
            binary = Path(directory) / "mrd"
            binary.write_bytes(b"test binary")
            expected = runner.provenance(config, binary)
            planned = runner.plan(config, expected["config_sha256"])
            checkpoint = {
                **expected,
                "protocol": config,
                "planned_points": planned,
                "point_results": [valid_point(config, point)],
                "retry_history": [],
            }
            for field in (
                "checkpoint_schema_version",
                "sample_schema_version",
                "config_sha256",
                "source_commit",
                "binary_sha256",
            ):
                with self.subTest(field=field):
                    malformed = copy.deepcopy(checkpoint)
                    malformed[field] = "different"
                    with self.assertRaisesRegex(ValueError, f"{field} mismatch"):
                        runner.validate_checkpoint(malformed, expected, planned, config)

    def test_completion_requires_valid_terminal_points_and_correctness(self) -> None:
        config = protocol()
        point = point_for(config)
        checkpoint = {
            "planned_points": [point],
            "point_results": [
                {
                    **point,
                    "state": "runner-timeout",
                    "message": "timeout",
                }
            ],
            "retry_history": [],
        }
        runner.refresh(checkpoint)
        self.assertFalse(checkpoint["completion"]["complete"])
        self.assertEqual(checkpoint["completion"]["missing_point_count"], 1)

        complete = valid_point(config, point)
        complete["correctness"][0]["outcome"] = "error"
        checkpoint["point_results"] = [complete]
        runner.refresh(checkpoint)
        self.assertFalse(checkpoint["completion"]["complete"])
        self.assertEqual(checkpoint["completion"]["correctness_failure_count"], 1)

    def test_csv_flattens_setup_timings_and_backend(self) -> None:
        config = protocol()
        point = point_for(config)
        result = valid_point(config, point)
        payload = {
            "config_sha256": "config",
            "campaign_identity": "campaign",
            "source_commit": "commit",
            "binary_sha256": "binary",
            "environment": {},
            "point_results": [result],
        }
        row = runner.csv_rows(payload)[0]
        self.assertEqual(row["boundary_discovery_backend"], "reference-edge-toggle")
        self.assertEqual(row["setup_timing_input_normalization_ns"], 10)
        self.assertIn("timing_scope_a_total_ns", row)


if __name__ == "__main__":
    unittest.main()
