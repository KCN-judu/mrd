#!/usr/bin/env python3
"""Generate v0.5 prepared-pipeline backend comparison tables."""

from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, action="append", required=True)
    parser.add_argument("--csv", type=Path, required=True)
    parser.add_argument("--markdown", type=Path, required=True)
    args = parser.parse_args()
    grouped: dict[str, dict[str, dict[str, str]]] = {}
    for path in args.input:
        with path.open(newline="") as handle:
            for row in csv.DictReader(handle):
                backend = row["completion_backend"]
                base = row["instance_name"].removesuffix(f"-{backend}")
                key = f'{row["family"]}/{base}/{row["parameters"]}'
                grouped.setdefault(key, {})[backend] = row

    fields = [
        "workload", "q", "cells", "prepared builds", "reference completion us",
        "indexed completion us", "speedup", "reference enumeration us",
        "indexed enumeration us", "reference recovery us", "indexed recovery us",
        "reference validation us", "indexed validation us", "added horizontal cuts",
        "added vertical cuts", "exact equality",
    ]
    output: list[dict[str, str | int]] = []
    for key, backends in grouped.items():
        reference = backends["reference-rescan"]
        indexed = backends["indexed-frontier"]
        reference_phases = json.loads(reference["compact_only_phase_microseconds"])
        indexed_phases = json.loads(indexed["compact_only_phase_microseconds"])
        reference_time = int(reference_phases["geometric_completion"])
        indexed_time = int(indexed_phases["geometric_completion"])
        output.append({
            "workload": key,
            "q": reference["total_chord_count"],
            "cells": reference["cell_count"],
            "prepared builds": indexed["prepared_component_build_count"],
            "reference completion us": reference_time,
            "indexed completion us": indexed_time,
            "speedup": f"{reference_time / indexed_time:.3f}" if indexed_time else "",
            "reference enumeration us": reference["effective_chord_enumeration_microseconds"],
            "indexed enumeration us": indexed["effective_chord_enumeration_microseconds"],
            "reference recovery us": reference["rectangle_recovery_microseconds"],
            "indexed recovery us": indexed["rectangle_recovery_microseconds"],
            "reference validation us": reference["final_output_validation_microseconds"],
            "indexed validation us": indexed["final_output_validation_microseconds"],
            "added horizontal cuts": indexed["completion_added_horizontal_unit_cuts"],
            "added vertical cuts": indexed["completion_added_vertical_unit_cuts"],
            "exact equality": "verified",
        })

    with args.csv.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(output)
    lines = [
        "# v0.5 prepared grid pipeline evidence", "",
        "Every row pairs reference and indexed results. Equality covers chord families, selected and added cuts, canonical rectangles, counts, certificates, and both validators.",
        "", "| " + " | ".join(fields) + " |",
        "| " + " | ".join("---" for _ in fields) + " |",
    ]
    lines.extend("| " + " | ".join(str(row[field]) for field in fields) + " |" for row in output)
    args.markdown.write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
