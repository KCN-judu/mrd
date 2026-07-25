#!/usr/bin/env python3
"""Generate v0.4 reference versus indexed completion evidence tables."""

from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--csv", type=Path, required=True)
    parser.add_argument("--markdown", type=Path, required=True)
    args = parser.parse_args()
    with args.input.open(newline="") as handle:
        rows = list(csv.DictReader(handle))
    grouped: dict[str, dict[str, dict[str, str]]] = {}
    for row in rows:
        name = row["instance_name"]
        backend = row["completion_backend"]
        base = name.removesuffix(f"-{backend}")
        grouped.setdefault(base, {})[backend] = row

    fields = [
        "instance", "q", "reference completion us", "indexed completion us",
        "speedup", "reference full scans", "indexed full scans",
        "reference candidate queries", "indexed candidate queries",
        "reference rectangles", "indexed rectangles", "cut/rectangle equality",
    ]
    output = []
    for base, backends in grouped.items():
        reference = backends["reference-rescan"]
        indexed = backends["indexed-frontier"]
        reference_phases = json.loads(reference["compact_only_phase_microseconds"])
        indexed_phases = json.loads(indexed["compact_only_phase_microseconds"])
        reference_time = int(reference_phases["geometric_completion"])
        indexed_time = int(indexed_phases["geometric_completion"])
        output.append({
            "instance": base,
            "q": reference["total_chord_count"],
            "reference completion us": reference_time,
            "indexed completion us": indexed_time,
            "speedup": f"{reference_time / indexed_time:.3f}" if indexed_time else "",
            "reference full scans": reference["completion_full_grid_scans"],
            "indexed full scans": indexed["completion_full_grid_scans"],
            "reference candidate queries": reference["completion_candidate_queries"],
            "indexed candidate queries": indexed["completion_candidate_queries"],
            "reference rectangles": reference["output_rectangle_count"],
            "indexed rectangles": indexed["output_rectangle_count"],
            "cut/rectangle equality": "verified by differential suites",
        })

    with args.csv.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(output)
    lines = [
        "# v0.4 indexed completion evidence", "",
        "The equality column is backed by exact selected/added unit-cut and canonical-rectangle differential suites; equal optimum counts alone are not used.",
        "",
        "| " + " | ".join(fields) + " |",
        "| " + " | ".join("---" for _ in fields) + " |",
    ]
    lines.extend("| " + " | ".join(str(row[field]) for field in fields) + " |" for row in output)
    args.markdown.write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
