#!/usr/bin/env python3
"""Check release metadata, tags, defaults, and generated evidence consistency."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

from check_p1_baseline import main as check_p1_baseline


ROOT = Path(__file__).resolve().parents[1]
VERSION = "1.3.0"
TAG = "v1.3.0-output-sensitive-sparse-geometry"
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
    "compact_polygon_subdivision_builder": "orthogonal-sweep",
    "compact_sparse_validator": "event-segment-tree",
    "compact_polygon_recovery_policy": "sparse-subdivision",
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
REQUIRED_AN19_EVENT_ARTIFACTS = (
    "results/an19-event-adversarial.json",
    "results/an19-event-adversarial.md",
)
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
REQUIRED_V13_ARTIFACTS = (
    "results/v1.3-external-oracle.json",
    "results/v1.3-polygon-differential-3x3.json",
    "results/v1.3-polygon-differential-4x4.json",
    "results/v1.3-polygon-backend-differential.json",
    "results/v1.3-polygon-negative.json",
    "results/v1.3-polygon-native-fixtures.json",
    "results/v1.3-output-sensitive-scaling.csv",
    "results/v1.3-output-sensitive-scaling.json",
)


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def fail(message: str) -> None:
    print(f"release consistency error: {message}", file=sys.stderr)
    raise SystemExit(1)


def check_an19_status_docs() -> None:
    required = {
        "README.md": ("P9.3.2d is a hard blocker", "10.1137/17M1115575"),
        "docs/KNOWN_LIMITATIONS.md": (
            "blocks P9.3.2d",
            "empirical counts do not close the proof",
        ),
        "docs/ALGORITHMS.md": (
            "P9.3.2d hard blocker",
            "tests do not verify the asymptotic runtime",
        ),
        "docs/EXPERIMENTS.md": (
            "247 passed and 3 existing ignored",
            "no experimental population is treated as a",
            "proof of that bound",
        ),
        "docs/TESTING.md": (
            "P9 AN19 QA boundary",
            "does not validate AN19's asymptotic runtime",
        ),
        "docs/REFERENCES.md": (
            "10.1137/17M1115575",
            "runtime chain therefore remains unverified",
        ),
        "docs/NEAR_LINEAR_FLOW_IMPLEMENTATION.md": (
            "Current P9.3.2d source blocker",
            "every dependent P9 milestone remain blocked",
        ),
        "docs/IMPLEMENTATION_MASTER_PLAN.md": (
            "P9.3.2d state: blocked. Hard blocker",
            "at least `N/2-1` distinct forward reduced costs",
            "vertex-dependent subtraction `2 d(x,v)`",
        ),
        "docs/AN19_LOCAL_EVENT_BOUND.md": (
            "3n + 4m + 2",
            "n + 2m + 2",
            "priority_queue_bound_proved`",
        ),
        "docs/phase-reports/P09-an19-static-lsst-source-map.md": (
            "Linear reduced-class lower bound and remaining proof obligation",
            "`Omega(N)` length classes",
            "unstated bounded-integer assumption is insufficient",
        ),
        "results/paper-tables.md": (
            "AN19 logarithmic reduced-class conversion",
            "Refuted",
            "AN19 exact reduced-event ordering replacement",
            "Implemented semantics / proof blocked",
            "AN19 fixed-snapshot event cardinality",
            "Proved",
        ),
    }
    contents = {}
    for relative, fragments in required.items():
        text = (ROOT / relative).read_text(encoding="utf-8")
        contents[relative] = text
        for fragment in fragments:
            if fragment not in text:
                fail(f"AN19 status documentation omits {fragment!r}: {relative}")
    forbidden = (
        "an19 runtime verified",
        "p9.3.2d state: complete",
        "p9.3.2d is complete",
        "siam paper proves the reduced-event",
        "workspace scans close the theoretical proof",
    )
    for relative, text in contents.items():
        normalized = text.lower()
        for phrase in forbidden:
            if phrase in normalized:
                fail(f"AN19 status documentation overclaims {phrase!r}: {relative}")


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
        "validation_backend: polygon::Backend::Experiment",
        "chord_backend: PolygonChordBackend::SoltanGorpinevichSweep",
        "completion_backend: PolygonCompletionBackend::IndexedFrontier",
        "cut_index_backend: polygon_cut_index::Backend::Experiment",
        "recovery_backend: PolygonRecoveryBackend::SparseSubdivision",
        "dissection_validator_backend: PolygonDissectionValidatorBackend::SparseSlab",
        "subdivision_builder_backend: polygon_sparse::subdivision::Backend::Experiment",
        "sparse_validator_backend: SparseValidatorBackend::EventSegmentTree",
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
    if "v1.3" not in paper_tables or "v1.3" not in experiments:
        fail("generated evidence does not contain a v1.3 section")
    generated_tables = set(manifest.get("generated_tables", []))
    for relative in REQUIRED_AN19_EVENT_ARTIFACTS:
        if not (ROOT / relative).is_file():
            fail(f"missing generated AN19 event evidence: {relative}")
        if relative not in generated_tables:
            fail(f"manifest omits generated AN19 event evidence: {relative}")
    an19_events = json.loads(
        (ROOT / "results/an19-event-adversarial.json").read_text(encoding="utf-8")
    )
    assert_reachable(an19_events.get("commit_sha", ""), head, "AN19 event evidence")
    expected_families = {
        "many_reduced_costs_few_source_lengths",
        "repeated_portal_splitting",
        "full_depth_persistence",
        "all_equal_reduced_keys",
        "all_distinct_reduced_keys",
        "alternating_partition_contraction",
        "highway_halving_reorder",
        "virtual_real_mixed_segments",
    }
    cases = an19_events.get("cases", [])
    if {case.get("input_family") for case in cases} != expected_families:
        fail("AN19 event evidence does not cover all adversarial families")
    if not cases or not all(case.get("oracle_agreement") is True for case in cases):
        fail("AN19 event evidence contains an Oracle disagreement")
    status = an19_events.get("runtime_status", {})
    for key in (
        "semantics_implemented",
        "exact_oracle_verified",
        "differential_verified",
        "trace_complete",
        "local_event_bound_proved",
    ):
        if status.get(key) is not True:
            fail(f"AN19 event evidence has incomplete implementation status: {key}")
    for key in (
        "global_amortization_proved",
        "priority_queue_bound_proved",
        "an19_runtime_verified",
    ):
        if status.get(key) is not False:
            fail(f"AN19 event evidence overclaims proof status: {key}")
    if an19_events.get("naive_reduced_class_conversion_survived") is not False:
        fail("AN19 adversarial evidence does not retain the reduced-class witness")
    for case in cases:
        for run_name in ("oracle_run", "reduced_run"):
            run = case.get(run_name, {})
            certificate = run.get("local_event_bound", {})
            vertices = certificate.get("vertex_count")
            edges = certificate.get("edge_count")
            if not isinstance(vertices, int) or not isinstance(edges, int):
                fail(f"AN19 {run_name} omits local-bound dimensions")
            if certificate.get("semantic_event_bound") != 3 * vertices + 4 * edges + 2:
                fail(f"AN19 {run_name} has an invalid semantic-event bound")
            if certificate.get("queue_item_bound") != vertices + 2 * edges + 2:
                fail(f"AN19 {run_name} has an invalid queue-item bound")
            semantic_count = certificate.get("semantic_event_count")
            semantic_bound = certificate.get("semantic_event_bound")
            queue_insertions = certificate.get("queue_insertion_count")
            queue_bound = certificate.get("queue_item_bound")
            if not all(
                isinstance(value, int)
                for value in (semantic_count, semantic_bound, queue_insertions, queue_bound)
            ):
                fail(f"AN19 {run_name} omits local-bound counts")
            if semantic_count > semantic_bound:
                fail(f"AN19 {run_name} exceeds its semantic-event bound")
            if queue_insertions > queue_bound:
                fail(f"AN19 {run_name} exceeds its queue-item bound")
            if certificate.get("queue_pop_count") != certificate.get("queue_insertion_count"):
                fail(f"AN19 {run_name} does not drain its exact event queue")
            if certificate.get("priority_queue_comparison_bound_included") is not False:
                fail(f"AN19 {run_name} overclaims a priority-queue comparison bound")
        if case.get("oracle_run", {}).get("practical_queue_bound") is not None:
            fail("AN19 Oracle run falsely carries a reduced-engine heap certificate")
        reduced_run = case.get("reduced_run", {})
        practical = reduced_run.get("practical_queue_bound")
        if not isinstance(practical, dict):
            fail("AN19 reduced run omits its practical heap certificate")
        insertions = practical.get("queue_insertion_count")
        edges = practical.get("edge_count")
        if not isinstance(insertions, int) or not isinstance(edges, int):
            fail("AN19 practical heap certificate omits dimensions")
        height = (max(insertions, 1) - 1).bit_length()
        push_bound = insertions * height
        pop_bound = 2 * insertions * height
        label_bound = 2 * edges
        total_bound = push_bound + pop_bound + label_bound
        observed = (
            practical.get("observed_push_comparisons", -1)
            + practical.get("observed_pop_comparisons", -1)
            + practical.get("observed_relaxation_label_comparisons", -1)
        )
        if (
            practical.get("schema_version") != 1
            or practical.get("strategy") != "stable_binary_min_heap"
            or practical.get("proof_scope") != "reduced_engine_fixed_snapshot"
            or practical.get("an19_priority_queue_target_proved") is not False
            or practical.get("queue_pop_count") != insertions
            or practical.get("heap_height_bound") != height
            or practical.get("push_comparison_bound") != push_bound
            or practical.get("pop_comparison_bound") != pop_bound
            or practical.get("relaxation_label_comparison_bound") != label_bound
            or practical.get("total_comparison_bound") != total_bound
            or practical.get("observed_push_comparisons", total_bound + 1) > push_bound
            or practical.get("observed_pop_comparisons", total_bound + 1) > pop_bound
            or practical.get("observed_relaxation_label_comparisons", total_bound + 1)
            > label_bound
            or practical.get("observed_total_comparisons") != observed
            or practical.get("observed_total_comparisons") > total_bound
            or practical.get("observed_total_comparisons")
            != reduced_run.get("metrics", {}).get("exact_comparison_count")
            or practical.get("observed_total_comparisons") != case.get("exact_comparisons")
        ):
            fail("AN19 reduced run has an invalid practical heap certificate")
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
    for relative in REQUIRED_V13_ARTIFACTS:
        if not (ROOT / relative).is_file():
            fail(f"missing generated v1.3 evidence: {relative}")
        if relative not in generated_tables:
            fail(f"manifest omits generated v1.3 evidence: {relative}")
    v13_scaling = json.loads(
        (ROOT / "results/v1.3-output-sensitive-scaling.json").read_text()
    )
    if (
        v13_scaling.get("verified_rows"),
        v13_scaling.get("solver_errors"),
        v13_scaling.get("disagreements"),
    ) != (56, 0, 0):
        fail("v1.3 scaling summary is stale")
    for row in v13_scaling.get("rows", []):
        if row.get("sweep_subdivision_candidate_pair_tests") != 0:
            fail("v1.3 sweep row performed candidate-pair traversal")
        if row.get("event_validator_boundary_edge_scans") != 0:
            fail("v1.3 event validator scanned boundary edges")
        if row.get("event_validator_active_rectangle_resorts") != 0:
            fail("v1.3 event validator resorted active rectangles")
        if not row.get("geometry_backends_equal"):
            fail("v1.3 geometry backends disagree")
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
        "polygon::experiment::Validator",
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
    check_an19_status_docs()
    check_p1_baseline()
    print(f"release consistency: {VERSION} {TAG} -> {peeled}")
    print(f"reachable manifest commits: {len(commits)}")


if __name__ == "__main__":
    main()
