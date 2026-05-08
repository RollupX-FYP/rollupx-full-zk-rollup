#!/usr/bin/env python3
"""
Generate a comprehensive analysis report from benchmark metrics.
Usage: python3 generate_analysis_report.py <metrics_root>
Output: <metrics_root>/analysis_report.md
"""

import json
import sys
from pathlib import Path
from datetime import datetime


def load_json(path):
    """Load JSON file, return None if not found."""
    if not Path(path).exists():
        return None
    try:
        with open(path, 'r') as f:
            return json.load(f)
    except Exception as e:
        return None


def load_jsonl(path):
    """Load JSONL file, return list of objects."""
    if not Path(path).exists():
        return []
    try:
        with open(path, 'r') as f:
            return [json.loads(line) for line in f if line.strip()]
    except Exception as e:
        return []


def generate_report(metrics_root):
    """Generate analysis report from metrics files."""
    metrics_path = Path(metrics_root)
    
    # Load all metrics
    status = load_json(metrics_path / "run_status.json")
    metadata = load_json(metrics_path / "run_metadata.json")
    workload = load_json(metrics_path / "workload_smoke_one_tx_calldata.json") or load_json(
        metrics_path / "workload_smoke_one_tx_calldata.json"
    )
    
    # Handle dynamic workload filename
    workload_files = list(metrics_path.glob("workload_*.json"))
    if not workload and workload_files:
        workload = load_json(workload_files[0])
    
    sequencer_metrics = load_jsonl(metrics_path / "sequencer_batch_metrics.jsonl")
    executor_metrics = load_jsonl(metrics_path / "executor_batch_metrics.jsonl")
    submitter_metrics = load_json(metrics_path / "submitter_metrics.json")
    l1_validation = load_json(metrics_path / "l1_state_validation.json")
    l1_deployment = load_json(metrics_path / "l1_deployment.json")
    resource_metrics = load_json(metrics_path / "resource_metrics.json")
    
    # Build report as markdown
    lines = []
    
    lines.append("# Benchmark Run Analysis Report")
    lines.append("")
    
    # Overall Status
    if status:
        status_badge = "✅ **PASS**" if status.get('status') == 'pass' else "❌ **FAIL**"
        lines.append(f"**Status:** {status_badge}")
        lines.append("")
        lines.append(f"**Run ID:** `{status.get('run_id', 'N/A')}`")
        lines.append("")
        lines.append(f"**Timestamp:** {status.get('timestamp', 'N/A')}")
        lines.append("")
        
        # Summary metrics
        lines.append("## Summary Metrics")
        lines.append("")
        lines.append("| Metric | Value |")
        lines.append("|--------|-------|")
        lines.append(f"| Total Transactions | {status.get('total_txs', 0)} |")
        lines.append(f"| Successful | {status.get('success_txs', 0)} |")
        lines.append(f"| Failed | {status.get('failed_txs', 0)} |")
        lines.append(f"| Success Rate | {status.get('success_rate', 0) * 100:.1f}% |")
        lines.append("")
    
    # Runtime
    if metadata:
        lines.append("## Execution Timeline")
        lines.append("")
        lines.append(f"- **Start:** {metadata.get('timestamp_start', 'N/A')}")
        lines.append(f"- **End:** {metadata.get('timestamp_end', 'N/A')}")
        
        if metadata.get('machine'):
            machine = metadata['machine']
            lines.append("")
            lines.append("### Machine Specs")
            lines.append("")
            lines.append(f"- **CPU:** {machine.get('cpu_model', 'N/A')} ({machine.get('cpu_cores', '?')} cores)")
            lines.append(f"- **RAM:** {machine.get('ram_gb', 'N/A')} GB")
            lines.append(f"- **OS:** {machine.get('os', 'N/A')}")
        
        if metadata.get('config_snapshot'):
            config = metadata['config_snapshot']
            lines.append("")
            lines.append("### Configuration")
            lines.append("")
            lines.append("| Parameter | Value |")
            lines.append("|-----------|-------|")
            lines.append(f"| Batch Size | {config.get('batch_size', 'N/A')} |")
            lines.append(f"| Timeout | {config.get('timeout_ms', 'N/A')}ms |")
            lines.append(f"| Policy | {config.get('policy', 'N/A')} |")
            lines.append(f"| DA Mode | {config.get('da_mode', 'N/A')} |")
            lines.append(f"| Prover | {config.get('prover', 'N/A')} |")
            lines.append(f"| Rate | {config.get('rate_tps', 'N/A')} TPS |")
            lines.append(f"| Duration | {config.get('duration_s', 'N/A')}s |")
        lines.append("")
    
    # Resource Metrics
    if resource_metrics:
        lines.append("### Resource Usage")
        lines.append("")
        lines.append("| Metric | Value |")
        lines.append("|--------|-------|")
        lines.append(f"| Max Memory (MB) | {resource_metrics.get('max_memory_usage_mb', 0):,.0f} |")
        lines.append(f"| Max Memory (GB) | {resource_metrics.get('max_memory_usage_gb', 0):.2f} |")
        lines.append("")
    
    # Sequencer Analysis
    lines.append("## Sequencer Component")
    lines.append("")
    if sequencer_metrics:
        lines.append(f"**Status:** ✅ **WORKING**")
        lines.append("")
        lines.append(f"- **Batches Produced:** {len(sequencer_metrics)}")
        
        total_txs = sum(b.get('tx_count', 0) for b in sequencer_metrics)
        total_gas = sum(b.get('total_gas_limit', 0) for b in sequencer_metrics)
        lines.append(f"- **Total Transactions Batched:** {total_txs}")
        lines.append(f"- **Total Gas:** {total_gas:,}")
        
        if sequencer_metrics:
            batch = sequencer_metrics[0]
            lines.append("")
            lines.append("### First Batch Details")
            lines.append("")
            lines.append("| Metric | Value |")
            lines.append("|--------|-------|")
            lines.append(f"| Batch ID | {batch.get('batch_id', 'N/A')} |")
            lines.append(f"| TX Count | {batch.get('tx_count', 0)} |")
            lines.append(f"| Seal Reason | {batch.get('seal_reason', 'N/A')} |")
            lines.append(f"| Policy | {batch.get('scheduling_policy', 'N/A')} |")
            lines.append(f"| Gas Utilization | {batch.get('gas_limit_utilization', 0) * 100:.4f}% |")
            lines.append(f"| Wait Time (mean) | {batch.get('wait_time_mean_ms', 0):.2f}ms |")
            lines.append(f"| Fairness Index | {batch.get('jains_fairness_index', 1.0):.4f} |")
            lines.append(f"| Reordering Events | {batch.get('reordering_events', 0)} |")
            lines.append(f"| Cache Hit Rate | {batch.get('cache_hit_rate', 0) * 100:.1f}% |")
        lines.append("")
    else:
        lines.append("**Status:** ⚠️ **MISSING METRICS**")
        lines.append("")
    
    # Executor Analysis
    lines.append("## Executor Component")
    lines.append("")
    if executor_metrics:
        lines.append(f"**Status:** ✅ **WORKING**")
        lines.append("")
        lines.append(f"- **Proofs Generated:** {len(executor_metrics)}")
        
        # Separate real and padding proofs
        real_proofs = [p for p in executor_metrics if p.get('tx_count', 0) > 0]
        padding_proofs = [p for p in executor_metrics if p.get('tx_count', 0) == 0]
        
        lines.append(f"  - Real Transaction Proofs: {len(real_proofs)}")
        lines.append(f"  - Padding/Empty Proofs: {len(padding_proofs)}")
        
        total_proof_time = sum(p.get('prover_metrics', {}).get('total_prover_wall_ms', 0) for p in executor_metrics)
        total_exec_time = sum(p.get('execution_phases', {}).get('total_execution_ms', 0) for p in executor_metrics)
        
        lines.append(f"- **Total Proof Generation Time:** {total_proof_time / 1000:.1f}s")
        lines.append(f"- **Total Execution Time:** {total_exec_time:.2f}ms")
        
        if real_proofs:
            proof = real_proofs[0]
            prover = proof.get('prover_metrics', {})
            exec_phases = proof.get('execution_phases', {})
            lines.append("")
            lines.append("### First Real Proof Details")
            lines.append("")
            lines.append("| Metric | Value |")
            lines.append("|--------|-------|")
            lines.append(f"| TX Count | {proof.get('tx_count', 0)} |")
            lines.append(f"| State Diffs | {proof.get('state_diff_count', 0)} |")
            lines.append(f"| Execution Time | {exec_phases.get('total_execution_ms', 0):.3f}ms |")
            lines.append(f"| Proof Mode | {prover.get('proof_mode', 'N/A')} |")
            lines.append(f"| ZK VM Execution | {prover.get('zkvm_execution_ms', 0) / 1000:.1f}s |")
            lines.append(f"| Cycles | {prover.get('total_cycles', 0):,} |")
            lines.append(f"| Proof Bytes | {prover.get('proof_bytes', 0)} |")
        lines.append("")
    else:
        lines.append("**Status:** ⚠️ **MISSING METRICS**")
        lines.append("")
    
    # Submitter Analysis
    lines.append("## Submitter Component")
    lines.append("")
    if submitter_metrics:
        lines.append(f"**Status:** ✅ **WORKING**")
        lines.append("")
        if isinstance(submitter_metrics, list):
            sub = submitter_metrics[0] if submitter_metrics else {}
        else:
            sub = submitter_metrics
        
        status_str = "✅ Submitted" if sub.get('submission_status') == 'submitted' else f"❌ {sub.get('submission_status', 'Unknown')}"
        lines.append(f"- **Submission Status:** {status_str}")
        lines.append(f"- **TX Hash:** `{sub.get('tx_hash', 'N/A')}`")
        lines.append(f"- **DA Mode:** {sub.get('da_mode', 'N/A')}")
        
        lines.append("")
        lines.append("### Finality & Latency")
        lines.append("")
        lines.append("| Metric | Value |")
        lines.append("|--------|-------|")
        lines.append(f"| L1 Gas Used | {sub.get('l1_gas_used', 0):,} |")
        lines.append(f"| Submission Latency | {sub.get('submission_latency_ms', 0):.0f}ms |")
        lines.append(f"| Soft Finality | {sub.get('soft_commit_ms', 0):.0f}ms |")
        lines.append(f"| Hard Finality | {sub.get('hard_finality_ms', 0):.0f}ms |")
        lines.append(f"| Confirmation Blocks | {sub.get('confirmation_blocks', 0)} |")
        
        if sub.get('total_cost_usd'):
            lines.append("")
            lines.append("### Cost Analysis")
            lines.append("")
            lines.append("| Metric | Value |")
            lines.append("|--------|-------|")
            lines.append(f"| Total Cost | $`{sub.get('total_cost_usd', 0):.6f}` |")
            lines.append(f"| Cost per TX | $`{sub.get('cost_per_tx_usd', 0):.6f}` |")
            lines.append(f"| Gas Price (Gwei) | {sub.get('regular_gas_price_gwei', 0):.6f} |")
            
            lines.append("")
            lines.append("#### Cost Breakdown")
            lines.append("")
            lines.append("| Component | Percentage |")
            lines.append("|-----------|------------|")
            lines.append(f"| Proof Verification | {sub.get('proof_verify_pct', 0):.1f}% |")
            lines.append(f"| Data Availability | {sub.get('da_pct', 0):.1f}% |")
            lines.append(f"| Overhead | {sub.get('overhead_pct', 0):.1f}% |")
        lines.append("")
    else:
        lines.append("**Status:** ⚠️ **MISSING METRICS**")
        lines.append("")
    
    # Workload Analysis
    lines.append("## Workload Generator")
    lines.append("")
    if workload:
        lines.append("**Status:** ✅ **WORKING**")
        lines.append("")
        details = workload.get('details', {})
        latency = workload.get('latency_metrics', {})
        
        lines.append("| Metric | Value |")
        lines.append("|--------|-------|")
        lines.append(f"| Total Transactions | {details.get('total_txs', 0)} |")
        lines.append(f"| Successful | {details.get('successful_txs', 0)} |")
        lines.append(f"| Failed | {details.get('failed_txs', 0)} |")
        lines.append(f"| Rate | {details.get('rate', 0):.1f} TPS |")
        lines.append(f"| Duration | {details.get('duration', 0)}s |")
        
        if workload.get('tx_mix'):
            lines.append(f"| User Action Latency | {latency.get('user_action_latency_ms', 0):.2f}ms |")
        lines.append("")
        
        if workload.get('tx_mix'):
            mix = workload['tx_mix']
            lines.append("### Transaction Mix")
            lines.append("")
            for tx_type, pct in mix.items():
                lines.append(f"- **Type {tx_type}:** {pct * 100:.1f}%")
        lines.append("")
    else:
        lines.append("**Status:** ⚠️ **MISSING METRICS**")
        lines.append("")
    
    # L1 State
    lines.append("## L1 Bridge State")
    lines.append("")
    if l1_validation:
        lines.append("**Status:** ✅ **VALIDATED**")
        lines.append("")
        lines.append("| Parameter | Value |")
        lines.append("|-----------|-------|")
        lines.append(f"| Chain ID | {l1_validation.get('chainId', 'N/A')} |")
        lines.append(f"| Bridge Address | `{l1_validation.get('bridge', 'N/A')}` |")
        lines.append(f"| Next Batch ID | {l1_validation.get('nextBatchId', 'N/A')} |")
        lines.append(f"| State Root Changed | {l1_validation.get('stateRootChanged', False)} |")
        
        if l1_validation.get('daProviders'):
            lines.append("")
            lines.append("### Data Availability Providers")
            lines.append("")
            for mode, addr in l1_validation['daProviders'].items():
                lines.append(f"- **{mode}:** `{addr}`")
        lines.append("")
    else:
        lines.append("**Status:** ⚠️ **NOT VALIDATED**")
        lines.append("")
    
    # Component Status Summary
    lines.append("## Component Status Summary")
    lines.append("")
    lines.append("| Component | Status |")
    lines.append("|-----------|--------|")
    lines.append(f"| Sequencer | {'✅ WORKING' if sequencer_metrics else '❌ MISSING'} |")
    lines.append(f"| Executor | {'✅ WORKING' if executor_metrics else '❌ MISSING'} |")
    lines.append(f"| Submitter | {'✅ WORKING' if submitter_metrics else '❌ MISSING'} |")
    lines.append(f"| L1 Bridge | {'✅ WORKING' if l1_validation else '❌ NOT VALIDATED'} |")
    lines.append("")
    
    # Validation Summary
    lines.append("## Validation Summary")
    lines.append("")
    all_working = bool(sequencer_metrics and executor_metrics and submitter_metrics and l1_validation and status and status.get('status') == 'pass')
    if all_working:
        lines.append("### ✅ **ALL SYSTEMS OPERATIONAL**")
        lines.append("")
        lines.append("All components are working correctly. End-to-end pipeline is functional.")
    else:
        lines.append("### ⚠️ **CHECK LOGS FOR ISSUES**")
        lines.append("")
        lines.append("Some components may have issues. Review the logs and diagnostics folder.")
    lines.append("")
    
    lines.append("---")
    lines.append("")
    lines.append(f"*Report generated: {datetime.now().isoformat()}*")
    
    return "\n".join(lines)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python3 generate_analysis_report.py <metrics_root>", file=sys.stderr)
        sys.exit(1)
    
    metrics_root = sys.argv[1]
    report = generate_report(metrics_root)
    
    # Write report
    output_file = Path(metrics_root) / "analysis_report.md"
    with open(output_file, 'w', encoding='utf-8') as f:
        f.write(report)
    
    # Print to stdout (skip if encoding issues)
    try:
        print(report)
    except UnicodeEncodeError:
        sys.stdout.reconfigure(encoding='utf-8')
        print(report)
    
    print(f"\n[report] written → {output_file}")
