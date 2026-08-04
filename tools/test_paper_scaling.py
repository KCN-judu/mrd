#!/usr/bin/env python3
"""Protocol tests for resumable paper-scaling campaign artifacts."""

from __future__ import annotations

import copy
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]


def load_module(name: str, path: Path):
    specification = importlib.util.spec_from_file_location(name, path)
    assert specification is not None and specification.loader is not None
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


runner = load_module("run_paper_scaling", ROOT / "tools/run_paper_scaling.py")
analyzer = load_module("analyze_paper_scaling", ROOT / "tools/analyze_paper_scaling.py")


def protocol() -> dict[str, Any]:
    return {
        "schema_version": 1,
        "campaign": "paper-scaling-test",
        "seed": 17,
        "oracle_cell_limit": 40,
        "families": ["random-connected"],
        "algorithms": ["compact-mrd", "explicit-hopcroft-karp"],
        "sizes": [1, 2, 3, 4, 5, 6],
        "warmups": 1,
        "repetitions": 3,
        "timeout_seconds": 1,
        "fit": {
            "minimum_target_size": 1,
            "minimum_size_levels": 6,
            "bootstrap_resamples": 10_000,
            "bootstrap_seed": 20260804,
        },
    }


class CheckpointProtocolTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="paper-scaling-tests-")
        self.directory = Path(self.temporary.name)
        self.binary = ROOT / "tools/run_paper_scaling.py"
        self.calls: list[str] = []

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def fake_launch(
        self,
        _binary: Path,
        request: dict[str, Any],
        pair_id: str,
        repetition: int,
        warmup: bool,
        execution_order: int,
        timeout_seconds: float,
    ) -> dict[str, Any]:
        record = runner.base_record(
            request,
            pair_id,
            repetition,
            warmup,
            execution_order,
            timeout_seconds,
            "success",
            0,
            100 + request["target_size"],
            None,
        )
        record.update(
            {
                "outcome": "success",
                "correctness": "valid",
                "optimum_rectangle_count": 2,
                "canonical_rectangles": [],
                "sizes": {
                    "foreground_cells_n": request["target_size"],
                    "boundary_size_b": request["target_size"] + 4,
                    "q": request["target_size"] + 1,
                    "explicit_conflict_edge_count_k": request["target_size"] + 2,
                    "compressed_network_node_count": request["target_size"] + 3,
                    "compressed_network_arc_count": request["target_size"] + 4,
                },
                "timings": {"total_in_process_solve_ns": 1},
            }
        )
        self.calls.append(f"{pair_id}:{request['algorithm']}:{warmup}:{repetition}")
        return record

    def checkpoint_path(self) -> Path:
        return self.directory / "checkpoint.json"

    def test_interrupted_run_resumes_only_missing_identities(self) -> None:
        config = protocol()
        first, _ = runner.run_campaign(
            config,
            self.binary,
            self.checkpoint_path(),
            sizes=[1],
            launch=self.fake_launch,
        )
        self.assertFalse(first["completion"]["complete"])
        first_calls = list(self.calls)
        second, _ = runner.run_campaign(
            config,
            self.binary,
            self.checkpoint_path(),
            resume=True,
            launch=self.fake_launch,
        )
        self.assertTrue(second["completion"]["complete"])
        self.assertEqual(second["completion"]["missing_record_count"], 0)
        self.assertEqual(len(second["records"]), len(second["planned_samples"]))
        self.assertEqual(len(self.calls), len(second["planned_samples"]))
        self.assertEqual(self.calls[: len(first_calls)], first_calls)
        self.assertEqual(
            [row["sample_identity"] for row in second["records"]],
            [sample["sample_identity"] for sample in second["planned_samples"]],
        )
        self.assertGreater(second["runner_wall_time_ns"], 0)
        self.assertEqual(len(second["runner_invocations"]), 2)
        self.assertTrue(all(not item["interrupted"] for item in second["runner_invocations"]))

    def test_duplicate_identity_is_rejected(self) -> None:
        config = protocol()
        runner.run_campaign(
            config,
            self.binary,
            self.checkpoint_path(),
            sizes=[1],
            launch=self.fake_launch,
        )
        checkpoint = json.loads(self.checkpoint_path().read_text())
        checkpoint["records"].append(copy.deepcopy(checkpoint["records"][0]))
        runner.atomic_write_json(self.checkpoint_path(), checkpoint)
        expected = runner.checkpoint_provenance(
            config, self.binary, runner.config_hash(config)
        )
        with self.assertRaisesRegex(ValueError, "duplicate completed sample identity"):
            runner.load_checkpoint(self.checkpoint_path(), expected)

    def create_partial_checkpoint(self) -> tuple[dict[str, Any], dict[str, Any]]:
        config = protocol()
        runner.run_campaign(
            config,
            self.binary,
            self.checkpoint_path(),
            sizes=[1],
            launch=self.fake_launch,
        )
        expected = runner.checkpoint_provenance(
            config, self.binary, runner.config_hash(config)
        )
        return config, expected

    def test_config_mismatch_is_rejected(self) -> None:
        config, _expected = self.create_partial_checkpoint()
        changed_config = copy.deepcopy(config)
        changed_config["seed"] = 18
        with self.assertRaisesRegex(ValueError, "config_sha256 mismatch"):
            runner.load_checkpoint(
                self.checkpoint_path(),
                runner.checkpoint_provenance(
                    changed_config, self.binary, runner.config_hash(changed_config)
                ),
            )

    def test_binary_mismatch_is_rejected(self) -> None:
        config, _expected = self.create_partial_checkpoint()
        changed_binary = self.directory / "different-binary"
        changed_binary.write_text("different binary contents\n")
        with self.assertRaisesRegex(ValueError, "binary_sha256 mismatch"):
            runner.load_checkpoint(
                self.checkpoint_path(),
                runner.checkpoint_provenance(
                    config, changed_binary, runner.config_hash(config)
                ),
            )

    def test_commit_mismatch_is_rejected(self) -> None:
        _config, expected = self.create_partial_checkpoint()
        changed_commit = dict(expected)
        changed_commit["source_commit"] = "different-commit"
        with self.assertRaisesRegex(ValueError, "source_commit mismatch"):
            runner.load_checkpoint(self.checkpoint_path(), changed_commit)

    def test_schema_mismatch_is_rejected(self) -> None:
        config, expected = self.create_partial_checkpoint()
        changed_schema = dict(expected)
        changed_schema["sample_schema_version"] = 999
        with self.assertRaisesRegex(ValueError, "sample_schema_version mismatch"):
            runner.load_checkpoint(self.checkpoint_path(), changed_schema)

    def test_incomplete_campaign_is_rejected_by_analyzer(self) -> None:
        partial, _ = runner.run_campaign(
            protocol(),
            self.binary,
            self.checkpoint_path(),
            sizes=[1],
            launch=self.fake_launch,
        )
        with self.assertRaisesRegex(ValueError, "incomplete campaign"):
            analyzer.summarize(partial)

    def test_atomic_checkpoint_recovery_ignores_orphaned_temp_file(self) -> None:
        checkpoint = self.checkpoint_path()
        runner.atomic_write_json(checkpoint, {"generation": 1})
        orphan = checkpoint.with_name(f".{checkpoint.name}.interrupted.tmp")
        orphan.write_text("{not valid json")
        self.assertEqual(json.loads(checkpoint.read_text()), {"generation": 1})
        runner.atomic_write_json(checkpoint, {"generation": 2})
        self.assertEqual(json.loads(checkpoint.read_text()), {"generation": 2})


if __name__ == "__main__":
    unittest.main()
