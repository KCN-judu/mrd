#!/usr/bin/env python3
"""Run bounded CP-SAT/Rust cross-language verification populations."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path
from typing import Any

from solve import solve_grid, split_components


Cell = tuple[int, int]
CanonicalShape = tuple[Cell, ...]
InputCase = tuple[str, Path]


def command_output(command: list[str]) -> str:
    return subprocess.check_output(command, text=True).strip()


def exhaustive_inputs(width: int, height: int, directory: Path) -> list[InputCase]:
    directory.mkdir(parents=True, exist_ok=True)
    inputs = []
    for mask in range(1 << (width * height)):
        path = directory / f"binary-{width}x{height}-{mask:0{width * height}b}.json"
        cells = [bool(mask & (1 << index)) for index in range(width * height)]
        path.write_text(json.dumps({"width": width, "height": height, "cells": cells}))
        inputs.append(("exhaustive-binary", path))
    return inputs


def normalize_shape(cells: list[Cell]) -> CanonicalShape:
    min_x = min(x for x, _ in cells)
    min_y = min(y for _, y in cells)
    return tuple(sorted((x - min_x, y - min_y) for x, y in cells))


def canonical_shape(cells: list[Cell]) -> CanonicalShape:
    variants = []
    for symmetry in range(8):
        transformed = []
        for x, y in cells:
            views = (
                (x, y),
                (-x, y),
                (x, -y),
                (-x, -y),
                (y, x),
                (-y, x),
                (y, -x),
                (-y, -x),
            )
            transformed.append(views[symmetry])
        variants.append(normalize_shape(transformed))
    return min(variants)


def enumerate_free_polyominoes(max_cells: int) -> list[set[CanonicalShape]]:
    if max_cells <= 0:
        return []
    levels = [{((0, 0),)}]
    for _ in range(2, max_cells + 1):
        children: set[CanonicalShape] = set()
        for shape in levels[-1]:
            occupied = set(shape)
            for x, y in shape:
                for candidate in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
                    if candidate not in occupied:
                        children.add(canonical_shape([*shape, candidate]))
        levels.append(children)
    return levels


def polyomino_inputs(max_cells: int, directory: Path) -> tuple[list[InputCase], dict[int, int]]:
    directory.mkdir(parents=True, exist_ok=True)
    inputs: list[InputCase] = []
    count_by_size: dict[int, int] = {}
    for size, shapes in enumerate(enumerate_free_polyominoes(max_cells), start=1):
        ordered = sorted(shapes)
        count_by_size[size] = len(ordered)
        for index, shape in enumerate(ordered, start=1):
            width = max(x for x, _ in shape) + 1
            height = max(y for _, y in shape) + 1
            occupied = set(shape)
            cells = [
                (x, y) in occupied for y in range(height) for x in range(width)
            ]
            path = directory / f"free-polyomino-{size}-{index}.json"
            path.write_text(json.dumps({"width": width, "height": height, "cells": cells}))
            inputs.append(("free-polyomino", path))
    return inputs, count_by_size


def adversarial_inputs(
    directory: Path | None,
    max_grid_cells: int,
    max_component_cells: int,
) -> tuple[list[InputCase], int]:
    if directory is None:
        return [], 0
    candidates = sorted(path for path in directory.glob("*.json") if path.name != "index.json")
    selected: list[InputCase] = []
    for path in candidates:
        grid = json.loads(path.read_text())
        grid_cells = int(grid["width"]) * int(grid["height"])
        component_sizes = [len(component["cells"]) for component in split_components(grid)]
        if grid_cells <= max_grid_cells and max(component_sizes, default=0) <= max_component_cells:
            selected.append(("adversarial", path))
    return selected, len(candidates) - len(selected)


def classify_case(
    external: dict[str, Any],
    comparison: dict[str, Any] | None,
    rust_exit_code: int,
) -> tuple[str, int]:
    statuses = [component["status"] for component in external["components"]]
    if any(status in {"unknown", "feasible"} for status in statuses):
        return "timeout", 0
    if any(status != "optimal" for status in statuses):
        return "unsupported", 0
    if comparison is None:
        return "solver-error", 0
    disagreement_count = sum(
        not component["agrees"] for component in comparison["components"]
    )
    if disagreement_count or not comparison["all_agree"]:
        return "counterexample", disagreement_count
    if rust_exit_code != 0:
        return "solver-error", 0
    return "verified", 0


def run_case(
    suite: str,
    input_path: Path,
    rect_cli: Path,
    work_dir: Path,
    max_time_seconds: float,
    exact_cover_cell_limit: int,
) -> dict[str, Any]:
    case_dir = work_dir / suite / input_path.stem
    case_dir.mkdir(parents=True, exist_ok=True)
    external_path = case_dir / "external.json"
    comparison_path = case_dir / "comparison.json"
    external = solve_grid(input_path, max_time_seconds)
    external_path.write_text(json.dumps(external, indent=2) + "\n")
    completed = subprocess.run(
        [
            str(rect_cli),
            "compare-external",
            "--input",
            str(input_path),
            "--external-result",
            str(external_path),
            "--exact-cover-cell-limit",
            str(exact_cover_cell_limit),
            "--output",
            str(comparison_path),
        ],
        text=True,
        capture_output=True,
        check=False,
    )
    comparison = json.loads(comparison_path.read_text()) if comparison_path.exists() else None
    status, disagreement_count = classify_case(external, comparison, completed.returncode)
    component_statuses = Counter(component["status"] for component in external["components"])
    return {
        "suite": suite,
        "name": input_path.stem,
        "input_hash": external["input_hash"],
        "component_count": len(external["components"]),
        "cp_sat_optimal_component_count": component_statuses["optimal"],
        "cp_sat_timeout_component_count": component_statuses["unknown"]
        + component_statuses["feasible"],
        "cp_sat_unsupported_component_count": sum(
            count
            for component_status, count in component_statuses.items()
            if component_status not in {"optimal", "unknown", "feasible"}
        ),
        "rust_comparison_component_count": len(comparison["components"])
        if comparison
        else 0,
        "exact_cover_comparison_count": sum(
            "exact-cover" in component["rust_optima"]
            for component in comparison["components"]
        )
        if comparison
        else 0,
        "disagreement_component_count": disagreement_count,
        "status": status,
        "rust_exit_code": completed.returncode,
        "stderr": completed.stderr.strip() or None,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mrd", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--exhaustive-width", type=int, default=3)
    parser.add_argument("--exhaustive-height", type=int, default=3)
    parser.add_argument("--polyomino-max-cells", type=int, default=8)
    parser.add_argument("--adversarial-dir", type=Path)
    parser.add_argument("--max-adversarial-grid-cells", type=int, default=20_000)
    parser.add_argument("--max-component-cells", type=int, default=40)
    parser.add_argument("--exact-cover-cell-limit", type=int, default=40)
    parser.add_argument("--max-time-seconds", type=float, default=30.0)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    started = time.perf_counter()
    exhaustive = exhaustive_inputs(
        args.exhaustive_width,
        args.exhaustive_height,
        args.work_dir / "exhaustive-inputs",
    )
    polyominoes, polyomino_count_by_size = polyomino_inputs(
        args.polyomino_max_cells,
        args.work_dir / "polyomino-inputs",
    )
    adversarial, skipped_adversarial = adversarial_inputs(
        args.adversarial_dir,
        args.max_adversarial_grid_cells,
        args.max_component_cells,
    )
    inputs = [*exhaustive, *polyominoes, *adversarial]
    cases = [
        run_case(
            suite,
            path,
            args.rect_cli,
            args.work_dir / "cases",
            args.max_time_seconds,
            args.exact_cover_cell_limit,
        )
        for suite, path in inputs
    ]
    status_counts = Counter(case["status"] for case in cases)
    suite_summaries = {}
    for suite in ("exhaustive-binary", "free-polyomino", "adversarial"):
        suite_cases = [case for case in cases if case["suite"] == suite]
        suite_summaries[suite] = {
            "input_count": len(suite_cases),
            "component_count": sum(case["component_count"] for case in suite_cases),
            "cp_sat_comparison_count": sum(
                case["cp_sat_optimal_component_count"] for case in suite_cases
            ),
            "exact_cover_comparison_count": sum(
                case["exact_cover_comparison_count"] for case in suite_cases
            ),
            "rust_comparison_component_count": sum(
                case["rust_comparison_component_count"] for case in suite_cases
            ),
            "disagreement_component_count": sum(
                case["disagreement_component_count"] for case in suite_cases
            ),
            "status_counts": dict(Counter(case["status"] for case in suite_cases)),
        }
    summary = {
        "schema_version": 2,
        "git_commit": command_output(["git", "rev-parse", "HEAD"]),
        "rustc_version": command_output(["rustc", "--version"]),
        "command": " ".join(sys.argv),
        "seed": 0,
        "timestamp": int(time.time()),
        "input_count": len(inputs),
        "component_count": sum(case["component_count"] for case in cases),
        "cp_sat_solved_component_count": sum(
            case["cp_sat_optimal_component_count"] for case in cases
        ),
        "cp_sat_timeout_component_count": sum(
            case["cp_sat_timeout_component_count"] for case in cases
        ),
        "unsupported_component_count": sum(
            case["cp_sat_unsupported_component_count"] for case in cases
        ),
        "rust_comparison_component_count": sum(
            case["rust_comparison_component_count"] for case in cases
        ),
        "exact_cover_comparison_count": sum(
            case["exact_cover_comparison_count"] for case in cases
        ),
        "disagreement_component_count": sum(
            case["disagreement_component_count"] for case in cases
        ),
        "status_counts": dict(status_counts),
        "input_model": "finite-colored-unit-cell-grid",
        "unsupported_input_features": [
            "ornaments",
            "isolated-formal-boundary-points",
            "line-segment-holes",
            "point-holes",
            "degenerate-formal-holes",
            "general-polygon-input",
        ],
        "cp_sat_max_time_seconds_per_component": args.max_time_seconds,
        "exact_cover_cell_limit": args.exact_cover_cell_limit,
        "max_component_cells": args.max_component_cells,
        "exhaustive_grid_count": len(exhaustive),
        "exhaustive_width": args.exhaustive_width,
        "exhaustive_height": args.exhaustive_height,
        "polyomino_input_count": len(polyominoes),
        "polyomino_count_by_size": polyomino_count_by_size,
        "adversarial_grid_count": len(adversarial),
        "skipped_adversarial_grid_count": skipped_adversarial,
        "suite_summaries": suite_summaries,
        "runtime_seconds": time.perf_counter() - started,
        "cases": cases,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(summary, indent=2) + "\n")
    if status_counts["counterexample"] or status_counts["solver-error"]:
        raise SystemExit(
            f"{status_counts['counterexample']} counterexamples and "
            f"{status_counts['solver-error']} solver errors"
        )


if __name__ == "__main__":
    main()
