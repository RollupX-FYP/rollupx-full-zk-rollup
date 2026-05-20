#!/usr/bin/env python3
"""
Generate benchmark plots for all 5 stages of the RollupX performance analysis.
"""

from __future__ import annotations
import argparse
import json
import re
from pathlib import Path
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

# Set academic/professional plotting defaults
plt.rcParams.update({
    'font.family': 'sans-serif',
    'font.size': 11,
    'axes.labelsize': 12,
    'axes.titlesize': 13,
    'xtick.labelsize': 10,
    'ytick.labelsize': 10,
    'figure.titlesize': 14,
    'legend.fontsize': 10,
    'grid.alpha': 0.3,
    'grid.linestyle': '--'
})

# Curated modern palette
COLORS = {
    'primary': '#1f77b4',     # Classic Blue
    'accent': '#ff7f0e',      # Classic Orange
    'success': '#2ca02c',     # Safe Green
    'danger': '#d62728',      # Warning Red
    'purple': '#9467bd',      # Deep Purple
    'brown': '#8c564b',       # Muted Brown
    'grey': '#7f7f7f',        # Cool Grey
    'fixed': '#4a90e2',       # Sky Blue for Fixed Batching
    'adaptive': '#f5a623'     # Amber for Adaptive Batching
}

def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate benchmark plots for RollupX Stages 1-5.")
    parser.add_argument(
        "--metrics-root",
        type=Path,
        default=Path("benchmark-suite/metrics"),
        help="Path to the metrics folder containing stage directories"
    )
    return parser.parse_args()

def read_jsonl(path: Path) -> list[dict]:
    rows: list[dict] = []
    if not path.exists():
        return rows
    with path.open("r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                try:
                    rows.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
    return rows

def ensure_dir(path: Path) -> Path:
    path.mkdir(parents=True, exist_ok=True)
    return path

# ==========================================
# STAGE 1: FIXED BATCHING
# ==========================================
def plot_stage1(metrics_root: Path, out_dir: Path) -> None:
    stage_dir = metrics_root / "final_stage1_fixed_batching"
    analysis_dir = stage_dir / "analysis"
    if not analysis_dir.exists():
        print(f"Skipping Stage 1: {analysis_dir} not found.")
        return
    
    print("Plotting Stage 1...")

    all_results_path = analysis_dir / "all_results.csv"
    all_batch_path = analysis_dir / "all_batch_results.csv"

    if all_results_path.exists():
        df_res = pd.read_csv(all_results_path)

        # Plot 1: Batch Size vs. Queue Latency & Gas Cost (Dual Axis)
        # Filter for s1_bs_* runs
        bs_df = df_res[df_res['experiment_id'].str.match(r'^s1_bs_\d+$', na=False)].copy()
        if not bs_df.empty:
            bs_df['batch_size_val'] = bs_df['experiment_id'].str.extract(r's1_bs_(\d+)').astype(int)
            bs_df = bs_df.sort_values('batch_size_val')

            fig, ax1 = plt.subplots(figsize=(9, 5))
            color = COLORS['primary']
            ax1.set_xlabel('Configured Batch Size (txs)')
            ax1.set_ylabel('Avg Queue Wait Time (ms)', color=color)
            line1 = ax1.plot(bs_df['batch_size_val'], bs_df['avg_queue_wait_ms'], marker='o', color=color, label='Avg Queue Wait Time')
            ax1.tick_params(axis='y', labelcolor=color)
            ax1.grid(True)

            ax2 = ax1.twinx()
            color = COLORS['accent']
            ax2.set_ylabel('Avg L1 Gas per Tx', color=color)
            line2 = ax2.plot(bs_df['batch_size_val'], bs_df['avg_gas_per_tx'], marker='s', linestyle='--', color=color, label='Avg L1 Gas/Tx')
            ax2.tick_params(axis='y', labelcolor=color)

            # Added legends
            lines = line1 + line2
            labels = [l.get_label() for l in lines]
            ax1.legend(lines, labels, loc='upper center')
            plt.title('Stage 1: Batch Size vs. Queue Latency & Gas Cost')
            plt.tight_layout()
            plt.savefig(out_dir / 's1_batch_size_vs_latency_gas.png', dpi=160)
            plt.close()

        # Plot 2: Timeout vs. Queue Latency & Batch Occupancy (Dual Axis)
        # Filter for s1_to_* runs
        to_df = df_res[df_res['experiment_id'].str.match(r'^s1_to_\d+$', na=False)].copy()
        if not to_df.empty:
            to_df['timeout_val'] = to_df['experiment_id'].str.extract(r's1_to_(\d+)').astype(int)
            to_df = to_df.sort_values('timeout_val')

            fig, ax1 = plt.subplots(figsize=(9, 5))
            color = COLORS['primary']
            ax1.set_xlabel('Timeout (ms)')
            ax1.set_ylabel('Avg Queue Wait Time (ms)', color=color)
            line1 = ax1.plot(to_df['timeout_val'], to_df['avg_queue_wait_ms'], marker='o', color=color, label='Avg Queue Wait Time')
            ax1.tick_params(axis='y', labelcolor=color)
            ax1.grid(True)

            ax2 = ax1.twinx()
            color = COLORS['success']
            ax2.set_ylabel('Avg Batch Occupancy (txs)', color=color)
            line2 = ax2.plot(to_df['timeout_val'], to_df['avg_batch_tx_count'], marker='^', linestyle='-.', color=color, label='Avg Batch Occupancy')
            ax2.tick_params(axis='y', labelcolor=color)

            lines = line1 + line2
            labels = [l.get_label() for l in lines]
            ax1.legend(lines, labels, loc='lower right')
            plt.title('Stage 1: Timeout vs. Queue Latency & Batch Occupancy')
            plt.tight_layout()
            plt.savefig(out_dir / 's1_timeout_vs_latency_occupancy.png', dpi=160)
            plt.close()

        # Plot 4: Workload Mix vs. Gas per Transaction (Bar Chart)
        # Filter for s1_mix_* and baseline (balanced mix)
        mix_ids = ['s1_mix_daheavy', 's1_mix_exeheavy', 's1_mix_balanced']
        # If s1_mix_balanced is missing, look for 'baseline'
        mix_df = df_res[df_res['experiment_id'].isin(mix_ids) | (df_res['experiment_id'] == 'baseline')].copy()
        if not mix_df.empty:
            # Map names
            name_map = {
                's1_mix_daheavy': 'DA Heavy',
                's1_mix_exeheavy': 'Execution Heavy',
                's1_mix_balanced': 'Balanced (25 TPS)',
                'baseline': 'Balanced (25 TPS)'
            }
            mix_df['label'] = mix_df['experiment_id'].map(name_map)
            mix_df = mix_df.drop_duplicates(subset=['label'])
            
            plt.figure(figsize=(8, 5))
            colors = [COLORS['danger'], COLORS['purple'], COLORS['primary']]
            plt.bar(mix_df['label'], mix_df['avg_gas_per_tx'], color=colors, width=0.5, edgecolor='black', alpha=0.85)
            plt.ylabel('Average L1 Gas per Transaction')
            plt.xlabel('Workload Profile')
            plt.title('Stage 1: L1 Gas Cost by Workload Type')
            plt.grid(axis='y', linestyle='--', alpha=0.5)
            plt.tight_layout()
            plt.savefig(out_dir / 's1_workload_mix_vs_gas.png', dpi=160)
            plt.close()

    # Plot 3: Queue Wait Time Distribution (Box Plot)
    if all_batch_path.exists():
        df_batch = pd.read_csv(all_batch_path)
        box_targets = ['s1_bs_0010', 's1_bs_0050', 's1_bs_0100', 's1_bs_0250', 's1_bs_0500']
        df_box = df_batch[df_batch['experiment_id'].isin(box_targets)].copy()
        if not df_box.empty:
            # Clean and map x labels
            df_box['size_num'] = df_box['experiment_id'].str.extract(r's1_bs_(\d+)').astype(int)
            df_box = df_box.sort_values('size_num')
            
            unique_sizes = sorted(df_box['size_num'].unique())
            data_groups = [df_box[df_box['size_num'] == sz]['wait_time_mean_ms'].dropna().values for sz in unique_sizes]
            
            plt.figure(figsize=(9, 5))
            plt.boxplot(data_groups, tick_labels=[f"BS {sz}" for sz in unique_sizes], patch_artist=True,
                        boxprops=dict(facecolor=COLORS['primary'], color='black', alpha=0.7),
                        medianprops=dict(color='red', linewidth=1.5))
            plt.ylabel('Mean Queue Wait Time per Batch (ms)')
            plt.xlabel('Configured Batch Size Limit')
            plt.title('Stage 1: Queue Wait Time Distribution by Batch Size')
            plt.grid(axis='y')
            plt.tight_layout()
            plt.savefig(out_dir / 's1_queue_wait_distribution.png', dpi=160)
            plt.close()

# ==========================================
# STAGE 2: ADAPTIVE BATCHING
# ==========================================
def plot_stage2(metrics_root: Path, out_dir: Path) -> None:
    stage_dir = metrics_root / "final_stage2_adaptive_batching"
    analysis_dir = stage_dir / "analysis"
    if not analysis_dir.exists():
        print(f"Skipping Stage 2: {analysis_dir} not found.")
        return
    
    print("Plotting Stage 2...")

    all_results_path = analysis_dir / "all_results.csv"
    all_batch_path = analysis_dir / "all_batch_results.csv"

    # Plot 1: Policy Performance Convergence under Load (Grouped Bar Chart)
    if all_results_path.exists():
        df_res = pd.read_csv(all_results_path)
        
        # Load Levels:
        # Low (10 TPS) offered: compare s2_fixed_low vs s2_adaptive_low
        # Med (25 TPS) offered: compare s2_fixed_medium vs s2_adaptive_medium
        # High (60 TPS) offered: compare s2_fixed_high vs s2_adaptive_high
        
        mapping = {
            's2_fixed_low': ('Low (10 TPS)', 'Fixed'),
            's2_adaptive_low': ('Low (10 TPS)', 'Adaptive'),
            's2_fixed_medium': ('Medium (25 TPS)', 'Fixed'),
            's2_adaptive_medium': ('Medium (25 TPS)', 'Adaptive'),
            's2_fixed_high': ('High (60 TPS)', 'Fixed'),
            's2_adaptive_high': ('High (60 TPS)', 'Adaptive')
        }
        
        comp_df = df_res[df_res['experiment_id'].isin(mapping.keys())].copy()
        if not comp_df.empty:
            comp_df['load'] = comp_df['experiment_id'].map(lambda x: mapping[x][0])
            comp_df['policy'] = comp_df['experiment_id'].map(lambda x: mapping[x][1])
            
            loads = ['Low (10 TPS)', 'Medium (25 TPS)', 'High (60 TPS)']
            
            # Pivot table for plotting
            pivot_latency = comp_df.pivot(index='load', columns='policy', values='avg_queue_wait_ms').reindex(loads)
            pivot_gas = comp_df.pivot(index='load', columns='policy', values='avg_gas_per_tx').reindex(loads)
            
            x = np.arange(len(loads))
            width = 0.35
            
            # Subplots side-by-side
            fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(14, 5.5))
            
            # Left: Latency
            ax1.bar(x - width/2, pivot_latency['Fixed'], width, label='Fixed (BS=100)', color=COLORS['fixed'], edgecolor='black', alpha=0.85)
            ax1.bar(x + width/2, pivot_latency['Adaptive'], width, label='Adaptive', color=COLORS['adaptive'], edgecolor='black', alpha=0.85)
            ax1.set_ylabel('Avg Queue Wait Time (ms)')
            ax1.set_title('Average Latency vs Offered Load')
            ax1.set_xticks(x)
            ax1.set_xticklabels(loads)
            ax1.legend()
            ax1.grid(axis='y')
            
            # Right: Gas
            ax2.bar(x - width/2, pivot_gas['Fixed'], width, label='Fixed (BS=100)', color=COLORS['fixed'], edgecolor='black', alpha=0.85)
            ax2.bar(x + width/2, pivot_gas['Adaptive'], width, label='Adaptive', color=COLORS['adaptive'], edgecolor='black', alpha=0.85)
            ax2.set_ylabel('Average L1 Gas per Tx')
            ax2.set_title('Average Gas Cost vs Offered Load')
            ax2.set_xticks(x)
            ax2.set_xticklabels(loads)
            ax2.legend()
            ax2.grid(axis='y')
            
            plt.suptitle('Stage 2: Fixed vs. Adaptive Batching Comparison')
            plt.tight_layout()
            plt.savefig(out_dir / 's2_policy_performance.png', dpi=160)
            plt.close()

    # Plot 2: Burst Load Batch Size and Trigger Reason Timeline (Scatter Plot)
    if all_batch_path.exists():
        df_batch = pd.read_csv(all_batch_path)
        burst_df = df_batch[df_batch['experiment_id'] == 's2_adaptive_burst'].copy()
        if not burst_df.empty:
            burst_df = burst_df.sort_values('batch_id')
            
            plt.figure(figsize=(10, 5))
            
            # Color by seal reason
            timeout_batches = burst_df[burst_df['seal_reason'] == 'Timeout']
            size_batches = burst_df[burst_df['seal_reason'] == 'SizeThreshold']
            
            plt.scatter(timeout_batches['batch_id'], timeout_batches['tx_count'], 
                        color=COLORS['danger'], marker='o', s=80, label='Timeout Seal', edgecolor='black', alpha=0.8)
            plt.scatter(size_batches['batch_id'], size_batches['tx_count'], 
                        color=COLORS['success'], marker='s', s=80, label='Size Seal', edgecolor='black', alpha=0.8)
            
            # Draw horizontal threshold reference lines
            plt.axhline(25, color='gray', linestyle=':', alpha=0.7, label='Small BS Target (25)')
            plt.axhline(100, color='gray', linestyle='--', alpha=0.7, label='Medium BS Target (100)')
            plt.axhline(500, color='gray', linestyle='-.', alpha=0.7, label='Large BS Target (500)')
            
            plt.xlabel('Timeline (Batch ID)')
            plt.ylabel('Batch Size (tx_count)')
            plt.title('Stage 2: Adaptive Batch Size & Trigger Timeline (Burst Load)')
            plt.legend(loc='upper right')
            plt.grid(True, alpha=0.3)
            plt.tight_layout()
            plt.savefig(out_dir / 's2_burst_timeline.png', dpi=160)
            plt.close()

    # Plot 3: The Hysteresis Trap Step-Function (Conceptual/Line Plot)
    # Generate data mathematically
    pending_txs = np.linspace(0, 150, 500)
    
    # Case A: Hysteresis Trap (S_b = 25, L_t = 25)
    target_trap = np.zeros_like(pending_txs)
    target_trap[pending_txs < 25] = 25
    target_trap[(pending_txs >= 25) & (pending_txs < 100)] = 100
    target_trap[pending_txs >= 100] = 500
    
    # Case B: Fixed/Corrected (S_b = 10, L_t = 25)
    target_fixed = np.zeros_like(pending_txs)
    target_fixed[pending_txs < 25] = 10
    target_fixed[(pending_txs >= 25) & (pending_txs < 100)] = 100
    target_fixed[pending_txs >= 100] = 500

    plt.figure(figsize=(9, 5.5))
    plt.plot(pending_txs, target_trap, label='Adaptive Target (Sb=25, Lt=25) - TRAPPED', color=COLORS['danger'], linewidth=2.5)
    plt.plot(pending_txs, target_fixed, label='Adaptive Target (Sb=10, Lt=25) - CORRECTED', color=COLORS['success'], linewidth=2, linestyle='--')
    plt.plot(pending_txs, pending_txs, label='Pending Transactions (T = P)', color='gray', linestyle=':', linewidth=1.5)
    
    # Shade the trap zone
    plt.fill_between(pending_txs, pending_txs, target_trap, 
                     where=(pending_txs < 25), color='red', alpha=0.15, label='Hysteresis Trap Zone')
    
    plt.xlabel('Pending Transactions in Mempool ($P$)')
    plt.ylabel('Target Batch Size ($T(P)$)')
    plt.title('Stage 2 Theory: The Hysteresis Trap Step-Function')
    plt.ylim(0, 160)
    plt.xlim(0, 150)
    plt.legend(loc='upper left')
    plt.grid(True, alpha=0.3)
    plt.tight_layout()
    plt.savefig(out_dir / 's2_hysteresis_trap.png', dpi=160)
    plt.close()

# ==========================================
# STAGE 3: SEQUENCER POLICY
# ==========================================
def plot_stage3(metrics_root: Path, out_dir: Path) -> None:
    stage_dir = metrics_root / "final_stage3_policy"
    analysis_dir = stage_dir / "analysis"
    if not analysis_dir.exists():
        print(f"Skipping Stage 3: {analysis_dir} not found.")
        return
    
    print("Plotting Stage 3...")

    all_results_path = analysis_dir / "all_results.csv"
    
    if all_results_path.exists():
        df_res = pd.read_csv(all_results_path)

        # Plot 1: Gas Efficiency vs. Policy Nonce Safety (Bar Chart)
        policy_ids = ['s3_pol_fcfs', 's3_pol_fairbft', 's3_pol_feepriority', 's3_pol_timeboost', 's3_pol_blobpacking']
        pol_df = df_res[df_res['experiment_id'].isin(policy_ids)].copy()
        if not pol_df.empty:
            pol_map = {
                's3_pol_fcfs': 'FCFS\n(Safe)',
                's3_pol_fairbft': 'FairBFT\n(Safe)',
                's3_pol_feepriority': 'FeePriority\n(UNSAFE)',
                's3_pol_timeboost': 'TimeBoost\n(UNSAFE)',
                's3_pol_blobpacking': 'BlobPacking\n(UNSAFE)'
            }
            pol_df['label'] = pol_df['experiment_id'].map(pol_map)
            
            # Success Rate Column
            pol_df['success_rate'] = (pol_df['success_txs'] / pol_df['total_txs']) * 100
            
            # Sort to keep standard order
            pol_df['order'] = pol_df['experiment_id'].map({p: i for i, p in enumerate(policy_ids)})
            pol_df = pol_df.sort_values('order')
            
            fig, ax1 = plt.subplots(figsize=(10, 5.5))
            
            # Bar Colors based on Safety
            colors = [COLORS['success'], COLORS['success'], COLORS['danger'], COLORS['danger'], COLORS['danger']]
            
            # Gas on primary axis
            bars = ax1.bar(pol_df['label'], pol_df['avg_gas_per_tx'], color=colors, width=0.5, edgecolor='black', alpha=0.8)
            ax1.set_ylabel('Average L1 Gas per Tx (Gas)')
            ax1.set_xlabel('Sequencing Policy')
            ax1.grid(axis='y')
            
            # Add stripes to unsafe policies
            for i in range(2, 5):
                bars[i].set_hatch('//')
            
            # Success Rate on secondary axis
            ax2 = ax1.twinx()
            line = ax2.plot(pol_df['label'], pol_df['success_rate'], color='blue', marker='o', linewidth=2, label='Success Rate')
            ax2.set_ylabel('Transaction Success Rate (%)')
            ax2.set_ylim(-5, 105)
            ax2.tick_params(axis='y', labelcolor='blue')
            
            plt.title('Stage 3: Gas Efficiency & Nonce Safety by Policy')
            plt.tight_layout()
            plt.savefig(out_dir / 's3_gas_efficiency_vs_safety.png', dpi=160)
            plt.close()

        # Plot 2: Jain's Fairness Index under Burst Workloads (Grouped Bar Chart)
        burst_ids = ['s3_burst_fairbft', 's3_burst_feepriority', 's3_burst_timeboost']
        burst_df = df_res[df_res['experiment_id'].isin(burst_ids)].copy()
        if not burst_df.empty:
            burst_map = {
                's3_burst_fairbft': 'FairBFT',
                's3_burst_feepriority': 'FeePriority',
                's3_burst_timeboost': 'TimeBoost'
            }
            burst_df['label'] = burst_df['experiment_id'].map(burst_map)
            burst_df = burst_df.sort_values('label')
            
            plt.figure(figsize=(8, 5))
            colors = [COLORS['success'], COLORS['danger'], COLORS['purple']]
            plt.bar(burst_df['label'], burst_df['jains_fairness'], color=colors, width=0.4, edgecolor='black', alpha=0.85)
            plt.ylabel("Jain's Fairness Index")
            plt.xlabel('Policy under Burst Workload')
            plt.title("Stage 3: Jain's Fairness Index under Burst Workloads")
            plt.ylim(0, 1.05)
            plt.grid(axis='y')
            plt.tight_layout()
            plt.savefig(out_dir / 's3_jains_fairness_burst.png', dpi=160)
            plt.close()

    # Plot 3: Executor Execution Time vs. Successfully Executed Transactions (Scatter Plot)
    # Search for executor metrics JSONLs for FCFS and FeePriority
    fcfs_glob = list(stage_dir.glob("s3_pol_fcfs/*/executor_batch_metrics.jsonl"))
    feeprio_glob = list(stage_dir.glob("s3_pol_feepriority/*/executor_batch_metrics.jsonl"))
    
    if fcfs_glob and feeprio_glob:
        fcfs_rows = read_jsonl(fcfs_glob[0])
        feeprio_rows = read_jsonl(feeprio_glob[0])
        
        fcfs_df = pd.DataFrame(fcfs_rows)
        feeprio_df = pd.DataFrame(feeprio_rows)
        
        plt.figure(figsize=(9, 5.5))
        if not fcfs_df.empty:
            plt.scatter(fcfs_df['tx_count'], fcfs_df['total_execution_ms'], 
                        color=COLORS['success'], label='FCFS (Success)', s=60, marker='o', edgecolor='black', alpha=0.8)
        if not feeprio_df.empty:
            plt.scatter(feeprio_df['tx_count'], feeprio_df['total_execution_ms'], 
                        color=COLORS['danger'], label='FeePriority (Aborted/Failed Nonce)', s=60, marker='x', alpha=0.8)
            
        plt.xlabel('Batch Size (tx_count)')
        plt.ylabel('Executor Execution Time (ms)')
        plt.title('Stage 3: Executor Performance - FCFS vs. FeePriority')
        plt.legend()
        plt.grid(True, alpha=0.3)
        plt.tight_layout()
        plt.savefig(out_dir / 's3_executor_time_vs_success_tx.png', dpi=160)
        plt.close()

# ==========================================
# STAGE 4: DA MODE AND BLOB PACKING
# ==========================================
def plot_stage4(metrics_root: Path, out_dir: Path) -> None:
    stage_dir = metrics_root / "final_stage4_da"
    analysis_dir = stage_dir / "analysis"
    if not analysis_dir.exists():
        print(f"Skipping Stage 4: {analysis_dir} not found.")
        return
    
    print("Plotting Stage 4...")

    all_results_path = analysis_dir / "all_results.csv"
    
    if all_results_path.exists():
        df_res = pd.read_csv(all_results_path)

        # Plot 1: Economic Efficiency of DA Modes (USD cost comparison bar chart)
        da_ids = ['s4_da_calldata', 's4_da_blob', 's4_da_offchain']
        da_df = df_res[df_res['experiment_id'].isin(da_ids)].copy()
        if not da_df.empty:
            da_map = {
                's4_da_calldata': 'Calldata',
                's4_da_blob': 'EIP-4844 Blob',
                's4_da_offchain': 'Off-chain DA'
            }
            da_df['label'] = da_df['experiment_id'].map(da_map)
            da_df = da_df.sort_values('avg_cost_per_tx_usd', ascending=False)
            
            plt.figure(figsize=(8, 5))
            colors = [COLORS['danger'], COLORS['primary'], COLORS['success']]
            plt.bar(da_df['label'], da_df['avg_cost_per_tx_usd'] * 100, color=colors, width=0.4, edgecolor='black', alpha=0.85)
            plt.ylabel('Average USD Cost per Tx (cents)')
            plt.xlabel('Data Availability Mode')
            plt.title('Stage 4: Economic Efficiency of DA Modes')
            plt.grid(axis='y')
            plt.tight_layout()
            plt.savefig(out_dir / 's4_da_mode_efficiency.png', dpi=160)
            plt.close()

        # Plot 2: Blob Utilization vs. Blob Target Bytes (Line Chart)
        target_ids = ['s4_blob_target_32768', 's4_blob_target_65536', 's4_blob_target_98304', 's4_blob_target_120000']
        target_df = df_res[df_res['experiment_id'].isin(target_ids)].copy()
        if not target_df.empty:
            target_df['target_kb'] = target_df['experiment_id'].str.extract(r's4_blob_target_(\d+)').astype(float) / 1024
            target_df = target_df.sort_values('target_kb')
            
            plt.figure(figsize=(9, 5))
            plt.plot(target_df['target_kb'], target_df['avg_blob_utilization'] * 100, 
                     marker='o', color=COLORS['primary'], linewidth=2.5, label='Experimental Utilization')
            
            # Theoretical asymptote: average serialized batch size is 17.89 KB
            theoretical_x = np.linspace(30, 125, 200)
            theoretical_y = (17.89 / theoretical_x) * 100
            plt.plot(theoretical_x, theoretical_y, color=COLORS['accent'], linestyle='--', label='Theoretical Limit (17.89 KB / Target)')
            
            plt.xlabel('Blob Target Size (KB)')
            plt.ylabel('Average Blob Space Utilization (%)')
            plt.title('Stage 4: Blob Space Utilization vs. Target Size')
            plt.grid(True)
            plt.legend()
            plt.tight_layout()
            plt.savefig(out_dir / 's4_blob_utilization_vs_target.png', dpi=160)
            plt.close()

        # Plot 3: Blob Fill Target Flatline (Scatter/Line Chart)
        fill_ids = ['s4_blob_fill_050', 's4_blob_fill_070', 's4_blob_fill_080', 's4_blob_fill_090', 's4_blob_fill_095']
        fill_df = df_res[df_res['experiment_id'].isin(fill_ids)].copy()
        if not fill_df.empty:
            fill_df['fill_target'] = fill_df['experiment_id'].str.extract(r's4_blob_fill_(\d+)').astype(float) / 100
            fill_df = fill_df.sort_values('fill_target')
            
            fig, ax1 = plt.subplots(figsize=(9, 5))
            
            color = COLORS['primary']
            ax1.set_xlabel('Blob Fill Target Fraction')
            ax1.set_ylabel('Avg Batch Size (txs)', color=color)
            line1 = ax1.plot(fill_df['fill_target'], fill_df['avg_batch_tx_count'], marker='s', color=color, label='Avg Batch Size')
            ax1.tick_params(axis='y', labelcolor=color)
            ax1.set_ylim(0, 100)
            ax1.grid(True)
            
            ax2 = ax1.twinx()
            color = COLORS['accent']
            ax2.set_ylabel('Avg Blob Space Utilization (%)', color=color)
            line2 = ax2.plot(fill_df['fill_target'], fill_df['avg_blob_utilization'] * 100, marker='o', linestyle='--', color=color, label='Avg Blob Utilization')
            ax2.tick_params(axis='y', labelcolor=color)
            ax2.set_ylim(0, 100)
            
            lines = line1 + line2
            labels = [l.get_label() for l in lines]
            ax1.legend(lines, labels, loc='center right')
            plt.title('Stage 4: FCFS Sealing Insensitivity to Blob Fill Target')
            plt.tight_layout()
            plt.savefig(out_dir / 's4_blob_fill_target_flatline.png', dpi=160)
            plt.close()

# ==========================================
# STAGE 5: PROVER BACKEND AND REAL PROOFS
# ==========================================
def plot_stage5(metrics_root: Path, out_dir: Path) -> None:
    stage_dir = metrics_root / "final_stage5_proofs"
    analysis_dir = stage_dir / "analysis"
    if not analysis_dir.exists():
        print(f"Skipping Stage 5: {analysis_dir} not found.")
        return
    
    print("Plotting Stage 5...")

    all_batch_path = analysis_dir / "all_batch_results.csv"

    # Load Batch Data
    if all_batch_path.exists():
        df_batch = pd.read_csv(all_batch_path)
        real_df = df_batch[df_batch['experiment_id'].str.startswith('s5_real_bs_')].copy()
        
        # Plot 1: Prover Execution Time vs. Batch Size (Scatter with Regression)
        if not real_df.empty:
            x_data = real_df['tx_count'].values
            y_data = real_df['proof_time_ms'].values / 1000.0  # seconds
            
            # Linear Fit: T = A + B * N
            # Since size 3 and 246 batch runs are present, fit regression
            slope, intercept = np.polyfit(x_data, y_data, 1)
            
            plt.figure(figsize=(9, 5.5))
            plt.scatter(x_data, y_data, color=COLORS['primary'], s=70, label='Individual Batches', edgecolor='black', alpha=0.8)
            
            x_fit = np.linspace(0, max(x_data) + 10, 200)
            y_fit = slope * x_fit + intercept
            plt.plot(x_fit, y_fit, color=COLORS['danger'], linestyle='--', linewidth=2,
                     label=f'Linear Fit: $T = {intercept:.2f} + {slope:.2f} \\times N$')
            
            plt.xlabel('Actual Batch Size (tx_count)')
            plt.ylabel('Prover Execution Time (seconds)')
            plt.title('Stage 5: Prover Wall-Clock Time vs. Batch Size')
            plt.grid(True, alpha=0.3)
            plt.legend()
            plt.tight_layout()
            plt.savefig(out_dir / 's5_prover_time_vs_batch_size.png', dpi=160)
            plt.close()

            # Plot 3: L1 Gas per Transaction vs. Batch Size (Amortization Curve)
            # Incorporate baseline (mock backend) for low size comparison if needed, or stick to real ones
            # Let's plot actual gas per tx: l1_gas_used / tx_count
            real_df['gas_per_tx'] = real_df['l1_gas_used'] / real_df['tx_count']
            
            # Include baseline points if present to enrich lower-bound scaling
            base_df = df_batch[df_batch['experiment_id'] == 'baseline'].copy()
            base_df['gas_per_tx'] = base_df['l1_gas_used'] / base_df['tx_count']
            
            plt.figure(figsize=(9, 5.5))
            plt.scatter(real_df['tx_count'], real_df['gas_per_tx'], 
                        color=COLORS['primary'], s=70, label='RISC0 (Real Prover)', edgecolor='black', alpha=0.8)
            plt.scatter(base_df['tx_count'], base_df['gas_per_tx'], 
                        color=COLORS['grey'], s=40, label='Baseline (Mock Prover)', marker='^', alpha=0.6)
            
            # Theoretical amortization curve: fixed overhead F = 40,000 gas, marginal cost M = 19,300 gas
            tx_range = np.linspace(1, 260, 500)
            theoretical_gas = 40000 / tx_range + 19300
            plt.plot(tx_range, theoretical_gas, color=COLORS['accent'], linestyle='--', linewidth=2, 
                     label=r'Model: $Y = \frac{40,000}{N} + 19,300$')
            
            plt.axhline(19300, color=COLORS['danger'], linestyle=':', label='Asymptotic Marginal Cost (19.3k Gas)')
            
            plt.xlabel('Actual Batch Size ($N$)')
            plt.ylabel('L1 Gas per Transaction')
            plt.title('Stage 5: L1 Gas Amortization Curve')
            plt.ylim(10000, 60000)
            plt.grid(True, alpha=0.3)
            plt.legend()
            plt.tight_layout()
            plt.savefig(out_dir / 's5_l1_gas_amortization.png', dpi=160)
            plt.close()

    # Plot 2: zkVM Cycles and Segments vs. Batch Size (Double Y-Axis Line Chart)
    # Collect batch details from executor metrics JSONL files directly
    s5_runs = [
        "s5_real_bs_0050/s5_real_bs_0050_r01_20260519_182153/executor_batch_metrics.jsonl",
        "s5_real_bs_0100/s5_real_bs_0100_r01_20260519_185755/executor_batch_metrics.jsonl",
        "s5_real_bs_0200/s5_real_bs_0200_r01_20260519_192927/executor_batch_metrics.jsonl",
        "s5_real_bs_0500/s5_real_bs_0500_r01_20260519_200024/executor_batch_metrics.jsonl",
    ]
    
    rows = []
    # Also scan baseline just in case
    baseline_paths = list(stage_dir.glob("baseline/*/executor_batch_metrics.jsonl"))
    if baseline_paths:
        rows.extend(read_jsonl(baseline_paths[0]))
        
    for run_subpath in s5_runs:
        full_path = stage_dir / run_subpath
        if full_path.exists():
            rows.extend(read_jsonl(full_path))
            
    if rows:
        batch_details = []
        for r in rows:
            p_metrics = r.get("prover_metrics", {})
            cycles = p_metrics.get("total_cycles")
            segments = p_metrics.get("total_segments")
            tx_count = r.get("tx_count")
            
            if cycles is not None and segments is not None and tx_count is not None:
                # Exclude mock runs where cycles or segments are zero
                if cycles > 0:
                    batch_details.append({
                        "tx_count": tx_count,
                        "cycles": cycles,
                        "segments": segments
                    })
        
        if batch_details:
            df_det = pd.DataFrame(batch_details).sort_values('tx_count')
            # Group by tx_count to get unique values for curve
            df_det_grouped = df_det.groupby('tx_count').first().reset_index()
            
            fig, ax1 = plt.subplots(figsize=(9.5, 5.5))
            
            color = COLORS['primary']
            ax1.set_xlabel('Actual Batch Size (tx_count)')
            ax1.set_ylabel('Total zkVM Cycles', color=color)
            line1 = ax1.plot(df_det_grouped['tx_count'], df_det_grouped['cycles'], 
                             marker='o', color=color, linewidth=2.5, label='Total Cycles')
            ax1.tick_params(axis='y', labelcolor=color)
            # Format large numbers on Y axis
            ax1.get_yaxis().set_major_formatter(plt.FuncFormatter(lambda x, loc: "{:,}".format(int(x))))
            ax1.grid(True)
            
            ax2 = ax1.twinx()
            color = COLORS['purple']
            ax2.set_ylabel('Total segments', color=color)
            line2 = ax2.plot(df_det_grouped['tx_count'], df_det_grouped['segments'], 
                             marker='s', linestyle='--', color=color, linewidth=2, label='Total Segments')
            ax2.tick_params(axis='y', labelcolor=color)
            
            lines = line1 + line2
            labels = [l.get_label() for l in lines]
            ax1.legend(lines, labels, loc='upper left')
            
            plt.title('Stage 5: zkVM Execution Cycles & Segments vs. Batch Size')
            plt.tight_layout()
            plt.savefig(out_dir / 's5_zkvm_cycles_segments.png', dpi=160)
            plt.close()

def main() -> None:
    args = parse_args()
    metrics_root = args.metrics_root
    
    if not metrics_root.exists():
        print(f"Error: Metrics root '{metrics_root}' does not exist.")
        return
        
    print(f"Generating benchmark plots using metrics folder: {metrics_root.resolve()}")
    
    plots_root = metrics_root / "plots"
    
    plot_stage1(metrics_root, ensure_dir(plots_root / "stage1"))
    plot_stage2(metrics_root, ensure_dir(plots_root / "stage2"))
    plot_stage3(metrics_root, ensure_dir(plots_root / "stage3"))
    plot_stage4(metrics_root, ensure_dir(plots_root / "stage4"))
    plot_stage5(metrics_root, ensure_dir(plots_root / "stage5"))
    
    print("Done! All available benchmark plots have been successfully generated.")

if __name__ == "__main__":
    main()
