#!/usr/bin/env python3
"""Check the finite source LSST production trace and no-fallback boundary."""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
MODULES = {
    "root": ROOT / "crates/graph/src/source_lsst.rs",
    "level": ROOT / "crates/graph/src/source_lsst/level.rs",
    "bucket": ROOT / "crates/graph/src/source_lsst/bucket.rs",
    "chain": ROOT / "crates/graph/src/source_lsst/chain.rs",
    "replay": ROOT / "crates/graph/src/source_lsst/replay.rs",
}
REQUIRED = {
    "root": ("SourceDynamicGraph", "apply_batch"),
    "level": ("LsfStructuralCertificate", "scaled_length"),
    "bucket": ("source_spanner", "RebuildState"),
    "chain": ("An19Lsst::construct", "tree_audit"),
    "replay": ("Chain::build", "full_snapshot_rebuilds"),
}
FORBIDDEN = (
    "source_lsf::oracle",
    "source_lsst::oracle",
    "build_greedy",
    "simple_paths",
    "decremental_spanner",
)


def production_source(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    return text.split("\n#[cfg(test)]", maxsplit=1)[0]


def fail(message: str) -> None:
    print(f"source_lsst audit error: {message}", file=sys.stderr)
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
    print("source_lsst audit: finite production trace and no-fallback boundary verified")


if __name__ == "__main__":
    main()
