import os
import json
import argparse
import pandas as pd
import numpy as np
import matplotlib.pyplot as plt
from pathlib import Path

# Professional Academic Color Palette
COLORS = {
    'primary': '#2563eb',     # Deep Blue
    'success': '#16a34a',     # Green
    'danger': '#dc2626',      # Red
    'warning': '#d97706',     # Orange/Amber
    'accent': '#7c3aed',      # Purple
    'gray': '#4b5563',        # Dark Slate Gray
    'fixed': '#ec4899',       # Pink
    'adaptive': '#0ea5e9',    # Sky Blue
    'calldata': '#d97706',    # Amber
    'blob': '#10b981',        # Emerald
    'offchain': '#6b7280',     # Gray
    'A': '#2563eb',           # Blue for Class A
    'B': '#16a34a',           # Green for Class B
    'C': '#7c3aed'            # Purple for Class C
}

# Apply global styling for professional look
plt.rcParams.update({
    'font.family': 'sans-serif',
    'font.size': 11,
    'axes.labelsize': 12,
    'axes.titlesize': 13,
    'xtick.labelsize': 10,
    'ytick.labelsize': 10,
    'legend.fontsize': 10,
    'grid.alpha': 0.3,
    'grid.linestyle': '--'
})

def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate benchmarking plan plots for RollupX Stages 1-5.")
    parser.add_argument(
        "--metrics-root",
        type=Path,
        default=Path("benchmark-suite/metrics"),
        help="Path to the metrics folder containing stage directories"
    )
    return parser.parse_args()

def ensure_dir(path: Path) -> Path:
    path.mkdir(parents=True, exist_ok=True)
    return path

# ==========================================
# STAGE 1: FIXED BATCHING
# ==========================================
def plot_stage1_plan(metrics_root: Path, out_dir: Path) -> None:
    stage_dir = metrics_root / "final_stage1_fixed_batching"
    analysis_dir = stage_dir / "analysis"
    if not analysis_dir.exists():
        print(f"Skipping Stage 1 Plan Plots: {analysis_dir} not found.")
        return
    
    print("Plotting Stage 1 (Plan Required Graphs)...")
    all_results_path = analysis_dir / "all_results.csv"
    if not all_results_path.exists():
        print(f"Skipping Stage 1: {all_results_path} not found.")
        return
        
    df = pd.read_csv(all_results_path)
    
    # 1. Filter for Batch Size experiments (s1_bs_*)
    bs_df = df[df['experiment_id'].str.match(r'^s1_bs_\d+$', na=False)].copy()
    if not bs_df.empty:
        bs_df['batch_size_val'] = bs_df['experiment_id'].str.extract(r's1_bs_(\d+)').astype(int)
        bs_df = bs_df.sort_values('batch_size_val')
        
        # Plot 1: stage1_batch_size_vs_goodput.png
        plt.figure(figsize=(7, 4.5))
        plt.plot(bs_df['batch_size_val'], bs_df['goodput_tps'], marker='o', color=COLORS['primary'], linewidth=2)
        plt.xlabel('Configured Batch Size (txs)')
        plt.ylabel('Goodput (TPS)')
        plt.title('Stage 1: Batch Size vs. Goodput TPS')
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage1_batch_size_vs_goodput.png', dpi=160)
        plt.close()

        # Plot 2: stage1_batch_size_vs_p95_latency.png
        plt.figure(figsize=(7, 4.5))
        plt.plot(bs_df['batch_size_val'], bs_df['p95_queue_wait_ms'], marker='s', color=COLORS['danger'], linewidth=2)
        plt.xlabel('Configured Batch Size (txs)')
        plt.ylabel('P95 Queue Wait Time (ms)')
        plt.title('Stage 1: Batch Size vs. P95 Queue Latency')
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage1_batch_size_vs_p95_latency.png', dpi=160)
        plt.close()

        # Plot 3: stage1_batch_size_vs_cost_per_tx.png
        plt.figure(figsize=(7, 4.5))
        plt.plot(bs_df['batch_size_val'], bs_df['avg_gas_per_tx'], marker='^', color=COLORS['warning'], linewidth=2)
        plt.xlabel('Configured Batch Size (txs)')
        plt.ylabel('L1 Gas per Tx')
        plt.title('Stage 1: Batch Size vs. Gas Cost per Tx')
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage1_batch_size_vs_cost_per_tx.png', dpi=160)
        plt.close()

        # Plot 4: stage1_batch_size_vs_batch_fill_ratio.png
        plt.figure(figsize=(7, 4.5))
        fill_ratio = (bs_df['avg_batch_tx_count'] / bs_df['batch_size_val']) * 100
        plt.plot(bs_df['batch_size_val'], fill_ratio, marker='d', color=COLORS['success'], linewidth=2)
        plt.xlabel('Configured Batch Size (txs)')
        plt.ylabel('Batch Fill Ratio (%)')
        plt.title('Stage 1: Batch Size vs. Batch Fill Ratio')
        plt.ylim(0, 105)
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage1_batch_size_vs_batch_fill_ratio.png', dpi=160)
        plt.close()

    # 2. Filter for Timeout experiments (s1_to_*)
    to_df = df[df['experiment_id'].str.match(r'^s1_to_\d+$', na=False)].copy()
    if not to_df.empty:
        to_df['timeout_val'] = to_df['experiment_id'].str.extract(r's1_to_(\d+)').astype(int)
        to_df = to_df.sort_values('timeout_val')
        
        # Plot 5: stage1_timeout_vs_p95_latency.png
        plt.figure(figsize=(7, 4.5))
        plt.plot(to_df['timeout_val'], to_df['p95_queue_wait_ms'], marker='s', color=COLORS['danger'], linewidth=2)
        plt.xlabel('Block Timeout Interval (ms)')
        plt.ylabel('P95 Queue Wait Time (ms)')
        plt.title('Stage 1: Timeout vs. P95 Queue Latency')
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage1_timeout_vs_p95_latency.png', dpi=160)
        plt.close()

        # Plot 6: stage1_timeout_vs_goodput.png
        plt.figure(figsize=(7, 4.5))
        plt.plot(to_df['timeout_val'], to_df['goodput_tps'], marker='o', color=COLORS['primary'], linewidth=2)
        plt.xlabel('Block Timeout Interval (ms)')
        plt.ylabel('Goodput (TPS)')
        plt.title('Stage 1: Timeout vs. Goodput TPS')
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage1_timeout_vs_goodput.png', dpi=160)
        plt.close()

    # Plot 7: stage1_throughput_latency_pareto.png
    plt.figure(figsize=(8, 5))
    s1_all = df[df['experiment_id'].str.startswith('s1_', na=False) | (df['experiment_id'] == 'baseline')].copy()
    
    # Sort for plotting Pareto frontier line
    s1_all = s1_all.sort_values('goodput_tps')
    plt.scatter(s1_all['goodput_tps'], s1_all['p95_queue_wait_ms'], color=COLORS['accent'], s=80, edgecolor='black', zorder=3)
    
    # Highlight Pareto front (low latency, high throughput)
    # Simple heuristic to compute pareto frontier points
    pareto_pts = []
    for idx, row in s1_all.iterrows():
        # A point is pareto-optimal if no other point has higher TPS and lower latency
        better_pts = s1_all[(s1_all['goodput_tps'] >= row['goodput_tps']) & (s1_all['p95_queue_wait_ms'] <= row['p95_queue_wait_ms'])]
        # Exclude exact self
        better_pts = better_pts[better_pts['experiment_id'] != row['experiment_id']]
        if better_pts.empty:
            pareto_pts.append(row)
            
    if pareto_pts:
        pareto_df = pd.DataFrame(pareto_pts).sort_values('goodput_tps')
        plt.plot(pareto_df['goodput_tps'], pareto_df['p95_queue_wait_ms'], linestyle='--', color='red', alpha=0.7, label='Pareto Frontier')
        
    for _, row in s1_all.iterrows():
        label = row['experiment_id'].replace('s1_bs_', 'BS ').replace('s1_to_', 'TO ')
        plt.annotate(label, (row['goodput_tps'], row['p95_queue_wait_ms']), textcoords="offset points", xytext=(0,5), ha='center', fontsize=8)

    plt.xlabel('Goodput (TPS)')
    plt.ylabel('P95 Queue Wait Time (ms)')
    plt.title('Stage 1 Pareto Analysis: Throughput vs. Latency')
    plt.legend(loc='upper right')
    plt.grid(True)
    plt.tight_layout()
    plt.savefig(out_dir / 'stage1_throughput_latency_pareto.png', dpi=160)
    plt.close()

# ==========================================
# STAGE 2: ADAPTIVE BATCHING
# ==========================================
def plot_stage2_plan(metrics_root: Path, out_dir: Path) -> None:
    stage_dir = metrics_root / "final_stage2_adaptive_batching"
    analysis_dir = stage_dir / "analysis"
    if not analysis_dir.exists():
        print(f"Skipping Stage 2 Plan Plots: {analysis_dir} not found.")
        return
        
    print("Plotting Stage 2 (Plan Required Graphs)...")
    all_results_path = analysis_dir / "all_results.csv"
    all_batch_path = analysis_dir / "all_batch_results.csv"
    
    if not all_results_path.exists():
        print(f"Skipping Stage 2: {all_results_path} not found.")
        return

    df_res = pd.read_csv(all_results_path)

    # 1. stage2_traffic_vs_selected_batch_size.png (Burst timeline response)
    if all_batch_path.exists():
        df_batch = pd.read_csv(all_batch_path)
        burst_df = df_batch[df_batch['experiment_id'] == 's2_adaptive_burst'].copy()
        if not burst_df.empty:
            burst_df = burst_df.sort_values('batch_id')
            
            fig, ax1 = plt.subplots(figsize=(8, 4.5))
            color = COLORS['adaptive']
            ax1.set_xlabel('Timeline (Batch ID)')
            ax1.set_ylabel('Selected Batch Size (txs)', color=color)
            ax1.plot(burst_df['batch_id'], burst_df['tx_count'], marker='o', color=color, linewidth=2, label='Batch Size')
            ax1.tick_params(axis='y', labelcolor=color)
            ax1.grid(True)

            ax2 = ax1.twinx()
            color = COLORS['danger']
            ax2.set_ylabel('Mempool Backlog (txs)', color=color)
            ax2.fill_between(burst_df['batch_id'], burst_df['mempool_depth_at_batch'], color=color, alpha=0.15, label='Backlog')
            ax2.plot(burst_df['batch_id'], burst_df['mempool_depth_at_batch'], color=color, linestyle=':', alpha=0.8)
            ax2.tick_params(axis='y', labelcolor=color)
            
            plt.title('Stage 2: Controller Response to Traffic Burst')
            fig.tight_layout()
            plt.savefig(out_dir / 'stage2_traffic_vs_selected_batch_size.png', dpi=160)
            plt.close()

    # Fixed vs Adaptive comparisons under load profiles
    profiles = ['low', 'medium', 'high', 'burst']
    paired_data = []
    
    for prof in profiles:
        f_row = df_res[df_res['experiment_id'] == f's2_fixed_{prof}']
        a_row = df_res[df_res['experiment_id'] == f's2_adaptive_{prof}']
        if not f_row.empty and not a_row.empty:
            paired_data.append({
                'Profile': prof.capitalize(),
                'Fixed_Latency': f_row.iloc[0]['p95_queue_wait_ms'],
                'Adaptive_Latency': a_row.iloc[0]['p95_queue_wait_ms'],
                'Fixed_Goodput': f_row.iloc[0]['goodput_tps'],
                'Adaptive_Goodput': a_row.iloc[0]['goodput_tps'],
                'Fixed_Gas': f_row.iloc[0]['avg_gas_per_tx'],
                'Adaptive_Gas': a_row.iloc[0]['avg_gas_per_tx']
            })
            
    if paired_data:
        pdf = pd.DataFrame(paired_data)
        x = np.arange(len(pdf['Profile']))
        width = 0.35
        
        # 2. stage2_adaptive_vs_fixed_p95_latency.png
        plt.figure(figsize=(7, 4.5))
        plt.bar(x - width/2, pdf['Fixed_Latency'], width, label='Fixed (BS=100)', color=COLORS['fixed'], edgecolor='black')
        plt.bar(x + width/2, pdf['Adaptive_Latency'], width, label='Adaptive', color=COLORS['adaptive'], edgecolor='black')
        plt.ylabel('P95 Queue Wait Time (ms)')
        plt.title('P95 Latency Comparison: Fixed vs. Adaptive')
        plt.xticks(x, pdf['Profile'])
        plt.legend()
        plt.grid(axis='y')
        plt.tight_layout()
        plt.savefig(out_dir / 'stage2_adaptive_vs_fixed_p95_latency.png', dpi=160)
        plt.close()

        # 3. stage2_adaptive_vs_fixed_goodput.png
        plt.figure(figsize=(7, 4.5))
        plt.bar(x - width/2, pdf['Fixed_Goodput'], width, label='Fixed (BS=100)', color=COLORS['fixed'], edgecolor='black')
        plt.bar(x + width/2, pdf['Adaptive_Goodput'], width, label='Adaptive', color=COLORS['adaptive'], edgecolor='black')
        plt.ylabel('Goodput (TPS)')
        plt.title('Goodput TPS Comparison: Fixed vs. Adaptive')
        plt.xticks(x, pdf['Profile'])
        plt.legend()
        plt.grid(axis='y')
        plt.tight_layout()
        plt.savefig(out_dir / 'stage2_adaptive_vs_fixed_goodput.png', dpi=160)
        plt.close()

        # 4. stage2_adaptive_vs_fixed_cost_per_tx.png
        plt.figure(figsize=(7, 4.5))
        plt.bar(x - width/2, pdf['Fixed_Gas'], width, label='Fixed (BS=100)', color=COLORS['fixed'], edgecolor='black')
        plt.bar(x + width/2, pdf['Adaptive_Gas'], width, label='Adaptive', color=COLORS['adaptive'], edgecolor='black')
        plt.ylabel('Avg Gas per Tx')
        plt.title('Gas Cost Comparison: Fixed vs. Adaptive')
        plt.xticks(x, pdf['Profile'])
        plt.legend()
        plt.grid(axis='y')
        plt.tight_layout()
        plt.savefig(out_dir / 'stage2_adaptive_vs_fixed_cost_per_tx.png', dpi=160)
        plt.close()

    # 5. stage2_burst_backlog_recovery.png
    if all_batch_path.exists():
        df_batch = pd.read_csv(all_batch_path)
        f_burst = df_batch[df_batch['experiment_id'] == 's2_fixed_burst'].sort_values('batch_id')
        a_burst = df_batch[df_batch['experiment_id'] == 's2_adaptive_burst'].sort_values('batch_id')
        
        plt.figure(figsize=(8, 4.5))
        if not f_burst.empty:
            plt.plot(f_burst['batch_id'], f_burst['mempool_depth_at_batch'], label='Fixed Sealing Backlog', color=COLORS['fixed'], linewidth=2)
        if not a_burst.empty:
            plt.plot(a_burst['batch_id'], a_burst['mempool_depth_at_batch'], label='Adaptive Sealing Backlog', color=COLORS['adaptive'], linewidth=2, linestyle='--')
        plt.xlabel('Timeline (Batch ID)')
        plt.ylabel('Mempool Backlog Depth (txs)')
        plt.title('Burst Mempool Backlog Recovery: Fixed vs. Adaptive')
        plt.legend()
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage2_burst_backlog_recovery.png', dpi=160)
        plt.close()

        # 6. stage2_batch_size_distribution.png
        plt.figure(figsize=(8, 4.5))
        adaptive_runs = df_batch[df_batch['experiment_id'].str.startswith('s2_adaptive_')].copy()
        if not adaptive_runs.empty:
            # Map profiles to labels
            adaptive_runs['load_profile'] = adaptive_runs['experiment_id'].str.replace('s2_adaptive_', '').str.capitalize()
            
            # Group batch counts
            grouped_bins = adaptive_runs.groupby(['load_profile', 'tx_count']).size().unstack(fill_value=0)
            grouped_bins.plot(kind='bar', stacked=True, color=[COLORS['primary'], COLORS['success'], COLORS['warning']], ax=plt.gca(), edgecolor='black')
            plt.xlabel('Load Profile')
            plt.ylabel('Batch Sealing Count')
            plt.title('Adaptive Batch Size Selections by Load Profile')
            plt.legend(title='Batch Size (txs)', loc='upper right')
            plt.grid(axis='y')
            plt.tight_layout()
            plt.savefig(out_dir / 'stage2_batch_size_distribution.png', dpi=160)
            plt.close()

    # 7. stage2_adaptive_pareto.png
    plt.figure(figsize=(8, 5))
    plt.scatter(df_res[df_res['experiment_id'].str.contains('fixed')]['goodput_tps'], 
                df_res[df_res['experiment_id'].str.contains('fixed')]['p95_queue_wait_ms'], 
                color=COLORS['fixed'], label='Fixed Runs', s=100, marker='x', zorder=3)
    plt.scatter(df_res[df_res['experiment_id'].str.contains('adapt')]['goodput_tps'], 
                df_res[df_res['experiment_id'].str.contains('adapt')]['p95_queue_wait_ms'], 
                color=COLORS['adaptive'], label='Adaptive Runs', s=100, marker='o', edgecolor='black', zorder=3)
    
    for _, row in df_res.iterrows():
        plt.annotate(row['experiment_id'].replace('s2_', ''), (row['goodput_tps'], row['p95_queue_wait_ms']), 
                     textcoords="offset points", xytext=(0,6), ha='center', fontsize=8)
                     
    plt.xlabel('Goodput (TPS)')
    plt.ylabel('P95 Queue Wait Time (ms)')
    plt.title('Stage 2 Pareto Front: Fixed vs. Adaptive Batching')
    plt.legend()
    plt.grid(True)
    plt.tight_layout()
    plt.savefig(out_dir / 'stage2_adaptive_pareto.png', dpi=160)
    plt.close()

# ==========================================
# STAGE 3: SEQUENCER POLICY
# ==========================================
def plot_stage3_plan(metrics_root: Path, out_dir: Path) -> None:
    stage_dir = metrics_root / "final_stage3_policy"
    analysis_dir = stage_dir / "analysis"
    if not analysis_dir.exists():
        print(f"Skipping Stage 3 Plan Plots: {analysis_dir} not found.")
        return
        
    print("Plotting Stage 3 (Plan Required Graphs)...")
    all_results_path = analysis_dir / "all_results.csv"
    if not all_results_path.exists():
        print(f"Skipping Stage 3: {all_results_path} not found.")
        return

    df_res = pd.read_csv(all_results_path)
    policy_ids = ['s3_pol_fcfs', 's3_pol_fairbft', 's3_pol_feepriority', 's3_pol_timeboost', 's3_pol_blobpacking']
    pol_df = df_res[df_res['experiment_id'].isin(policy_ids)].copy()
    
    if pol_df.empty:
        print("Skipping Stage 3 standard runs: no pol_* records found.")
        return

    # Map policy labels
    pol_df['policy_label'] = pol_df['policy'].replace({
        'FCFS': 'FCFS', 'FairBFT': 'FairBFT', 'FeePriority': 'FeePriority', 
        'TimeBoost': 'TimeBoost', 'BlobPacking': 'BlobPacking'
    })

    # 1. stage3_policy_vs_goodput.png
    plt.figure(figsize=(7.5, 4.5))
    bars = plt.bar(pol_df['policy_label'], pol_df['goodput_tps'], color=COLORS['primary'], edgecolor='black', width=0.5)
    plt.ylabel('Goodput (TPS)')
    plt.title('Stage 3: Goodput TPS across Scheduling Policies')
    plt.grid(axis='y')
    plt.tight_layout()
    plt.savefig(out_dir / 'stage3_policy_vs_goodput.png', dpi=160)
    plt.close()

    # Load Transaction Logs dynamically
    # Look for files like: final_stage3_policy/s3_pol_*/s3_pol_*_r01_*/tx_log_*.csv
    tx_logs = list(stage_dir.glob("s3_pol_*/s3_pol_*_r01_*/tx_log_*.csv"))
    
    tx_dfs = []
    for log_path in tx_logs:
        # Extract policy name from the parent dir or filename
        parent_name = log_path.parent.parent.name # e.g. s3_pol_fcfs
        policy_key = parent_name.replace('s3_pol_', '')
        
        try:
            temp_df = pd.read_csv(log_path)
            # convert latency to ms
            temp_df['latency_ms'] = temp_df['latency'] * 1000.0
            temp_df['policy'] = policy_key
            tx_dfs.append(temp_df)
        except Exception as e:
            print(f"Failed to load {log_path}: {e}")

    if tx_dfs:
        all_tx_df = pd.concat(tx_dfs, ignore_index=True)
        
        # 2. stage3_policy_vs_p95_latency_by_class.png
        # Group by policy and transaction class (tx_type)
        class_p95 = all_tx_df.groupby(['policy', 'tx_type'])['latency_ms'].quantile(0.95).unstack(fill_value=0)
        
        # Normalize index names for clean labels
        class_p95.index = class_p95.index.map(lambda x: x.upper())
        
        plt.figure(figsize=(8.5, 5))
        class_p95.plot(kind='bar', color=[COLORS['A'], COLORS['B'], COLORS['C']], ax=plt.gca(), edgecolor='black', width=0.6)
        plt.ylabel('P95 Sequencer Wait Latency (ms)')
        plt.xlabel('Scheduling Policy')
        plt.title('P95 Latency by Fee Class (A=Low, B=Medium, C=High)')
        plt.grid(axis='y')
        plt.legend(title='Workload Class')
        plt.xticks(rotation=0)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage3_policy_vs_p95_latency_by_class.png', dpi=160)
        plt.close()

        # 4. stage3_policy_vs_starvation_count.png
        # Starvation: defined relative to the FCFS median latency.
        # Let's count transactions with latency > 3.0 ms
        starved_tx = all_tx_df[all_tx_df['latency_ms'] > 3.0]
        starvation_counts = starved_tx.groupby('policy').size()
        
        # Ensure all policies are represented in index
        for p in class_p95.index.map(lambda x: x.lower()):
            if p not in starvation_counts.index:
                starvation_counts[p] = 0
                
        starvation_counts = starvation_counts.loc[class_p95.index.map(lambda x: x.lower())]
        starvation_counts.index = starvation_counts.index.map(lambda x: x.upper())
        
        plt.figure(figsize=(7.5, 4.5))
        plt.bar(starvation_counts.index, starvation_counts.values, color=COLORS['danger'], edgecolor='black', width=0.5)
        plt.ylabel('Starvation Event Count (Latency > 3ms)')
        plt.title('Stage 3 Starvation Counts across Policies')
        plt.grid(axis='y')
        plt.tight_layout()
        plt.savefig(out_dir / 'stage3_policy_vs_starvation_count.png', dpi=160)
        plt.close()

        # 6. stage3_priority_latency_cdf.png
        # CDF Comparison for FCFS vs FeePriority
        fcfs_tx = all_tx_df[all_tx_df['policy'] == 'fcfs'].copy()
        feeprio_tx = all_tx_df[all_tx_df['policy'] == 'feepriority'].copy()
        
        plt.figure(figsize=(9, 5.5))
        for tx_type, col in zip(['A', 'B', 'C'], [COLORS['A'], COLORS['B'], COLORS['C']]):
            # FCFS CDF
            f_sub = fcfs_tx[fcfs_tx['tx_type'] == tx_type]['latency_ms'].dropna().values
            if len(f_sub) > 0:
                f_sorted = np.sort(f_sub)
                f_y = np.arange(1, len(f_sorted) + 1) / len(f_sorted)
                plt.plot(f_sorted, f_y, color=col, linestyle='--', label=f'FCFS (Class {tx_type})')
                
            # FeePriority CDF
            fp_sub = feeprio_tx[feeprio_tx['tx_type'] == tx_type]['latency_ms'].dropna().values
            if len(fp_sub) > 0:
                fp_sorted = np.sort(fp_sub)
                fp_y = np.arange(1, len(fp_sorted) + 1) / len(fp_sorted)
                plt.plot(fp_sorted, fp_y, color=col, linestyle='-', linewidth=2, label=f'FeePrio (Class {tx_type})')
                
        plt.xlabel('Transaction Queue Wait Time (ms)')
        plt.ylabel('Cumulative Probability')
        plt.title('Stage 3: Transaction Wait Latency CDF Comparison')
        plt.xlim(0, 5.0) # Zoom into wait timeline
        plt.legend()
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage3_priority_latency_cdf.png', dpi=160)
        plt.close()

    # 3. stage3_policy_vs_jain_fairness.png
    plt.figure(figsize=(7.5, 4.5))
    plt.bar(pol_df['policy_label'], pol_df['jains_fairness'], color=COLORS['success'], edgecolor='black', width=0.5)
    plt.ylabel("Jain's Fairness Index")
    plt.ylim(0, 1.05)
    plt.title("Stage 3: Jain's Fairness Index across Policies")
    plt.grid(axis='y')
    plt.tight_layout()
    plt.savefig(out_dir / 'stage3_policy_vs_jain_fairness.png', dpi=160)
    plt.close()

    # 5. stage3_policy_vs_reordering_distance.png
    plt.figure(figsize=(7.5, 4.5))
    plt.bar(pol_df['policy_label'], pol_df['total_reordering_events'], color=COLORS['warning'], edgecolor='black', width=0.5)
    plt.ylabel('Total Reordering Events')
    plt.title('Stage 3: Sequencer Reordering Count by Policy')
    plt.grid(axis='y')
    plt.tight_layout()
    plt.savefig(out_dir / 'stage3_policy_vs_reordering_distance.png', dpi=160)
    plt.close()

    # 7. stage3_blobpacking_vs_fcfs_blob_utilization.png
    plt.figure(figsize=(8, 5))
    compare_df = pol_df[pol_df['policy'].isin(['FCFS', 'BlobPacking'])].copy()
    if len(compare_df) >= 2:
        fig, ax1 = plt.subplots(figsize=(7.5, 4.5))
        
        # Dual axis comparison
        color = COLORS['blob']
        ax1.set_xlabel('Scheduling Policy')
        ax1.set_ylabel('Average Blob Utilization (%)', color=color)
        ax1.bar(compare_df['policy_label'], compare_df['avg_blob_utilization'] * 100, color=color, alpha=0.6, edgecolor='black', width=0.4)
        ax1.tick_params(axis='y', labelcolor=color)
        ax1.set_ylim(0, 105)
        
        ax2 = ax1.twinx()
        color = COLORS['danger']
        ax2.set_ylabel('Average Gas per Tx', color=color)
        ax2.plot(compare_df['policy_label'], compare_df['avg_gas_per_tx'], color=color, marker='o', linewidth=2.5)
        ax2.tick_params(axis='y', labelcolor=color)
        
        plt.title('Blob Utilization vs. Gas Efficiency: FCFS vs. BlobPacking')
        fig.tight_layout()
        plt.savefig(out_dir / 'stage3_blobpacking_vs_fcfs_blob_utilization.png', dpi=160)
        plt.close()

# ==========================================
# STAGE 4: DA MODE AND BLOB PACKING
# ==========================================
def plot_stage4_plan(metrics_root: Path, out_dir: Path) -> None:
    stage_dir = metrics_root / "final_stage4_da"
    analysis_dir = stage_dir / "analysis"
    if not analysis_dir.exists():
        print(f"Skipping Stage 4 Plan Plots: {analysis_dir} not found.")
        return
        
    print("Plotting Stage 4 (Plan Required Graphs)...")
    all_results_path = analysis_dir / "all_results.csv"
    if not all_results_path.exists():
        print(f"Skipping Stage 4: {all_results_path} not found.")
        return

    df_res = pd.read_csv(all_results_path)

    # 1. stage4_da_mode_vs_cost_per_tx.png
    modes_df = df_res[df_res['experiment_id'].isin(['s4_da_calldata', 's4_da_blob', 's4_da_offchain'])].copy()
    if not modes_df.empty:
        plt.figure(figsize=(7.5, 4.5))
        plt.bar(modes_df['da_mode'].str.upper(), modes_df['avg_cost_per_tx_usd'], color=[COLORS['calldata'], COLORS['blob'], COLORS['offchain']], edgecolor='black', width=0.5)
        plt.ylabel('Average Cost per Tx ($)')
        plt.title('Stage 4: Cost per Tx by DA Mode')
        plt.grid(axis='y')
        plt.tight_layout()
        plt.savefig(out_dir / 'stage4_da_mode_vs_cost_per_tx.png', dpi=160)
        plt.close()

        # 2. stage4_da_mode_vs_hard_finality_latency.png
        plt.figure(figsize=(7.5, 4.5))
        plt.bar(modes_df['da_mode'].str.upper(), modes_df['avg_hard_finality_ms'] / 1000.0, color=[COLORS['calldata'], COLORS['blob'], COLORS['offchain']], edgecolor='black', width=0.5)
        plt.ylabel('Hard Finality Latency (s)')
        plt.title('Stage 4: L1 Hard Finality Latency by DA Mode')
        plt.grid(axis='y')
        plt.tight_layout()
        plt.savefig(out_dir / 'stage4_da_mode_vs_hard_finality_latency.png', dpi=160)
        plt.close()

    # 3. stage4_batch_payload_vs_blob_fill_ratio.png
    targets_df = df_res[df_res['experiment_id'].str.startswith('s4_blob_target_')].copy()
    if not targets_df.empty:
        targets_df['capacity_val'] = targets_df['experiment_id'].str.extract(r's4_blob_target_(\d+)').astype(int)
        targets_df = targets_df.sort_values('capacity_val')
        
        plt.figure(figsize=(7.5, 4.5))
        plt.plot(targets_df['capacity_val'] / 1024.0, targets_df['avg_blob_utilization'] * 100, marker='o', color=COLORS['blob'], linewidth=2)
        plt.xlabel('Blob Target Size (KB)')
        plt.ylabel('Blob Utilization (%)')
        plt.title('Stage 4: Blob Target Size vs. Average Utilization')
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage4_batch_payload_vs_blob_fill_ratio.png', dpi=160)
        plt.close()

    # Blob fill fractions: s4_blob_fill_050 to s4_blob_fill_095
    fill_df = df_res[df_res['experiment_id'].str.startswith('s4_blob_fill_')].copy()
    if not fill_df.empty:
        fill_df['fill_target_val'] = fill_df['experiment_id'].str.extract(r's4_blob_fill_(\d+)').astype(float) / 100.0
        fill_df = fill_df.sort_values('fill_target_val')

        # 4. stage4_blob_fill_target_vs_cost.png
        plt.figure(figsize=(7.5, 4.5))
        plt.plot(fill_df['fill_target_val'], fill_df['avg_cost_per_tx_usd'], marker='o', color=COLORS['warning'], linewidth=2)
        plt.xlabel('Blob Fill Target Fraction')
        plt.ylabel('Average Cost per Tx ($)')
        plt.title('Stage 4: Blob Fill Target vs. Transaction Cost')
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage4_blob_fill_target_vs_cost.png', dpi=160)
        plt.close()

        # 5. stage4_blob_fill_target_vs_p95_latency.png
        plt.figure(figsize=(7.5, 4.5))
        plt.plot(fill_df['fill_target_val'], fill_df['p95_queue_wait_ms'], marker='s', color=COLORS['danger'], linewidth=2)
        plt.xlabel('Blob Fill Target Fraction')
        plt.ylabel('P95 Queue wait Latency (ms)')
        plt.title('Stage 4: Blob Fill Target vs. Latency')
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage4_blob_fill_target_vs_p95_latency.png', dpi=160)
        plt.close()

        # 8. stage4_blob_waste_ratio.png
        plt.figure(figsize=(7.5, 4.5))
        waste_ratio = (1.0 - fill_df['avg_blob_utilization']) * 100
        plt.plot(fill_df['fill_target_val'], waste_ratio, marker='x', color=COLORS['gray'], linewidth=2)
        plt.xlabel('Blob Fill Target Fraction')
        plt.ylabel('Blob Waste Ratio (%)')
        plt.title('Stage 4: Blob Waste Ratio vs. Fill Target')
        plt.grid(True)
        plt.tight_layout()
        plt.savefig(out_dir / 'stage4_blob_waste_ratio.png', dpi=160)
        plt.close()

    # 6. stage4_calldata_blob_crossover.png
    # Mathematical modeled crossover point
    tx_counts = np.arange(1, 201)
    
    # Constants based on gas metrics:
    # Calldata: avg_gas_per_batch = 749000 fixed, 20700 marginal
    # Blob: avg_gas_per_batch = 118000 fixed, 2600 marginal (plus modeled blob cost of 131,072 gas)
    c_fixed_calldata = 750000.0
    c_marginal_calldata = 20700.0
    c_fixed_blob = 118000.0 + 131072.0 * 0.1 # Modeled blob fee factor
    c_marginal_blob = 2600.0
    
    gas_price_gwei = 10.0
    eth_price_usd = 3000.0
    
    cost_calldata = (c_fixed_calldata + c_marginal_calldata * tx_counts) * gas_price_gwei * 1e-9 * eth_price_usd / tx_counts
    cost_blob = (c_fixed_blob + c_marginal_blob * tx_counts) * gas_price_gwei * 1e-9 * eth_price_usd / tx_counts
    
    plt.figure(figsize=(8.5, 5))
    plt.plot(tx_counts, cost_calldata, color=COLORS['calldata'], label='Calldata DA Mode', linewidth=2.5)
    plt.plot(tx_counts, cost_blob, color=COLORS['blob'], label='Blob DA Mode (EIP-4844)', linewidth=2.5)
    
    # Find intersection
    idx = np.argwhere(np.diff(np.sign(cost_calldata - cost_blob))).flatten()
    if len(idx) > 0:
        crossover_x = tx_counts[idx[0]]
        crossover_y = cost_calldata[idx[0]]
        plt.scatter([crossover_x], [crossover_y], color='red', s=100, zorder=5)
        plt.annotate(f'Crossover: {crossover_x} txs', (crossover_x, crossover_y), textcoords="offset points", xytext=(20, 15), arrowprops=dict(arrowstyle="->", color='red'))
        
    plt.xlabel('Transaction Count per L1 Batch ($N$)')
    plt.ylabel('Amortized DA Cost per Tx ($)')
    plt.title('Stage 4: Calldata vs. Blob Cost Crossover Point')
    plt.ylim(0, 50.0)
    plt.legend()
    plt.grid(True)
    plt.tight_layout()
    plt.savefig(out_dir / 'stage4_calldata_blob_crossover.png', dpi=160)
    plt.close()

    # 7. stage4_blobpacking_vs_fcfs_cost.png
    compare_df = df_res[df_res['experiment_id'].isin(['s4_da_blob', 's4_da_blobpacking'])].copy()
    if len(compare_df) >= 2:
        compare_df['policy_label'] = compare_df['policy'].replace({'FCFS': 'FCFS (Naive)', 'BlobPacking': 'BlobPacking'})
        
        plt.figure(figsize=(7.5, 4.5))
        plt.bar(compare_df['policy_label'], compare_df['avg_cost_per_tx_usd'], color=[COLORS['primary'], COLORS['success']], edgecolor='black', width=0.4)
        plt.ylabel('Average Cost per Tx ($)')
        plt.title('Stage 4: Economic Advantage of BlobPacking')
        plt.grid(axis='y')
        plt.tight_layout()
        plt.savefig(out_dir / 'stage4_blobpacking_vs_fcfs_cost.png', dpi=160)
        plt.close()

# ==========================================
# STAGE 5: PROVER BACKEND AND REAL PROOFS
# ==========================================
def plot_stage5_plan(metrics_root: Path, out_dir: Path) -> None:
    stage_dir = metrics_root / "final_stage5_proofs"
    analysis_dir = stage_dir / "analysis"
    if not analysis_dir.exists():
        print(f"Skipping Stage 5 Plan Plots: {analysis_dir} not found.")
        return
        
    print("Plotting Stage 5 (Plan Required Graphs)...")
    all_results_path = analysis_dir / "all_results.csv"
    if not all_results_path.exists():
        print(f"Skipping Stage 5: {all_results_path} not found.")
        return

    df_res = pd.read_csv(all_results_path)
    real_runs = df_res[df_res['experiment_id'].str.startswith('s5_real_bs_') | (df_res['experiment_id'] == 'baseline')].copy()
    
    if real_runs.empty:
        print("Skipping Stage 5: No real bs prover runs found.")
        return

    # Sort by batch size
    real_runs = real_runs.sort_values('avg_batch_tx_count')

    # 1. stage5_batch_size_vs_proof_time.png
    plt.figure(figsize=(7.5, 4.5))
    plt.plot(real_runs['avg_batch_tx_count'], real_runs['avg_prove_ms'] / 1000.0, marker='o', color=COLORS['primary'], linewidth=2)
    plt.xlabel('Average Batch size (txs)')
    plt.ylabel('Proving Time (s)')
    plt.title('Stage 5: Batch Size vs. Proof Generation Time')
    plt.grid(True)
    plt.tight_layout()
    plt.savefig(out_dir / 'stage5_batch_size_vs_proof_time.png', dpi=160)
    plt.close()

    # 2. stage5_batch_size_vs_proof_time_per_tx.png
    plt.figure(figsize=(7.5, 4.5))
    prove_time_per_tx_s = (real_runs['avg_prove_ms'] / real_runs['avg_batch_tx_count']) / 1000.0
    plt.plot(real_runs['avg_batch_tx_count'], prove_time_per_tx_s, marker='s', color=COLORS['success'], linewidth=2)
    plt.xlabel('Average Batch size (txs)')
    plt.ylabel('Proving Time per Tx (s)')
    plt.title('Stage 5: Prover Amortization per Transaction')
    plt.grid(True)
    plt.tight_layout()
    plt.savefig(out_dir / 'stage5_batch_size_vs_proof_time_per_tx.png', dpi=160)
    plt.close()

    # 3. stage5_batch_size_vs_peak_memory.png
    plt.figure(figsize=(7.5, 4.5))
    plt.plot(real_runs['avg_batch_tx_count'], real_runs['max_memory_usage_mb'], marker='^', color=COLORS['warning'], linewidth=2)
    plt.xlabel('Average Batch size (txs)')
    plt.ylabel('Peak Prover Memory (MB)')
    plt.title('Stage 5: zkVM Prover Peak RAM Footprint')
    plt.grid(True)
    plt.tight_layout()
    plt.savefig(out_dir / 'stage5_batch_size_vs_peak_memory.png', dpi=160)
    plt.close()

    # 4. stage5_batch_size_vs_proven_tps.png
    plt.figure(figsize=(7.5, 4.5))
    plt.plot(real_runs['avg_batch_tx_count'], real_runs['goodput_tps'], marker='d', color=COLORS['accent'], linewidth=2)
    plt.xlabel('Average Batch size (txs)')
    plt.ylabel('Proven Throughput (TPS)')
    plt.title('Stage 5: Cryptographically Proven TPS')
    plt.grid(True)
    plt.tight_layout()
    plt.savefig(out_dir / 'stage5_batch_size_vs_proven_tps.png', dpi=160)
    plt.close()

    # Compare Mock vs Real (using Stage 1 mock runs vs Stage 5 real runs)
    stage1_res_path = metrics_root / "final_stage1_fixed_batching" / "analysis" / "all_results.csv"
    if stage1_res_path.exists():
        df_s1 = pd.read_csv(stage1_res_path)
        mock_runs = df_s1[df_s1['experiment_id'].str.match(r'^s1_bs_\d+$', na=False)].copy()
        mock_runs['batch_size_val'] = mock_runs['experiment_id'].str.extract(r's1_bs_(\d+)').astype(int)
        
        # We'll align by configured batch sizes (50, 100, 200/250, 500)
        # Stage 5 has batch sizes: 50, 100, 200, 500
        # Stage 1 has batch sizes: 50, 100, 250, 500
        real_runs['batch_size_config'] = real_runs['batch_size'].astype(int)
        mock_runs['batch_size_config'] = mock_runs['batch_size_val']
        
        merged = pd.merge(real_runs, mock_runs, on='batch_size_config', suffixes=('_real', '_mock'))
        
        if not merged.empty:
            merged = merged.sort_values('batch_size_config')
            x_labels = [f"BS {sz}" for sz in merged['batch_size_config']]
            x = np.arange(len(x_labels))
            width = 0.35
            
            # 5. stage5_mock_vs_real_finality_latency.png
            plt.figure(figsize=(7.5, 4.5))
            plt.bar(x - width/2, merged['avg_hard_finality_ms_mock'] / 1000.0, width, label='Mock Proof Mode', color=COLORS['gray'], edgecolor='black')
            plt.bar(x + width/2, merged['avg_hard_finality_ms_real'] / 1000.0, width, label='Real Proof Mode', color=COLORS['primary'], edgecolor='black')
            plt.ylabel('Hard Finality Latency (s)')
            plt.xlabel('Configured Batch Size')
            plt.title('Mock vs. Real Proof Mode: Finality Latency')
            plt.xticks(x, x_labels)
            plt.legend()
            plt.grid(axis='y')
            plt.tight_layout()
            plt.savefig(out_dir / 'stage5_mock_vs_real_finality_latency.png', dpi=160)
            plt.close()

            # 6. stage5_mock_vs_real_goodput.png
            plt.figure(figsize=(7.5, 4.5))
            plt.bar(x - width/2, merged['goodput_tps_mock'], width, label='Mock Proof Mode', color=COLORS['gray'], edgecolor='black')
            plt.bar(x + width/2, merged['goodput_tps_real'], width, label='Real Proof Mode', color=COLORS['primary'], edgecolor='black')
            plt.ylabel('Goodput (TPS)')
            plt.xlabel('Configured Batch Size')
            plt.title('Mock vs. Real Proof Mode: Goodput Throughput')
            plt.xticks(x, x_labels)
            plt.legend()
            plt.grid(axis='y')
            plt.tight_layout()
            plt.savefig(out_dir / 'stage5_mock_vs_real_goodput.png', dpi=160)
            plt.close()

    # 7. stage5_proof_failure_fallback_count.png
    plt.figure(figsize=(7.5, 4.5))
    # Count of failed batches or proof fallback runs (total_retries or fallback_count)
    plt.bar([f"BS {int(sz)}" for sz in real_runs['batch_size']], real_runs['failed_batches'], color=COLORS['danger'], edgecolor='black', width=0.4)
    plt.ylabel('Proving Failures / Retries Count')
    plt.xlabel('Configured Batch Size')
    plt.title('Stage 5: Prover Backend Robustness & Fallback Count')
    plt.grid(axis='y')
    plt.ylim(0, 5) # Scale to show 0 clearly
    plt.tight_layout()
    plt.savefig(out_dir / 'stage5_proof_failure_fallback_count.png', dpi=160)
    plt.close()

def main() -> None:
    args = parse_args()
    metrics_root = args.metrics_root
    
    if not metrics_root.exists():
        print(f"Error: Metrics root '{metrics_root}' does not exist.")
        return
        
    print(f"Generating benchmark plan plots using metrics folder: {metrics_root.resolve()}")
    
    plan_plots_root = metrics_root / "plan_plots"
    
    plot_stage1_plan(metrics_root, ensure_dir(plan_plots_root / "stage1"))
    plot_stage2_plan(metrics_root, ensure_dir(plan_plots_root / "stage2"))
    plot_stage3_plan(metrics_root, ensure_dir(plan_plots_root / "stage3"))
    plot_stage4_plan(metrics_root, ensure_dir(plan_plots_root / "stage4"))
    plot_stage5_plan(metrics_root, ensure_dir(plan_plots_root / "stage5"))
    
    print("Done! All required benchmarking plan plots have been successfully generated.")

if __name__ == "__main__":
    main()
