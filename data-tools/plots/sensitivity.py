"""
plots/sensitivity.py — Factor sensitivity analysis.

For each metric, shows the normalised % change vs baseline as each factor
is varied, giving a quick visual of which knob matters most.

Reads:  metrics/all_results.csv  (or --input)
        metrics/stats_summary.csv (optional — pre-computed deltas)
Writes: figures/sensitivity_<metric>.png
        figures/sensitivity_heatmap.png  (all factors × all metrics)
"""

import argparse
import math
import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd


# ── which experiments belong to which factor ──────────────────────────────────
FACTOR_PREFIXES: dict[str, list[str]] = {
    "Batch Size":   ["bs_"],
    "Timeout":      ["to_"],
    "Policy":       ["pol_"],
    "DA Mode":      ["da_"],
    "Prover":       ["pv_"],
    "Input Rate":   ["tps_"],
    "Tx Mix":       ["mix_"],
}

# Metrics to include in sensitivity analysis (col, display_label, lower_is_better)
SENSITIVITY_METRICS: list[tuple[str, str, bool]] = [
    ("tps_committed",   "TPS Committed",        False),
    ("avg_l2_l1_ms",    "Avg L2→L1 (ms)",       True),
    ("p95_l2_l1_ms",    "P95 L2→L1 (ms)",       True),
    ("avg_prove_ms",    "Avg Prove (ms)",         True),
    ("avg_gas_per_tx",  "Avg Gas/tx",            True),
    ("avg_comp_ratio",  "Compression Ratio",     False),
    ("jains_fairness",  "Jain's Fairness",       False),
    ("starvation_count","Starvation Count",       True),
]


def _delta_pct(val: float, base: float) -> float | None:
    if base == 0 or math.isnan(base) or math.isnan(val):
        return None
    return (val - base) / abs(base) * 100.0


def _factor_of(exp_id: str) -> str | None:
    for factor, prefixes in FACTOR_PREFIXES.items():
        for p in prefixes:
            if exp_id.startswith(p):
                return factor
    return None


def compute_sensitivity_matrix(df: pd.DataFrame) -> pd.DataFrame:
    """
    Returns a DataFrame indexed by experiment_id with columns for each metric's
    % delta vs baseline.
    """
    baseline_df = df[df["experiment_id"] == "baseline"]
    if baseline_df.empty:
        print("[sensitivity] WARNING: no baseline row — deltas will be NaN")
        baseline_means: dict[str, float] = {}
    else:
        baseline_means = {
            col: pd.to_numeric(baseline_df[col], errors="coerce").mean()
            for col, _, _ in SENSITIVITY_METRICS
            if col in baseline_df.columns
        }

    rows = []
    for exp_id, group in df.groupby("experiment_id"):
        if exp_id == "baseline":
            continue
        row: dict = {"experiment_id": exp_id, "factor": _factor_of(exp_id) or "other"}
        for col, label, _ in SENSITIVITY_METRICS:
            if col not in group.columns:
                row[label] = None
                continue
            val = pd.to_numeric(group[col], errors="coerce").mean()
            base = baseline_means.get(col, float("nan"))
            row[label] = _delta_pct(val, base)
        rows.append(row)

    return pd.DataFrame(rows)


def plot_per_metric(sensitivity: pd.DataFrame, output_dir: str):
    """One horizontal bar chart per metric — experiments on y-axis, delta on x-axis."""
    os.makedirs(output_dir, exist_ok=True)
    metric_cols = [lbl for _, lbl, _ in SENSITIVITY_METRICS]

    for col_idx, (_, label, lower_is_better) in enumerate(SENSITIVITY_METRICS):
        if label not in sensitivity.columns:
            continue
        sub = sensitivity[["experiment_id", "factor", label]].dropna(subset=[label])
        if sub.empty:
            continue
        sub = sub.sort_values(label, ascending=not lower_is_better)

        fig, ax = plt.subplots(figsize=(8, max(4, len(sub) * 0.4)))

        colors = []
        for val in sub[label]:
            if lower_is_better:
                # green = improvement (negative delta), red = worse (positive delta)
                colors.append("#4CAF50" if val <= 0 else "#F44336")
            else:
                colors.append("#4CAF50" if val >= 0 else "#F44336")

        bars = ax.barh(sub["experiment_id"], sub[label], color=colors, alpha=0.8)
        ax.axvline(0, color="black", linewidth=0.8)
        ax.bar_label(bars, fmt="%.1f%%", fontsize=7, padding=3)

        ax.set_xlabel("% Change vs Baseline")
        ax.set_title(
            f"Sensitivity: {label}\n"
            f"({'lower is better' if lower_is_better else 'higher is better'})"
        )
        ax.grid(axis="x", alpha=0.3)
        fig.tight_layout()

        slug = label.lower().replace(" ", "_").replace("/", "_per_").replace("→", "_")
        out = os.path.join(output_dir, f"sensitivity_{slug}.png")
        fig.savefig(out, dpi=150)
        plt.close(fig)
        print(f"[sensitivity] saved → {out}")


def plot_heatmap(sensitivity: pd.DataFrame, output_dir: str):
    """Heatmap: rows = experiments, columns = metrics, values = % delta."""
    os.makedirs(output_dir, exist_ok=True)

    metric_cols = [lbl for _, lbl, _ in SENSITIVITY_METRICS if lbl in sensitivity.columns]
    if not metric_cols:
        return

    # sort rows by factor then experiment id
    heat = sensitivity.set_index("experiment_id")[metric_cols].astype(float)
    heat = heat.sort_index()

    if heat.empty:
        return

    fig, ax = plt.subplots(figsize=(max(10, len(metric_cols) * 1.4), max(6, len(heat) * 0.5)))

    # symmetric colour scale
    vmax = heat.abs().max().max()
    vmax = max(vmax, 1.0)

    im = ax.imshow(heat.values, cmap="RdYlGn", aspect="auto", vmin=-vmax, vmax=vmax)

    ax.set_xticks(np.arange(len(metric_cols)))
    ax.set_yticks(np.arange(len(heat)))
    ax.set_xticklabels(metric_cols, rotation=40, ha="right", fontsize=8)
    ax.set_yticklabels(heat.index, fontsize=8)
    ax.set_title("Factor Sensitivity Heatmap — % change vs baseline\n(green = improvement, red = degradation)")

    # annotate
    for i in range(len(heat)):
        for j in range(len(metric_cols)):
            val = heat.values[i, j]
            if not np.isnan(val):
                ax.text(j, i, f"{val:+.0f}%", ha="center", va="center",
                        fontsize=6,
                        color="white" if abs(val) > vmax * 0.6 else "black")

    fig.colorbar(im, ax=ax, label="% Δ vs baseline")
    fig.tight_layout()
    out = os.path.join(output_dir, "sensitivity_heatmap.png")
    fig.savefig(out, dpi=150)
    plt.close(fig)
    print(f"[sensitivity] saved → {out}")


def main():
    p = argparse.ArgumentParser(description="RollupX factor sensitivity analysis")
    p.add_argument("--input",      default="metrics/all_results.csv")
    p.add_argument("--output_dir", default="figures")
    args = p.parse_args()

    if not os.path.exists(args.input):
        print(f"[sensitivity] Input not found: {args.input}")
        sys.exit(1)

    df = pd.read_csv(args.input)
    print(f"[sensitivity] Loaded {len(df)} rows")

    sensitivity = compute_sensitivity_matrix(df)

    if sensitivity.empty:
        print("[sensitivity] No non-baseline experiments found")
        sys.exit(0)

    plot_per_metric(sensitivity, args.output_dir)
    plot_heatmap(sensitivity, args.output_dir)

    # save sensitivity matrix for reference
    out_csv = os.path.join(os.path.dirname(args.input), "sensitivity_matrix.csv")
    sensitivity.to_csv(out_csv, index=False)
    print(f"[sensitivity] matrix saved → {out_csv}")


if __name__ == "__main__":
    main()
