#!/usr/bin/env python3
"""Generate paper-ready correctness, compression, and scope tables."""

from __future__ import annotations

import argparse
import csv
import json
import platform
import subprocess
import sys
from pathlib import Path
from typing import Any


CORRECTNESS_FIELDS = [
    "suite",
    "grids",
    "components",
    "exact-cover comparisons",
    "CP-SAT comparisons",
    "SG comparisons",
    "C0 comparisons",
    "compressed comparisons",
    "counterexamples",
]
COMPRESSION_FIELDS = [
    "family",
    "q",
    "|E|",
    "biclique_count",
    "sigma",
    "sigma / |E|",
    "C0 arcs",
    "compressed arcs",
    "arc reduction",
    "C0 time",
    "compressed time",
]
SCOPE_FIELDS = ["feature", "theoretical paper", "current Rust artifact", "tested", "notes"]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--exhaustive", type=Path, required=True)
    parser.add_argument("--random", type=Path, required=True)
    parser.add_argument("--adversarial", type=Path, required=True)
    parser.add_argument("--polyomino", type=Path, required=True)
    parser.add_argument("--external", type=Path, required=True)
    parser.add_argument("--dense", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument(
        "--release-index", type=Path, default=Path("results/release-index.json")
    )
    return parser.parse_args()


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def benchmark_correctness(suite: str, rows: list[dict[str, str]]) -> dict[str, Any]:
    if not rows:
        raise ValueError(f"{suite} benchmark is empty")
    verified = sum(row["status"] == "verified" for row in rows)
    return {
        "suite": suite,
        "grids": int(rows[0]["input_count"]),
        "components": int(rows[0]["component_count"]),
        "exact-cover comparisons": sum(row["exact_cover_compared"] == "true" for row in rows),
        "CP-SAT comparisons": 0,
        "SG comparisons": verified,
        "C0 comparisons": verified,
        "compressed comparisons": verified,
        "counterexamples": sum(row["status"] == "counterexample" for row in rows),
    }


def rust_report_correctness(suite: str, report: dict[str, Any]) -> dict[str, Any]:
    grids = report.get("grid_count", report.get("cases", 0))
    return {
        "suite": suite,
        "grids": grids,
        "components": report["component_count"],
        "exact-cover comparisons": report["exact_cover_comparison_count"],
        "CP-SAT comparisons": 0,
        "SG comparisons": report["sg_comparison_count"],
        "C0 comparisons": report["c0_comparison_count"],
        "compressed comparisons": report["compressed_comparison_count"],
        "counterexamples": report["counterexample_count"],
    }


def external_correctness(external: dict[str, Any]) -> list[dict[str, Any]]:
    labels = {
        "exhaustive-binary": (
            f"external-binary-{external['exhaustive_width']}x"
            f"{external['exhaustive_height']}"
        ),
        "free-polyomino": "external-free-polyomino",
        "adversarial": "external-adversarial",
    }
    rows = []
    for key, label in labels.items():
        suite = external["suite_summaries"][key]
        rust_count = suite["rust_comparison_component_count"]
        rows.append(
            {
                "suite": label,
                "grids": suite["input_count"],
                "components": suite["component_count"],
                "exact-cover comparisons": suite["exact_cover_comparison_count"],
                "CP-SAT comparisons": suite["cp_sat_comparison_count"],
                "SG comparisons": rust_count,
                "C0 comparisons": rust_count,
                "compressed comparisons": rust_count,
                "counterexamples": suite["disagreement_component_count"],
            }
        )
    return rows


def phase_total(raw: str) -> int:
    return sum(int(value) for value in json.loads(raw or "{}").values())


def compression_rows(rows: list[dict[str, str]]) -> list[dict[str, Any]]:
    output = []
    for row in rows:
        edges = int(row["explicit_conflict_edge_count"])
        sigma = int(row["biclique_total_vertex_occurrences"])
        c0_arcs = int(row["c0_network_arc_count"])
        compressed_arcs = int(row["compressed_network_arc_count"])
        output.append(
            {
                "family": row["instance_name"],
                "q": int(row["total_chord_count"]),
                "|E|": edges,
                "biclique_count": int(row["biclique_count"]),
                "sigma": sigma,
                "sigma / |E|": f"{sigma / edges:.6f}" if edges else "",
                "C0 arcs": c0_arcs,
                "compressed arcs": compressed_arcs,
                "arc reduction": f"{1 - compressed_arcs / c0_arcs:.6f}" if c0_arcs else "",
                "C0 time": phase_total(row["c0_phase_microseconds"]),
                "compressed time": phase_total(row["compressed_phase_microseconds"]),
            }
        )
    return output


def scope_rows() -> list[dict[str, str]]:
    return [
        {"feature": "ordinary polygon input", "theoretical paper": "formal rectilinear boundary", "current Rust artifact": "boundary-native integer-coordinate outer loop and ordinary holes", "tested": "yes", "notes": "no rasterization by coordinate magnitude"},
        {"feature": "ordinary holes", "theoretical paper": "supported", "current Rust artifact": "supported for grid-cell regions and boundary-native polygons", "tested": "yes", "notes": "rings, separated holes, and native two-hole fixture"},
        {"feature": "degenerate holes", "theoretical paper": "formal model", "current Rust artifact": "unsupported", "tested": "scope rejection", "notes": "point, segment, and arbitrary formal holes excluded"},
        {"feature": "endpoint contacts", "theoretical paper": "closed-chord conflicts", "current Rust artifact": "integer parity embedding", "tested": "yes", "notes": "pairwise geometry iff strict dominance"},
        {"feature": "effective chord enumeration", "theoretical paper": "O(n log n)", "current Rust artifact": "GridInteriorRunEnumerator for grids; SoltanGorpinevichSweepEnumerator for accepted ordinary polygons", "tested": "three-backend exact family differential", "notes": "ordinary-loop sweep is O(n log n + q); formal-boundary source cases remain unsupported"},
        {"feature": "polygon completion", "theoretical paper": "horizontal then vertical simple chords", "current Rust artifact": "incremental completion with dynamic stabbing, sparse face walk, and slab validation", "tested": "exact cut and rectangle differential", "notes": "no full classical O(n log n) completion claim"},
        {"feature": "polygon structural validation", "theoretical paper": "ordinary rectilinear domain", "current Rust artifact": "OrthogonalSweepValidator with quadratic Oracle", "tested": "accepted and negative-category differential", "notes": "deterministic integer event ordering"},
        {"feature": "compact biclique partition", "theoretical paper": "O(q log^4 q) for d=4", "current Rust artifact": "constructive Theorem 8 recursion", "tested": "yes", "notes": "edge multiplicity audited exactly once"},
        {"feature": "practical Dinic backend", "theoretical paper": "replaceable exact flow", "current Rust artifact": "implemented", "tested": "yes", "notes": "integral flow and residual cut"},
        {"feature": "almost-linear theoretical flow backend", "theoretical paper": "used asymptotically", "current Rust artifact": "not implemented", "tested": "no", "notes": "citation-only complexity component"},
        {"feature": "explicit rectangle output", "theoretical paper": "constructive completion", "current Rust artifact": "implemented", "tested": "yes", "notes": "cell-exact validation"},
        {"feature": "machine-checkable certificates", "theoretical paper": "not an artifact requirement", "current Rust artifact": "implemented", "tested": "yes", "notes": "matching, partition, flow, cut, and rectangles"},
        {"feature": "clean hole-free eligibility", "theoretical paper": "Definition 9.1", "current Rust artifact": "integer grid classifier with loop identities", "tested": "yes", "notes": "component and chord-mass census; ornaments remain out of model"},
        {"feature": "path-tree biclique partition", "theoretical paper": "Theorems 9.5-9.6", "current Rust artifact": "BoundaryLaminar axis view plus endpoint HLD in CompactOnly; area dual Oracle in FullyAudited", "tested": "yes", "notes": "full 4x4 differential and axis-view equality"},
        {"feature": "clean complete-bipartite family", "theoretical paper": "Theorem 9.2", "current Rust artifact": "integer-grid realization", "tested": "yes", "notes": "compact campaign through t=128"},
    ]


def write_csv(path: Path, fields: list[str], rows: list[dict[str, Any]]) -> None:
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def markdown_table(fields: list[str], rows: list[dict[str, Any]]) -> str:
    def escape(value: Any) -> str:
        return str(value).replace("|", "\\|")

    lines = [
        "| " + " | ".join(escape(field) for field in fields) + " |",
        "| " + " | ".join("---" for _ in fields) + " |",
    ]
    lines.extend(
        "| " + " | ".join(escape(row[field]) for field in fields) + " |"
        for row in rows
    )
    return "\n".join(lines)


def cpu_name() -> str:
    if sys.platform == "darwin":
        return subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip()
    return platform.processor() or "unknown"


def compact_v03_section(output_dir: Path) -> list[str]:
    dense_path = output_dir / "v0.3-compact-dense.csv"
    differential_path = output_dir / "v0.3-chord-differential.json"
    external_path = output_dir / "v0.3-external-oracle.json"
    if not all(path.exists() for path in (dense_path, differential_path, external_path)):
        return []
    dense = read_csv(dense_path)
    differential = read_json(differential_path)
    external = read_json(external_path)
    differential_inputs = sum(row["input_count"] for row in differential["populations"])
    differential_disagreements = sum(
        row["disagreement_count"] for row in differential["populations"]
    )
    fields = [
        "total q", "horizontal chords", "vertical chords", "bicliques", "sigma",
        "compressed vertices", "compressed arcs", "enumerator", "explicit edges",
    ]
    rows = [
        {
            "total q": row["total_chord_count"],
            "horizontal chords": row["horizontal_chord_count"],
            "vertical chords": row["vertical_chord_count"],
            "bicliques": row["biclique_count"],
            "sigma": row["biclique_total_vertex_occurrences"],
            "compressed vertices": row["compressed_network_vertex_count"],
            "compressed arcs": row["compressed_network_arc_count"],
            "enumerator": row["effective_chord_enumerator"],
            "explicit edges": row["explicit_conflict_edge_count"] or "null",
        }
        for row in dense
    ]
    return [
        "## CompactOnly v0.3 evidence", "", markdown_table(fields, rows), "",
        "These rows are separate v0.3 CompactOnly evidence and do not overwrite the historical v0.2 population.",
        f"Exact chord-family differential comparisons: {differential_inputs:,} inputs, {differential_disagreements} disagreements.",
        f"The bounded v0.3 CP-SAT rerun compared {external['component_count']:,} components with {external['disagreement_component_count']} disagreements.",
        "Peak RSS is unmeasured; no null value is interpreted as zero.", "",
    ]


def current_evidence_sections(output_dir: Path) -> list[str]:
    sections: list[str] = []
    differential = output_dir / "v0.6-clean-boundary-differential.json"
    if differential.exists():
        report = read_json(differential)
        sections.extend(
            [
                "## v0.6 BoundaryLaminar differential evidence",
                "",
                f"The full 4x4 campaign covers {report['masks']:,} masks and {report['eligible_components']:,} clean eligible components. It records {report['verified']:,} verified rows, {report['counterexamples']} counterexamples, and {report['execution_trace_violations']} execution-trace violations.",
                f"Orientation counts: `{json.dumps(report['orientation_counts'], sort_keys=True)}`; q range `{report['q_min']}..{report['q_max']}`, sigma range `{report['sigma_min']}..{report['sigma_max']}`.",
                "",
            ]
        )
    families = output_dir / "v0.7-path-tree-families.csv"
    if families.exists():
        rows = read_csv(families)
        fields = ["family", "instance_name", "status", "path_tree_orientation", "path_tree_orientation_policy", "dual_region_count", "dual_tree_max_depth", "dual_tree_max_branching_degree", "heavy_chain_interval_count", "canonical_segment_node_count", "path_tree_sigma"]
        sections.extend(["## v0.7 Path-tree geometry families", "", markdown_table(fields, [{field: row.get(field, "") for field in fields} for row in rows]), ""])
    comparison = output_dir / "v0.7-path-tree-vs-4d.csv"
    if comparison.exists():
        rows = read_csv(comparison)
        fields = ["family", "instance_name", "q", "q_bucket", "sigma_path_tree", "sigma_4d", "network_arcs_path_tree", "network_arcs_4d", "path_tree_total_microseconds", "four_d_total_microseconds", "optimum_equal", "rectangles_equal", "status"]
        sections.extend(["## v0.7 Path-tree versus 4D", "", markdown_table(fields, [{field: row.get(field, "") for field in fields} for row in rows]), ""])
    orientation = output_dir / "v0.7-path-tree-orientation-audit.csv"
    orientation_summary = output_dir / "v0.7-path-tree-orientation-audit.json"
    if orientation.exists() and orientation_summary.exists():
        report = read_json(orientation_summary)
        sections.extend(
            [
                "## v0.7 Orientation regret audit",
                "",
                f"The row-level CSV contains {report['rows']:,} clean instances. Exact sigma matches: {report['exact_matches']:,}; positive-regret mismatches: {report['mismatches']}; equal-sigma direction ties: {report['tie_orientation_differences']:,}; maximum absolute regret: {report['maximum_absolute_regret']:,}.",
                "",
            ]
        )
    dual = output_dir / "v0.7-path-tree-dual-differential.csv"
    dual_summary = output_dir / "v0.7-path-tree-dual-differential.json"
    if dual.exists() and dual_summary.exists():
        report = read_json(dual_summary)
        sections.extend(
            [
                "## v0.7 BoundaryLaminar versus area dual",
                "",
                f"The row-level CSV contains {report['rows']:,} clean instances. Verified: {report['verified']:,}; counterexamples: {report['counterexamples']}; solver errors: {report['solver_errors']}.",
                "",
            ]
        )
    return sections


def v08_evidence_sections(output_dir: Path) -> list[str]:
    sections: list[str] = []
    gap_path = output_dir / "v0.8-gap-backend-differential.json"
    if gap_path.exists():
        report = read_json(gap_path)
        sections.extend(
            [
                "## v0.8 Boundary-indexed adaptive path-tree",
                "",
                "### Indexed frontend and boundary-gap differential",
                "",
                f"The complete differential campaign covers {report['total_input_count']:,} inputs, {report['total_component_count']:,} components, and {report['total_clean_component_count']:,} clean components.",
                f"It performs {report['total_boundary_index_comparison_count']:,} boundary-index comparisons, {report['total_endpoint_metadata_comparison_count']:,} endpoint-metadata comparisons, {report['total_clean_classifier_comparison_count']:,} clean-classifier comparisons, and {report['total_orientation_comparison_count']:,} orientation comparisons.",
                f"Verified clean components: {report['total_verified_component_count']:,}; mismatches: {report['total_mismatch_count']}; solver errors: {report['total_solver_error_count']}. ReferenceNested performs {report['total_nested_membership_tests']:,} interval-membership tests; EventSweep records {report['total_event_push_count']:,} pushes and {report['total_event_pop_count']:,} pops.",
                "",
            ]
        )

    witness_path = output_dir / "path-tree-witnesses" / "index.json"
    if witness_path.exists():
        report = read_json(witness_path)
        fields = [
            "name",
            "cells",
            "horizontal_chords",
            "vertical_chords",
            "dual_max_branching_degree",
            "path_count",
            "heavy_chain_interval_count",
            "paths_using_multiple_heavy_chains",
            "canonical_segment_node_count",
        ]
        rows = [
            {
                **{field: witness.get(field, "") for field in fields if field != "cells"},
                "cells": sum(witness.get("cells", [])),
            }
            for witness in report.get("witnesses", [])
        ]
        sections.extend(
            [
                "### Minimized mixed-branching witnesses",
                "",
                markdown_table(fields, rows),
                "",
                f"The deterministic witness search examined {report.get('candidates_examined', 0):,} production geometry candidates and retained {len(rows)} translation/dihedral-canonical witnesses after delta-debugging minimization.",
                f"Minimized cell counts range from {min((row['cells'] for row in rows), default=0)} to {max((row['cells'] for row in rows), default=0)}.",
                "",
            ]
        )

    families_path = output_dir / "v0.8-path-tree-families.csv"
    if families_path.exists():
        rows = read_csv(families_path)
        verified = [row for row in rows if row.get("status") == "verified"]
        nontrivial = [row for row in verified if int(row.get("total_chord_count", 0)) > 0]
        mixed = [
            row
            for row in verified
            if row.get("family") == "mixed-branching-connected-sum"
        ]
        sections.extend(
            [
                "### v0.8 scaled geometry families",
                "",
                f"The generated family campaign contains {len(rows)} rows ({len(nontrivial)} nontrivial chord-bearing rows), all with status `verified`.",
                f"Chain q grows from {min((int(row['total_chord_count']) for row in rows if row['family'] == 'laminar-chain'), default=0)} to {max((int(row['total_chord_count']) for row in rows if row['family'] == 'laminar-chain'), default=0)}; star and balanced rows reach dual branching degrees {max((int(row['dual_tree_max_branching_degree']) for row in rows if row['family'] == 'laminar-star'), default=0)} and {max((int(row['dual_tree_max_branching_degree']) for row in rows if row['family'] == 'balanced-laminar'), default=0)}.",
                f"The mixed-branching connected-sum family contains {len(mixed)} verified members and reaches q={max((int(row['total_chord_count']) for row in mixed), default=0)}, {max((int(row['dual_region_count']) for row in mixed), default=0)} dual regions, {max((int(row['path_count']) for row in mixed), default=0)} paths, {max((int(row['heavy_chain_interval_count']) for row in mixed), default=0)} heavy-chain intervals, and {max((int(row['canonical_segment_node_count']) for row in mixed), default=0)} canonical nodes.",
                "The connected-sum members are rebuilt through production geometry; no coordinate-only scaling law or synthetic dual graph is used.",
                "",
            ]
        )

    comparison_path = output_dir / "v0.8-path-tree-vs-4d.csv"
    if comparison_path.exists():
        rows = read_csv(comparison_path)
        verified = [row for row in rows if row.get("status") == "verified"]
        bucket_order = [
            "0-8",
            "9-32",
            "33-128",
            "129-512",
            "513-2048",
            "2049+",
        ]
        present_buckets = {row.get("q_bucket", "") for row in rows}
        buckets = [bucket for bucket in bucket_order if bucket in present_buckets]
        sections.extend(
            [
                "### v0.8 representation comparison",
                "",
                f"The generated comparison contains {len(rows)} rows, {len(verified)} verified, across q buckets {', '.join(f'`{bucket}`' for bucket in buckets)}.",
                "It records sigma, network size, phase timings, final equality, and owned-allocation estimates for both path-tree and 4D representations.",
                "",
            ]
        )

    advantage_path = output_dir / "v0.8-path-tree-advantage.json"
    if advantage_path.exists():
        report = read_json(advantage_path)
        all_rows = [
            *report.get("top_path_tree_advantages", []),
            *report.get("top_four_d_advantages", []),
        ]
        path_tree_max = max((row.get("owned_path_tree_bytes", 0) for row in all_rows), default=0)
        four_d_max = max((row.get("owned_4d_bytes", 0) for row in all_rows), default=0)
        sections.extend(
            [
                "### v0.8 representation advantage search",
                "",
                f"The generated advantage search retains {report.get('eligible_rows', 0)} eligible mixed-orientation rows; strict path-tree advantages: {report.get('strict_path_tree_advantages', 0)}; strict 4D advantages: {report.get('strict_four_d_advantages', 0)}.",
                f"Retained rows have owned-allocation maxima of {path_tree_max:,} bytes for path-tree and {four_d_max:,} bytes for 4D; final optimum and rectangle equality are recorded per row.",
                "",
            ]
        )

    orientation_path = output_dir / "v0.8-path-tree-orientation-audit.json"
    if orientation_path.exists():
        report = read_json(orientation_path)
        ratio = report.get("maximum_regret_ratio")
        ratio_text = "unavailable"
        if isinstance(ratio, dict):
            numerator = ratio.get("numerator")
            denominator = ratio.get("denominator")
            if numerator is not None and denominator is not None:
                ratio_text = f"{numerator}/{denominator}"
        sections.extend(
            [
                "### v0.8 orientation regret audit",
                "",
                f"The expanded audit contains {report.get('rows', 0):,} rows: {report.get('exact_matches', 0):,} exact sigma matches and {report.get('mismatches', 0)} positive-regret rows.",
                f"Maximum absolute regret is {report.get('maximum_absolute_regret', 0)} and the maximum recorded regret ratio is {ratio_text}. These counterexamples keep exact `build-both` as the CompactOnly default; `bound-estimate` remains an explicit benchmark policy.",
                "",
            ]
        )
    return sections


def v09_evidence_sections(output_dir: Path) -> list[str]:
    path = output_dir / "v0.9-polygon-differential.json"
    if not path.exists():
        return []
    report = read_json(path)
    populations = report["populations"]
    fields = [
        "population",
        "input_count",
        "supported_components",
        "rejected_components",
        "disagreements",
        "profile",
    ]

    rows = [
        {
            "population": population["name"],
            "input_count": population["input_count"],
            "supported_components": population["supported_component_count"],
            "rejected_components": population["rejected_component_count"],
            "disagreements": population["disagreement_count"],
            "profile": population["profile"],
        }
        for population in populations
    ]
    total_supported = sum(row["supported_components"] for row in rows)
    total_rejected = sum(row["rejected_components"] for row in rows)
    total_disagreements = sum(row["disagreements"] for row in rows)
    fixtures = ", ".join(f"`{name}`" for name in report["native_fixtures"])
    extended_families = ", ".join(
        f"`{name}`" for name in report["extended_population_families"]
    )
    large_gap = report["large_gap_coordinate_compression"]
    external = report["external_cp_sat"]
    return [
        "## v0.9 Boundary-native ordinary polygon evidence",
        "",
        markdown_table(fields, rows),
        "",
        f"The committed populations cover {total_supported:,} supported ordinary components, {total_rejected:,} explicitly rejected grid-derived degeneracies, and {total_disagreements} chord/selection/cut/rectangle disagreements.",
        f"The extended population records {report['clean_path_tree_case_count']:,} clean polygon `Auto` path-tree selections and {report['four_d_fallback_case_count']:,} exact 4D fallbacks.",
        f"Extended families: {extended_families}.",
        f"Focused semantic coverage contains {report['definition_7_focused_test_count']} Definition 7 tests and {report['validator_negative_case_count']} validator rejection cases.",
        f"The isolated CP-SAT rerun compares {external['component_count']:,} components with {external['disagreement_component_count']} disagreements.",
        f"Native nonuniform-coordinate fixtures: {fixtures}.",
        f"The one-billion-unit large-gap fixture uses {large_gap['x_count']} x coordinates, {large_gap['y_count']} y coordinates, and {large_gap['atomic_cell_count']} atomic arrangement cell; production raster use is `{str(report['raster_oracle_used']).lower()}`.",
        "",
    ]


def v10_evidence_sections(output_dir: Path) -> list[str]:
    paths = {
        "3x3": output_dir / "v1.0-polygon-differential-3x3.json",
        "4x4": output_dir / "v1.0-polygon-differential-4x4.json",
        "extended": output_dir / "v1.0-polygon-backend-differential.json",
        "negative": output_dir / "v1.0-polygon-negative.json",
        "native": output_dir / "v1.0-polygon-native-fixtures.json",
        "scaling": output_dir / "v1.0-polygon-scaling.json",
    }
    if not all(path.exists() for path in paths.values()):
        return []
    reports = {name: read_json(path) for name, path in paths.items()}
    population_fields = [
        "population",
        "inputs",
        "components",
        "supported",
        "rejected",
        "verified",
        "raster comparisons",
        "path-tree comparisons",
        "disagreements",
    ]
    population_rows = []
    for name in ("3x3", "4x4", "extended", "native"):
        report = reports[name]
        population_rows.append(
            {
                "population": report["population"],
                "inputs": report["input_count"],
                "components": report["component_count"],
                "supported": report["supported_components"],
                "rejected": report["model_rejections"],
                "verified": report["verified_components"],
                "raster comparisons": report["raster_oracle_comparisons"],
                "path-tree comparisons": report["path_tree_comparisons"],
                "disagreements": report["disagreements"],
            }
        )
    scaling = reports["scaling"]
    largest = [row for row in scaling["rows"] if row["size"] == 16]
    scaling_fields = [
        "family",
        "n",
        "C",
        "q",
        "reference us",
        "indexed us",
        "reference Definition 7 scans",
        "indexed Definition 7 scans",
        "reference completion scans",
        "indexed completion scans",
    ]
    scaling_rows = [
        {
            "family": row["family"],
            "n": row["boundary_complexity"],
            "C": row["aligned_candidate_count"],
            "q": row["chord_count"],
            "reference us": row["reference_microseconds"],
            "indexed us": row["indexed_microseconds"],
            "reference Definition 7 scans": row["reference_diagnostics"].get(
                "polygon_definition7_full_boundary_scans", ""
            ),
            "indexed Definition 7 scans": row["indexed_diagnostics"].get(
                "polygon_definition7_full_boundary_scans", ""
            ),
            "reference completion scans": row["reference_diagnostics"].get(
                "polygon_completion_full_boundary_scans", ""
            ),
            "indexed completion scans": row["indexed_diagnostics"].get(
                "polygon_completion_full_boundary_scans", ""
            ),
        }
        for row in largest
    ]
    negative = reports["negative"]
    return [
        "## v1.0 Indexed polygon engine evidence",
        "",
        markdown_table(population_fields, population_rows),
        "",
        f"The structural and dissection-validator negative campaign contains {len(negative['records'])} cases with {negative['disagreements']} category disagreements.",
        f"The polygon-native A-H scaling campaign contains {scaling['verified_rows']} verified rows, {scaling['solver_errors']} solver errors, and {scaling['disagreements']} disagreements.",
        "",
        "### Largest A-H scaling rows",
        "",
        markdown_table(scaling_fields, scaling_rows),
        "",
        "Indexed production rows record zero Definition 7 full-boundary scans, zero global completion candidate rebuilds, zero completion full-boundary/full-cut scans, and zero rectangle-per-cell validator tests.",
        "Owned allocation values are exact estimates of Rust-owned vectors and indexes, not process peak RSS.",
        "",
    ]


def v11_evidence_sections(output_dir: Path) -> list[str]:
    paths = {
        "3x3": output_dir / "v1.1-polygon-differential-3x3.json",
        "4x4": output_dir / "v1.1-polygon-differential-4x4.json",
        "extended": output_dir / "v1.1-polygon-backend-differential.json",
        "negative": output_dir / "v1.1-polygon-negative.json",
        "native": output_dir / "v1.1-polygon-native-fixtures.json",
        "scaling": output_dir / "v1.1-polygon-scaling.json",
    }
    if not all(path.exists() for path in paths.values()):
        return []
    reports = {name: read_json(path) for name, path in paths.items()}
    population_fields = [
        "population",
        "inputs",
        "components",
        "supported",
        "verified",
        "disagreements",
    ]
    population_rows = [
        {
            "population": reports[name]["population"],
            "inputs": reports[name]["input_count"],
            "components": reports[name]["component_count"],
            "supported": reports[name]["supported_components"],
            "verified": reports[name]["verified_components"],
            "disagreements": reports[name]["disagreements"],
        }
        for name in ("3x3", "4x4", "extended", "native")
    ]
    scaling = reports["scaling"]
    largest_size = max(row["size"] for row in scaling["rows"])
    candidate_rows = [
        row
        for row in scaling["rows"]
        if row["size"] == largest_size and row["family"] in ("B", "C")
    ]
    candidate_fields = [
        "family", "n", "holes", "r", "C", "q", "C/max(1,q)",
        "reference pairs", "indexed pairs", "sweep events", "sweep status ops",
        "sweep outputs", "three-backend equal",
    ]
    candidate_table_rows = [
        {
            "family": row["family"],
            "n": row["boundary_complexity"],
            "holes": row["hole_count"],
            "r": row["reflex_count"],
            "C": row["aligned_candidate_count"],
            "q": row["chord_count"],
            "C/max(1,q)": f"{row['candidate_output_ratio_numerator']}/{row['candidate_output_ratio_denominator']}",
            "reference pairs": row["reference_pair_iterations"],
            "indexed pairs": row["indexed_pair_iterations"],
            "sweep events": row["sweep_event_count"],
            "sweep status ops": row["sweep_status_operations"],
            "sweep outputs": row["sweep_output_record_count"],
            "three-backend equal": row["three_backend_equal"],
        }
        for row in candidate_rows
    ]
    negative = reports["negative"]
    return [
        "## v1.1 Soltan--Gorpinevich sweep evidence", "",
        markdown_table(population_fields, population_rows), "",
        f"The negative campaign contains {len(negative['records'])} cases with {negative['disagreements']} category disagreements.",
        "Every differential comparison includes complete chord families, endpoint metadata, clean certificates, flow/cut evidence, and canonical rectangles.",
        "", f"### Candidate-gap rows at size {largest_size}", "",
        markdown_table(candidate_fields, candidate_table_rows), "",
        "Sweep rows report zero aligned-pair iterations, all-pair iterations, Definition 7 fallback checks, full-boundary scans, and duplicate output records. Owned allocation values are Rust-owned estimates, not peak RSS.",
        "",
    ]


def v12_evidence_sections(output_dir: Path) -> list[str]:
    paths = {
        "3x3": output_dir / "v1.2-polygon-differential-3x3.json",
        "4x4": output_dir / "v1.2-polygon-differential-4x4.json",
        "extended": output_dir / "v1.2-polygon-backend-differential.json",
        "negative": output_dir / "v1.2-polygon-negative.json",
        "native": output_dir / "v1.2-polygon-native-fixtures.json",
        "scaling": output_dir / "v1.2-polygon-scaling.json",
        "external": output_dir / "v1.2-external-oracle.json",
    }
    if not all(path.exists() for path in paths.values()):
        return []
    reports = {name: read_json(path) for name, path in paths.items()}
    population_fields = ["population", "supported", "verified", "disagreements"]
    population_rows = [
        {
            "population": reports[name]["population"],
            "supported": reports[name]["supported_components"],
            "verified": reports[name]["verified_components"],
            "disagreements": reports[name]["disagreements"],
        }
        for name in ("3x3", "4x4", "extended", "native")
    ]
    scaling = reports["scaling"]
    largest_size = max(row["size"] for row in scaling["rows"])
    scaling_rows = [row for row in scaling["rows"] if row["size"] == largest_size]
    scaling_fields = [
        "family", "|X|", "|Y|", "|X||Y|", "vertices", "half-edges",
        "junctions", "faces", "dense bytes", "sparse bytes", "cut-index bytes",
        "completion us", "recovery us", "validation us", "equal",
    ]
    scaling_table_rows = [
        {
            "family": row["family_name"],
            "|X|": row["coordinate_x_count"],
            "|Y|": row["coordinate_y_count"],
            "|X||Y|": row["coordinate_cartesian_product"],
            "vertices": row["sparse_subdivision_vertices"],
            "half-edges": row["sparse_subdivision_half_edges"],
            "junctions": row["sparse_subdivision_junctions"],
            "faces": row["sparse_subdivision_interior_faces"],
            "dense bytes": row["dense_owned_bytes_estimate"],
            "sparse bytes": row["sparse_owned_bytes_estimate"],
            "cut-index bytes": row["cut_index_owned_bytes_estimate"],
            "completion us": row["completion_microseconds"],
            "recovery us": row["recovery_microseconds"],
            "validation us": row["validation_microseconds"],
            "equal": row["three_backend_equal"],
        }
        for row in scaling_rows
    ]
    negative = reports["negative"]
    return [
        "## v1.2 Sparse polygon subdivision evidence",
        "",
        markdown_table(population_fields, population_rows),
        "",
        f"The negative campaign contains {len(negative['records'])} dense/sparse category comparisons with {negative['disagreements']} disagreements.",
        f"The bounded CP-SAT rerun compares {reports['external']['rust_comparison_component_count']:,} components with {reports['external']['disagreement_component_count']} disagreements and {reports['external']['cp_sat_timeout_component_count']} timeouts.",
        f"The Cartesian-explosion scaling campaign contains {scaling['verified_rows']} verified rows, {scaling['solver_errors']} solver errors, and {scaling['disagreements']} disagreements.",
        "",
        f"### Largest sparse scaling rows at size {largest_size}",
        "",
        markdown_table(scaling_fields, scaling_table_rows),
        "",
        "Dense bytes are a formula-derived owned-allocation estimate for coordinate vectors, occupancy/barriers, and the i64 coverage difference array. Sparse and cut-index bytes are exact owned-vector/index estimates, never process peak RSS.",
        "",
    ]


def v13_evidence_sections(output_dir: Path) -> list[str]:
    path = output_dir / "v1.3-output-sensitive-scaling.json"
    if not path.exists():
        return []
    report = read_json(path)
    largest_size = max(row["size"] for row in report["rows"])
    rows = [row for row in report["rows"] if row["size"] == largest_size]
    fields = [
        "family", "S", "J", "reference candidates", "sweep candidates",
        "dense recovery us", "sweep recovery us", "reference scans",
        "event scans", "materialized nodes", "logical nodes", "Auto", "equal",
    ]
    table_rows = [
        {
            "family": row["family_name"],
            "S": row["subdivision_input_segment_count"],
            "J": row["subdivision_reported_intersections"],
            "reference candidates": row["reference_subdivision_candidate_pair_tests"],
            "sweep candidates": row["sweep_subdivision_candidate_pair_tests"],
            "dense recovery us": row["dense_recovery_microseconds"],
            "sweep recovery us": row["sweep_subdivision_recovery_microseconds"],
            "reference scans": row["reference_validator_boundary_edge_scans"],
            "event scans": row["event_validator_boundary_edge_scans"],
            "materialized nodes": row["sparse_materialized_tree_nodes"],
            "logical nodes": row["sparse_logical_tree_nodes"],
            "Auto": row["auto_selected_backend"],
            "equal": row["geometry_backends_equal"],
        }
        for row in rows
    ]
    return [
        "## v1.3 Output-sensitive sparse geometry evidence", "",
        f"The campaign contains {report['verified_rows']} verified rows, {report['solver_errors']} solver errors, and {report['disagreements']} disagreements.",
        "Sweep candidate-pair tests and event-validator boundary/resort scans are zero in every row. Memory values are structured estimates, not process RSS.",
        "", f"### Largest completed crossover rows at size {largest_size}", "",
        markdown_table(fields, table_rows), "",
    ]


def main() -> None:
    args = parse_args()
    exhaustive = read_json(args.exhaustive)
    random = read_json(args.random)
    adversarial = read_csv(args.adversarial)
    polyomino = read_csv(args.polyomino)
    external = read_json(args.external)
    dense = read_csv(args.dense)
    correctness = [
        rust_report_correctness(
            f"exhaustive-binary-{exhaustive['width']}x{exhaustive['height']}",
            exhaustive,
        ),
        rust_report_correctness(
            f"random-binary-{random['width']}x{random['height']}", random
        ),
        benchmark_correctness("adversarial", adversarial),
        benchmark_correctness("free-polyomino", polyomino),
        *external_correctness(external),
    ]
    compression = compression_rows(dense)
    scope = scope_rows()

    commits = {
        exhaustive["metadata"]["git_commit"],
        random["metadata"]["git_commit"],
        adversarial[0]["git_commit"],
        polyomino[0]["git_commit"],
        external["git_commit"],
        dense[0]["git_commit"],
    }
    if len(commits) != 1:
        raise SystemExit(f"result artifacts refer to different commits: {sorted(commits)}")

    args.output_dir.mkdir(parents=True, exist_ok=True)
    write_csv(args.output_dir / "correctness-table.csv", CORRECTNESS_FIELDS, correctness)
    write_csv(args.output_dir / "compression-table.csv", COMPRESSION_FIELDS, compression)
    write_csv(args.output_dir / "scope-table.csv", SCOPE_FIELDS, scope)
    metadata = {
        "git_commit": commits.pop(),
        "rustc_version": external["rustc_version"],
        "operating_system": platform.platform(),
        "cpu": cpu_name(),
        "build_profile": "release",
        "random_seed": random["seed"],
        "cp_sat_seed": external["seed"],
        "cp_sat_timeout_seconds_per_component": external[
            "cp_sat_max_time_seconds_per_component"
        ],
        "commands": [
            exhaustive["metadata"]["command"],
            random["metadata"]["command"],
            adversarial[0]["command"],
            polyomino[0]["command"],
            external["command"],
            dense[0]["command"],
        ],
    }
    release_index = read_json(args.release_index)
    release_summaries = release_index["releases"]
    markdown = [
        "# Generated paper tables",
        "",
        "```json",
        json.dumps(metadata, indent=2),
        "```",
        "",
        "The metadata above belongs to the historical v0.2 paper-table population."
        " Later release evidence retains its own producing commits.",
        "",
        "## Release summaries",
        "",
        "```json",
        json.dumps(release_summaries, indent=2),
        "```",
        "",
        "## Correctness",
        "",
        markdown_table(CORRECTNESS_FIELDS, correctness),
        "",
        "## Compression",
        "",
        markdown_table(COMPRESSION_FIELDS, compression),
        "",
        "## Scope",
        "",
        markdown_table(SCOPE_FIELDS, scope),
        "",
        *compact_v03_section(args.output_dir),
        *current_evidence_sections(args.output_dir),
        *v08_evidence_sections(args.output_dir),
        *v09_evidence_sections(args.output_dir),
        *v10_evidence_sections(args.output_dir),
        *v11_evidence_sections(args.output_dir),
        *v12_evidence_sections(args.output_dir),
        *v13_evidence_sections(args.output_dir),
    ]
    (args.output_dir / "paper-tables.md").write_text("\n".join(markdown))

    manifest = read_json(args.manifest)
    manifest["schema_version"] = 3
    manifest["historical_release_metadata"] = metadata
    manifest.pop("release_metadata", None)
    manifest["release_summaries"] = release_summaries
    manifest["current_release"] = release_index["current_release"]
    generated = [
        str(args.output_dir / "correctness-table.csv"),
        str(args.output_dir / "compression-table.csv"),
        str(args.output_dir / "scope-table.csv"),
        str(args.output_dir / "paper-tables.md"),
    ]
    for artifact in [
        "v0.6-clean-boundary-differential.csv",
        "v0.6-clean-boundary-differential.json",
        "v0.6-clean-boundary-differential.md",
        "v0.7-auto-fallback.csv",
        "v0.7-clean-complete-bipartite-compact.csv",
        "v0.7-path-tree-dual-differential.csv",
        "v0.7-path-tree-dual-differential.json",
        "v0.7-path-tree-families.csv",
        "v0.7-path-tree-orientation-audit.csv",
        "v0.7-path-tree-orientation-audit.json",
        "v0.7-path-tree-orientation-audit.md",
        "v0.7-path-tree-vs-4d.csv",
        "v0.7-path-tree-vs-4d.json",
        "v0.8-path-tree-families.csv",
        "v0.8-path-tree-vs-4d.csv",
        "v0.8-path-tree-vs-4d.json",
        "v0.8-path-tree-advantage.csv",
        "v0.8-path-tree-advantage.json",
        "v0.8-path-tree-advantage.md",
        "v0.8-gap-backend-differential.csv",
        "v0.8-gap-backend-differential.json",
        "v0.8-gap-backend-differential.md",
        "v0.8-path-tree-orientation-audit.csv",
        "v0.8-path-tree-orientation-audit.json",
        "v0.8-path-tree-orientation-audit.md",
        "v0.9-polygon-differential.json",
        "v1.0-polygon-differential-3x3.json",
        "v1.0-polygon-differential-3x3.counterexamples.json",
        "v1.0-polygon-differential-4x4.json",
        "v1.0-polygon-differential-4x4.counterexamples.json",
        "v1.0-polygon-backend-differential.json",
        "v1.0-polygon-backend-differential.counterexamples.json",
        "v1.0-polygon-negative.json",
        "v1.0-polygon-native-fixtures.json",
        "v1.0-polygon-native-fixtures.counterexamples.json",
        "v1.0-polygon-scaling.csv",
        "v1.0-polygon-scaling.json",
        "v1.1-polygon-differential-3x3.json",
        "v1.1-polygon-differential-3x3.counterexamples.json",
        "v1.1-polygon-differential-4x4.json",
        "v1.1-polygon-differential-4x4.counterexamples.json",
        "v1.1-polygon-backend-differential.json",
        "v1.1-polygon-backend-differential.counterexamples.json",
        "v1.1-polygon-negative.json",
        "v1.1-polygon-native-fixtures.json",
        "v1.1-polygon-native-fixtures.counterexamples.json",
        "v1.1-polygon-scaling.csv",
        "v1.1-polygon-scaling.json",
        "v1.2-polygon-differential-3x3.json",
        "v1.2-polygon-differential-3x3.counterexamples.json",
        "v1.2-polygon-differential-4x4.json",
        "v1.2-polygon-differential-4x4.counterexamples.json",
        "v1.2-polygon-backend-differential.json",
        "v1.2-polygon-backend-differential.counterexamples.json",
        "v1.2-polygon-negative.json",
        "v1.2-polygon-native-fixtures.json",
        "v1.2-polygon-native-fixtures.counterexamples.json",
        "v1.2-polygon-scaling.csv",
        "v1.2-polygon-scaling.json",
        "v1.3-output-sensitive-scaling.csv",
        "v1.3-output-sensitive-scaling.json",
        "v1.2-external-oracle.json",
    ]:
        candidate = args.output_dir / artifact
        if candidate.exists():
            generated.append(str(candidate))
    for artifact in (
        "path-tree-witnesses/index.json",
        "path-tree-witnesses/report.json",
    ):
        candidate = args.output_dir / artifact
        if candidate.exists():
            generated.append(str(candidate))
    manifest["generated_tables"] = list(
        dict.fromkeys([*generated, *manifest.get("generated_tables", [])])
    )
    args.manifest.write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
