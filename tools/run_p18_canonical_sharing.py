#!/usr/bin/env python3
"""Run the paired P18 canonical ownership campaign.

The existing kernel runner remains the protocol implementation for one
canonical backend. This wrapper executes the two predeclared ownership paths
with independent atomic checkpoints and stores them together for pairing.
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))

import run_paper_kernel_scaling as kernel  # noqa: E402


P18_SCHEMA_VERSION = 1
CAMPAIGN = "p18-canonical-sharing"
BACKENDS = ("clone-canonical-reference", "borrowed-canonical")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    self_test_only = "--self-test" in sys.argv
    parser.add_argument("--config", type=Path, required=not self_test_only)
    parser.add_argument("--binary", type=Path, default=Path("target/release/mrd"))
    parser.add_argument("--output", type=Path, required=not self_test_only)
    parser.add_argument("--checkpoint-dir", type=Path)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--family", action="append", choices=kernel.FAMILIES)
    parser.add_argument("--size", action="append", type=int)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def root_path(path: Path) -> Path:
    return path if path.is_absolute() else ROOT / path


def validate_p18_config(config: dict[str, Any]) -> None:
    if config.get("p18_schema_version") != P18_SCHEMA_VERSION:
        raise ValueError("p18_schema_version must be 1")
    if config.get("campaign") != "paper-kernel-scaling":
        raise ValueError("P18 uses the versioned paper-kernel-scaling protocol")
    if config.get("canonical_backends") != list(BACKENDS):
        raise ValueError(
            "canonical_backends must be clone reference followed by borrowed"
        )
    for backend in BACKENDS:
        backend_config = {**config, "canonical_backend": backend}
        backend_config.pop("canonical_backends", None)
        kernel.validate_config(backend_config)


def backend_config(config: dict[str, Any], backend: str) -> dict[str, Any]:
    normalized = {**config, "canonical_backend": backend}
    normalized.pop("canonical_backends", None)
    return normalized


def require_external_checkpoint_dir(path: Path) -> Path:
    resolved = root_path(path).resolve()
    try:
        resolved.relative_to(ROOT.resolve())
    except ValueError:
        return resolved
    raise ValueError("P18 checkpoints must be outside the repository")


def validate_backend_provenance(
    payload: dict[str, Any],
    backend: str,
    config: dict[str, Any],
    initial_environment: dict[str, Any],
    allow_dirty: bool,
) -> None:
    expected_config = backend_config(config, backend)
    expected_config_sha = kernel.sha256_bytes(
        kernel.canonical_json(expected_config).encode()
    )
    if payload.get("config_sha256") != expected_config_sha:
        raise ValueError(f"{backend} config SHA-256 mismatch")
    if payload.get("source_commit") != initial_environment["git_commit"]:
        raise ValueError(f"{backend} source commit changed during the campaign")
    if payload.get("binary_sha256") != initial_environment["binary_sha256"]:
        raise ValueError(f"{backend} release binary changed during the campaign")
    environment = payload.get("environment")
    if not isinstance(environment, dict):
        raise ValueError(f"{backend} environment is missing")
    for field in kernel.RESUME_ENVIRONMENT_FIELDS:
        if environment.get(field) != initial_environment.get(field):
            raise ValueError(
                f"{backend} environment.{field} changed during the campaign"
            )
    if environment.get("git_dirty") and not allow_dirty:
        raise ValueError(f"{backend} recorded dirty source provenance")


def validate_environment_stability(
    initial: dict[str, Any], current: dict[str, Any], stage: str
) -> None:
    for field in kernel.RESUME_ENVIRONMENT_FIELDS:
        if current.get(field) != initial.get(field):
            raise ValueError(
                f"P18 environment.{field} changed during {stage}: "
                f"expected {initial.get(field)!r}, found {current.get(field)!r}"
            )


def run_campaign(
    config: dict[str, Any],
    binary: Path,
    output: Path,
    checkpoint_dir: Path,
    resume: bool,
    families: list[str] | None,
    sizes: list[int] | None,
    allow_dirty: bool = False,
) -> dict[str, Any]:
    validate_p18_config(config)
    binary = root_path(binary)
    output = root_path(output)
    checkpoint_dir = require_external_checkpoint_dir(checkpoint_dir)
    checkpoint_dir.mkdir(parents=True, exist_ok=True)
    initial_environment = kernel.environment(binary)
    if initial_environment["git_dirty"] and not allow_dirty:
        raise ValueError(
            "P18 requires a clean source tree; use --allow-dirty only for exploratory runs"
        )
    payloads: dict[str, dict[str, Any]] = {}
    for backend in BACKENDS:
        one_config = backend_config(config, backend)
        checkpoint = checkpoint_dir / f"p18-{backend}-checkpoint.json"
        payload = kernel.run_campaign(
            one_config,
            binary,
            checkpoint,
            resume=resume,
            families=families,
            sizes=sizes,
        )
        validate_environment_stability(
            initial_environment,
            kernel.environment(binary),
            f"the {backend} campaign",
        )
        validate_backend_provenance(
            payload,
            backend,
            config,
            initial_environment,
            allow_dirty,
        )
        payloads[backend] = payload

    validate_environment_stability(
        initial_environment,
        kernel.environment(binary),
        "the paired campaign",
    )

    for backend, payload in payloads.items():
        csv_path = output.with_name(f"{output.stem}-{backend}.csv")
        kernel.write_csv(csv_path, kernel.csv_rows(payload))

    result = {
        "schema_version": P18_SCHEMA_VERSION,
        "campaign": CAMPAIGN,
        "protocol": config,
        "config_sha256": kernel.sha256_bytes(kernel.canonical_json(config).encode()),
        "canonical_backends": list(BACKENDS),
        "source_commit": payloads[BACKENDS[0]]["source_commit"],
        "binary_sha256": payloads[BACKENDS[0]]["binary_sha256"],
        "environment": payloads[BACKENDS[0]]["environment"],
        "backends": payloads,
        "completion": {
            "complete": all(
                payload["completion"]["complete"] for payload in payloads.values()
            ),
            "backend_completion": {
                backend: payload["completion"] for backend, payload in payloads.items()
            },
        },
    }
    kernel.atomic_write_json(output, result)
    return result


def self_test() -> None:
    config = {
        "p18_schema_version": P18_SCHEMA_VERSION,
        "schema_version": 2,
        "campaign": "paper-kernel-scaling",
        "canonical_backends": list(BACKENDS),
        "boundary_discovery_backend": "prepared-exposed-edges",
        "families": ["comb-staircase", "representation-crossover"],
        "initial_size_levels": [16, 32, 64, 128, 256, 512],
        "family_parameter_rule": {
            "comb-staircase": "ceil(sqrt(target_size))",
            "representation-crossover": "ceil(sqrt(target_size))",
            "all_other_families": "target_size",
        },
        "algorithms": list(kernel.ALGORITHMS),
        "scopes": list(kernel.SCOPES),
        "seed": 42,
        "oracle_cell_limit": 40,
        "warmup_rule": {"minimum": 5, "maximum": 5, "cv_threshold_ppm": 50_000},
        "repetition_rule": {
            "target_measured_ns": 500_000_000,
            "fast_threshold_ns": 10_000_000,
            "medium_threshold_ns": 100_000_000,
            "fast_minimum": 31,
            "medium_minimum": 15,
            "slow_minimum": 7,
            "maximum": 31,
        },
        "stop_conditions": {
            "max_explicit_edges": 20_000_000,
            "max_iteration_ns": 5_000_000_000,
            "max_point_ns": 120_000_000_000,
            "max_estimated_structural_bytes": 12_000_000_000,
        },
        "partition_timeout_seconds": 135,
        "fit_rule": {"minimum_valid_size_levels": 6, "bootstrap_resamples": 10_000},
    }
    validate_p18_config(config)
    one = backend_config(config, BACKENDS[0])
    assert one["canonical_backend"] == BACKENDS[0]
    assert "canonical_backends" not in one
    assert kernel.sha256_bytes(kernel.canonical_json(config).encode())
    require_external_checkpoint_dir(Path(tempfile.gettempdir()) / "mrd-p18-self-test")
    try:
        require_external_checkpoint_dir(ROOT / "results" / "p18-checkpoints")
    except ValueError as error:
        assert "outside the repository" in str(error)
    else:
        raise AssertionError("repository-local P18 checkpoints were accepted")
    stable = {field: "same" for field in kernel.RESUME_ENVIRONMENT_FIELDS}
    validate_environment_stability(stable, dict(stable), "self-test")
    changed = dict(stable)
    changed["binary_sha256"] = "different"
    try:
        validate_environment_stability(stable, changed, "self-test")
    except ValueError as error:
        assert "binary_sha256" in str(error)
    else:
        raise AssertionError("binary mutation was accepted")
    print("p18 canonical-sharing runner self-test: ok")


def main() -> int:
    arguments = parse_args()
    if arguments.self_test:
        self_test()
        return 0
    config = kernel.strict_json_loads(root_path(arguments.config).read_text())
    checkpoint_dir = arguments.checkpoint_dir or Path(tempfile.gettempdir()) / (
        f"mrd-{arguments.output.stem}-checkpoints"
    )
    result = run_campaign(
        config,
        arguments.binary,
        arguments.output,
        checkpoint_dir,
        arguments.resume,
        arguments.family,
        arguments.size,
        arguments.allow_dirty,
    )
    print(json.dumps(result["completion"], sort_keys=True))
    return 0 if result["completion"]["complete"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
