#!/usr/bin/env bash
# run_pipeline.sh — orchestrate data tools pipeline

set -euo pipefail

# ── args ──────────────────────────────────────────────────────────────────────
EXP_ID=${1:-}

METRICS_ROOT=${METRICS_ROOT:-"benchmark-suite/metrics"}
FIGURES_DIR="$METRICS_ROOT/figures"

echo "======================================================================"
echo "  Running Data Tools Pipeline"
if [ -n "$EXP_ID" ]; then
    echo "  Filtering for Experiment: $EXP_ID"
fi
echo "  Metrics Root: $METRICS_ROOT"
echo "  Figures Dir:  $FIGURES_DIR"
echo "======================================================================"

echo "── 1. Aggregating metrics ──────────────────────────────────────────────"
python3 data-tools/aggregate.py --metrics_root "$METRICS_ROOT" --output "$METRICS_ROOT/all_results.csv" --include_failed

if [ ! -f "$METRICS_ROOT/all_results.csv" ]; then
    echo "Error: all_results.csv not found. Aggregation failed."
    exit 1
fi

echo "── 2. Computing statistics ─────────────────────────────────────────────"
python3 data-tools/stats.py --input "$METRICS_ROOT/all_results.csv" --output "$METRICS_ROOT/stats_summary.csv"

echo "── 3. Generating plots ─────────────────────────────────────────────────"
PLOT_ERRORS=0

run_plot() {
    local name="$1"; shift
    if ! "$@" 2>&1; then
        echo "  [WARN] $name failed (non-fatal, continuing)"
        PLOT_ERRORS=$((PLOT_ERRORS + 1))
    fi
}

run_plot "pareto_frontier" python3 data-tools/plots/pareto_frontier.py --input "$METRICS_ROOT/all_results.csv" --output_dir "$FIGURES_DIR"
run_plot "throughput_bar"   python3 data-tools/plots/throughput_bar.py --input "$METRICS_ROOT/all_results.csv" --output_dir "$FIGURES_DIR"
run_plot "latency_cdf"      python3 data-tools/plots/latency_cdf.py --metrics_root "$METRICS_ROOT" --output_dir "$FIGURES_DIR"
run_plot "latency_boxplot"  python3 data-tools/plots/latency_boxplot.py --input "$METRICS_ROOT/all_results.csv" --output_dir "$FIGURES_DIR"
run_plot "fairness"         python3 data-tools/plots/fairness.py --input "$METRICS_ROOT/all_results.csv" --output_dir "$FIGURES_DIR"
run_plot "cost_heatmap"     python3 data-tools/plots/cost_heatmap.py --input "$METRICS_ROOT/all_results.csv" --output_dir "$FIGURES_DIR"
run_plot "sensitivity"      python3 data-tools/plots/sensitivity.py --input "$METRICS_ROOT/all_results.csv" --output_dir "$FIGURES_DIR"
run_plot "final_report"     python3 data-tools/plots/final_report_graphs.py --results "$METRICS_ROOT/all_results.csv" --batch_results "$METRICS_ROOT/all_batch_results.csv" --output_dir "$FIGURES_DIR"

if [ "$PLOT_ERRORS" -gt 0 ]; then
    echo "  [NOTE] $PLOT_ERRORS plot(s) had warnings (likely insufficient data for that chart type)"
fi

echo "── 4. Generating report ────────────────────────────────────────────────"
python3 data-tools/report/generate_md.py --input "$METRICS_ROOT/all_results.csv" --stats "$METRICS_ROOT/stats_summary.csv" --output "$METRICS_ROOT/thesis_summary.md" --figures_dir "$FIGURES_DIR"

echo "======================================================================"
echo "  Pipeline completed successfully."
echo "  Outputs:"
echo "    CSV:     $METRICS_ROOT/all_results.csv"
echo "    Stats:   $METRICS_ROOT/stats_summary.csv"
echo "    Plots:   $FIGURES_DIR/"
echo "    Report:  $METRICS_ROOT/thesis_summary.md"
echo "======================================================================"
