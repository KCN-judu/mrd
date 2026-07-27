#!/usr/bin/env python3
"""Validate the committed P1 v1.3 baseline freeze artifacts."""

from __future__ import annotations

import csv
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results" / "p1-baseline"
BASELINE_SHA = "deee489dda9967f3a5558f5ab9c0f9640ce7a70f"


def fail(message: str) -> None:
    raise SystemExit(f"P1 baseline consistency failure: {message}")


def load_json(name: str) -> dict[str, Any]:
    path = RESULTS / name
    if not path.is_file():
        fail(f"missing {path.relative_to(ROOT)}")
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path.relative_to(ROOT)}: {error}")
    if not isinstance(value, dict):
        fail(f"{path.relative_to(ROOT)} is not a JSON object")
    return value


def expect(value: Any, expected: Any, label: str) -> None:
    if value != expected:
        fail(f"{label}: expected {expected!r}, found {value!r}")


def check_metadata(report: dict[str, Any], label: str) -> None:
    metadata = report.get("metadata")
    if not isinstance(metadata, dict):
        fail(f"{label}.metadata is missing")
    expect(metadata.get("git_commit"), BASELINE_SHA, f"{label}.metadata.git_commit")


def check_grid_reports() -> None:
    exhaustive = load_json("exhaustive-4x4.json")
    check_metadata(exhaustive, "exhaustive")
    for field, expected in {
        "grid_count": 65_536,
        "component_count": 337_058,
        "exact_cover_comparison_count": 337_058,
        "sg_comparison_count": 337_058,
        "c0_comparison_count": 337_058,
        "compressed_comparison_count": 337_058,
        "counterexample_count": 0,
    }.items():
        expect(exhaustive.get(field), expected, f"exhaustive.{field}")

    random = load_json("random-8x8-seed42.json")
    check_metadata(random, "random")
    for field, expected in {
        "cases": 10_000,
        "seed": 42,
        "component_count": 162_162,
        "exact_cover_comparison_count": 160_900,
        "sg_comparison_count": 162_162,
        "c0_comparison_count": 162_162,
        "compressed_comparison_count": 162_162,
        "counterexample_count": 0,
    }.items():
        expect(random.get(field), expected, f"random.{field}")

    polyomino = load_json("polyomino-max12-summary.json")
    expect(polyomino.get("git_commit"), BASELINE_SHA, "polyomino.git_commit")
    expect(polyomino.get("max_cells"), 12, "polyomino.max_cells")
    expect(polyomino.get("explicit_hole_count"), 2, "polyomino.explicit_hole_count")
    expect(polyomino.get("input_count"), 87_148, "polyomino.input_count")
    expect(polyomino.get("component_count"), 87_148, "polyomino.component_count")
    expect(polyomino.get("status_counts"), {"verified": 87_148}, "polyomino.status_counts")
    expect(polyomino.get("counterexample_count"), 0, "polyomino.counterexample_count")
    expected_counts = [1, 1, 2, 5, 12, 35, 108, 369, 1285, 4655, 17073, 63600]
    actual_counts = [polyomino.get("free_count_by_size", {}).get(str(size)) for size in range(1, 13)]
    expect(actual_counts, expected_counts, "polyomino.free_count_by_size")


def check_csv(name: str, expected_rows: int) -> list[dict[str, str]]:
    path = RESULTS / name
    if not path.is_file():
        fail(f"missing {path.relative_to(ROOT)}")
    with path.open(newline="") as stream:
        rows = list(csv.DictReader(stream))
    expect(len(rows), expected_rows, f"{name}.row_count")
    for index, row in enumerate(rows):
        expect(row.get("git_commit"), BASELINE_SHA, f"{name}[{index}].git_commit")
        expect(row.get("status"), "verified", f"{name}[{index}].status")
    return rows


def check_polygon_report(name: str, expected: dict[str, int]) -> None:
    report = load_json(name)
    check_metadata(report, name)
    for field, value in expected.items():
        expect(report.get(field), value, f"{name}.{field}")
    for field in ("disagreements", "solver_errors", "timeouts"):
        expect(report.get(field), 0, f"{name}.{field}")
    counterexamples = report.get("minimized_counterexamples")
    expect(counterexamples, [], f"{name}.minimized_counterexamples")
    sidecar = RESULTS / name.replace(".json", ".counterexamples.json")
    if sidecar.is_file():
        expect(json.loads(sidecar.read_text()), [], f"{sidecar.name}")


def check_polygon_reports() -> None:
    check_polygon_report(
        "polygon-differential-3x3.json",
        {"input_count": 511, "component_count": 897, "supported_components": 893, "verified_components": 893},
    )
    check_polygon_report(
        "polygon-differential-4x4.json",
        {
            "input_count": 65_535,
            "component_count": 168_529,
            "supported_components": 166_189,
            "verified_components": 166_189,
        },
    )
    check_polygon_report(
        "polygon-backend-differential.json",
        {"input_count": 7_809, "component_count": 7_811, "supported_components": 7_546, "verified_components": 7_546},
    )
    check_polygon_report(
        "polygon-native-fixtures.json",
        {"input_count": 70, "component_count": 70, "supported_components": 70, "verified_components": 70},
    )

    negative = load_json("polygon-negative.json")
    check_metadata(negative, "polygon-negative")
    expect(negative.get("disagreements"), 0, "polygon-negative.disagreements")
    expect(negative.get("solver_errors"), 0, "polygon-negative.solver_errors")
    records = negative.get("records")
    if not isinstance(records, list):
        fail("polygon-negative.records is missing")
    expect(len(records), 13, "polygon-negative.records")
    for index, record in enumerate(records):
        expect(record.get("deterministic_match"), True, f"polygon-negative.records[{index}]")

    scaling = load_json("output-sensitive-scaling.json")
    check_metadata(scaling, "output-sensitive-scaling")
    expect(scaling.get("verified_rows"), 56, "output-sensitive-scaling.verified_rows")
    expect(scaling.get("disagreements"), 0, "output-sensitive-scaling.disagreements")
    expect(scaling.get("solver_errors"), 0, "output-sensitive-scaling.solver_errors")
    rows = scaling.get("rows")
    if not isinstance(rows, list):
        fail("output-sensitive-scaling.rows is missing")
    expect(len(rows), 56, "output-sensitive-scaling.rows")
    equality_fields = (
        "geometry_backends_equal",
        "chord_families_equal",
        "optimum_equal",
        "cuts_equal",
        "rectangles_equal",
        "three_backend_equal",
    )
    for index, row in enumerate(rows):
        expect(row.get("status"), "verified", f"scaling.rows[{index}].status")
        for field in equality_fields:
            expect(row.get(field), True, f"scaling.rows[{index}].{field}")
        expect(row.get("sweep_subdivision_candidate_pair_tests"), 0, f"scaling.rows[{index}].candidate_pairs")
        expect(row.get("event_validator_boundary_edge_scans"), 0, f"scaling.rows[{index}].boundary_scans")
        expect(row.get("event_validator_active_rectangle_resorts"), 0, f"scaling.rows[{index}].rectangle_resorts")
        diagnostics = row.get("sweep_diagnostics", {})
        for field in (
            "sweep_aligned_pair_iterations",
            "sweep_all_pair_iterations",
            "sweep_definition7_fallback_checks",
            "sweep_full_boundary_scans",
        ):
            expect(diagnostics.get(field), 0, f"scaling.rows[{index}].{field}")


def check_external_oracle() -> None:
    report = load_json("external-oracle.json")
    expect(report.get("git_commit"), BASELINE_SHA, "external.git_commit")
    for field, expected in {
        "input_count": 6_998,
        "component_count": 27_228,
        "cp_sat_solved_component_count": 27_228,
        "cp_sat_timeout_component_count": 0,
        "disagreement_component_count": 0,
        "unsupported_component_count": 0,
        "rust_comparison_component_count": 27_228,
        "exact_cover_comparison_count": 27_228,
    }.items():
        expect(report.get(field), expected, f"external.{field}")
    expect(report.get("status_counts"), {"verified": 6_998}, "external.status_counts")


def check_manifest() -> None:
    manifest = load_json("manifest.json")
    runs = manifest.get("runs")
    if not isinstance(runs, list):
        fail("manifest.runs is missing")
    expect(len(runs), 10, "manifest.run_count")
    expected_populations = {
        "adversarial.csv": (17, 19),
        "dense-conflict.csv": (6, 6),
        "polyomino-max10.csv": (6_474, 6_474),
        "polygon-differential-3x3.json": (511, 897),
        "polygon-negative.json": (13, 13),
        "polygon-native-fixtures.json": (70, 70),
        "polygon-backend-differential.json": (7_809, 7_811),
        "polygon-differential-4x4.json": (65_535, 168_529),
        "output-sensitive-scaling.csv": (56, 56),
        "auto-fallback.csv": (8, 8),
    }
    seen: set[str] = set()
    for index, run in enumerate(runs):
        expect(run.get("git_commit"), BASELINE_SHA, f"manifest.runs[{index}].git_commit")
        command = run.get("command")
        if not isinstance(command, str):
            fail(f"manifest.runs[{index}].command is missing")
        matches = [name for name in expected_populations if name in command]
        expect(len(matches), 1, f"manifest.runs[{index}].artifact_match")
        name = matches[0]
        if name in seen:
            fail(f"manifest contains duplicate run for {name}")
        seen.add(name)
        expected_inputs, expected_components = expected_populations[name]
        expect(run.get("input_count"), expected_inputs, f"manifest.{name}.input_count")
        expect(run.get("component_count"), expected_components, f"manifest.{name}.component_count")
    expect(seen, set(expected_populations), "manifest.artifacts")


def main() -> None:
    for path in RESULTS.iterdir():
        if path.is_file() and "/Users/" in path.read_text():
            fail(f"machine-local absolute path remains in {path.relative_to(ROOT)}")
    check_grid_reports()
    check_csv("adversarial.csv", 19)
    check_csv("dense-conflict.csv", 6)
    check_csv("polyomino-max10.csv", 6_474)
    fallback_rows = check_csv("auto-fallback.csv", 8)
    representations = {row.get("conflict_representation") for row in fallback_rows}
    expect(representations, {"dominance-4d", "path-tree"}, "auto-fallback.representations")
    check_polygon_reports()
    check_external_oracle()
    check_manifest()
    print(
        "P1 baseline consistency: 10 manifest runs, 499220 grid comparisons, "
        "174767 supported polygon components/rows, and 27228 CP-SAT components verified"
    )


if __name__ == "__main__":
    main()
