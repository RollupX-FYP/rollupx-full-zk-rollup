"""
plots/latency_boxplot.py — Boxplot of avg_l2_l1_ms across experiment groups.

Shows run-to-run variance per configuration.
Reads: metrics/all_results.csv
"""

import argparse
import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import pandas as pd


FACTOR_PREFIXES = {
    "batch_size": ["bs_", "baseline"],
    "timeout":    ["to_", "baseline"],
    "policy":     ["pol_", "baseline"],
    "da_mode":    ["da_", "baseline"],
    "rate":       ["tps_", "baseline"],
    "tx_mix":     ["mix_", "baseline"],
    "prover":     ["pv_", "baseline"],
}


def plot_boxplot(df: pd.DataFrame, factor: str, prefixes: list[str], output_dir: str):
    os.makedirs(output_dir, exist_ok=True)

    mask = df["experiment_id"].apply(
        lambda x: any(x.startswith(p) or x == p for p in prefixes)
    )
    sub = df[mask].copy()
    if sub.empty or "avg_l2_l1_ms" not in sub.columns:
        print(f"[boxplot] No data for factor '{factor}'")
        return

    sub["avg_l2_l1_ms"] = pd.to_numeric(sub["avg_l2_l1_ms"], errors="coerce")

    # group by experiment
    groups = {eid: grp["avg_l2_l1_ms"].dropna().tolist()
              for eid, grp in sub.groupby("experiment_id")}
    if not groups:
        return

    labels = sorted(groups.keys())
    data   = [groups[l] for l in labels]

    fig, ax = plt.subplots(figsize=(max(6, len(labels) * 1.2), 5))
    bp = ax.boxplot(data, labels=labels, patch_artist=True, notch=False)

    colors = plt.cm.tab10.colors
    for patch, color in zip(bp["boxes"], colors):
        patch.set_facecolor(color)
        patch.set_alpha(0.7)

    ax.set_xlabel("Experiment")
    ax.set_ylabel("Avg L2→L1 Latency (ms)")
    ax.set_title(f"Latency Variance — Factor: {factor}")
    ax.tick_params(axis="x", rotation=30)
    ax.grid(axis="y", alpha=0.3)
    fig.tight_layout()

    out = os.path.join(output_dir, f"latency_boxplot_{factor}.png")
    fig.savefig(out, dpi=150)
    plt.close(fig)
    print(f"[boxplot] saved → {out}")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--input",      default="metrics/all_results.csv")
    p.add_argument("--output_dir", default="figures")
    p.add_argument("--factor",     default=None)
    args = p.parse_args()

    if not os.path.exists(args.input):
        print(f"Input not found: {args.input}")
        sys.exit(1)

    df = pd.read_csv(args.input)

    factors = (
        {args.factor: FACTOR_PREFIXES.get(args.factor, [args.factor])}
        if args.factor
        else FACTOR_PREFIXES
    )

    for factor, prefixes in factors.items():
        plot_boxplot(df, factor, prefixes, args.output_dir)


if __name__ == "__main__":
    main()
