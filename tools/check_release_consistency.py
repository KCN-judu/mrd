#!/usr/bin/env python3
"""Check release metadata, tags, defaults, and generated evidence consistency."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERSION = "1.2.0"
TAG = "v1.2.0-sparse-polygon-subdivision"
EXPECTED_DEFAULTS = {
    "compact_chord_enumerator": "grid-interior-runs",
    "compact_completion_backend": "indexed-frontier",
    "compact_rectangle_recovery_backend": "dense-grid",
    "compact_conflict_representation": "dominance-4d",
    "compact_path_tree_orientation": "build-both",
    "compact_region_dual": "boundary-laminar",
    "fully_audited_completion_backend": "reference-rescan",
    "fully_audited_region_dual": "reference-area",
    "fully_audited_path_tree_orientation": "build-both",
    "compact_polygon_geometry": "indexed",
    "compact_polygon_validator": "orthogonal-sweep",
    "compact_polygon_chords": "sg-sweep",
    "compact_polygon_completion": "indexed-frontier",
    "compact_polygon_arrangement": "sparse-subdivision",
    "compact_polygon_cut_index": "dynamic-stabbing",
    "compact_polygon_dissection_validator": "sparse-slab",
}
HISTORICAL_V08_DEFAULTS = {
    **EXPECTED_DEFAULTS,
    "compact_path_tree_orientation": "bound-estimate",
}
REQUIRED_V08_ARTIFACTS = (
    "results/v0.8-gap-backend-differential.csv",
    "results/v0.8-gap-backend-differential.json",
    "results/v0.8-gap-backend-differential.md",
    "results/v0.8-path-tree-families.csv",
    "results/v0.8-path-tree-vs-4d.csv",
    "results/v0.8-path-tree-vs-4d.json",
    "results/v0.8-path-tree-advantage.csv",
    "results/v0.8-path-tree-advantage.json",
    "results/v0.8-path-tree-advantage.md",
    "results/v0.8-path-tree-orientation-audit.csv",
    "results/v0.8-path-tree-orientation-audit.json",
    "results/v0.8-path-tree-orientation-audit.md",
    "results/path-tree-witnesses/index.json",
    "results/path-tree-witnesses/report.json",
)
REQUIRED_V09_ARTIFACTS = ("results/v0.9-polygon-differential.json",)
REQUIRED_V10_ARTIFACTS = (
    "results/v1.0-polygon-differential-3x3.json",
    "results/v1.0-polygon-differential-4x4.json",
    "results/v1.0-polygon-backend-differential.json",
    "results/v1.0-polygon-negative.json",
    "results/v1.0-polygon-native-fixtures.json",
    "results/v1.0-polygon-scaling.csv",
    "results/v1.0-polygon-scaling.json",
)
REQUIRED_V11_ARTIFACTS = (
    "results/v1.1-polygon-differential-3x3.json",
    "results/v1.1-polygon-differential-4x4.json",
    "results/v1.1-polygon-backend-differential.json",
    "results/v1.1-polygon-negative.json",
    "results/v1.1-polygon-native-fixtures.json",
    "results/v1.1-polygon-scaling.csv",
    "results/v1.1-polygon-scaling.json",
)
REQUIRED_V12_ARTIFACTS = (
    "results/v1.2-external-oracle.json",
    "results/v1.2-polygon-differential-3x3.json",
    "results/v1.2-polygon-differential-4x4.json",
    "results/v1.2-polygon-backend-differential.json",
    "results/v1.2-polygon-negative.json",
    "results/v1.2-polygon-native-fixtures.json",
    "results/v1.2-polygon-scaling.csv",
    "results/v1.2-polygon-scaling.json",
)


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def fail(message: str) -> None:
    print(f"release consistency error: {message}", file=sys.stderr)
    raise SystemExit(1)


def assert_defaults(
    value: dict, label: str, expected_defaults: dict[str, str] = EXPECTED_DEFAULTS
) -> None:
    defaults = value.get("defaults", {})
    for key, expected in expected_defaults.items():
        if defaults.get(key) != expected:
            fail(f"{label} default {key}={defaults.get(key)!r}, expected {expected!r}")


def resolve_commit(commit: str, label: str) -> str:
    try:
        return git("rev-parse", f"{commit}^{{commit}}")
    except subprocess.CalledProcessError:
        fail(f"{label} does not resolve to a commit: {commit}")


def assert_reachable(commit: str, head: str, label: str) -> None:
    resolved = resolve_commit(commit, label)
    reachable = subprocess.run(
        ["git", "merge-base", "--is-ancestor", resolved, head],
        cwd=ROOT,
    ).returncode == 0
    if not reachable:
        fail(f"{label} is not reachable from HEAD: {commit}")


def assert_implementation_defaults() -> None:
    implementation = (ROOT / "crates/rect-dominance/src/lib.rs").read_text(
        encoding="utf-8"
    )
    merged = re.search(
        r"VerificationMode::FullyAudited\s*\|\s*VerificationMode::CompactOnly\s*=>\s*\{?\s*PathTreeOrientationPolicy::BuildBothExact",
        implementation,
    )
    if merged:
        return
    compact = re.search(
        r"VerificationMode::CompactOnly\s*=>\s*PathTreeOrientationPolicy::([A-Za-z0-9_]+)",
        implementation,
    )
    audited = re.search(
        r"VerificationMode::FullyAudited\s*=>\s*PathTreeOrientationPolicy::([A-Za-z0-9_]+)",
        implementation,
    )
    if not compact or compact.group(1) != "BuildBothExact":
        fail("CompactOnly implementation default is not BuildBothExact")
    if not audited or audited.group(1) != "BuildBothExact":
        fail("FullyAudited implementation default is not BuildBothExact")
    defaults = re.search(
        r"impl Default for PolygonSolveOptions.*?\n}\n",
        implementation,
        re.DOTALL,
    )
    if defaults is None:
        fail("PolygonSolveOptions default implementation is missing")
    for expected in (
        "geometry_backend: PolygonGeometryBackend::Indexed",
        "validation_backend: PolygonValidationBackend::OrthogonalSweep",
        "chord_backend: PolygonChordBackend::SoltanGorpinevichSweep",
        "completion_backend: PolygonCompletionBackend::IndexedFrontier",
        "cut_index_backend: PolygonCutIndexBackend::DynamicStabbing",
        "recovery_backend: PolygonRecoveryBackend::SparseSubdivision",
        "dissection_validator_backend: PolygonDissectionValidatorBackend::SparseSlab",
        "arrangement_backend: PolygonArrangementBackend::Indexed",
    ):
        if expected not in defaults.group(0):
            fail(f"polygon implementation default does not select the sweep: {expected}")


def main() -> None:
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version\s*=\s*"([^"]+)"', cargo, re.MULTILINE)
    if not match or match.group(1) != VERSION:
        fail(f"workspace version is not {VERSION}")

    release_index = json.loads((ROOT / "results/release-index.json").read_text())
    if release_index.get("current_workspace_version") != VERSION:
        fail("release-index workspace version mismatch")
    assert_defaults(release_index, "release-index workspace")
    current = release_index.get("current_release", {})
    if current.get("version") != VERSION or current.get("tag") != TAG:
        fail("release-index current release mismatch")
    peeled = current.get("peeled_commit")
    if not peeled or peeled == "PENDING":
        fail("release-index current release still has a pending commit")
    assert_implementation_defaults()

    manifest = json.loads((ROOT / "results/manifest.json").read_text())
    if manifest.get("schema_version", 0) < 3:
        fail("manifest schema is older than v0.7")
    manifest_current = manifest.get("current_release", {})
    if manifest_current.get("version") != VERSION or manifest_current.get("tag") != TAG:
        fail("manifest current release mismatch")
    if current.get("defaults") != manifest_current.get("defaults"):
        fail("manifest current release defaults differ from release-index")
    if current.get("version") == "0.8.0":
        assert_defaults(
            current, "historical v0.8 release", HISTORICAL_V08_DEFAULTS
        )
    else:
        assert_defaults(current, "current release")

    releases = release_index.get("releases", [])
    if manifest.get("release_summaries") != releases:
        fail("manifest release summaries differ from release-index releases")

    try:
        tagged = git("rev-parse", f"{TAG}^{{}}")
    except subprocess.CalledProcessError:
        fail(f"tag {TAG} does not exist")
    if tagged != peeled:
        fail(f"tag peels to {tagged}, release-index records {peeled}")

    head = git("rev-parse", "HEAD")
    for release in releases:
        release_tag = release.get("tag")
        release_commit = release.get("peeled_commit")
        if not release_tag or not release_commit or release_commit == "PENDING":
            fail(f"invalid release entry: {release!r}")
        try:
            actual = git("rev-parse", f"{release_tag}^{{}}")
        except subprocess.CalledProcessError:
            fail(f"release tag does not exist: {release_tag}")
        if actual != resolve_commit(release_commit, f"release {release_tag}"):
            fail(f"release tag {release_tag} peels to {actual}, expected {release_commit}")
        assert_reachable(release_commit, head, f"release {release_tag}")
        for result_commit in release.get("result_commits", []):
            assert_reachable(
                result_commit,
                head,
                f"result commit for release {release_tag}",
            )

    commits = []
    for run in manifest.get("runs", []):
        commit = run.get("git_commit")
        if not commit or commit in commits:
            continue
        commits.append(commit)
        assert_reachable(commit, head, "manifest run commit")

    paper_tables = (ROOT / "results/paper-tables.md").read_text(encoding="utf-8")
    experiments = (ROOT / "docs/EXPERIMENTS.md").read_text(encoding="utf-8")
    if "v0.8" not in paper_tables or "v0.8" not in experiments:
        fail("generated evidence does not contain a v0.8 section")
    if "v0.9" not in paper_tables or "v0.9" not in experiments:
        fail("generated evidence does not contain a v0.9 section")
    if "v1.0" not in paper_tables or "v1.0" not in experiments:
        fail("generated evidence does not contain a v1.0 section")
    if "v1.1" not in paper_tables or "v1.1" not in experiments:
        fail("generated evidence does not contain a v1.1 section")
    if "v1.2" not in paper_tables or "v1.2" not in experiments:
        fail("generated evidence does not contain a v1.2 section")
    generated_tables = set(manifest.get("generated_tables", []))
    for relative in REQUIRED_V08_ARTIFACTS:
        if not (ROOT / relative).is_file():
            fail(f"missing generated v0.8 evidence: {relative}")
        if relative not in generated_tables:
            fail(f"manifest omits generated v0.8 evidence: {relative}")
    for relative in REQUIRED_V09_ARTIFACTS:
        if not (ROOT / relative).is_file():
            fail(f"missing generated v0.9 evidence: {relative}")
        if relative not in generated_tables:
            fail(f"manifest omits generated v0.9 evidence: {relative}")
    for relative in REQUIRED_V10_ARTIFACTS:
        if not (ROOT / relative).is_file():
            fail(f"missing generated v1.0 evidence: {relative}")
        if relative not in generated_tables:
            fail(f"manifest omits generated v1.0 evidence: {relative}")
    for relative in REQUIRED_V11_ARTIFACTS:
        if not (ROOT / relative).is_file():
            fail(f"missing generated v1.1 evidence: {relative}")
        if relative not in generated_tables:
            fail(f"manifest omits generated v1.1 evidence: {relative}")
    for relative in REQUIRED_V12_ARTIFACTS:
        if not (ROOT / relative).is_file():
            fail(f"missing generated v1.2 evidence: {relative}")
        if relative not in generated_tables:
            fail(f"manifest omits generated v1.2 evidence: {relative}")
    v09_report = json.loads(
        (ROOT / "results/v0.9-polygon-differential.json").read_text()
    )
    v09_release = next(
        (release for release in releases if release.get("version") == "0.9.0"), None
    )
    if v09_release is None:
        fail("release index omits v0.9")
    producing_commit = v09_report.get("producing_commit")
    if producing_commit not in v09_release.get("result_commits", []):
        fail("v0.9 polygon evidence producer is not a v0.9 result commit")
    assert_reachable(producing_commit, head, "v0.9 polygon evidence producer")
    extended = next(
        (
            population
            for population in v09_report.get("populations", [])
            if population.get("name")
            == "polyomino-adversarial-complete-bipartite-random"
        ),
        None,
    )
    if extended is None or (
        extended.get("input_count"),
        extended.get("supported_component_count"),
        extended.get("rejected_component_count"),
        extended.get("disagreement_count"),
    ) != (7529, 7276, 255, 0):
        fail("v0.9 extended polygon differential counts are stale")
    if v09_report.get("definition_7_focused_test_count") != 5:
        fail("v0.9 Definition 7 focused test count is stale")
    if v09_report.get("validator_negative_case_count") != 11:
        fail("v0.9 validator rejection count is stale")
    required_fixtures = {
        "scaled-complete-bipartite.json",
        "reflex-heavy-stretched.json",
    }
    if not required_fixtures.issubset(v09_report.get("native_fixtures", [])):
        fail("v0.9 native polygon stress fixtures are missing")
    normalized_tables = paper_tables.lower()
    for required_text in (
        "strict path-tree advantages: 14",
        "positive-regret rows",
        "950,557 inputs",
    ):
        if required_text not in normalized_tables:
            fail(f"generated paper tables omit v0.8 evidence: {required_text}")
    if "retains three canonical clean witnesses" in experiments:
        fail("experiment documentation still reports only three witnesses")
    if "No strict sigma advantage was found" in experiments:
        fail("experiment documentation still reports no strict sigma advantage")
    algorithms = (ROOT / "docs/ALGORITHMS.md").read_text(encoding="utf-8")
    if "GridInteriorRunEnumerator" not in algorithms or "BoundaryIndex" not in algorithms:
        fail("algorithm documentation does not name the indexed production path")
    for required in (
        "IndexedPolygonPairwiseEnumerator",
        "SoltanGorpinevichSweepEnumerator",
        "IndexedPolygonCompletion",
        "OrthogonalSweepValidator",
        "PreparedPolygonContext",
    ):
        if required not in algorithms:
            fail(f"algorithm documentation omits v1.0 indexed symbol: {required}")
    for relative in REQUIRED_V10_ARTIFACTS:
        if not relative.endswith(".json"):
            continue
        report = json.loads((ROOT / relative).read_text())
        if report.get("disagreements", 0) != 0:
            fail(f"v1.0 report has disagreements: {relative}")
    for relative in REQUIRED_V11_ARTIFACTS:
        if not relative.endswith(".json"):
            continue
        report = json.loads((ROOT / relative).read_text())
        if report.get("disagreements", 0) != 0:
            fail(f"v1.1 report has disagreements: {relative}")
    print(f"release consistency: {VERSION} {TAG} -> {peeled}")
    print(f"reachable manifest commits: {len(commits)}")


if __name__ == "__main__":
    main()
