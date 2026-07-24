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
        {"feature": "fast chord enumeration", "theoretical paper": "O(n log n)", "current Rust artifact": "exact aligned-reflex pair tests", "tested": "yes", "notes": "classical sweep not implemented"},
        {"feature": "compact biclique partition", "theoretical paper": "O(q log^4 q) for d=4", "current Rust artifact": "constructive Theorem 8 recursion", "tested": "yes", "notes": "edge multiplicity audited exactly once"},
        {"feature": "practical Dinic backend", "theoretical paper": "replaceable exact flow", "current Rust artifact": "implemented", "tested": "yes", "notes": "integral flow and residual cut"},
        {"feature": "almost-linear theoretical flow backend", "theoretical paper": "used asymptotically", "current Rust artifact": "not implemented", "tested": "no", "notes": "citation-only complexity component"},
        {"feature": "explicit rectangle output", "theoretical paper": "constructive completion", "current Rust artifact": "implemented", "tested": "yes", "notes": "cell-exact validation"},
        {"feature": "machine-checkable certificates", "theoretical paper": "not an artifact requirement", "current Rust artifact": "implemented", "tested": "yes", "notes": "matching, partition, flow, cut, and rectangles"},
    ]


def write_csv(path: Path, fields: list[str], rows: list[dict[str, Any]]) -> None:
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
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
    markdown = [
        "# Generated paper tables",
        "",
        "```json",
        json.dumps(metadata, indent=2),
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
    ]
    (args.output_dir / "paper-tables.md").write_text("\n".join(markdown))

    manifest = read_json(args.manifest)
    manifest["schema_version"] = 2
    manifest["release_metadata"] = metadata
    manifest["generated_tables"] = [
        str(args.output_dir / "correctness-table.csv"),
        str(args.output_dir / "compression-table.csv"),
        str(args.output_dir / "scope-table.csv"),
        str(args.output_dir / "paper-tables.md"),
    ]
    args.manifest.write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
