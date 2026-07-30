#!/usr/bin/env python3
"""Check the P9.4a production tree-chain boundary has no Oracle fallback."""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
MODULES = {
    "root": ROOT / "crates/graph/src/source_min_ratio.rs",
    "candidate": ROOT / "crates/graph/src/source_min_ratio/candidate.rs",
    "input": ROOT / "crates/graph/src/source_min_ratio/input.rs",
    "model": ROOT / "crates/graph/src/source_min_ratio/model.rs",
    "chain": ROOT / "crates/graph/src/source_min_ratio/chain.rs",
    "cycle": ROOT / "crates/graph/src/source_min_ratio/cycle.rs",
    "spanner": ROOT / "crates/graph/src/source_min_ratio/spanner.rs",
    "terminal": ROOT / "crates/graph/src/source_min_ratio/terminal.rs",
}
REQUIRED = {
    "root": ("pub mod candidate", "pub mod input", "pub mod chain", "pub mod model", "pub mod spanner"),
    "candidate": (
        "pub struct Registry",
        "pub fn best",
        "FundamentalSpanner",
        "validate_spanner_path",
    ),
    "input": ("pub struct Input", "pub fn materialize", "ArcBindings::new"),
    "model": ("struct BranchId", "struct Tree", "struct Level"),
    "chain": ("fn validate_tree", "pub fn initial_shifts", "pub fn shift"),
    "cycle": (
        "struct ArcBindings",
        "struct EmbeddingEdge",
        "SpannerPath",
        "pub fn decode",
        "validate_signed_circulation",
    ),
    "spanner": (
        "pub struct Snapshot",
        "pub struct Transition",
        "pub fn transition",
        "FundamentalSpanner",
        "SpannerPath",
        "source_lsst",
    ),
    "terminal": ("pub struct Tree", "pub fn build", "Lsst::construct", "FundamentalTree"),
}
FORBIDDEN = (
    "use crate::dynamic_min_ratio",
    "dynamic_min_ratio::oracle",
    "minimum_ratio_cycle",
    "source_lsf::oracle",
    "source_lsst::oracle",
    "build_greedy",
    "simple_paths",
    "enumerate_cycles",
)


def production_source(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    return text.split("\n#[cfg(test)]", maxsplit=1)[0]


def fail(message: str) -> None:
    print(f"source_min_ratio audit error: {message}", file=sys.stderr)
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
    print("source_min_ratio audit: finite tree-chain and sparsified-core boundaries have no Oracle fallback")


if __name__ == "__main__":
    main()
