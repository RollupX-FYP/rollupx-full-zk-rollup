"""
pareto.py — Load, analyse, and plot RollupX benchmark metrics.

Extended from original to support:
  - Multi-directory loading (pass metrics_root to load_metrics)
  - Non-dominated Pareto frontier (not convex hull)
  - Richer plot styling
  - CLI with --metrics_root and --output_dir flags
"""

import argparse
import glob
import json
import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import pandas as pd


# ── loader ────────────────────────────────────────────────────────────────────

def load_metrics(metrics_root: str = None) -> dict:
    """
    Load all workload / executor / submitter JSON files from metrics_root.
    Each experiment_id maps to {"workload": ..., "executor": ..., "submitter": [...]}.
    """
    if metrics_root is None:
        metrics_root = os.environ.get("METRICS_ROOT", "metrics")

    experiments: dict = {}

    # ── workload ──────────────────────────────────────────────────────────────
    for f in sorted(glob.glob(os.path.join(metrics_root, "**", "workload_*.json"), recursive=True)):
        try:
            with open(f) as fp:
                data = json.load(fp)
            exp_id = data.get("experiment_id")
            if exp_id:
                if exp_id not in experiments:
                    experiments[exp_id] = {}
                # merge: prefer entry with more detail
                if "workload" not in experiments[exp_id]:
                    experiments[exp_id]["workload"] = data
        except Exception as e:
            print(f"[pareto] Error reading {f}: {e}", file=sys.stderr)

    # ── executor ──────────────────────────────────────────────────────────────
    for f in sorted(glob.glob(os.path.join(metrics_root, "**", "executor_*.json"), recursive=True)):
        try:
            with open(f) as fp:
                data = json.load(fp)
            exp_id = data.get("experiment_id")
            if exp_id:
                if exp_id not in experiments:
                    experiments[exp_id] = {}
                experiments[exp_id]["executor"] = data
        except Exception as e:
            print(f"[pareto] Error reading {f}: {e}", file=sys.stderr)

    # ── submitter (JSONL) ─────────────────────────────────────────────────────
    for f in sorted(glob.glob(os.path.join(metrics_root, "**", "submitter_metrics.json"), recursive=True)):
        try:
            with open(f) as fp:
                for line in fp:
                    line = line.strip()
                    if not line:
                        continue
                    data = json.loads(line)
                    exp_id = data.get("experiment_id")
                    if exp_id:
                        if exp_id not in experiments:
                            experiments[exp_id] = {}
                        experiments[exp_id].setdefault("submitter", []).append(data)
        except Exception as e:
            print(f"[pareto] Error reading {f}: {e}", file=sys.stderr)

    return experiments


# ── analyser ──────────────────────────────────────────────────────────────────

def analyze(experiments: dict) -> pd.DataFrame:
    rows = []

    for exp_id, data in experiments.items():
        row: dict = {"experiment_id": exp_id}

        # ── workload ──────────────────────────────────────────────────────────
        wl = data.get("workload", {})
        row["tps_in"]    = wl.get("rate", 0)
        row["total_txs"] = wl.get("details", {}).get("total_txs", 0)

        # ── executor ──────────────────────────────────────────────────────────
        ex = data.get("executor", {})
        row["prover"]    = ex.get("prover_backend", "unknown")
        prove_times      = ex.get("proof_generation_times_ms", [])
        row["avg_prove_ms"] = sum(prove_times) / len(prove_times) if prove_times else 0

        # ── submitter ─────────────────────────────────────────────────────────
        sub = data.get("submitter", [])
        if sub:
            l2_l1      = [s.get("l2_l1_latency_ms", 0) or 0    for s in sub]
            gas_saved  = [s.get("gas_saved", 0) or 0             for s in sub]
            comp_r     = [s.get("compression_ratio", 0) or 0     for s in sub]
            gas_per_tx = [s.get("gas_used_per_tx", 0) or 0       for s in sub]

            row["avg_l2_l1_ms"]   = sum(l2_l1) / len(l2_l1)
            row["avg_gas_saved"]  = sum(gas_saved) / len(gas_saved)
            row["avg_comp_ratio"] = sum(comp_r) / len(comp_r)
            row["avg_gas_per_tx"] = sum(gas_per_tx) / len(gas_per_tx)
            row["da_mode"]        = sub[0].get("da_mode", "unknown")
        else:
            row["avg_l2_l1_ms"]   = 0
            row["avg_gas_saved"]  = 0
            row["avg_comp_ratio"] = 0
            row["avg_gas_per_tx"] = 0
            row["da_mode"]        = "unknown"

        rows.append(row)

    return pd.DataFrame(rows)


# ── Pareto frontier (non-dominated filter) ────────────────────────────────────

def pareto_frontier(df: pd.DataFrame, x_col: str, y_col: str, minimize_x=True, minimize_y=False) -> pd.DataFrame:
    """
    Return the non-dominated subset of df with respect to (x_col, y_col).
    By default: minimize x (latency), maximize y (gas saved).
    """
    points = df[[x_col, y_col]].dropna().copy()
    non_dominated = []

    for i, row_i in points.iterrows():
        xi, yi = row_i[x_col], row_i[y_col]
        is_dominated = False
        for j, row_j in points.iterrows():
            if i == j:
                continue
            xj, yj = row_j[x_col], row_j[y_col]
            # j dominates i if j is at least as good on both and strictly better on one
            x_ok = (xj <= xi) if minimize_x else (xj >= xi)
            y_ok = (yj >= yi) if not minimize_y else (yj <= yi)
            x_better = (xj < xi) if minimize_x else (xj > xi)
            y_better = (yj > yi) if not minimize_y else (yj < yi)
            if x_ok and y_ok and (x_better or y_better):
                is_dominated = True
                break
        if not is_dominated:
            non_dominated.append(i)

    return df.loc[non_dominated].sort_values(x_col)


# ── plots ─────────────────────────────────────────────────────────────────────

def plot_pareto(df: pd.DataFrame, output_dir: str = "figures"):
    os.makedirs(output_dir, exist_ok=True)

    if df.empty:
        print("[pareto] No data to plot")
        return

    # ── Cost vs Latency Pareto ────────────────────────────────────────────────
    fig, ax = plt.subplots(figsize=(10, 6))
    modes = df["da_mode"].unique()
    colors = plt.cm.tab10.colors

    for idx, mode in enumerate(modes):
        subset = df[df["da_mode"] == mode]
        ax.scatter(
            subset["avg_l2_l1_ms"], subset["avg_gas_saved"],
            label=mode, s=100, alpha=0.75,
            color=colors[idx % len(colors)], zorder=3,
        )
        # annotate experiment IDs
        for _, r in subset.iterrows():
            ax.annotate(
                r["experiment_id"],
                (r["avg_l2_l1_ms"], r["avg_gas_saved"]),
                fontsize=6, alpha=0.6, xytext=(3, 3), textcoords="offset points",
            )

    # draw non-dominated frontier
    frontier = pareto_frontier(df, "avg_l2_l1_ms", "avg_gas_saved")
    if not frontier.empty:
        ax.plot(
            frontier["avg_l2_l1_ms"], frontier["avg_gas_saved"],
            "k--", linewidth=1.2, label="Pareto frontier", zorder=4,
        )

    ax.set_xlabel("Avg L2→L1 Latency (ms)")
    ax.set_ylabel("Avg Gas Saved per Batch")
    ax.set_title("Pareto Frontier: Cost vs Latency (by DA Mode)")
    ax.legend()
    ax.grid(True, alpha=0.3)
    fig.tight_layout()
    out = os.path.join(output_dir, "pareto_cost_latency.png")
    fig.savefig(out, dpi=150)
    plt.close(fig)
    print(f"[pareto] saved → {out}")

    # ── Prover Performance ────────────────────────────────────────────────────
    fig2, ax2 = plt.subplots(figsize=(10, 6))
    provers = df["prover"].unique()

    for idx, prv in enumerate(provers):
        subset = df[df["prover"] == prv]
        ax2.scatter(
            subset["avg_prove_ms"], subset["tps_in"],
            label=prv, s=100, alpha=0.75,
            color=colors[idx % len(colors)],
        )

    ax2.set_xlabel("Avg Proving Time (ms)")
    ax2.set_ylabel("Input Rate (TPS)")
    ax2.set_title("Prover Performance: Proving Time vs Input Rate")
    ax2.legend()
    ax2.grid(True, alpha=0.3)
    fig2.tight_layout()
    out2 = os.path.join(output_dir, "prover_performance.png")
    fig2.savefig(out2, dpi=150)
    plt.close(fig2)
    print(f"[pareto] saved → {out2}")


# ── markdown report ───────────────────────────────────────────────────────────

def generate_markdown(df: pd.DataFrame, output: str = "thesis_summary.md"):
    if df.empty:
        return

    lines = [
        "# RollupX — Experimental Results Summary\n",
        "## Results Table\n",
        "| Experiment | Prover | DA Mode | Avg L2→L1 (ms) | Avg Prove (ms) | Gas Saved | Comp Ratio |",
        "|---|---|---|---|---|---|---|",
    ]
    for _, row in df.iterrows():
        lines.append(
            f"| {row['experiment_id']} | {row['prover']} | {row['da_mode']} "
            f"| {row['avg_l2_l1_ms']:.1f} | {row['avg_prove_ms']:.1f} "
            f"| {row['avg_gas_saved']:.0f} | {row['avg_comp_ratio']:.2f} |"
        )

    lines += [
        "\n## Pareto Analysis\n",
        "See `figures/pareto_cost_latency.png` and `figures/prover_performance.png`.\n",
    ]

    with open(output, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"[pareto] saved → {output}")


# ── CLI ───────────────────────────────────────────────────────────────────────

def main():
    p = argparse.ArgumentParser(description="RollupX Pareto analysis")
    p.add_argument("--metrics_root", default=os.environ.get("METRICS_ROOT", "metrics"))
    p.add_argument("--output_dir",   default="figures")
    p.add_argument("--csv",          default=None, help="Load from pre-aggregated CSV instead")
    args = p.parse_args()

    if args.csv and os.path.exists(args.csv):
        df = pd.read_csv(args.csv)
        # rename columns to match analyze() output if needed
        col_map = {
            "avg_l2_l1_ms": "avg_l2_l1_ms",
            "avg_prove_ms": "avg_prove_ms",
        }
        df = df.rename(columns=col_map)
    else:
        exps = load_metrics(args.metrics_root)
        if not exps:
            print("[pareto] No metrics found.")
            sys.exit(0)
        df = analyze(exps)

    print(df.to_string())
    plot_pareto(df, args.output_dir)
    generate_markdown(df)


if __name__ == "__main__":
    main()
