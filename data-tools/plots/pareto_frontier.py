"""
plots/pareto_frontier.py — Standalone Pareto frontier visualisation.

Extends pareto.py's plot_pareto() with additional axes:
  - Latency vs TPS (throughput–latency frontier)
  - Gas/tx vs Prove time (cost–proving frontier)
  - DA mode comparison bar chart

Reads:  metrics/all_results.csv  OR raw metrics root
Writes: figures/pareto_*.png
"""

import argparse
import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

# re-use loader and frontier from pareto.py
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))
from pareto import load_metrics, analyze, pareto_frontier


MARKERS = ["o", "s", "D", "^", "v", "P", "X", "*"]


def _scatter_with_frontier(
    ax, df: pd.DataFrame,
    x_col: str, y_col: str,
    group_col: str,
    minimize_x: bool, minimize_y: bool,
    annotate: bool = True,
):
    groups = df[group_col].unique() if group_col in df.columns else ["all"]
    colors = plt.cm.tab10.colors

    for idx, grp in enumerate(sorted(groups)):
        if group_col in df.columns:
            sub = df[df[group_col] == grp]
        else:
            sub = df

        ax.scatter(
            sub[x_col], sub[y_col],
            label=str(grp), s=90, alpha=0.75,
            color=colors[idx % len(colors)],
            marker=MARKERS[idx % len(MARKERS)],
            zorder=3,
        )
        if annotate:
            for _, row in sub.iterrows():
                ax.annotate(
                    row["experiment_id"],
                    (row[x_col], row[y_col]),
                    fontsize=5, alpha=0.55,
                    xytext=(3, 3), textcoords="offset points",
                )

    frontier = pareto_frontier(df, x_col, y_col, minimize_x=minimize_x, minimize_y=minimize_y)
    if not frontier.empty and len(frontier) > 1:
        ax.plot(
            frontier[x_col], frontier[y_col],
            "k--", linewidth=1.2, label="Pareto frontier", zorder=4,
        )

    ax.grid(True, alpha=0.3)
    ax.legend(fontsize=7)


def plot_all_frontiers(df: pd.DataFrame, output_dir: str):
    os.makedirs(output_dir, exist_ok=True)

    req_cols = {
        "avg_l2_l1_ms", "avg_gas_saved", "avg_gas_per_tx",
        "avg_prove_ms", "tps_committed", "da_mode", "prover",
    }
    missing = req_cols - set(df.columns)
    if missing:
        # fill with zeros so plots still render
        for col in missing:
            df[col] = 0

    # ── 1. Cost vs Latency (main) ─────────────────────────────────────────────
    fig, ax = plt.subplots(figsize=(10, 6))
    _scatter_with_frontier(
        ax, df,
        x_col="avg_l2_l1_ms", y_col="avg_gas_saved",
        group_col="da_mode",
        minimize_x=True, minimize_y=False,
    )
    ax.set_xlabel("Avg L2→L1 Latency (ms)")
    ax.set_ylabel("Avg Gas Saved per Batch")
    ax.set_title("Pareto Frontier: Cost vs Latency (by DA Mode)")
    fig.tight_layout()
    fig.savefig(os.path.join(output_dir, "pareto_cost_latency.png"), dpi=150)
    plt.close(fig)
    print(f"[pareto_frontier] saved pareto_cost_latency.png")

    # ── 2. Throughput vs Latency ──────────────────────────────────────────────
    if "tps_committed" in df.columns and df["tps_committed"].sum() > 0:
        fig, ax = plt.subplots(figsize=(10, 6))
        _scatter_with_frontier(
            ax, df,
            x_col="avg_l2_l1_ms", y_col="tps_committed",
            group_col="da_mode",
            minimize_x=True, minimize_y=False,
        )
        ax.set_xlabel("Avg L2→L1 Latency (ms)")
        ax.set_ylabel("TPS Committed")
        ax.set_title("Pareto Frontier: Throughput vs Latency")
        fig.tight_layout()
        fig.savefig(os.path.join(output_dir, "pareto_throughput_latency.png"), dpi=150)
        plt.close(fig)
        print(f"[pareto_frontier] saved pareto_throughput_latency.png")

    # ── 3. Gas/tx vs Prove time ────────────────────────────────────────────────
    if df["avg_prove_ms"].sum() > 0:
        fig, ax = plt.subplots(figsize=(10, 6))
        _scatter_with_frontier(
            ax, df,
            x_col="avg_prove_ms", y_col="avg_gas_per_tx",
            group_col="prover",
            minimize_x=True, minimize_y=True,
        )
        ax.set_xlabel("Avg Proof Generation Time (ms)")
        ax.set_ylabel("Avg Gas per Transaction")
        ax.set_title("Pareto Frontier: Proving Cost vs Gas Cost (by Prover)")
        fig.tight_layout()
        fig.savefig(os.path.join(output_dir, "pareto_prove_gas.png"), dpi=150)
        plt.close(fig)
        print(f"[pareto_frontier] saved pareto_prove_gas.png")

    # ── 4. DA mode comparison bar ─────────────────────────────────────────────
    if "da_mode" in df.columns and "avg_gas_per_tx" in df.columns:
        da_agg = (
            df.groupby("da_mode")[["avg_gas_per_tx", "avg_l2_l1_ms", "avg_comp_ratio"]]
            .mean()
            .reset_index()
        )
        if not da_agg.empty:
            fig, axes = plt.subplots(1, 3, figsize=(14, 5))
            for ax_i, (col, label) in enumerate([
                ("avg_gas_per_tx",  "Avg Gas/tx"),
                ("avg_l2_l1_ms",    "Avg L2→L1 (ms)"),
                ("avg_comp_ratio",  "Avg Compression Ratio"),
            ]):
                bars = axes[ax_i].bar(
                    da_agg["da_mode"], da_agg[col],
                    color=plt.cm.tab10.colors[:len(da_agg)], alpha=0.85,
                )
                axes[ax_i].bar_label(bars, fmt="%.2f", fontsize=8, padding=3)
                axes[ax_i].set_title(label)
                axes[ax_i].set_xlabel("DA Mode")
                axes[ax_i].grid(axis="y", alpha=0.3)
            fig.suptitle("DA Mode Comparison", fontsize=12, fontweight="bold")
            fig.tight_layout()
            fig.savefig(os.path.join(output_dir, "pareto_da_comparison.png"), dpi=150)
            plt.close(fig)
            print(f"[pareto_frontier] saved pareto_da_comparison.png")


def main():
    p = argparse.ArgumentParser(description="RollupX Pareto frontier plots")
    p.add_argument("--input",        default=None,
                   help="Pre-aggregated CSV (metrics/all_results.csv)")
    p.add_argument("--metrics_root", default=os.environ.get("METRICS_ROOT", "metrics"),
                   help="Raw metrics root (used if --input not provided)")
    p.add_argument("--output_dir",   default="figures")
    args = p.parse_args()

    if args.input and os.path.exists(args.input):
        df = pd.read_csv(args.input)
        print(f"[pareto_frontier] Loaded {len(df)} rows from {args.input}")
    else:
        exps = load_metrics(args.metrics_root)
        if not exps:
            print("[pareto_frontier] No metrics found.")
            sys.exit(0)
        df = analyze(exps)
        print(f"[pareto_frontier] Analysed {len(df)} experiments from {args.metrics_root}")

    plot_all_frontiers(df, args.output_dir)


if __name__ == "__main__":
    main()
