#!/usr/bin/env python3
"""Reject reference-flow and exact-recovery fallback dependencies in P9.5."""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
MODULES = {
    "root": ROOT / "crates/graph/src/source_flow.rs",
    "certificate": ROOT / "crates/graph/src/source_flow/certificate.rs",
    "coordinates": ROOT / "crates/graph/src/source_flow/coordinates.rs",
    "recovery": ROOT / "crates/graph/src/source_flow/recovery.rs",
    "iteration": ROOT / "crates/graph/src/source_flow/iteration.rs",
    "compressed": ROOT / "crates/dominance/src/compressed_flow/experiment/source.rs",
}
REQUIRED = {
    "root": (
        "pub mod recovery",
        "pub mod iteration",
        "pub mod coordinates",
        "pub fn recover_terminated",
        "pub fn recover_terminated_at_most",
        "recover_augmented_terminated",
        "recover_augmented_terminated_at_most",
        "recover_lower_bounded_terminated",
        "pub fn begin_iterations",
        "pub fn begin_source_iterations",
        "pub fn begin_with_target",
        "pub struct TargetDriver",
        "TargetNotMet",
        "pub fn prove_infeasible_below",
        "CertificateInsufficient",
        "does not classify a failed run",
        "failures do not classify a target for binary search",
        "exceeds the caller-supplied target",
        "verify_feasible_solution",
        "recover_original_feasible",
        "an19_runtime_verified: false",
    ),
    "coordinates": (
        "pub fn reciprocal_slack_input",
        "snapshot.flow()",
        "snapshot.optimal_cost()",
        "network.fractional_slacks",
    ),
    "certificate": (
        "pub struct DualLowerBoundCertificate",
        "pub fn from_potentials",
        "pub fn verify",
        "pub struct InfeasibilityProof",
        "exact dual objective",
    ),
    "recovery": ("pub fn round", "validate_signed_circulation", "verify_fractional_solution"),
    "iteration": (
        "pub struct Step",
        "from_compact_candidate",
        "from_terminal_candidate",
        "from_maintained_candidates",
        "query::decode",
        "SpannerParameters::derive(&input)",
        "pub fn apply",
        "pub fn apply_source_selected",
        "pub struct SourceSelected",
        "pub struct Projection",
        "pub trait Factory",
        "pub struct FixedProjectionFactory",
        "pub struct ReciprocalSlackProjectionFactory",
        "pub const fn preparation_count",
        "pub struct Driver",
        "certify_approximations",
        "pub fn run",
        "IterationLimit",
        "StaleCertifiedSnapshot",
        "IpmDetectLedger",
        "MismatchedCandidateCoordinates",
    ),
    "compressed": (
        "pub struct Circulation",
        "pub fn recover_certified",
        "pub fn recover_source_session",
        "pub fn run_source",
        "pub fn run_with_target",
        "pub struct TargetRun",
        "pub fn certify_cover_below",
        "pub struct CoverBelowProof",
        "SourceIteration",
        "recover_terminated",
        "verify_feasible_solution",
        "interprets a source failure as evidence about a different target",
    ),
}
FORBIDDEN = (
    "Dinic",
    "PushRelabel",
    "FlowBackendKind",
    "min_cost::oracle",
    "min_cost::experiment",
    "round_fractional_costed",
    "recover_additive_half",
    "recover_isolation_perturbed",
    "dynamic_min_ratio",
    ".verify_solution(",
)
COORDINATE_FORBIDDEN = (
    "snapshot.lengths()",
    "snapshot.gradients()",
)
MODULE_FORBIDDEN = {
    "iteration": (
        "decode_candidate",
        "StableMinRatioLedger",
        "StableWitness",
    ),
}


def production_source(path: Path) -> str:
    return path.read_text(encoding="utf-8").split("\n#[cfg(test)]", maxsplit=1)[0]


def fail(message: str) -> None:
    print(f"source_flow audit error: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    for name, path in MODULES.items():
        source = production_source(path)
        for required in REQUIRED[name]:
            if required not in source:
                fail(f"missing {required!r} from {path.relative_to(ROOT)}")
        for forbidden in FORBIDDEN:
            if forbidden in source:
                fail(f"forbidden {forbidden!r} in {path.relative_to(ROOT)}")
        for forbidden in MODULE_FORBIDDEN.get(name, ()):
            if forbidden in source:
                fail(f"forbidden {forbidden!r} in {path.relative_to(ROOT)}")
        if name == "coordinates":
            for forbidden in COORDINATE_FORBIDDEN:
                if forbidden in source:
                    fail(f"forbidden interval read {forbidden!r} in {path.relative_to(ROOT)}")
    print(
        "source_flow audit: derived source configuration with no reference-flow, "
        "recovery fallback, or hidden-stability execution dependency"
    )


if __name__ == "__main__":
    main()
