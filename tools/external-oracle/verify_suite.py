#!/usr/bin/env python3
"""Run a bounded CP-SAT/Rust cross-language verification population."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from solve import solve_grid


def command_output(command: list[str]) -> str:
    return subprocess.check_output(command, text=True).strip()


def exhaustive_inputs(width: int, height: int, directory: Path) -> list[Path]:
    directory.mkdir(parents=True, exist_ok=True)
    inputs = []
    for mask in range(1 << (width * height)):
        path = directory / f"binary-{width}x{height}-{mask:0{width * height}b}.json"
        cells = [bool(mask & (1 << index)) for index in range(width * height)]
        path.write_text(json.dumps({"width": width, "height": height, "cells": cells}))
        inputs.append(path)
    return inputs


def adversarial_inputs(directory: Path | None, max_cells: int) -> tuple[list[Path], int]:
    if directory is None:
        return [], 0
    candidates = sorted(path for path in directory.glob("*.json") if path.name != "index.json")
    selected = []
    for path in candidates:
        grid = json.loads(path.read_text())
        if int(grid["width"]) * int(grid["height"]) <= max_cells:
            selected.append(path)
    return selected, len(candidates) - len(selected)


def run_case(input_path: Path, rect_cli: Path, work_dir: Path) -> dict[str, Any]:
    case_dir = work_dir / input_path.stem
    case_dir.mkdir(parents=True, exist_ok=True)
    external_path = case_dir / "external.json"
    comparison_path = case_dir / "comparison.json"
    external = solve_grid(input_path, None)
    external_path.write_text(json.dumps(external, indent=2) + "\n")
    completed = subprocess.run(
        [
            str(rect_cli),
            "compare-external",
            "--input",
            str(input_path),
            "--external-result",
            str(external_path),
            "--output",
            str(comparison_path),
        ],
        text=True,
        capture_output=True,
        check=False,
    )
    comparison = json.loads(comparison_path.read_text()) if comparison_path.exists() else None
    return {
        "name": input_path.stem,
        "input_hash": external["input_hash"],
        "component_count": len(external["components"]),
        "external_status": external["status"],
        "rust_exit_code": completed.returncode,
        "all_agree": bool(comparison and comparison["all_agree"]),
        "stderr": completed.stderr.strip() or None,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rect-cli", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--exhaustive-width", type=int, default=2)
    parser.add_argument("--exhaustive-height", type=int, default=3)
    parser.add_argument("--adversarial-dir", type=Path)
    parser.add_argument("--max-adversarial-grid-cells", type=int, default=64)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    started = time.perf_counter()
    exhaustive = exhaustive_inputs(
        args.exhaustive_width,
        args.exhaustive_height,
        args.work_dir / "inputs",
    )
    adversarial, skipped_adversarial = adversarial_inputs(
        args.adversarial_dir, args.max_adversarial_grid_cells
    )
    inputs = exhaustive + adversarial
    cases = [run_case(path, args.rect_cli, args.work_dir / "cases") for path in inputs]
    discrepancies = sum(not case["all_agree"] for case in cases)
    summary = {
        "schema_version": 1,
        "git_commit": command_output(["git", "rev-parse", "HEAD"]),
        "rustc_version": command_output(["rustc", "--version"]),
        "command": " ".join(sys.argv),
        "seed": None,
        "timestamp": int(time.time()),
        "input_count": len(inputs),
        "component_count": sum(case["component_count"] for case in cases),
        "exhaustive_grid_count": len(exhaustive),
        "adversarial_grid_count": len(inputs) - len(exhaustive),
        "skipped_adversarial_grid_count": skipped_adversarial,
        "discrepancy_count": discrepancies,
        "runtime_seconds": time.perf_counter() - started,
        "cases": cases,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(summary, indent=2) + "\n")
    if discrepancies:
        raise SystemExit(f"{discrepancies} external-oracle discrepancies")


if __name__ == "__main__":
    main()
