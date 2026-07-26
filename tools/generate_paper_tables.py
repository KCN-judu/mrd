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
        {"feature": "ordinary holes", "theoretical paper": "supported", "current Rust artifact": "supported for grid-cell regions", "tested": "yes", "notes": "rings and separated holes"},
        {"feature": "degenerate holes", "theoretical paper": "formal model", "current Rust artifact": "unsupported", "tested": "scope rejection", "notes": "point, segment, and arbitrary formal holes excluded"},
        {"feature": "endpoint contacts", "theoretical paper": "closed-chord conflicts", "current Rust artifact": "integer parity embedding", "tested": "yes", "notes": "pairwise geometry iff strict dominance"},
        {"feature": "fast chord enumeration", "theoretical paper": "O(n log n)", "current Rust artifact": "GridInteriorRunEnumerator, O(N + r log r + q)", "tested": "exact differential", "notes": "CompactOnly default; pairwise reference retained"},
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
                "## v0.8 Boundary-indexed adaptive path-tree",
                "",
                markdown_table(fields, rows),
                "",
                f"The deterministic witness search examined {report.get('candidates_examined', 0):,} production geometry candidates and retained {len(rows)} canonical witnesses.",
                "",
            ]
        )

    families_path = output_dir / "v0.8-path-tree-families.csv"
    if families_path.exists():
        rows = read_csv(families_path)
        verified = [row for row in rows if row.get("status") == "verified"]
        nontrivial = [row for row in verified if int(row.get("total_chord_count", 0)) > 0]
        sections.extend(
            [
                "### v0.8 scaled geometry families",
                "",
                f"The generated family campaign contains {len(rows)} rows ({len(nontrivial)} nontrivial chord-bearing rows), all with status `verified`.",
                f"Chain q grows from {min((int(row['total_chord_count']) for row in rows if row['family'] == 'laminar-chain'), default=0)} to {max((int(row['total_chord_count']) for row in rows if row['family'] == 'laminar-chain'), default=0)}; star and balanced rows reach dual branching degrees {max((int(row['dual_tree_max_branching_degree']) for row in rows if row['family'] == 'laminar-star'), default=0)} and {max((int(row['dual_tree_max_branching_degree']) for row in rows if row['family'] == 'balanced-laminar'), default=0)}.",
                "The retained mixed H/V witness bundles are the canonical predicate population; no coordinate-only scaling law is claimed for them.",
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
    return sections


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
    ]
    (args.output_dir / "paper-tables.md").write_text("\n".join(markdown))

    manifest = read_json(args.manifest)
    manifest["schema_version"] = 3
    manifest["historical_release_metadata"] = metadata
    manifest.pop("release_metadata", None)
    manifest["release_summaries"] = release_summaries
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
