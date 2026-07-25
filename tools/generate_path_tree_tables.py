#!/usr/bin/env python3
"""Generate v0.6 compact path-tree summaries from the benchmark CSV."""

from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--csv", type=Path, default=Path("results/v0.6-clean-complete-bipartite-compact.csv"))
    parser.add_argument("--summary", type=Path, default=Path("results/v0.6-clean-complete-bipartite-compact-summary.json"))
    parser.add_argument("--markdown", type=Path, default=Path("results/v0.6-clean-complete-bipartite-compact.md"))
    args = parser.parse_args()

    rows = list(csv.DictReader(args.csv.open(newline="")))
    summary = {
        "schema_version": 1,
        "source_csv": str(args.csv),
        "git_commits": sorted({row["git_commit"] for row in rows}),
        "population": len(rows),
        "verified": sum(row["status"] == "verified" for row in rows),
        "counterexamples": sum(row["status"] == "counterexample" for row in rows),
        "solver_errors": sum(row["status"] == "solver-error" for row in rows),
        "rows": [
            {
                "t": int(json.loads(row["parameters"])["t"]),
                "q": int(row["total_chord_count"]),
                "horizontal_chords": int(row["horizontal_chord_count"]),
                "vertical_chords": int(row["vertical_chord_count"]),
                "dual_regions": int(row["dual_region_count"]),
                "path_count": int(row["path_count"]),
                "path_edge_incidences": int(row["path_edge_incidence_count"]),
                "canonical_nodes": int(row["canonical_segment_node_count"]),
                "sigma": int(row["path_tree_sigma"]),
                "explicit_edges": None if not row["explicit_conflict_edge_count"] else int(row["explicit_conflict_edge_count"]),
                "explicit_path_records": int(row["explicit_path_records_materialized"]),
                "dual_backend": row["region_dual_backend"],
            }
            for row in rows
        ],
    }
    args.summary.write_text(json.dumps(summary, indent=2) + "\n")
    lines = [
        "# v0.6 compact path-tree benchmark",
        "",
        f"Generated from `{args.csv}`; verified rows: {summary['verified']}/{summary['population']}.",
        "",
        "| t | q | H | V | dual regions | path count | path-edge incidences | canonical nodes | sigma | explicit E |",
        "| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    lines.extend(
        "| {t} | {q} | {horizontal_chords} | {vertical_chords} | {dual_regions} | {path_count} | {path_edge_incidences} | {canonical_nodes} | {sigma} | {explicit_edges} |".format(**row)
        for row in summary["rows"]
    )
    args.markdown.write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
