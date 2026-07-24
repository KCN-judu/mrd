#!/usr/bin/env python3
"""Independent CP-SAT exact-cover oracle for colored integer grids."""

from __future__ import annotations

import argparse
import hashlib
import json
import time
from collections import deque
from pathlib import Path
from typing import Any

from ortools.sat.python import cp_model


def color_key(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def split_components(grid: dict[str, Any]) -> list[dict[str, Any]]:
    width = int(grid["width"])
    height = int(grid["height"])
    cells = grid["cells"]
    if width < 0 or height < 0 or len(cells) != width * height:
        raise ValueError("grid dimensions and cell count disagree")
    visited = [False] * len(cells)
    components: list[dict[str, Any]] = []
    for seed in range(len(cells)):
        if visited[seed]:
            continue
        visited[seed] = True
        target_key = color_key(cells[seed])
        queue = deque([(seed % width, seed // width)])
        component_cells: list[tuple[int, int]] = []
        while queue:
            x, y = queue.popleft()
            component_cells.append((x, y))
            for nx, ny in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
                if not (0 <= nx < width and 0 <= ny < height):
                    continue
                index = ny * width + nx
                if not visited[index] and color_key(cells[index]) == target_key:
                    visited[index] = True
                    queue.append((nx, ny))
        component_cells.sort()
        components.append(
            {
                "component_id": len(components),
                "color": cells[seed],
                "cells": component_cells,
            }
        )
    return components


def enumerate_rectangles(cells: list[tuple[int, int]]) -> list[tuple[int, int, int, int]]:
    occupied = set(cells)
    min_x = min(x for x, _ in cells)
    max_x = max(x for x, _ in cells) + 1
    min_y = min(y for _, y in cells)
    max_y = max(y for _, y in cells) + 1
    rectangles = []
    for y0 in range(min_y, max_y):
        for y1 in range(y0 + 1, max_y + 1):
            for x0 in range(min_x, max_x):
                for x1 in range(x0 + 1, max_x + 1):
                    if all((x, y) in occupied for y in range(y0, y1) for x in range(x0, x1)):
                        rectangles.append((x0, y0, x1, y1))
    rectangles.sort(key=lambda rect: (-(rect[2] - rect[0]) * (rect[3] - rect[1]), rect))
    return rectangles


def solve_component(
    component: dict[str, Any], max_time_seconds: float | None
) -> dict[str, Any]:
    started = time.perf_counter()
    cells = component["cells"]
    rectangles = enumerate_rectangles(cells)
    model = cp_model.CpModel()
    selected = [model.new_bool_var(f"rectangle_{index}") for index in range(len(rectangles))]
    for cell in cells:
        covering = [
            selected[index]
            for index, (x0, y0, x1, y1) in enumerate(rectangles)
            if x0 <= cell[0] < x1 and y0 <= cell[1] < y1
        ]
        model.add_exactly_one(covering)
    model.minimize(sum(selected))
    solver = cp_model.CpSolver()
    solver.parameters.num_search_workers = 1
    solver.parameters.random_seed = 0
    if max_time_seconds is not None:
        solver.parameters.max_time_in_seconds = max_time_seconds
    status_code = solver.solve(model)
    status_names = {
        cp_model.OPTIMAL: "optimal",
        cp_model.FEASIBLE: "feasible",
        cp_model.INFEASIBLE: "infeasible",
        cp_model.MODEL_INVALID: "model-invalid",
        cp_model.UNKNOWN: "unknown",
    }
    status = status_names.get(status_code, f"status-{status_code}")
    chosen = [rectangles[index] for index, variable in enumerate(selected) if solver.boolean_value(variable)] if status in {"optimal", "feasible"} else []
    optimum = len(chosen) if status == "optimal" else None
    validate_component_solution(cells, chosen, optimum)
    runtime_seconds = time.perf_counter() - started
    return {
        "component_id": component["component_id"],
        "color": component["color"],
        "cell_count": len(cells),
        "status": status,
        "optimum_rectangle_count": optimum,
        "rectangles": [
            {"x0": x0, "y0": y0, "x1": x1, "y1": y1}
            for x0, y0, x1, y1 in chosen
        ],
        "candidate_rectangle_count": len(rectangles),
        "runtime_seconds_micros": round(runtime_seconds * 1_000_000),
    }


def validate_component_solution(
    cells: list[tuple[int, int]],
    rectangles: list[tuple[int, int, int, int]],
    optimum: int | None,
) -> None:
    if optimum is None:
        return
    if len(rectangles) != optimum:
        raise AssertionError("declared optimum differs from selected rectangle count")
    expected = set(cells)
    coverage = {cell: 0 for cell in cells}
    for x0, y0, x1, y1 in rectangles:
        if x0 >= x1 or y0 >= y1:
            raise AssertionError("non-positive rectangle")
        for y in range(y0, y1):
            for x in range(x0, x1):
                if (x, y) not in expected:
                    raise AssertionError("rectangle covers an outside cell")
                coverage[(x, y)] += 1
    if any(count != 1 for count in coverage.values()):
        raise AssertionError("rectangle output is not an exact cover")


def solve_grid(input_path: Path, max_time_seconds: float | None) -> dict[str, Any]:
    raw = input_path.read_bytes()
    grid = json.loads(raw)
    started = time.perf_counter()
    components = [
        solve_component(component, max_time_seconds)
        for component in split_components(grid)
    ]
    statuses = {component["status"] for component in components}
    overall_status = "optimal" if statuses <= {"optimal"} else "partial"
    return {
        "schema_version": 1,
        "solver": "ortools-cp-sat",
        "status": overall_status,
        "input_hash": hashlib.sha256(raw).hexdigest(),
        "runtime_seconds": time.perf_counter() - started,
        "components": components,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--max-time-seconds", type=float)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    result = solve_grid(args.input, args.max_time_seconds)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()

