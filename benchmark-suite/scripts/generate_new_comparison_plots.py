#!/usr/bin/env python3
"""
Generate comparison plots from benchmark results.

Plot A — Gas/Tx vs Batch Size by DA Mode (calldata, blob, offchain)
Plot B — All Batch Policies comparison (fixed vs adaptive vs DPDAB): wait time & gas cost

Usage:
  # From existing consolidated data:
  python scripts/generate_new_comparison_plots.py <path_to_consolidated_results.csv>

  # Or use default location:
  python scripts/generate_new_comparison_plots.py
"""

import os
import sys
import pandas as pd
import numpy as np
import matplotlib.pyplot as plt
import matplotlib.ticker as mticker
import seaborn as sns

sns.set_theme(style="whitegrid", rc={
    "font.family": "sans-serif",
    "font.sans-serif": ["Inter", "Roboto", "Helvetica", "Arial"],
    "grid.color": "#e0e0e0",
    "axes.edgecolor": "#a0a0a0",
    "axes.labelcolor": "#2c3e50",
    "xtick.color": "#2c3e50",
    "ytick.color": "#2c3e50"
})

# Colour palette
C_CALLDATA  = "#34495e"  # Slate
C_BLOB      = "#3498db"  # Blue
C_OFFCHAIN  = "#2ecc71"  # Green
C_FIXED     = "#34495e"  # Slate
C_ADAPTIVE  = "#3498db"  # Blue
C_DPDAB     = "#e67e22"  # Orange

# ── Resolve CSV ──────────────────────────────────────────────────────
if len(sys.argv) > 1:
    csv_path = sys.argv[1]
else:
    # Try default locations
    candidates = [
        os.path.join(os.path.dirname(__file__), "..", "Metrics", "consolidated_results.csv"),
        r"C:\Users\malin\Desktop\rollupx-full-zk-rollup\Metrics\consolidated_results.csv",
        r"C:\Users\malin\.gemini\antigravity\brain\d8962380-1b1b-449a-b9b2-e12c0aa85276\consolidated_results.csv",
    ]
    csv_path = None
    for c in candidates:
        if os.path.exists(c):
            csv_path = c
            break
    if csv_path is None:
        print("ERROR: No consolidated_results.csv found. Pass path as argument.")
        sys.exit(1)

df = pd.read_csv(csv_path)
print(f"Loaded {len(df)} rows from {csv_path}")

# Output directories
figures_dir = os.path.join(os.path.dirname(csv_path), "figures")
brain_dir = r"C:\Users\malin\.gemini\antigravity\brain\d8962380-1b1b-449a-b9b2-e12c0aa85276\figures"
os.makedirs(figures_dir, exist_ok=True)
os.makedirs(brain_dir, exist_ok=True)


def save_fig(fig, name):
    for d in (figures_dir, brain_dir):
        fig.savefig(os.path.join(d, name), dpi=300, bbox_inches="tight")
    print(f"  Saved: {name}")


# =====================================================================
# PLOT A: Gas/Tx vs Batch Size by DA Mode
# =====================================================================
print("\n-- Plot A: Gas/Tx vs Batch Size by DA Mode --")

# Strategy: use BOTH the new cmp_da_* experiments AND existing s1_bs_* / s4_da_* data
# 1) New comparison sweep data (cmp_da_*)
# 2) Existing Stage 1 batch-size sweep (calldata only)
# 3) Existing Stage 4 DA points

fig, ax = plt.subplots(figsize=(10, 6))

da_configs = {
    "calldata": {"color": C_CALLDATA, "marker": "o", "label": "Calldata DA"},
    "blob":     {"color": C_BLOB,     "marker": "D", "label": "Blob DA (EIP-4844)"},
    "offchain": {"color": C_OFFCHAIN, "marker": "s", "label": "Offchain DA"},
}

plotted_any = False

for da_mode, style in da_configs.items():
    # Try new comparison sweep data first
    mask_cmp = df["experiment_id"].str.startswith(f"cmp_da_{da_mode}_bs")
    df_cmp = df[mask_cmp].sort_values("avg_batch_size")

    if not df_cmp.empty:
        # Use regular gas for calldata, total (regular+blob) for blob
        gas_col = "true_weighted_gas_per_tx"
        ax.plot(
            df_cmp["avg_batch_size"],
            df_cmp[gas_col],
            marker=style["marker"],
            color=style["color"],
            linewidth=2.5,
            markersize=8,
            label=style["label"],
            zorder=3,
        )
        plotted_any = True
    else:
        # Fallback: for calldata use Stage 1 batch-size sweep
        if da_mode == "calldata":
            mask_s1 = df["experiment_id"].str.contains("s1_bs_|baseline")
            df_s1 = df[mask_s1].sort_values("avg_batch_size")
            if not df_s1.empty:
                ax.plot(
                    df_s1["avg_batch_size"],
                    df_s1["true_weighted_gas_per_tx"],
                    marker=style["marker"],
                    color=style["color"],
                    linewidth=2.5,
                    markersize=8,
                    label=style["label"] + " (Stage 1)",
                    zorder=3,
                )
                plotted_any = True

        # Fallback: single-point from Stage 4
        mask_s4 = df["experiment_id"] == f"s4_da_{da_mode}"
        df_s4 = df[mask_s4]
        if not df_s4.empty:
            row = df_s4.iloc[0]
            ax.scatter(
                row["avg_batch_size"],
                row["true_weighted_gas_per_tx"],
                color=style["color"],
                marker=style["marker"],
                s=120,
                zorder=4,
                edgecolors="white",
                linewidth=1.5,
                label=style["label"] + " (Stage 4 point)",
            )
            plotted_any = True

if plotted_any:
    ax.set_title("L1 Gas per Transaction vs. Batch Size by DA Mode",
                 fontsize=14, fontweight="bold", pad=15)
    ax.set_xlabel("Average Batch Size (Transactions)", fontsize=12)
    ax.set_ylabel("L1 Gas per Transaction", fontsize=12)
    ax.set_xscale("log")
    ax.xaxis.set_major_formatter(mticker.ScalarFormatter())
    ax.xaxis.set_minor_formatter(mticker.NullFormatter())
    ax.set_xticks([25, 50, 100, 200, 500])
    ax.get_xaxis().set_major_formatter(mticker.ScalarFormatter())
    ax.legend(frameon=True, facecolor="white", edgecolor="none", fontsize=11)
    ax.grid(True, alpha=0.3)
    save_fig(fig, "plot_da_gas_per_tx_vs_batch_size.png")
else:
    print("  WARNING: No data found for DA gas sweep. Run the benchmark first.")
plt.close()


# =====================================================================
# PLOT B: All Batch Policies Comparison — Wait Time & Gas Cost
# =====================================================================
print("\n-- Plot B: Batch Policy Comparison (Wait Time & Gas Cost) --")

# Strategy: use cmp_pol_* data, fallback to s2_* data
load_levels = ["low", "medium", "high", "burst"]
policy_names = ["fixed", "adaptive", "dpdab"]
policy_labels = {"fixed": "Fixed", "adaptive": "Adaptive", "dpdab": "DPDAB"}
policy_colors_wait = {"fixed": C_FIXED, "adaptive": C_ADAPTIVE, "dpdab": C_DPDAB}
policy_colors_gas  = {"fixed": "#576574", "adaptive": "#74b9ff", "dpdab": "#fab1a0"}

# Collect data
data_wait = {p: [] for p in policy_names}
data_gas  = {p: [] for p in policy_names}
valid_loads = []

for load in load_levels:
    has_all = True
    for pol in policy_names:
        # Try new comparison data
        exp_id = f"cmp_pol_{pol}_{load}"
        row = df[df["experiment_id"] == exp_id]

        if row.empty and pol == "fixed":
            row = df[df["experiment_id"] == f"s2_fixed_{load}"]
        if row.empty and pol == "adaptive":
            row = df[df["experiment_id"] == f"s2_adaptive_{load}"]

        if row.empty:
            has_all = False
            data_wait[pol].append(0)
            data_gas[pol].append(0)
        else:
            data_wait[pol].append(row.iloc[0]["true_weighted_wait_time_ms"])
            data_gas[pol].append(row.iloc[0]["true_weighted_gas_per_tx"])
    valid_loads.append(load.title())

if any(any(v > 0 for v in data_wait[p]) for p in policy_names):
    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(16, 6))

    x = np.arange(len(valid_loads))
    n_policies = len(policy_names)
    width = 0.25

    # Subplot 1: Wait Time
    for i, pol in enumerate(policy_names):
        offset = (i - (n_policies - 1) / 2) * width
        bars = ax1.bar(
            x + offset,
            data_wait[pol],
            width,
            label=policy_labels[pol],
            color=policy_colors_wait[pol],
        )
        for bar in bars:
            h = bar.get_height()
            if h > 0:
                ax1.annotate(
                    f"{h:.0f}",
                    xy=(bar.get_x() + bar.get_width() / 2, h),
                    xytext=(0, 3),
                    textcoords="offset points",
                    ha="center", va="bottom", fontsize=8, fontweight="bold",
                )

    ax1.set_ylabel("Average Mempool Wait Time (ms)", fontsize=12, fontweight="bold")
    ax1.set_title("Mempool Queue Latency", fontsize=13, fontweight="bold", pad=12)
    ax1.set_xticks(x)
    ax1.set_xticklabels(valid_loads, fontsize=11, fontweight="bold")
    ax1.legend(frameon=True, facecolor="white", edgecolor="none")
    max_wait = max(max(data_wait[p]) for p in policy_names)
    ax1.set_ylim(0, max_wait * 1.2 if max_wait > 0 else 100)

    # Subplot 2: Gas Cost
    for i, pol in enumerate(policy_names):
        offset = (i - (n_policies - 1) / 2) * width
        bars = ax2.bar(
            x + offset,
            data_gas[pol],
            width,
            label=policy_labels[pol],
            color=policy_colors_gas[pol],
        )
        for bar in bars:
            h = bar.get_height()
            if h > 0:
                ax2.annotate(
                    f"{h:,.0f}",
                    xy=(bar.get_x() + bar.get_width() / 2, h),
                    xytext=(0, 3),
                    textcoords="offset points",
                    ha="center", va="bottom", fontsize=8, fontweight="bold",
                )

    ax2.set_ylabel("L1 Gas per Transaction", fontsize=12, fontweight="bold")
    ax2.set_title("L1 Gas Cost", fontsize=13, fontweight="bold", pad=12)
    ax2.set_xticks(x)
    ax2.set_xticklabels(valid_loads, fontsize=11, fontweight="bold")
    ax2.legend(frameon=True, facecolor="white", edgecolor="none")
    max_gas = max(max(data_gas[p]) for p in policy_names)
    ax2.set_ylim(0, max_gas * 1.15 if max_gas > 0 else 25000)

    fig.suptitle(
        "Batch Policy Comparison: Fixed vs. Adaptive vs. DPDAB",
        fontsize=15, fontweight="bold", y=0.98,
    )
    plt.tight_layout()
    save_fig(fig, "plot_batch_policy_comparison.png")
    plt.close()
else:
    print("  WARNING: No batch policy comparison data found.")
    print("  Run: python scripts/run_new_comparison_benchmark.py --group policy_compare")

print("\nDone.")
