import argparse
import os
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

def _save(fig, path: str):
    fig.tight_layout()
    fig.savefig(path, dpi=150)
    plt.close(fig)
    print(f"[final_report_graphs] saved -> {path}")

def plot_scatter_line(df: pd.DataFrame, x_col: str, y_col: str, output_dir: str, title: str, xlabel: str, ylabel: str, filename: str):
    if x_col not in df.columns or y_col not in df.columns:
        return
    clean = df[[x_col, y_col]].dropna()
    if clean.empty:
        return
    
    # Aggregate by x_col if there are multiple runs for the same parameter
    agg = clean.groupby(x_col)[y_col].agg(['mean', 'std']).reset_index().sort_values(by=x_col)
    
    fig, ax = plt.subplots(figsize=(8, 5))
    ax.errorbar(agg[x_col], agg['mean'], yerr=agg['std'].fillna(0), fmt='-o', capsize=4, alpha=0.85, color="#1f77b4")
    ax.set_xlabel(xlabel)
    ax.set_ylabel(ylabel)
    ax.set_title(title)
    ax.grid(True, alpha=0.3)
    _save(fig, os.path.join(output_dir, filename))

def plot_mempool_backlog(batch_df: pd.DataFrame, output_dir: str):
    if "mempool_depth_at_batch" not in batch_df.columns or "run_id" not in batch_df.columns:
        return
    
    # We can plot for a few runs, maybe those with "burst" in the run_id or just take a sample
    burst_runs = batch_df[batch_df['run_id'].str.contains("burst", case=False, na=False)]['run_id'].unique()
    if len(burst_runs) == 0:
        # fallback to any run
        burst_runs = batch_df['run_id'].unique()[:3]
    
    if len(burst_runs) == 0:
        return
    
    fig, ax = plt.subplots(figsize=(10, 5))
    for run_id in burst_runs[:5]:
        run_data = batch_df[batch_df['run_id'] == run_id].reset_index()
        ax.plot(run_data.index, run_data['mempool_depth_at_batch'], label=f"Run {run_id}", alpha=0.8)
    
    ax.set_xlabel("Batch Sequence Number")
    ax.set_ylabel("Mempool Depth")
    ax.set_title("Mempool Backlog Over Time (Burst Workload)")
    ax.legend(fontsize=8)
    ax.grid(True, alpha=0.3)
    _save(fig, os.path.join(output_dir, "stage2_burst_backlog_recovery.png"))

def plot_payload_vs_blob_utilization(batch_df: pd.DataFrame, output_dir: str):
    if "estimated_batch_bytes" not in batch_df.columns or "blob_utilization_submitter" not in batch_df.columns:
        return
    clean = batch_df[["estimated_batch_bytes", "blob_utilization_submitter"]].dropna()
    if clean.empty:
        return
    
    fig, ax = plt.subplots(figsize=(8, 5))
    ax.scatter(clean["estimated_batch_bytes"], clean["blob_utilization_submitter"], s=15, alpha=0.6, color="#2ca02c")
    ax.set_xlabel("Estimated Batch Payload (bytes)")
    ax.set_ylabel("Blob Fill Ratio")
    ax.set_title("Batch Payload Size vs Blob Fill Ratio")
    ax.grid(True, alpha=0.3)
    _save(fig, os.path.join(output_dir, "stage4_batch_payload_vs_blob_fill_ratio.png"))

def plot_blob_fill_target(df: pd.DataFrame, output_dir: str):
    if "blob_fill_target" not in df.columns or "avg_cost_per_tx_usd" not in df.columns or "p95_l2_l1_ms" not in df.columns:
        return
    
    clean = df[["blob_fill_target", "avg_cost_per_tx_usd", "p95_l2_l1_ms"]].dropna()
    if clean.empty or clean["blob_fill_target"].nunique() <= 1:
        return
    
    agg = clean.groupby("blob_fill_target")[["avg_cost_per_tx_usd", "p95_l2_l1_ms"]].mean().reset_index().sort_values("blob_fill_target")
    
    fig, ax1 = plt.subplots(figsize=(8, 5))
    ax2 = ax1.twinx()
    
    ax1.plot(agg["blob_fill_target"], agg["avg_cost_per_tx_usd"], '-o', color="#d62728", label="Cost/tx (USD)")
    ax2.plot(agg["blob_fill_target"], agg["p95_l2_l1_ms"], '-s', color="#1f77b4", label="P95 Latency (ms)")
    
    ax1.set_xlabel("Blob Fill Target")
    ax1.set_ylabel("Cost per TX (USD)", color="#d62728")
    ax2.set_ylabel("P95 Latency (ms)", color="#1f77b4")
    
    ax1.tick_params(axis='y', labelcolor="#d62728")
    ax2.tick_params(axis='y', labelcolor="#1f77b4")
    
    plt.title("Blob Fill Target vs P95 Latency and Cost/tx")
    ax1.grid(True, alpha=0.3)
    
    fig.tight_layout()
    _save(fig, os.path.join(output_dir, "stage4_blob_fill_target_tradeoff.png"))

def plot_blobpacking_vs_fcfs(df: pd.DataFrame, output_dir: str):
    if "policy" not in df.columns or "avg_blob_utilization" not in df.columns:
        return
    
    mask = df["policy"].isin(["FCFS", "BlobPacking"])
    clean = df[mask][["policy", "avg_blob_utilization"]].dropna()
    if clean.empty:
        return
    
    agg = clean.groupby("policy")["avg_blob_utilization"].agg(['mean', 'std']).reset_index()
    
    fig, ax = plt.subplots(figsize=(6, 5))
    bars = ax.bar(agg["policy"], agg["mean"], yerr=agg["std"].fillna(0), capsize=5, color=["#1f77b4", "#ff7f0e"], alpha=0.8)
    ax.bar_label(bars, fmt="%.3f", padding=3)
    
    ax.set_xlabel("Sequencing Policy")
    ax.set_ylabel("Average Blob Utilization")
    ax.set_title("Blob Utilization: BlobPacking vs FCFS")
    ax.set_ylim(0, 1.1)
    ax.grid(axis='y', alpha=0.3)
    _save(fig, os.path.join(output_dir, "stage4_blobpacking_vs_fcfs_utilization.png"))

def main():
    parser = argparse.ArgumentParser(description="Generate final report graphs")
    parser.add_argument("--results", required=True, help="all_results.csv path")
    parser.add_argument("--batch_results", required=False, help="all_batch_results.csv path")
    parser.add_argument("--output_dir", required=True, help="Output directory")
    args = parser.parse_args()
    
    os.makedirs(args.output_dir, exist_ok=True)
    
    if os.path.exists(args.results):
        df = pd.read_csv(args.results)
        
        # 2. Batch size vs gas/tx
        plot_scatter_line(df, "batch_size", "avg_gas_per_tx", args.output_dir, 
                          "Batch Size vs Gas per TX", "Max Batch Size", "Avg Gas / TX", 
                          "stage1_batch_size_vs_gas_per_tx.png")
        
        # 3. Batch size vs proof time
        plot_scatter_line(df, "batch_size", "avg_prove_ms", args.output_dir, 
                          "Batch Size vs Proof Time", "Max Batch Size", "Avg Proof Time (ms)", 
                          "stage5_batch_size_vs_proof_time.png")
                          
        # 4. Traffic rate vs goodput
        plot_scatter_line(df, "rate_tps", "goodput_tps", args.output_dir, 
                          "Traffic Rate vs Goodput", "Offered Rate (TPS)", "Goodput (TPS)", 
                          "stage1_traffic_rate_vs_goodput.png")
        
        # 8. Blob fill target vs P95 latency and cost/tx
        plot_blob_fill_target(df, args.output_dir)
        
        # 13. BlobPacking vs FCFS blob utilization
        plot_blobpacking_vs_fcfs(df, args.output_dir)
        
        # 14. Publish timeout vs failed batch count
        plot_scatter_line(df, "sequencer_executor_publish_timeout_ms", "failed_batches", args.output_dir, 
                          "Publish Timeout vs Failed Batches", "Timeout (ms)", "Failed Batch Count", 
                          "stage7_timeout_vs_failed_batches.png")
                          
        # 15. Retry count vs recovery success rate
        plot_scatter_line(df, "sequencer_executor_publish_retries", "run_success_rate", args.output_dir, 
                          "Retry Count vs Success Rate", "Max Retries", "Run Success Rate", 
                          "stage7_retries_vs_success_rate.png")
                          
        # 16. Mining interval vs hard finality latency
        plot_scatter_line(df, "hardhat_mining_interval", "avg_hard_finality_ms", args.output_dir, 
                          "L1 Mining Interval vs Hard Finality", "Mining Interval (ms)", "P95 Hard Finality Latency (ms)", 
                          "stage6_mining_interval_vs_hard_finality.png")

    if args.batch_results and os.path.exists(args.batch_results):
        batch_df = pd.read_csv(args.batch_results)
        
        # 5. Mempool backlog over time
        plot_mempool_backlog(batch_df, args.output_dir)
        
        # 7. Batch payload vs blob fill ratio
        plot_payload_vs_blob_utilization(batch_df, args.output_dir)

if __name__ == "__main__":
    main()
