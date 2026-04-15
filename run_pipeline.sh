#!/usr/bin/env bash
# run_pipeline.sh — orchestrate data tools pipeline

set -euo pipefail

# ── args ──────────────────────────────────────────────────────────────────────
EXP_ID=${1:-}

METRICS_ROOT=${METRICS_ROOT:-"benchmark-suite/metrics"}

echo "======================================================================"
echo "  Running Data Tools Pipeline"
if [ -n "$EXP_ID" ]; then
    echo "  Filtering for Experiment: $EXP_ID"
fi
echo "  Metrics Root: $METRICS_ROOT"
echo "======================================================================"

echo "── 1. Aggregating metrics ──────────────────────────────────────────────"
python3 data-tools/aggregate.py --metrics_root "$METRICS_ROOT" --output "$METRICS_ROOT/all_results.csv" --include_failed

if [ ! -f "$METRICS_ROOT/all_results.csv" ]; then
    echo "Error: all_results.csv not found. Aggregation failed."
    return 1
fi

echo "── 2. Computing statistics ─────────────────────────────────────────────"
python3 data-tools/stats.py --input "$METRICS_ROOT/all_results.csv" --output "$METRICS_ROOT/stats_summary.csv"

echo "── 3. Generating plots ─────────────────────────────────────────────────"
python3 data-tools/plots/pareto_frontier.py --input "$METRICS_ROOT/all_results.csv"
python3 data-tools/plots/throughput_bar.py --input "$METRICS_ROOT/all_results.csv"
python3 data-tools/plots/latency_cdf.py --metrics_root "$METRICS_ROOT"
python3 data-tools/plots/latency_boxplot.py --input "$METRICS_ROOT/all_results.csv"
python3 data-tools/plots/fairness.py --input "$METRICS_ROOT/all_results.csv"
python3 data-tools/plots/cost_heatmap.py --input "$METRICS_ROOT/all_results.csv"
python3 data-tools/plots/sensitivity.py --input "$METRICS_ROOT/all_results.csv"

echo "── 4. Generating report ────────────────────────────────────────────────"
python3 data-tools/report/generate_md.py --input "$METRICS_ROOT/all_results.csv" --stats "$METRICS_ROOT/stats_summary.csv" --output "$METRICS_ROOT/thesis_summary.md"

echo "======================================================================"
echo "  Pipeline completed successfully."
echo "======================================================================"
