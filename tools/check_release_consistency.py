#!/usr/bin/env python3
"""Check release provenance, generated metadata, and implementation labels."""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def main() -> None:
    cargo = (ROOT / "Cargo.toml").read_text()
    match = re.search(r'\[workspace\.package\].*?version = "([^"]+)"', cargo, re.S)
    require(match is not None, "workspace version is missing")
    workspace_version = match.group(1)
    index = json.loads((ROOT / "results/release-index.json").read_text())
    require(
        index["current_workspace_version"] == workspace_version,
        "release index and workspace versions differ",
    )

    manifest = json.loads((ROOT / "results/manifest.json").read_text())
    indexed_commits = {
        commit
        for release in index["releases"]
        for commit in release["result_commits"]
    }
    for run in manifest["runs"]:
        commit = git("rev-parse", f"{run['git_commit']}^{{commit}}")
        require(bool(commit), f"manifest commit is unreachable: {run['git_commit']}")
    for commit in indexed_commits:
        require(bool(git("rev-parse", f"{commit}^{{commit}}")), f"unknown result commit: {commit}")

    for release in index["releases"]:
        if release["peeled_commit"] == "PENDING":
            continue
        peeled = git("rev-parse", f"{release['tag']}^{{commit}}")
        require(peeled == release["peeled_commit"], f"tag target mismatch: {release['tag']}")

    algorithms = (ROOT / "docs/ALGORITHMS.md").read_text()
    experiments = (ROOT / "docs/EXPERIMENTS.md").read_text()
    tables = (ROOT / "results/paper-tables.md").read_text()
    scope = (ROOT / "results/scope-table.csv").read_text()
    require("GridInteriorRunEnumerator" in algorithms, "algorithm enumerator label is stale")
    require("GridInteriorRunEnumerator" in scope, "generated scope enumerator label is stale")
    require("path-tree" in algorithms, "path-tree representation label is stale")
    require((ROOT / "docs/CLEAN_HOLE_FREE.md").exists(), "clean eligibility document missing")
    require((ROOT / "docs/PATH_TREE_REPRESENTATION.md").exists(), "path-tree document missing")
    require("v0.3 compact execution evidence" in experiments, "v0.3 evidence section missing")
    require('"version": "0.3.0"' in tables, "generated v0.3 release summary missing")
    require(index["defaults"]["compact_chord_enumerator"] == "grid-interior-runs", "wrong CompactOnly enumerator default")
    require(index["defaults"]["compact_completion_backend"] in {"reference-rescan", "indexed-frontier"}, "unknown completion backend label")
    require(index["defaults"]["compact_rectangle_recovery_backend"] == "dense-grid", "wrong CompactOnly recovery backend")
    require(index["defaults"].get("compact_conflict_representation", "dominance-4d") in {"dominance-4d", "auto", "path-tree"}, "unknown conflict representation label")
    print(f"release consistency: ok (workspace {workspace_version})")


if __name__ == "__main__":
    main()
