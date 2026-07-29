#!/usr/bin/env python3
"""Reject reference-flow and exact-recovery fallback dependencies in P9.5."""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
MODULES = {
    "root": ROOT / "crates/graph/src/source_flow.rs",
    "recovery": ROOT / "crates/graph/src/source_flow/recovery.rs",
    "iteration": ROOT / "crates/graph/src/source_flow/iteration.rs",
    "compressed": ROOT / "crates/dominance/src/compressed_flow/experiment/source.rs",
}
REQUIRED = {
    "root": (
        "pub mod recovery",
        "pub mod iteration",
        "pub fn recover_terminated",
        "recover_augmented_terminated",
        "recover_lower_bounded_terminated",
        "pub fn begin_iterations",
        "verify_feasible_solution",
        "recover_original_feasible",
        "an19_runtime_verified: false",
    ),
    "recovery": ("pub fn round", "validate_signed_circulation", "verify_fractional_solution"),
    "iteration": (
        "pub struct Step",
        "from_compact_candidate",
        "from_terminal_candidate",
        "decode_candidate",
        "pub fn apply",
        "IpmDetectLedger",
        "MismatchedTerminalCoordinates",
    ),
    "compressed": (
        "pub struct Circulation",
        "pub fn recover_certified",
        "verify_feasible_solution",
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
    print("source_flow audit: no reference-flow or recovery fallback dependency")


if __name__ == "__main__":
    main()
