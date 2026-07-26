#!/usr/bin/env python3
"""Check release metadata, tags, defaults, and generated evidence consistency."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERSION = "0.8.0"
TAG = "v0.8.0-boundary-indexed-adaptive-path-tree"
EXPECTED_DEFAULTS = {
    "compact_chord_enumerator": "grid-interior-runs",
    "compact_completion_backend": "indexed-frontier",
    "compact_path_tree_orientation": "bound-estimate",
    "compact_region_dual": "boundary-laminar",
    "fully_audited_completion_backend": "reference-rescan",
    "fully_audited_path_tree_orientation": "build-both",
}


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def fail(message: str) -> None:
    print(f"release consistency error: {message}", file=sys.stderr)
    raise SystemExit(1)


def assert_defaults(value: dict, label: str) -> None:
    defaults = value.get("defaults", {})
    for key, expected in EXPECTED_DEFAULTS.items():
        if defaults.get(key) != expected:
            fail(f"{label} default {key}={defaults.get(key)!r}, expected {expected!r}")


def main() -> None:
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version\s*=\s*"([^"]+)"', cargo, re.MULTILINE)
    if not match or match.group(1) != VERSION:
        fail("workspace version is not 0.8.0")

    release_index = json.loads((ROOT / "results/release-index.json").read_text())
    if release_index.get("current_workspace_version") != VERSION:
        fail("release-index workspace version mismatch")
    assert_defaults(release_index, "release-index")
    current = release_index.get("current_release", {})
    if current.get("version") != VERSION or current.get("tag") != TAG:
        fail("release-index current release mismatch")
    peeled = current.get("peeled_commit")
    if not peeled or peeled == "PENDING":
        fail("release-index current release still has a pending commit")

    manifest = json.loads((ROOT / "results/manifest.json").read_text())
    if manifest.get("schema_version", 0) < 3:
        fail("manifest schema is older than v0.7")
    manifest_current = manifest.get("current_release", {})
    if manifest_current.get("version") != VERSION or manifest_current.get("tag") != TAG:
        fail("manifest current release mismatch")
    assert_defaults(manifest_current, "manifest")

    try:
        tagged = git("rev-parse", f"{TAG}^{{}}")
    except subprocess.CalledProcessError:
        fail(f"tag {TAG} does not exist")
    if tagged != peeled:
        fail(f"tag peels to {tagged}, release-index records {peeled}")

    head = git("rev-parse", "HEAD")
    commits = []
    for run in manifest.get("runs", []):
        commit = run.get("git_commit")
        if not commit or commit in commits:
            continue
        commits.append(commit)
        try:
            reachable = subprocess.run(
                ["git", "merge-base", "--is-ancestor", commit, head],
                cwd=ROOT,
            ).returncode == 0
        except OSError:
            reachable = False
        if not reachable:
            fail(f"manifest run commit is not reachable from HEAD: {commit}")

    paper_tables = (ROOT / "results/paper-tables.md").read_text(encoding="utf-8")
    experiments = (ROOT / "docs/EXPERIMENTS.md").read_text(encoding="utf-8")
    if "v0.8" not in paper_tables or "v0.8" not in experiments:
        fail("generated evidence does not contain a v0.8 section")
    for relative in (
        "results/v0.8-path-tree-families.csv",
        "results/v0.8-path-tree-vs-4d.csv",
        "results/v0.8-path-tree-advantage.csv",
        "results/path-tree-witnesses/index.json",
    ):
        if not (ROOT / relative).is_file():
            fail(f"missing generated v0.8 evidence: {relative}")
    algorithms = (ROOT / "docs/ALGORITHMS.md").read_text(encoding="utf-8")
    if "GridInteriorRunEnumerator" not in algorithms or "BoundaryIndex" not in algorithms:
        fail("algorithm documentation does not name the indexed production path")
    print(f"release consistency: {VERSION} {TAG} -> {peeled}")
    print(f"reachable manifest commits: {len(commits)}")


if __name__ == "__main__":
    main()
