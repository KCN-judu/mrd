#!/usr/bin/env python3
"""Analyze raw paper-kernel-scaling observations without dropping outliers."""

from __future__ import annotations

import argparse
import csv
import html
import io
import json
import math
import random
import statistics
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import Path
from typing import Any, Callable


ROOT = Path(__file__).resolve().parents[1]
SCOPES = ("solve-from-canonical-instance", "representation-and-solver-kernel")
ALGORITHMS = ("compact-mrd", "explicit-hopcroft-karp", "explicit-c0-flow")
EXPLICIT = ("explicit-hopcroft-karp", "explicit-c0-flow")
VARIABLES = {
    "N": "foreground_cells_n",
    "B": "boundary_size_b",
    "q": "q",
    "K": "explicit_conflict_edge_count_k",
    "M": "compressed_size_m",
}
PHASES = (
    "geometry_preprocessing_ns",
    "chord_generation_ns",
    "embedding_ns",
    "explicit_conflict_construction_ns",
    "biclique_construction_ns",
    "explicit_network_construction_ns",
    "compressed_network_construction_ns",
    "matching_ns",
    "max_flow_ns",
    "vertex_cover_recovery_ns",
    "chord_selection_ns",
    "rectangle_completion_recovery_ns",
    "verification_ns",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--summary-json", type=Path, required=True)
    parser.add_argument("--summary-csv", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--tables", type=Path, required=True)
    parser.add_argument("--figure-dir", type=Path, required=True)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def root_path(path: Path) -> Path:
    return path if path.is_absolute() else ROOT / path


def quantile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    location = (len(ordered) - 1) * fraction
    lower = math.floor(location)
    upper = math.ceil(location)
    if lower == upper:
        return ordered[lower]
    weight = location - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight


def distribution(values: list[float]) -> dict[str, Any]:
    median = statistics.median(values)
    mad = statistics.median(abs(value - median) for value in values)
    mean = statistics.fmean(values)
    return {
        "n": len(values),
        "min": min(values),
        "q1": quantile(values, 0.25),
        "median": median,
        "q3": quantile(values, 0.75),
        "max": max(values),
        "mad": mad,
        "coefficient_of_variation": statistics.pstdev(values) / mean if mean else 0.0,
    }


def bootstrap_statistic(values: list[float], statistic: Callable[[list[float]], float], count: int, seed: int) -> list[float]:
    generator = random.Random(seed)
    return [
        statistic([values[generator.randrange(len(values))] for _ in values])
        for _ in range(count)
    ]


def bootstrap_ci(values: list[float], statistic: Callable[[list[float]], float], count: int, seed: int) -> list[float]:
    samples = bootstrap_statistic(values, statistic, count, seed)
    return [quantile(samples, 0.025), quantile(samples, 0.975)]


def geometric_mean(values: list[float]) -> float:
    return math.exp(statistics.fmean(math.log(value) for value in values))


def ols(points: list[tuple[float, float]]) -> dict[str, float]:
    x_mean = statistics.fmean(x for x, _ in points)
    y_mean = statistics.fmean(y for _, y in points)
    denominator = sum((x - x_mean) ** 2 for x, _ in points)
    slope = sum((x - x_mean) * (y - y_mean) for x, y in points) / denominator
    intercept = y_mean - slope * x_mean
    residual = sum((y - (intercept + slope * x)) ** 2 for x, y in points)
    total = sum((y - y_mean) ** 2 for _, y in points)
    return {"slope": slope, "intercept": intercept, "r_squared": 1 - residual / total if total else 1.0}


def theil_sen(points: list[tuple[float, float]]) -> float:
    slopes = [
        (right_y - left_y) / (right_x - left_x)
        for index, (left_x, left_y) in enumerate(points)
        for right_x, right_y in points[index + 1 :]
        if right_x != left_x
    ]
    return statistics.median(slopes)


def bootstrap_slope(points: list[tuple[float, float]], count: int, seed: int) -> list[float] | None:
    generator = random.Random(seed)
    slopes: list[float] = []
    for _ in range(count):
        sampled = [points[generator.randrange(len(points))] for _ in points]
        if len({x for x, _ in sampled}) < 2:
            continue
        slopes.append(ols(sampled)["slope"])
    return [quantile(slopes, 0.025), quantile(slopes, 0.975)] if slopes else None


def point_sizes(point: dict[str, Any]) -> dict[str, Any]:
    sizes = dict(point.get("sizes", {}))
    structure = point.get("structure", {})
    nodes = structure.get("compact_node_count")
    arcs = structure.get("compact_arc_count")
    sizes["compressed_size_m"] = nodes + arcs if nodes is not None and arcs is not None else None
    return sizes


def flatten(raw: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for point in raw.get("point_results", []):
        sizes = point_sizes(point)
        warmups = {
            (row["scope"], row["algorithm"]): row
            for row in point.get("warmups", [])
        }
        for run in point.get("runs", []):
            row = {
                **run,
                "family": point["family"],
                "target_size": point["target_size"],
                "point_state": point["state"],
                "sizes": sizes,
                "structure": point.get("structure", {}),
                "warmup": warmups.get((run["scope"], run["algorithm"]), {}),
            }
            rows.append(row)
    return rows


def distribution_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    groups: dict[tuple[str, int, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        groups[(row["family"], row["target_size"], row["scope"], row["algorithm"])].append(row)
    result = []
    for (family, size, scope, algorithm), group in sorted(groups.items()):
        result.append(
            {
                "family": family,
                "target_size": size,
                "scope": scope,
                "algorithm": algorithm,
                "elapsed_ns": distribution([float(row["elapsed_ns"]) for row in group]),
                "warmup_count": group[0].get("warmup", {}).get("count"),
                "warmup_converged": group[0].get("warmup", {}).get("converged"),
                "state": group[0]["point_state"],
                "sizes": group[0]["sizes"],
            }
        )
    return result


def paired_rows(rows: list[dict[str, Any]], config: dict[str, Any]) -> list[dict[str, Any]]:
    indexed = {
        (row["family"], row["target_size"], row["scope"], row["algorithm"], row["iteration"]): row
        for row in rows
    }
    bootstrap_count = int(config["fit_rule"]["bootstrap_resamples"])
    seed = int(config["fit_rule"]["bootstrap_seed"])
    result = []
    families = sorted({row["family"] for row in rows})
    sizes = sorted({row["target_size"] for row in rows})
    for family in families:
        for size in sizes:
            for scope in SCOPES:
                for explicit in EXPLICIT:
                    ratios = []
                    iterations = sorted(
                        row["iteration"]
                        for row in rows
                        if row["family"] == family
                        and row["target_size"] == size
                        and row["scope"] == scope
                        and row["algorithm"] == "compact-mrd"
                    )
                    for iteration in iterations:
                        compact = indexed.get((family, size, scope, "compact-mrd", iteration))
                        reference = indexed.get((family, size, scope, explicit, iteration))
                        if compact is not None and reference is not None:
                            ratios.append(float(compact["elapsed_ns"]) / float(reference["elapsed_ns"]))
                    if not ratios:
                        continue
                    result.append(
                        {
                            "family": family,
                            "target_size": size,
                            "scope": scope,
                            "explicit_algorithm": explicit,
                            "ratios": distribution(ratios),
                            "geometric_mean_ratio": geometric_mean(ratios),
                            "median_ratio_ci95": bootstrap_ci(ratios, statistics.median, bootstrap_count, seed ^ size),
                            "q": indexed[(family, size, scope, "compact-mrd", iterations[0])]["sizes"].get("q"),
                        }
                    )
    return result


def classifications(paired: list[dict[str, Any]], config: dict[str, Any]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in paired:
        grouped[(row["family"], row["scope"], row["explicit_algorithm"])].append(row)
    required = int(config["fit_rule"]["minimum_valid_size_levels"])
    result = []
    for (family, scope, explicit), levels in sorted(grouped.items()):
        levels.sort(key=lambda row: row["target_size"])
        all_ratios = [value for level in levels for value in [level["ratios"]["median"]]]
        aggregate_ci = bootstrap_ci(
            all_ratios,
            statistics.median,
            int(config["fit_rule"]["bootstrap_resamples"]),
            int(config["fit_rule"]["bootstrap_seed"]),
        )
        if len(levels) < required:
            classification = "insufficient"
        elif aggregate_ci[1] < 1:
            classification = "compact-clearly-faster"
        elif aggregate_ci[0] > 1:
            classification = "compact-clearly-slower"
        else:
            classification = "unresolved"
        crossover = None
        for index in range(len(levels) - 2):
            suffix = levels[index:]
            first_three = suffix[:3]
            if all(level["ratios"]["median"] < 1 for level in first_three) and sum(
                level["median_ratio_ci95"][1] < 1 for level in first_three
            ) >= 2:
                crossover = first_three[0]["target_size"]
                break
        result.append(
            {
                "family": family,
                "scope": scope,
                "explicit_algorithm": explicit,
                "valid_size_levels": len(levels),
                "classification": classification,
                "aggregate_median_ratio": statistics.median(all_ratios),
                "aggregate_median_ratio_ci95": aggregate_ci,
                "stable_crossover_target_size": crossover,
            }
        )
    return result


def fit_rows(distributions: list[dict[str, Any]], config: dict[str, Any]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in distributions:
        grouped[(row["family"], row["scope"], row["algorithm"])].append(row)
    minimum = int(config["fit_rule"]["minimum_valid_size_levels"])
    count = int(config["fit_rule"]["bootstrap_resamples"])
    seed = int(config["fit_rule"]["bootstrap_seed"])
    result = []
    for (family, scope, algorithm), levels in sorted(grouped.items()):
        for variable, field in VARIABLES.items():
            valid = [
                level
                for level in levels
                if level["sizes"].get(field) not in (None, 0) and level["elapsed_ns"]["median"] > 0
            ]
            valid.sort(key=lambda row: row["target_size"])
            base = {
                "family": family,
                "scope": scope,
                "algorithm": algorithm,
                "independent_variable": variable,
                "valid_size_levels": len(valid),
                "fit_target_sizes": [level["target_size"] for level in valid],
                "excluded_target_sizes": sorted(set(level["target_size"] for level in levels) - set(level["target_size"] for level in valid)),
            }
            if len(valid) < minimum or len({level["sizes"][field] for level in valid}) < 2:
                result.append({**base, "ols_slope": None, "ols_slope_ci95": None, "r_squared": None, "theil_sen_slope": None, "status": "insufficient"})
                continue
            points = [(math.log(float(level["sizes"][field])), math.log(level["elapsed_ns"]["median"])) for level in valid]
            fitted = ols(points)
            result.append(
                {
                    **base,
                    "ols_slope": fitted["slope"],
                    "ols_slope_ci95": bootstrap_slope(points, count, seed),
                    "r_squared": fitted["r_squared"],
                    "theil_sen_slope": theil_sen(points),
                    "status": "estimated",
                }
            )
    return result


def phase_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    groups: dict[tuple[str, int, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        groups[(row["family"], row["target_size"], row["scope"], row["algorithm"])].append(row)
    result = []
    for (family, size, scope, algorithm), group in sorted(groups.items()):
        medians = {}
        for phase in PHASES:
            values = [float(row["timings"][phase]) for row in group if row["timings"].get(phase) is not None]
            medians[phase] = statistics.median(values) if values else None
        result.append({
            "family": family,
            "target_size": size,
            "scope": scope,
            "algorithm": algorithm,
            "sizes": group[0]["sizes"],
            "phase_medians_ns": medians,
        })
    return result


def coverage(raw: dict[str, Any], rows: list[dict[str, Any]]) -> dict[str, Any]:
    points = raw.get("point_results", [])
    checks = [check for point in points for check in point.get("correctness", [])]
    mismatches = [point for point in points if point.get("state") == "invalid"]
    identities = [row["sample_identity"] for row in rows]
    return {
        "planned_points": raw.get("completion", {}).get("planned_point_count"),
        "observed_points": len(points),
        "complete_points": sum(point.get("state") == "complete" for point in points),
        "stopped_points": sum(point.get("state") == "stopped" for point in points),
        "invalid_points": len(mismatches),
        "runner_failures": sum(point.get("state") in {"runner-error", "runner-timeout"} for point in points),
        "measured_iterations": len(rows),
        "correctness_checks": len(checks),
        "correctness_failures": sum(check.get("outcome") != "success" for check in checks),
        "duplicate_sample_identities": len(identities) - len(set(identities)),
        "missing_planned_points": raw.get("completion", {}).get("missing_point_count"),
    }


def summarize(raw: dict[str, Any]) -> dict[str, Any]:
    rows = flatten(raw)
    distributions = distribution_rows(rows)
    paired = paired_rows(rows, raw["protocol"])
    return {
        "schema_version": 1,
        "campaign": raw["campaign"],
        "source_commit": raw["source_commit"],
        "binary_sha256": raw["binary_sha256"],
        "config_sha256": raw["config_sha256"],
        "environment": raw["environment"],
        "protocol": raw["protocol"],
        "coverage": coverage(raw, rows),
        "distributions": distributions,
        "paired_comparisons": paired,
        "family_classifications": classifications(paired, raw["protocol"]),
        "fits": fit_rows(distributions, raw["protocol"]),
        "phase_decomposition": phase_rows(rows),
        "limitations": [
            "In-process maximum RSS deltas were unavailable and remain null.",
            "Structural byte counts are declared estimates, not allocator measurements.",
            "Results are specific to the recorded host, compiler, families, and measured range.",
            "Empirical timing does not prove asymptotic complexity.",
        ],
    }


def format_number(value: Any) -> str:
    if value is None:
        return "NA"
    if isinstance(value, float):
        return f"{value:.4g}"
    return str(value)


def report_markdown(summary: dict[str, Any]) -> str:
    coverage_row = summary["coverage"]
    environment_row = summary["environment"]
    lines = [
        "# Paper Kernel Scaling Full Report",
        "",
        "## Scope and protocol",
        "",
        "This campaign measures three exact implementations in one release process per family/size partition. Scope A starts from the canonical component and includes geometry, solving, completion, and verification. Scope B starts after shared geometry and chord generation and measures representation construction, matching or flow, and cover recovery only.",
        "",
        f"Source commit: `{summary['source_commit']}`. Binary SHA-256: `{summary['binary_sha256']}`. Config SHA-256: `{summary['config_sha256']}`.",
        "",
        f"Host: {environment_row.get('cpu_model')} on {environment_row.get('operating_system')}; compiler {environment_row.get('rustc_version')}; power source {environment_row.get('power_source')}.",
        "",
        "## Correctness and coverage",
        "",
        f"The campaign contains {coverage_row['measured_iterations']} retained measured iterations across {coverage_row['complete_points']} complete points. It has {coverage_row['invalid_points']} invalid points, {coverage_row['correctness_failures']} failed production gates, {coverage_row['duplicate_sample_identities']} duplicate identities, and {coverage_row['missing_planned_points']} missing planned points.",
        "",
        "## Family-level paired results",
        "",
        "| Family | Scope | Explicit reference | Median ratio | 95% CI | Classification | Stable crossover target |",
        "| --- | --- | --- | ---: | ---: | --- | ---: |",
    ]
    for row in summary["family_classifications"]:
        lines.append(
            f"| {row['family']} | {row['scope']} | {row['explicit_algorithm']} | {format_number(row['aggregate_median_ratio'])} | [{format_number(row['aggregate_median_ratio_ci95'][0])}, {format_number(row['aggregate_median_ratio_ci95'][1])}] | {row['classification']} | {format_number(row['stable_crossover_target_size'])} |"
        )
    lines += [
        "",
        "Ratios are compact divided by the named explicit implementation; values below one favor compact. A crossover is emitted only after three consecutive larger measured levels favor compact and at least two corresponding confidence intervals lie wholly below one.",
        "",
        "## Scaling and phases",
        "",
        "Empirical exponents use one median per predeclared size level. OLS, fixed-seed bootstrap intervals, R-squared, and Theil-Sen estimates are retained in the machine-readable summary. Explicit conflict construction, biclique construction, network construction, matching or flow, recovery, completion, and verification remain separate nullable fields.",
        "",
        "## Relationship to P15",
        "",
        "P15 measures fresh-process wall time and remains valid for reproducibility at its measured sizes. Scope A removes process creation and CLI/config/serialization overhead while retaining the solve pipeline. Scope B additionally removes common geometry and final completion/verification. Differences between the three scopes identify whether fixed process cost masked a kernel effect; they do not invalidate or overwrite P15.",
        "",
        "## Claim boundary",
        "",
        "These measurements are finite, host-specific empirical evidence. They do not prove asymptotic complexity, universal speedup, AN19 runtime, or a crossover outside the measured families and host. Scope B is not end-to-end runtime.",
        "",
    ]
    return "\n".join(lines)


def latex(value: Any) -> str:
    return str(value).replace("_", "\\_").replace("%", "\\%")


def latex_tables(summary: dict[str, Any]) -> str:
    tables: list[str] = ["% Generated by tools/analyze_paper_kernel_scaling.py", "% Requires booktabs."]
    coverage_row = summary["coverage"]
    tables += [
        "\\begin{table}[t]", "\\caption{Kernel campaign correctness and coverage.}",
        "\\label{tab:kernel-coverage}", "\\begin{tabular}{rrrrrr}", "\\toprule",
        "Points & Complete & Stopped & Invalid & Iterations & Mismatches \\\\", "\\midrule",
        f"{coverage_row['observed_points']} & {coverage_row['complete_points']} & {coverage_row['stopped_points']} & {coverage_row['invalid_points']} & {coverage_row['measured_iterations']} & {coverage_row['correctness_failures']} \\\\",
        "\\bottomrule", "\\end{tabular}", "\\end{table}", "",
    ]
    for scope, number in zip(SCOPES, ("a", "b"), strict=True):
        tables += [
            "\\begin{table}[t]", f"\\caption{{Scope {number.upper()} representative median timings.}}",
            f"\\label{{tab:kernel-scope-{number}}}", "\\begin{tabular}{llrrr}", "\\toprule",
            "Family & Algorithm & Target & Median (ns) & $q$ \\\\", "\\midrule",
        ]
        for row in summary["distributions"]:
            if row["scope"] == scope and row["target_size"] == max(level["target_size"] for level in summary["distributions"] if level["family"] == row["family"] and level["scope"] == scope):
                tables.append(f"{latex(row['family'])} & {latex(row['algorithm'])} & {row['target_size']} & {row['elapsed_ns']['median']:.4g} & {format_number(row['sizes'].get('q'))} \\\\")
        tables += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]
    tables += ["\\begin{table}[t]", "\\caption{Empirical log--log exponents.}", "\\label{tab:kernel-exponents}", "\\begin{tabular}{llllrrr}", "\\toprule", "Family & Scope & Algorithm & Variable & OLS & Theil--Sen & $R^2$ \\\\", "\\midrule"]
    for row in summary["fits"]:
        if row["status"] == "estimated":
            tables.append(f"{latex(row['family'])} & {latex(row['scope'])} & {latex(row['algorithm'])} & {row['independent_variable']} & {row['ols_slope']:.3g} & {row['theil_sen_slope']:.3g} & {row['r_squared']:.3g} \\\\")
    tables += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]
    tables += ["\\begin{table}[t]", "\\caption{Predeclared crossover classifications.}", "\\label{tab:kernel-crossover}", "\\begin{tabular}{llllrr}", "\\toprule", "Family & Scope & Reference & Class & Ratio & Crossover \\\\", "\\midrule"]
    for row in summary["family_classifications"]:
        tables.append(f"{latex(row['family'])} & {latex(row['scope'])} & {latex(row['explicit_algorithm'])} & {latex(row['classification'])} & {row['aggregate_median_ratio']:.3g} & {format_number(row['stable_crossover_target_size'])} \\\\")
    tables += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]
    tables += ["\\begin{table}[t]", "\\caption{Structural compression at the largest complete level.}", "\\label{tab:kernel-compression}", "\\begin{tabular}{lrrrrr}", "\\toprule", "Family & Target & $q$ & $K$ & $M$ & $K/M$ \\\\", "\\midrule"]
    for family in sorted({row["family"] for row in summary["distributions"]}):
        level = max((row for row in summary["distributions"] if row["family"] == family and row["algorithm"] == "compact-mrd"), key=lambda row: row["target_size"])
        k_value = level["sizes"].get("explicit_conflict_edge_count_k")
        m_value = level["sizes"].get("compressed_size_m")
        ratio = k_value / m_value if k_value and m_value else None
        tables.append(f"{latex(family)} & {level['target_size']} & {format_number(level['sizes'].get('q'))} & {format_number(k_value)} & {format_number(m_value)} & {format_number(ratio)} \\\\")
    tables += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]
    tables += ["\\begin{table}[t]", "\\caption{Median phase decomposition at representative largest levels.}", "\\label{tab:kernel-phases}", "\\begin{tabular}{lllrrrr}", "\\toprule", "Family & Scope & Algorithm & Representation & Solve & Recovery & Verification \\\\", "\\midrule"]
    for row in summary["phase_decomposition"]:
        largest = max(item["target_size"] for item in summary["phase_decomposition"] if item["family"] == row["family"] and item["scope"] == row["scope"])
        if row["target_size"] != largest:
            continue
        phases = row["phase_medians_ns"]
        representation = sum(phases.get(key) or 0 for key in ("embedding_ns", "explicit_conflict_construction_ns", "biclique_construction_ns", "explicit_network_construction_ns", "compressed_network_construction_ns"))
        solve = sum(phases.get(key) or 0 for key in ("matching_ns", "max_flow_ns"))
        recovery = sum(phases.get(key) or 0 for key in ("vertex_cover_recovery_ns", "chord_selection_ns", "rectangle_completion_recovery_ns"))
        tables.append(f"{latex(row['family'])} & {latex(row['scope'])} & {latex(row['algorithm'])} & {representation:.4g} & {solve:.4g} & {recovery:.4g} & {format_number(phases.get('verification_ns'))} \\\\")
    tables += ["\\bottomrule", "\\end{tabular}", "\\end{table}", ""]
    return "\n".join(tables)


def svg_chart(path: Path, title: str, x_label: str, y_label: str, series: list[tuple[str, list[tuple[float, float]]]], horizontal_one: bool = False) -> None:
    width, height = 960, 560
    left, top, right, bottom = 90, 55, 220, 70
    points = [point for _, values in series for point in values]
    if not points:
        points = [(0, 0), (1, 1)]
    x_min, x_max = min(x for x, _ in points), max(x for x, _ in points)
    y_min, y_max = min(y for _, y in points), max(y for _, y in points)
    if x_min == x_max:
        x_max += 1
    if y_min == y_max:
        y_max += 1
    def locate(x: float, y: float) -> tuple[float, float]:
        return (
            left + (x - x_min) / (x_max - x_min) * (width - left - right),
            height - bottom - (y - y_min) / (y_max - y_min) * (height - top - bottom),
        )
    colors = ("#007f73", "#c65d00", "#4c5f7a", "#a33f5d", "#6b5ca5", "#2e7d32", "#8d6e63")
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">', '<rect width="100%" height="100%" fill="white"/>', f'<text x="{width/2}" y="28" text-anchor="middle" font-family="sans-serif" font-size="18">{html.escape(title)}</text>', f'<line x1="{left}" y1="{height-bottom}" x2="{width-right}" y2="{height-bottom}" stroke="#263238"/>', f'<line x1="{left}" y1="{top}" x2="{left}" y2="{height-bottom}" stroke="#263238"/>', f'<text x="{(left+width-right)/2}" y="{height-18}" text-anchor="middle" font-family="sans-serif" font-size="13">{html.escape(x_label)}</text>', f'<text x="20" y="{height/2}" text-anchor="middle" transform="rotate(-90 20 {height/2})" font-family="sans-serif" font-size="13">{html.escape(y_label)}</text>']
    if horizontal_one and y_min <= 1 <= y_max:
        _, y = locate(x_min, 1)
        parts.append(f'<line x1="{left}" y1="{y}" x2="{width-right}" y2="{y}" stroke="#666" stroke-dasharray="5 4"/>')
    for index, (label, values) in enumerate(series):
        color = colors[index % len(colors)]
        coordinates = [locate(x, y) for x, y in values]
        if coordinates:
            path_data = " ".join(("M" if point_index == 0 else "L") + f" {x:.2f},{y:.2f}" for point_index, (x, y) in enumerate(coordinates))
            parts.append(f'<path d="{path_data}" fill="none" stroke="{color}" stroke-width="2"/>')
            parts.extend(f'<circle cx="{x:.2f}" cy="{y:.2f}" r="3" fill="{color}"/>' for x, y in coordinates)
        parts.append(f'<text x="{width-right+12}" y="{top+16*index}" font-family="sans-serif" font-size="10" fill="{color}">{html.escape(label)}</text>')
    parts.append("</svg>")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(parts) + "\n")


def figures(summary: dict[str, Any], output: Path) -> None:
    distributions = summary["distributions"]
    def time_series(scope: str, x_field: str, algorithm_x: bool = False) -> list[tuple[str, list[tuple[float, float]]]]:
        result = []
        for family in sorted({row["family"] for row in distributions}):
            for algorithm in ALGORITHMS:
                values = []
                for row in distributions:
                    if row["family"] != family or row["scope"] != scope or row["algorithm"] != algorithm:
                        continue
                    field = "compressed_size_m" if algorithm_x and algorithm == "compact-mrd" else x_field
                    x = row["sizes"].get(field)
                    if x and row["elapsed_ns"]["median"]:
                        values.append((math.log10(x), math.log10(row["elapsed_ns"]["median"])))
                if values:
                    result.append((f"{family}:{algorithm}", sorted(values)))
        return result
    svg_chart(output / "scope-a-time-vs-q.svg", "Scope A time versus q", "log10 q", "log10 median time (ns)", time_series(SCOPES[0], "q"))
    svg_chart(output / "scope-b-time-vs-structure.svg", "Scope B time versus K or M", "log10 K (explicit) or M (compact)", "log10 median time (ns)", time_series(SCOPES[1], "explicit_conflict_edge_count_k", True))
    ratio_series = []
    for family in sorted({row["family"] for row in summary["paired_comparisons"]}):
        for scope in SCOPES:
            for explicit in EXPLICIT:
                values = [(row["q"], row["ratios"]["median"]) for row in summary["paired_comparisons"] if row["family"] == family and row["scope"] == scope and row["explicit_algorithm"] == explicit and row["q"]]
                if values:
                    ratio_series.append((f"{family}:{scope}:{explicit}", sorted(values)))
    svg_chart(output / "paired-ratio-vs-q.svg", "Compact / explicit paired ratio", "q (chords)", "median paired ratio", ratio_series, True)
    structure_series = []
    for family in sorted({row["family"] for row in distributions}):
        values = []
        for row in distributions:
            if row["family"] == family and row["algorithm"] == "compact-mrd" and row["scope"] == SCOPES[0]:
                k_value, m_value = row["sizes"].get("explicit_conflict_edge_count_k"), row["sizes"].get("compressed_size_m")
                if k_value is not None and m_value is not None:
                    values.append((k_value, m_value))
        if values:
            structure_series.append((family, sorted(values)))
    svg_chart(output / "k-vs-m.svg", "Explicit K versus compressed M", "explicit conflict edges K", "compressed nodes + arcs M", structure_series)
    representative = []
    for family in sorted({row["family"] for row in summary["phase_decomposition"]}):
        family_rows = [row for row in summary["phase_decomposition"] if row["family"] == family and row["scope"] == SCOPES[1]]
        if family_rows:
            largest = max(row["target_size"] for row in family_rows)
            representative.extend(row for row in family_rows if row["target_size"] == largest)
    phase_series = []
    for index, row in enumerate(representative, start=1):
        for phase in PHASES:
            value = row["phase_medians_ns"].get(phase)
            if value is not None:
                phase_series.append((f"{row['family']}:{row['algorithm']}:{phase}", [(index, value)]))
    svg_chart(output / "phase-decomposition.svg", "Scope B phase decomposition at largest levels", "representative family/solver index", "median phase time (ns)", phase_series)
    construction_series = []
    for family in sorted({row["family"] for row in summary["phase_decomposition"]}):
        for algorithm in ALGORITHMS:
            values = []
            for row in summary["phase_decomposition"]:
                if row["family"] != family or row["algorithm"] != algorithm or row["scope"] != SCOPES[1]:
                    continue
                construction = sum(row["phase_medians_ns"].get(key) or 0 for key in ("embedding_ns", "explicit_conflict_construction_ns", "biclique_construction_ns", "explicit_network_construction_ns", "compressed_network_construction_ns"))
                structure_field = "compressed_size_m" if algorithm == "compact-mrd" else "explicit_conflict_edge_count_k"
                structural_size = row["sizes"].get(structure_field)
                if structural_size and construction:
                    values.append((math.log10(structural_size), math.log10(construction)))
            if values:
                construction_series.append((f"{family}:{algorithm}", sorted(values)))
    svg_chart(output / "construction-time-vs-structure.svg", "Construction time versus K or M", "log10 K (explicit) or M (compact)", "log10 construction time (ns)", construction_series)
    memory_series = []
    for family in sorted({row["family"] for row in distributions}):
        explicit_values, compact_values = [], []
        for row in distributions:
            if row["family"] == family and row["algorithm"] == "compact-mrd" and row["scope"] == SCOPES[0]:
                structure = next((item for item in summary.get("_point_structures", []) if item[0] == family and item[1] == row["target_size"]), None)
                if structure:
                    explicit_values.append((row["target_size"], structure[2].get("explicit_estimated_structural_bytes", 0)))
                    compact_values.append((row["target_size"], structure[2].get("compact_estimated_structural_bytes", 0)))
        if explicit_values:
            memory_series.extend(((f"{family}:explicit", explicit_values), (f"{family}:compact", compact_values)))
    svg_chart(output / "structural-memory.svg", "Estimated structural storage", "target size", "estimated structural bytes", memory_series)


def summary_csv(path: Path, summary: dict[str, Any]) -> None:
    rows = []
    for key in ("distributions", "paired_comparisons", "family_classifications", "fits", "phase_decomposition"):
        rows.extend({"record_type": key, **row} for row in summary[key])
    fields = sorted({key for row in rows for key in row}) or ["record_type"]
    buffer = io.StringIO(newline="")
    writer = csv.DictWriter(buffer, fieldnames=fields, lineterminator="\n")
    writer.writeheader()
    for row in rows:
        writer.writerow({key: json.dumps(value, sort_keys=True, separators=(",", ":")) if isinstance(value, (dict, list)) else value for key, value in row.items()})
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(buffer.getvalue())


def validate_artifacts(figure_dir: Path, tables: str) -> None:
    figures = sorted(figure_dir.glob("*.svg"))
    if len(figures) != 7:
        raise ValueError(f"expected seven SVG figures, found {len(figures)}")
    for figure in figures:
        if figure.stat().st_size == 0:
            raise ValueError(f"empty SVG: {figure}")
        ET.parse(figure)
    if tables.count("\\begin{table}") != 7 or tables.count("\\end{table}") != 7:
        raise ValueError("LaTeX output must contain seven balanced tables")
    if tables.count("\\toprule") != 7 or tables.count("\\bottomrule") != 7:
        raise ValueError("every LaTeX table must have booktabs rules")


def self_test() -> None:
    assert distribution([1, 2, 3])["median"] == 2
    points = [(math.log(1), math.log(2)), (math.log(2), math.log(4)), (math.log(4), math.log(8))]
    assert abs(ols(points)["slope"] - 1) < 1e-12
    assert abs(theil_sen(points) - 1) < 1e-12
    assert bootstrap_ci([1, 2, 3], statistics.median, 100, 42) == bootstrap_ci([1, 2, 3], statistics.median, 100, 42)


def main() -> int:
    arguments = parse_args()
    if arguments.self_test:
        self_test()
        print("paper-kernel-scaling analyzer self-test: ok")
        return 0
    raw = json.loads(root_path(arguments.input).read_text())
    summary = summarize(raw)
    summary["_point_structures"] = [(point["family"], point["target_size"], point.get("structure", {})) for point in raw.get("point_results", [])]
    figure_dir = root_path(arguments.figure_dir)
    figures(summary, figure_dir)
    summary.pop("_point_structures", None)
    tables = latex_tables(summary)
    validate_artifacts(figure_dir, tables)
    root_path(arguments.summary_json).write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    summary_csv(root_path(arguments.summary_csv), summary)
    root_path(arguments.report).write_text(report_markdown(summary))
    root_path(arguments.tables).write_text(tables + "\n")
    print(json.dumps({"coverage": summary["coverage"], "figures": 7, "tables": 7}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
