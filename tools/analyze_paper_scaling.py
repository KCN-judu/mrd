#!/usr/bin/env python3
"""Analyze raw paper-scaling samples without post-hoc filtering.

The analyzer reports descriptive distributions, paired ratios, and declared
empirical fits. It never treats a timeout as an exact runtime and never uses
individual repetitions as independent size-level fit points.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import random
import statistics
from pathlib import Path
from typing import Any, Callable


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = 1
BOOTSTRAP_SEED = 20260804
DEFAULT_RESAMPLES = 10_000
ALGORITHMS = (
    "compact-mrd",
    "explicit-hopcroft-karp",
    "explicit-c0-flow",
    "exact-cover-oracle",
)
SIZE_VARIABLES: dict[str, str] = {
    "N": "foreground_cells_n",
    "B": "boundary_size_b",
    "q": "q",
    "K": "explicit_conflict_edge_count_k",
    "M": "compressed-representation-size",
}
PHASES = (
    "geometry_preprocessing_ns",
    "chord_generation_ns",
    "embedding_ns",
    "explicit_conflict_graph_ns",
    "biclique_construction_ns",
    "network_construction_ns",
    "matching_or_flow_ns",
    "vertex_cover_recovery_ns",
    "chord_selection_ns",
    "geometric_completion_ns",
    "rectangle_recovery_ns",
    "verification_ns",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=False)
    parser.add_argument(
        "--summary-json", type=Path, default=Path("results/paper-scaling-summary.json")
    )
    parser.add_argument(
        "--summary-csv", type=Path, default=Path("results/paper-scaling-summary.csv")
    )
    parser.add_argument(
        "--report", type=Path, default=Path("results/paper-scaling-report.md")
    )
    parser.add_argument(
        "--tables", type=Path, default=Path("results/paper-scaling-tables.tex")
    )
    parser.add_argument(
        "--figure-dir", type=Path, default=Path("results/paper-scaling-figures")
    )
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def root_path(path: Path) -> Path:
    return path if path.is_absolute() else ROOT / path


def relative_path(path: Path) -> str:
    return path.resolve().relative_to(ROOT).as_posix()


def scalar(value: Any) -> Any:
    if isinstance(value, (dict, list)):
        return json.dumps(value, sort_keys=True, separators=(",", ":"))
    return value


def percentile(values: list[float], proportion: float) -> float:
    ordered = sorted(values)
    if not ordered:
        raise ValueError("percentile of an empty sample")
    index = (len(ordered) - 1) * proportion
    lower = math.floor(index)
    upper = math.ceil(index)
    if lower == upper:
        return ordered[lower]
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (index - lower)


def median_absolute_deviation(values: list[float]) -> float:
    center = statistics.median(values)
    return statistics.median([abs(value - center) for value in values])


def distribution(values: list[float]) -> dict[str, float | int] | None:
    if not values:
        return None
    result: dict[str, float | int] = {
        "count": len(values),
        "min": min(values),
        "q1": percentile(values, 0.25),
        "median": statistics.median(values),
        "q3": percentile(values, 0.75),
        "max": max(values),
        "mad": median_absolute_deviation(values),
    }
    for key, value in list(result.items()):
        if isinstance(value, float) and value.is_integer():
            result[key] = int(value)
    return result


def metric_value(row: dict[str, Any], variable: str) -> float | None:
    if variable == "M":
        sizes = row.get("sizes", {}) or {}
        paired = row.get("paired_structural", {}) or {}
        nodes = sizes.get("compressed_network_node_count")
        arcs = sizes.get("compressed_network_arc_count")
        if nodes is None:
            nodes = paired.get("compressed_network_node_count")
        if arcs is None:
            arcs = paired.get("compressed_network_arc_count")
        if nodes is None or arcs is None:
            return None
        return float(nodes + arcs)
    field = SIZE_VARIABLES[variable]
    sizes = row.get("sizes", {}) or {}
    value = sizes.get(field)
    if value is None:
        value = (row.get("paired_structural", {}) or {}).get(field)
    return float(value) if value is not None else None


def usable(row: dict[str, Any]) -> bool:
    return (
        bool(row.get("measured"))
        and row.get("state") == "success"
        and row.get("correctness") == "valid"
        and isinstance(row.get("process_wall_time_ns"), (int, float))
        and row["process_wall_time_ns"] > 0
    )


def group_rows(records: list[dict[str, Any]], family: str, algorithm: str) -> list[dict[str, Any]]:
    return [
        row
        for row in records
        if row.get("family") == family and row.get("algorithm") == algorithm
    ]


def size_level_values(
    rows: list[dict[str, Any]],
    variable: str | None = None,
    value: Callable[[dict[str, Any]], float | None] | None = None,
) -> dict[int, list[float]]:
    levels: dict[int, list[float]] = {}
    for row in rows:
        if not usable(row):
            continue
        observed = value(row) if value else metric_value(row, variable or "N")
        if observed is None or observed <= 0:
            continue
        levels.setdefault(int(row["target_size"]), []).append(float(observed))
    return levels


def ols(points: list[tuple[float, float]]) -> dict[str, Any] | None:
    if len(points) < 2:
        return None
    x_values = [point[0] for point in points]
    y_values = [point[1] for point in points]
    x_mean = statistics.mean(x_values)
    y_mean = statistics.mean(y_values)
    denominator = sum((x - x_mean) ** 2 for x in x_values)
    if denominator == 0:
        return None
    slope = sum((x - x_mean) * (y - y_mean) for x, y in points) / denominator
    intercept = y_mean - slope * x_mean
    residuals = [y - (intercept + slope * x) for x, y in points]
    total = sum((y - y_mean) ** 2 for y in y_values)
    r_squared = 1.0 if total == 0 else 1 - sum(r * r for r in residuals) / total
    return {
        "alpha_ols": slope,
        "intercept": intercept,
        "r_squared": r_squared,
        "residuals": residuals,
    }


def theil_sen(points: list[tuple[float, float]]) -> float | None:
    slopes = []
    for index, (x0, y0) in enumerate(points):
        for x1, y1 in points[index + 1 :]:
            if x1 != x0:
                slopes.append((y1 - y0) / (x1 - x0))
    return statistics.median(slopes) if slopes else None


def bootstrap_slopes(
    size_values: dict[int, list[float]],
    points_builder: Callable[[dict[int, float]], list[tuple[float, float]]],
    resamples: int,
    seed: int,
) -> list[float]:
    rng = random.Random(seed)
    sizes = sorted(size_values)
    results: list[float] = []
    for _ in range(resamples):
        medians = {
            size: statistics.median(
                values[rng.randrange(len(values))] for _ in values
            )
            for size, values in size_values.items()
        }
        fit = ols(points_builder(medians))
        if fit is not None and math.isfinite(fit["alpha_ols"]):
            results.append(float(fit["alpha_ols"]))
    return results


def fit_group(
    rows: list[dict[str, Any]],
    variable: str,
    minimum_target_size: int,
    minimum_size_levels: int,
    resamples: int,
    seed: int,
) -> dict[str, Any]:
    values_by_size = size_level_values(
        rows, value=lambda row: row.get("process_wall_time_ns")
    )
    excluded: list[dict[str, Any]] = []
    points: list[tuple[float, float]] = []
    medians: dict[int, float] = {}
    field_variable = variable
    for row in rows:
        size = int(row["target_size"])
        if row.get("measured") and size < minimum_target_size:
            excluded.append({"target_size": size, "reason": "below-predeclared-fit-minimum"})
    for size in sorted(values_by_size):
        if size < minimum_target_size:
            continue
        metric_rows = [row for row in rows if int(row["target_size"]) == size and usable(row)]
        metric = [metric_value(row, variable) for row in metric_rows]
        metric = [value for value in metric if value is not None and value > 0]
        times = [float(row["process_wall_time_ns"]) for row in metric_rows]
        if not metric or not times:
            excluded.append({"target_size": size, "reason": "missing-size-measure-or-valid-time"})
            continue
        if len(metric) != len(times):
            excluded.append({"target_size": size, "reason": "paired-size-measure-incomplete"})
            continue
        medians[size] = statistics.median(metric)
        points.append((math.log(medians[size]), math.log(statistics.median(times))))
    fit = ols(points) if len(points) >= minimum_size_levels else None
    robust = theil_sen(points) if fit is not None else None
    ci: list[float] = []
    if fit is not None:
        size_values = {
            size: [
                float(row["process_wall_time_ns"])
                for row in rows
                if int(row["target_size"]) == size and usable(row)
            ]
            for size in medians
        }

        def points_builder(sampled_times: dict[int, float]) -> list[tuple[float, float]]:
            return [
                (math.log(medians[size]), math.log(sampled_times[size]))
                for size in sorted(sampled_times)
            ]

        ci = bootstrap_slopes(size_values, points_builder, resamples, seed)
    fit_min = min(medians) if medians else None
    fit_max = max(medians) if medians else None
    fit_result = {
        "independent_variable": variable,
        "independent_variable_definition": (
            "M = compressed network node count + compressed network arc count"
            if variable == "M"
            else SIZE_VARIABLES[variable]
        ),
        "algorithm": rows[0]["algorithm"] if rows else None,
        "family": rows[0]["family"] if rows else None,
        "alpha_ols": fit["alpha_ols"] if fit else None,
        "alpha_ols_ci95": [percentile(ci, 0.025), percentile(ci, 0.975)] if ci else None,
        "alpha_theil_sen": robust,
        "r_squared": fit["r_squared"] if fit else None,
        "size_level_count": len(points),
        "fit_target_size_range": [minimum_target_size, max(medians) if medians else None],
        "fit_independent_range": [fit_min, fit_max],
        "excluded_sizes": excluded,
        "residuals_log_time": fit["residuals"] if fit else [],
        "bootstrap_seed": seed,
        "bootstrap_resamples": resamples,
        "claim_status": "empirical-over-measured-range"
        if fit
        else f"insufficient-size-levels-requires-{minimum_size_levels}",
    }
    return fit_result


def bootstrap_median(values: list[float], resamples: int, seed: int) -> list[float]:
    rng = random.Random(seed)
    return [
        statistics.median(values[rng.randrange(len(values))] for _ in values)
        for _ in range(resamples)
    ]


def paired_comparison(records: list[dict[str, Any]], family: str, resamples: int) -> dict[str, Any]:
    pairs: dict[str, dict[str, dict[str, Any]]] = {}
    for row in records:
        if row.get("family") != family or not row.get("measured"):
            continue
        pairs.setdefault(row["pair_id"], {})[row["algorithm"]] = row
    ratios: list[float] = []
    by_size: dict[int, list[float]] = {}
    for rows in pairs.values():
        compact = rows.get("compact-mrd")
        explicit = rows.get("explicit-hopcroft-karp")
        if not compact or not explicit or not usable(compact) or not usable(explicit):
            continue
        ratio = compact["process_wall_time_ns"] / explicit["process_wall_time_ns"]
        if ratio > 0 and math.isfinite(ratio):
            ratios.append(ratio)
            by_size.setdefault(int(compact["target_size"]), []).append(ratio)
    bootstrap = bootstrap_median(ratios, resamples, BOOTSTRAP_SEED) if ratios else []
    size_medians = {
        size: statistics.median(values) for size, values in sorted(by_size.items())
    }
    crossover = None
    if len(size_medians) >= 6:
        for size in sorted(size_medians):
            later = [value for other, value in size_medians.items() if other >= size]
            if len(later) >= 2 and all(value < 1 for value in later):
                crossover = size
                break
    return {
        "family": family,
        "comparison": "compact-mrd / explicit-hopcroft-karp process wall time",
        "paired_count": len(ratios),
        "ratio_distribution": distribution(ratios),
        "median_ratio": statistics.median(ratios) if ratios else None,
        "geometric_mean_ratio": math.exp(statistics.mean(math.log(value) for value in ratios))
        if ratios
        else None,
        "bootstrap_ci95": [percentile(bootstrap, 0.025), percentile(bootstrap, 0.975)]
        if bootstrap
        else None,
        "bootstrap_seed": BOOTSTRAP_SEED,
        "bootstrap_resamples": resamples,
        "median_ratio_by_target_size": size_medians,
        "stable_crossover_target_size": crossover,
    }


def coverage_summary(records: list[dict[str, Any]], family: str) -> dict[str, Any]:
    rows = [row for row in records if row.get("family") == family and row.get("measured")]
    sizes = sorted({int(row["target_size"]) for row in rows})
    pair_ids = {row["pair_id"] for row in rows}
    mismatch_count = sum(
        row.get("correctness", "").startswith("invalid-") for row in rows
    )
    return {
        "family": family,
        "target_size_range": [min(sizes), max(sizes)] if sizes else None,
        "instance_count": len(pair_ids),
        "successful_paired_comparisons": paired_comparison(records, family, 10_000)[
            "paired_count"
        ],
        "mismatches": mismatch_count,
        "timeouts": sum(row.get("state") == "timeout" for row in rows),
        "unsupported": sum(row.get("state") == "unsupported" for row in rows),
        "errors": sum(row.get("state") == "error" for row in rows),
    }


def phase_rows(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    output = []
    for family in sorted({row["family"] for row in records}):
        rows = [row for row in records if row.get("family") == family and usable(row)]
        sizes = sorted({int(row["target_size"]) for row in rows})
        if not sizes:
            continue
        selected = [sizes[0], sizes[len(sizes) // 2], sizes[-1]]
        for size in dict.fromkeys(selected):
            for algorithm in ("compact-mrd", "explicit-hopcroft-karp"):
                candidates = [
                    row
                    for row in rows
                    if row.get("algorithm") == algorithm
                    and int(row["target_size"]) == size
                ]
                if not candidates:
                    continue
                phases = candidates[0].get("timings", {}) or {}
                output.append(
                    {
                        "family": family,
                        "target_size": size,
                        "algorithm": algorithm,
                        "phase_medians_ns": {
                            phase: statistics.median(
                                float(row.get("timings", {}).get(phase) or 0)
                                for row in candidates
                            )
                            for phase in PHASES
                            if phases.get(phase) is not None
                        },
                    }
                )
    return output


def timing_size_levels(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    levels: list[dict[str, Any]] = []
    for size in sorted({int(row["target_size"]) for row in rows if usable(row)}):
        valid = [row for row in rows if int(row["target_size"]) == size and usable(row)]
        if not valid:
            continue
        level: dict[str, Any] = {
            "target_size": size,
            "process_wall_time_ns": distribution(
                [float(row["process_wall_time_ns"]) for row in valid]
            ),
        }
        for variable in ("N", "B", "q", "K", "M"):
            values = [metric_value(row, variable) for row in valid]
            values = [value for value in values if value is not None and value > 0]
            level[variable] = statistics.median(values) if values else None
        levels.append(level)
    return levels


def summarize(raw: dict[str, Any]) -> dict[str, Any]:
    records = raw.get("records", [])
    protocol = raw.get("protocol", {})
    fit_config = protocol.get("fit", {})
    minimum_target_size = int(fit_config.get("minimum_target_size", 1))
    minimum_size_levels = int(fit_config.get("minimum_size_levels", 6))
    resamples = max(DEFAULT_RESAMPLES, int(fit_config.get("bootstrap_resamples", 10_000)))
    families = protocol.get("families") or sorted({row["family"] for row in records})
    algorithms = protocol.get("algorithms") or list(ALGORITHMS)
    distributions = []
    fits = []
    for family in families:
        for algorithm in algorithms:
            rows = group_rows(records, family, algorithm)
            values = [
                float(row["process_wall_time_ns"])
                for row in rows
                if usable(row)
            ]
            distributions.append(
                {
                    "family": family,
                    "algorithm": algorithm,
                    "sample_count": len(values),
                    "distribution_ns": distribution(values),
                    "timeout_count": sum(
                        row.get("measured") and row.get("state") == "timeout" for row in rows
                    ),
                    "failure_count": sum(
                        row.get("measured") and row.get("state") == "error" for row in rows
                    ),
                    "unsupported_count": sum(
                        row.get("measured") and row.get("state") == "unsupported" for row in rows
                    ),
                    "invalid_count": sum(
                        row.get("measured") and str(row.get("correctness", "")).startswith("invalid-")
                        for row in rows
                    ),
                    "size_levels": timing_size_levels(rows),
                }
            )
            for variable in SIZE_VARIABLES:
                fits.append(
                    fit_group(
                        rows,
                        variable,
                        minimum_target_size,
                        minimum_size_levels,
                        resamples,
                        BOOTSTRAP_SEED,
                    )
                )
    paired = [paired_comparison(records, family, resamples) for family in families]
    coverage = [coverage_summary(records, family) for family in families]
    return {
        "schema_version": SCHEMA_VERSION,
        "campaign": "paper-scaling",
        "source_git_commit": raw.get("environment", {}).get("git_commit"),
        "protocol": {
            "fit_minimum_target_size": minimum_target_size,
            "fit_minimum_size_levels": minimum_size_levels,
            "fit_exclusion_rule": "target_size < fit.minimum_target_size; missing or invalid values are excluded",
            "fit_time_variable": "process_wall_time_ns",
            "timeout_treatment": "censored and retained; excluded from exact-time fits",
            "bootstrap_seed": BOOTSTRAP_SEED,
            "bootstrap_resamples": resamples,
        },
        "coverage": coverage,
        "distributions": distributions,
        "paired_comparisons": paired,
        "fits": fits,
        "phase_decomposition": phase_rows(records),
        "claim_boundary": "Empirical fits describe the measured population and are not asymptotic runtime proofs.",
    }


def markdown_table(fields: list[str], rows: list[dict[str, Any]]) -> str:
    def cell(value: Any) -> str:
        return str(value).replace("|", "\\|").replace("\n", " ")

    lines = [
        "| " + " | ".join(fields) + " |",
        "| " + " | ".join("---" for _ in fields) + " |",
    ]
    lines.extend("| " + " | ".join(cell(row.get(field, "")) for field in fields) + " |" for row in rows)
    return "\n".join(lines)


def format_number(value: Any) -> str:
    if value is None:
        return "NA"
    if isinstance(value, float):
        return f"{value:.3g}"
    return f"{value:,}" if isinstance(value, int) else str(value)


def report_markdown(summary: dict[str, Any]) -> str:
    lines = [
        "# Paper Scaling Benchmark Report",
        "",
        "This report is generated from `results/paper-scaling.json` by the committed analyzer. It reports finite local measurements, not an asymptotic runtime theorem.",
        "",
        "## Protocol",
        "",
        f"- Fit time variable: `{summary['protocol']['fit_time_variable']}`.",
        f"- Predeclared fit exclusion: `{summary['protocol']['fit_exclusion_rule']}`.",
        f"- Timeout policy: {summary['protocol']['timeout_treatment']}.",
        f"- Bootstrap: seed `{summary['protocol']['bootstrap_seed']}`, `{summary['protocol']['bootstrap_resamples']:,}` resamples.",
        f"- A slope is emitted only after `{summary['protocol']['fit_minimum_size_levels']}` valid size levels satisfy the predeclared rule.",
        "- `M` is the compressed network node count plus compressed network arc count; `K` is the explicit conflict-edge count.",
        "",
        "## Coverage",
        "",
    ]
    coverage_rows = []
    for row in summary["coverage"]:
        coverage_rows.append(
            {
                "Family": row["family"],
                "Size range": row["target_size_range"],
                "Instances": row["instance_count"],
                "Paired": row["successful_paired_comparisons"],
                "Mismatches": row["mismatches"],
                "Timeouts": row["timeouts"],
                "Unsupported": row["unsupported"],
            }
        )
    lines.append(markdown_table(list(coverage_rows[0]) if coverage_rows else [], coverage_rows))
    lines.extend(["", "## Paired timing ratios", ""])
    ratio_rows = []
    for row in summary["paired_comparisons"]:
        ratio_rows.append(
            {
                "Family": row["family"],
                "Paired": row["paired_count"],
                "Median compact/explicit": format_number(row["median_ratio"]),
                "Bootstrap 95% CI": row["bootstrap_ci95"],
                "Stable crossover target": row["stable_crossover_target_size"] or "none",
            }
        )
    lines.append(markdown_table(list(ratio_rows[0]) if ratio_rows else [], ratio_rows))
    lines.extend(["", "## Empirical scaling fits", ""])
    fit_rows = []
    fit_row_count = 0
    for row in summary["fits"]:
        if row["alpha_ols"] is None:
            continue
        fit_row_count += 1
        fit_rows.append(
            {
                "Family": row["family"],
                "Algorithm": row["algorithm"],
                "Variable": row["independent_variable"],
                "alpha (OLS)": format_number(row["alpha_ols"]),
                "95% CI": row["alpha_ols_ci95"],
                "Theil-Sen": format_number(row["alpha_theil_sen"]),
                "R2": format_number(row["r_squared"]),
                "Sizes": row["size_level_count"],
                "Fit range": row["fit_target_size_range"],
            }
        )
    if fit_rows:
        lines.append(markdown_table(list(fit_rows[0]), fit_rows))
    else:
        lines.append(
            "No empirical exponent is reported: this run does not meet the predeclared six-size-level minimum."
        )
    lines.extend(
        [
            "",
            "## Phase decomposition",
            "",
            "The phase rows expose geometry, representation, flow/matching, recovery, and verification costs. Missing phases are not zero-cost claims; they are not applicable to that solver path.",
            "",
            "## Interpretation boundary",
            "",
            "A fitted slope is an empirical exponent over the declared fit interval and independent variable. It is not the exponent of the algorithm and cannot establish the unproved AN19/source-flow runtime claim. Exact-cover rows are a separate correctness Oracle category.",
            "",
        ]
    )
    return "\n".join(lines)


def latex_tables(summary: dict[str, Any]) -> str:
    def latex(value: Any) -> str:
        return str(value).replace("_", "\\_").replace("%", "\\%")

    lines = [
        "% Generated by tools/analyze_paper_scaling.py",
        "% Requires booktabs; no vertical rules are used.",
        "\\begin{table}[t]",
        "\\caption{Correctness and coverage of the paired paper-scaling population.}",
        "\\label{tab:paper-scaling-coverage}",
        "\\begin{tabular}{lrrrrrr}",
        "\\toprule",
        "Family & Instances & Paired & Mismatches & Timeouts & Unsupported & Errors \\\\",
        "\\midrule",
    ]
    for row in summary["coverage"]:
        lines.append(
            f"{latex(row['family'])} & {row['instance_count']} & {row['successful_paired_comparisons']} & {row['mismatches']} & {row['timeouts']} & {row['unsupported']} & {row['errors']} \\\\"  # noqa: E501
        )
    lines += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]
    ratio_by_family = {
        row["family"]: row["median_ratio_by_target_size"]
        for row in summary["paired_comparisons"]
    }
    timing_rows = []
    for family in sorted({row["family"] for row in summary["distributions"]}):
        compact = next(
            (
                row
                for row in summary["distributions"]
                if row["family"] == family and row["algorithm"] == "compact-mrd"
            ),
            None,
        )
        explicit = next(
            (
                row
                for row in summary["distributions"]
                if row["family"] == family and row["algorithm"] == "explicit-hopcroft-karp"
            ),
            None,
        )
        if compact is None or explicit is None:
            continue
        compact_levels = {row["target_size"]: row for row in compact["size_levels"]}
        explicit_levels = {row["target_size"]: row for row in explicit["size_levels"]}
        sizes = sorted(set(compact_levels) & set(explicit_levels))
        for size in dict.fromkeys([sizes[0], sizes[len(sizes) // 2], sizes[-1]]) if sizes else []:
            level = compact_levels[size]
            timing_rows.append((family, size, level, explicit_levels[size]))
    lines += [
        "\\begin{table}[t]",
        "\\caption{End-to-end process-wall timing on representative paired sizes. Ratios below one favor the compact path.}",
        "\\label{tab:paper-scaling-timing}",
        "\\begin{tabular}{lrrrrrr}",
        "\\toprule",
        "Family & Size & Compact (ns) & Explicit (ns) & Ratio & $q$ & $K$/$M$ \\\\",
        "\\midrule",
    ]
    for family, size, compact, explicit in timing_rows:
        ratio = ratio_by_family.get(family, {}).get(str(size))
        if ratio is None:
            ratio = ratio_by_family.get(family, {}).get(size)
        compact_time = compact["process_wall_time_ns"]["median"]
        explicit_time = explicit["process_wall_time_ns"]["median"]
        lines.append(
            f"{latex(family)} & {size} & {compact_time:.3g} & {explicit_time:.3g} & {latex(format_number(ratio))} & {latex(format_number(compact.get('q')))} & {latex(format_number(compact.get('K')))}/{latex(format_number(compact.get('M')))} \\\\"  # noqa: E501
        )
    lines += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]
    lines += [
        "\\begin{table}[t]",
        "\\caption{Empirical log--log slopes over the predeclared measured range.}",
        "\\label{tab:paper-scaling-exponents}",
        "\\begin{tabular}{lllrrrr}",
        "\\toprule",
        "Family & Algorithm & Variable & $\\alpha$ & 95\\% CI & $R^2$ & Sizes \\\\",
        "\\midrule",
    ]
    fit_row_count = 0
    for row in summary["fits"]:
        if row["alpha_ols"] is None:
            continue
        fit_row_count += 1
        ci = row["alpha_ols_ci95"]
        ci_value = "NA" if ci is None else f"[{ci[0]:.3g}, {ci[1]:.3g}]"
        lines.append(
            f"{latex(row['family'])} & {latex(row['algorithm'])} & {row['independent_variable']} & {row['alpha_ols']:.3g} & {ci_value} & {row['r_squared']:.3g} & {row['size_level_count']} \\\\"  # noqa: E501
        )
    if fit_row_count == 0:
        lines.append(
            "\\multicolumn{7}{l}{No estimate: the run did not meet the predeclared six-size-level minimum.} \\\\"
        )
    lines += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]
    lines += [
        "\\begin{table}[t]",
        "\\caption{Phase decomposition at representative points. Values are median nanoseconds; blank cells are inapplicable.}",
        "\\label{tab:paper-scaling-phases}",
        "\\begin{tabular}{llrrrrr}",
        "\\toprule",
        "Family & Solver/size & Geometry & Representation & Flow/matching & Recovery & Verification \\\\",
        "\\midrule",
    ]
    for row in summary["phase_decomposition"]:
        phases = row["phase_medians_ns"]
        geometry = phases.get("geometry_preprocessing_ns", 0) + phases.get("chord_generation_ns", 0)
        representation = sum(
            phases.get(key, 0)
            for key in (
                "embedding_ns",
                "explicit_conflict_graph_ns",
                "biclique_construction_ns",
                "network_construction_ns",
            )
        )
        flow = phases.get("matching_or_flow_ns", 0) + phases.get("vertex_cover_recovery_ns", 0)
        recovery = sum(
            phases.get(key, 0)
            for key in ("chord_selection_ns", "geometric_completion_ns", "rectangle_recovery_ns")
        )
        validation = phases.get("verification_ns", 0)
        lines.append(
            f"{latex(row['family'])} & {latex(row['algorithm'])}/{row['target_size']} & {geometry:.3g} & {representation:.3g} & {flow:.3g} & {recovery:.3g} & {validation:.3g} \\\\"  # noqa: E501
        )
    lines += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]
    return "\n".join(lines)


def svg_chart(
    path: Path,
    title: str,
    series: list[tuple[str, list[tuple[float, float]]]],
    x_label: str,
    y_label: str,
    bands: dict[str, list[tuple[float, float, float]]] | None = None,
    hline: float | None = None,
) -> None:
    width, height = 900, 540
    margin = (85, 35, 65, 65)
    all_points = [point for _, points in series for point in points]
    if not all_points:
        all_points = [(1, 1), (2, 2)]
    min_x = min(point[0] for point in all_points)
    max_x = max(point[0] for point in all_points)
    min_y = min(point[1] for point in all_points)
    max_y = max(point[1] for point in all_points)
    if min_x == max_x:
        max_x += 1
    if min_y == max_y:
        max_y += 1

    def point(x: float, y: float) -> tuple[float, float]:
        px = margin[0] + (x - min_x) / (max_x - min_x) * (width - margin[0] - margin[2])
        py = height - margin[3] - (y - min_y) / (max_y - min_y) * (height - margin[1] - margin[3])
        return px, py

    colors = ["#0f766e", "#b45309", "#334155", "#9f1239", "#4f46e5"]
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
        '<rect width="100%" height="100%" fill="white"/>',
        f'<text x="{width / 2:.1f}" y="28" text-anchor="middle" font-family="sans-serif" font-size="18" fill="#111827">{title}</text>',
        f'<line x1="{margin[0]}" y1="{height - margin[3]}" x2="{width - margin[2]}" y2="{height - margin[3]}" stroke="#334155"/>',
        f'<line x1="{margin[0]}" y1="{margin[1]}" x2="{margin[0]}" y2="{height - margin[3]}" stroke="#334155"/>',
        f'<text x="{width / 2:.1f}" y="{height - 15}" text-anchor="middle" font-family="sans-serif" font-size="13" fill="#334155">{x_label}</text>',
        f'<text x="18" y="{height / 2:.1f}" text-anchor="middle" transform="rotate(-90 18 {height / 2:.1f})" font-family="sans-serif" font-size="13" fill="#334155">{y_label}</text>',
    ]
    if hline is not None and min_y <= hline <= max_y:
        _, hline_y = point(min_x, hline)
        parts.append(
            f'<line x1="{margin[0]}" y1="{hline_y:.2f}" x2="{width - margin[2]}" y2="{hline_y:.2f}" stroke="#64748b" stroke-dasharray="6 4"/>'
        )
    for index, (label, points) in enumerate(series):
        color = colors[index % len(colors)]
        for x, low, high in (bands or {}).get(label, []):
            px, low_y = point(x, low)
            _, high_y = point(x, high)
            parts.append(
                f'<line x1="{px:.2f}" y1="{low_y:.2f}" x2="{px:.2f}" y2="{high_y:.2f}" stroke="{color}" stroke-width="1" opacity="0.55"/>'
            )
        coords = [point(x, y) for x, y in points]
        if coords:
            path_data = " ".join(
                ("M" if point_index == 0 else "L") + f" {px:.2f},{py:.2f}"
                for point_index, (px, py) in enumerate(coords)
            )
            parts.append(f'<path d="{path_data}" fill="none" stroke="{color}" stroke-width="2"/>')
            parts.extend(
                f'<circle cx="{px:.2f}" cy="{py:.2f}" r="3.5" fill="{color}"/>'
                for px, py in coords
            )
        parts.append(
            f'<text x="{width - margin[2] + 8}" y="{margin[1] + 18 * index + 5}" font-family="sans-serif" font-size="12" fill="{color}">{label}</text>'
        )
    parts.append("</svg>")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(parts) + "\n")


def figures(summary: dict[str, Any], output_dir: Path) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    # Required named figures are deterministic summaries even for an empty or
    # censored family; no plotting dependency is needed in the release tree.
    loglog_series = []
    loglog_bands: dict[str, list[tuple[float, float, float]]] = {}
    for family in sorted({row["family"] for row in summary["distributions"]}):
        for algorithm in ("compact-mrd", "explicit-hopcroft-karp"):
            rows = [
                row
                for row in summary["distributions"]
                if row["family"] == family and row["algorithm"] == algorithm
            ]
            if not rows:
                continue
            points = [
                (math.log(level["q"]), math.log(level["process_wall_time_ns"]["median"]))
                for level in rows[0]["size_levels"]
                if level.get("q") and level.get("process_wall_time_ns")
            ]
            if points:
                label = f"{family}:{algorithm}"
                loglog_series.append((label, points))
                loglog_bands[label] = [
                    (
                        math.log(level["q"]),
                        math.log(level["process_wall_time_ns"]["q1"]),
                        math.log(level["process_wall_time_ns"]["q3"]),
                    )
                    for level in rows[0]["size_levels"]
                    if level.get("q") and level.get("process_wall_time_ns")
                ]
    svg_chart(
        output_dir / "paper-scaling-loglog.svg",
        "Log-log process timing versus q",
        loglog_series,
        "log q",
        "log process wall time (ns)",
        loglog_bands,
    )
    ratio_series = []
    for row in summary["paired_comparisons"]:
        points = [
            (float(size), float(value))
            for size, value in row["median_ratio_by_target_size"].items()
        ]
        if points:
            ratio_series.append((row["family"], points))
    svg_chart(
        output_dir / "paper-scaling-ratio.svg",
        "Compact / explicit paired ratio",
        ratio_series,
        "target size",
        "ratio",
        hline=1.0,
    )
    k_m_series = []
    for family in sorted({row["family"] for row in summary["distributions"]}):
        rows = [
            row
            for row in summary["distributions"]
            if row["family"] == family and row["algorithm"] == "compact-mrd"
        ]
        if rows:
            points = [
                (float(level["K"]), float(level["M"]))
                for level in rows[0]["size_levels"]
                if level.get("K") and level.get("M")
            ]
            if points:
                k_m_series.append((family, points))
    svg_chart(
        output_dir / "paper-scaling-k-vs-m.svg",
        "Explicit K and compressed M",
        k_m_series,
        "explicit conflict edges K",
        "compressed representation size M",
    )
    phase_series = []
    for row in summary["phase_decomposition"][:8]:
        total = sum(row["phase_medians_ns"].values()) or 1
        phase_series.append((f"{row['family']}:{row['algorithm']}:{row['target_size']}", [(1, total)]))
    svg_chart(
        output_dir / "paper-scaling-phases.svg",
        "Representative phase totals",
        phase_series,
        "representative point",
        "phase time (ns)",
    )


def summary_csv(path: Path, summary: dict[str, Any]) -> None:
    rows: list[dict[str, Any]] = []
    for row in summary["distributions"]:
        rows.append({"record_type": "distribution", **row})
    for row in summary["paired_comparisons"]:
        rows.append({"record_type": "paired", **row})
    for row in summary["fits"]:
        rows.append({"record_type": "fit", **row})
    fields = sorted({key for row in rows for key in row}) or ["record_type"]
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, extrasaction="ignore", lineterminator="\n")
        writer.writeheader()
        writer.writerows({key: scalar(value) for key, value in row.items()} for row in rows)


def main() -> int:
    arguments = parse_args()
    if arguments.self_test:
        self_test()
        print("paper-scaling analyzer self-test: ok")
        return 0
    input_path = root_path(arguments.input or Path("results/paper-scaling.json"))
    raw = json.loads(input_path.read_text())
    summary = summarize(raw)
    summary_path = root_path(arguments.summary_json)
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    summary_csv(root_path(arguments.summary_csv), summary)
    root_path(arguments.report).write_text(report_markdown(summary))
    root_path(arguments.tables).write_text(latex_tables(summary))
    figures(summary, root_path(arguments.figure_dir))
    print(json.dumps({"families": len(summary["coverage"]), "fits": len(summary["fits"]), "source": relative_path(input_path)}, sort_keys=True))
    return 0


def self_test() -> None:
    values = [1.0, 2.0, 4.0]
    assert distribution(values)["median"] == 2.0
    points = [(math.log(1), math.log(2)), (math.log(2), math.log(4)), (math.log(4), math.log(8))]
    assert abs(ols(points)["alpha_ols"] - 1.0) < 1e-12
    assert abs(theil_sen(points) - 1.0) < 1e-12
    assert bootstrap_median(values, 100, BOOTSTRAP_SEED) == bootstrap_median(
        values, 100, BOOTSTRAP_SEED
    )
    rows = [
        {"measured": True, "state": "success", "correctness": "valid", "process_wall_time_ns": 1, "target_size": 1, "sizes": {"foreground_cells_n": 1}, "paired_structural": {}},
        {"measured": True, "state": "success", "correctness": "valid", "process_wall_time_ns": 2, "target_size": 2, "sizes": {"foreground_cells_n": 2}, "paired_structural": {}},
    ]
    assert len(size_level_values(rows, value=lambda row: row["process_wall_time_ns"])) == 2
    summary = {
        "coverage": [],
        "distributions": [],
        "paired_comparisons": [],
        "fits": [],
        "phase_decomposition": [],
    }
    assert "\\toprule" in latex_tables(summary)


if __name__ == "__main__":
    raise SystemExit(main())
