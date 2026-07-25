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
    require(manifest["schema_version"] >= 3, "manifest schema is not release-separated")
    require("historical_release_metadata" in manifest, "historical metadata is missing")
    require("release_metadata" not in manifest, "legacy release metadata key remains")
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

    current = index.get("current_release")
    require(current is not None, "current release metadata is missing")
    require(current["version"] == workspace_version, "current release version differs")
    manifest_current = manifest.get("current_release")
    require(manifest_current == current, "manifest and release-index current release differ")
    if current["peeled_commit"] != "PENDING":
        peeled = git("rev-parse", f"{current['tag']}^{{commit}}")
        require(peeled == current["peeled_commit"], f"current tag target mismatch: {current['tag']}")

    algorithms = (ROOT / "docs/ALGORITHMS.md").read_text()
    experiments = (ROOT / "docs/EXPERIMENTS.md").read_text()
    tables = (ROOT / "results/paper-tables.md").read_text()
    scope = (ROOT / "results/scope-table.csv").read_text()
    require("GridInteriorRunEnumerator" in algorithms, "algorithm enumerator label is stale")
    require("GridInteriorRunEnumerator" in scope, "generated scope enumerator label is stale")
    require("path-tree" in algorithms, "path-tree representation label is stale")
    require((ROOT / "docs/CLEAN_HOLE_FREE.md").exists(), "clean eligibility document missing")
    require((ROOT / "docs/PATH_TREE_REPRESENTATION.md").exists(), "path-tree document missing")
    require((ROOT / "docs/PATH_TREE_ORIENTATION.md").exists(), "orientation document missing")
    require((ROOT / "docs/PATH_TREE_BENCHMARK_FAMILIES.md").exists(), "family document missing")
    require((ROOT / "docs/BOUNDARY_DUAL_CONSTRUCTION.md").exists(), "boundary dual document missing")
    require((ROOT / "docs/RELEASE_NOTES_V0.6.md").exists(), "v0.6 release notes missing")
    require("v0.3 compact execution evidence" in experiments, "v0.3 evidence section missing")
    require('"version": "0.3.0"' in tables, "generated v0.3 release summary missing")
    require(index["defaults"]["compact_chord_enumerator"] == "grid-interior-runs", "wrong CompactOnly enumerator default")
    require(index["defaults"]["compact_completion_backend"] in {"reference-rescan", "indexed-frontier"}, "unknown completion backend label")
    require(index["defaults"]["compact_rectangle_recovery_backend"] == "dense-grid", "wrong CompactOnly recovery backend")
    require(index["defaults"].get("compact_region_dual") == "boundary-laminar", "wrong CompactOnly region dual default")
    require(index["defaults"].get("fully_audited_region_dual") == "reference-area", "wrong FullyAudited region dual default")
    require(index["defaults"].get("compact_conflict_representation", "dominance-4d") in {"dominance-4d", "auto", "path-tree"}, "unknown conflict representation label")
    require(index["defaults"].get("compact_path_tree_orientation") in {"build-both", "bound-estimate", "vertical-tree", "horizontal-tree"}, "unknown path-tree orientation default")
    require("BoundaryLaminar" in algorithms, "boundary dual implementation label is stale")
    require("v0.6 true compact path-tree evidence" in experiments, "v0.6 evidence section missing")
    require((ROOT / "results/v0.6-clean-complete-bipartite-compact.csv").exists(), "v0.6 compact benchmark missing")
    require((ROOT / "results/v0.6-clean-complete-bipartite-compact-summary.json").exists(), "v0.6 compact summary missing")
    require((ROOT / "results/v0.6-clean-complete-bipartite-compact.md").exists(), "v0.6 compact generated table missing")
    require('"version": "0.6.0"' in tables, "generated v0.6 release summary missing")
    require('"version": "0.7.0"' in tables, "generated v0.7 release summary missing")
    require("v0.7 structural path-tree evidence" in experiments, "v0.7 evidence section missing")
    orientation = json.loads((ROOT / "results/v0.7-path-tree-orientation-audit.json").read_text())
    require(orientation["mismatches"] == 0, "orientation audit has positive regret")
    require(orientation["exact_matches"] == orientation["rows"], "orientation audit is incomplete")
    dual = json.loads((ROOT / "results/v0.7-path-tree-dual-differential.json").read_text())
    require(dual["counterexamples"] == 0, "dual differential has counterexamples")
    require(dual["solver_errors"] == 0, "dual differential has solver errors")
    for artifact in [
        "results/v0.7-path-tree-orientation-audit.csv",
        "results/v0.7-path-tree-orientation-audit.json",
        "results/v0.7-path-tree-orientation-audit.md",
        "results/v0.7-path-tree-dual-differential.csv",
        "results/v0.7-path-tree-dual-differential.json",
        "results/v0.7-auto-fallback.csv",
    ]:
        require(artifact in manifest["generated_tables"], f"generated artifact is not indexed: {artifact}")
        require((ROOT / artifact).exists(), f"generated artifact missing: {artifact}")
    print(f"release consistency: ok (workspace {workspace_version})")


if __name__ == "__main__":
    main()
