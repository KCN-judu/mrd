#!/usr/bin/env python3
"""Reject an incorrect log^3 bound near four-dimensional biclique claims."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path


FOUR_DIMENSIONAL = r"(?:four[- ]dimensional|four coordinates|4D|d\s*=\s*4)"
LOG_CUBED = r"(?:log\s*\^?\s*3|log³)"
REQUIRED_NOTE = (
    "The implementation starts the comparability-bigraph recursion with four\n"
    "coordinates. Therefore the general Cardinal--Yuditsky bound specializes to\n"
    "`O(q log^4 q)`, not `O(q log^3 q)`."
)


def tracked_text_files() -> list[Path]:
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        text=True,
    ).splitlines()
    return [
        Path(path)
        for path in paths
        if Path(path).is_file()
        if Path(path).resolve() != Path(__file__).resolve()
        if Path(path).suffix
        in {".md", ".rs", ".py", ".toml", ".txt", ".yml", ".yaml"}
    ]


def main() -> None:
    failures: list[str] = []
    proximity = re.compile(
        rf"(?:{FOUR_DIMENSIONAL}.{{0,240}}{LOG_CUBED}|"
        rf"{LOG_CUBED}.{{0,240}}{FOUR_DIMENSIONAL})",
        re.IGNORECASE | re.DOTALL,
    )
    for path in tracked_text_files():
        text = path.read_text(encoding="utf-8")
        sanitized = text.replace(REQUIRED_NOTE, "")
        if proximity.search(sanitized):
            failures.append(str(path))
    algorithms = Path("docs/ALGORITHMS.md").read_text(encoding="utf-8")
    if REQUIRED_NOTE not in algorithms:
        failures.append("docs/ALGORITHMS.md (required specialization note missing)")
    if failures:
        raise SystemExit(
            "incorrect or incomplete four-dimensional biclique bound documentation: "
            + ", ".join(failures)
        )


if __name__ == "__main__":
    main()
