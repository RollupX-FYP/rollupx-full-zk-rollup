#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
SESSION_DIR="${1:-}"
MODE="${2:-local}"

if [[ -z "$SESSION_DIR" ]]; then
    echo "Usage: bash scripts/generate_plan_artifacts.sh <session_dir> [local|docker]"
    exit 1
fi

if [[ ! -d "$SESSION_DIR" ]]; then
    echo "[artifacts] session directory not found: $SESSION_DIR"
    exit 1
fi

ANALYSIS_DIR="${SESSION_DIR}/analysis"
FIGURES_DIR="${SESSION_DIR}/figures"
mkdir -p "$ANALYSIS_DIR" "$FIGURES_DIR"

echo "[artifacts] session: $SESSION_DIR"
echo "[artifacts] analysis: $ANALYSIS_DIR"
echo "[artifacts] figures: $FIGURES_DIR"

if [[ "$MODE" == "docker" ]]; then
    export METRICS_DIR="$SESSION_DIR"
    (
        cd "$PROJECT_ROOT"
        docker compose --profile report build data-tools --no-cache
        docker compose --profile report run -T --rm data-tools
    )
    exit 0
fi

(
    cd "$PROJECT_ROOT"
    python3 data-tools/aggregate.py \
        --metrics_root "$SESSION_DIR" \
        --output "${ANALYSIS_DIR}/all_results.csv"

    python3 data-tools/stats.py \
        --input "${ANALYSIS_DIR}/all_results.csv" \
        --output "${ANALYSIS_DIR}/stats_summary.csv"

    python3 data-tools/plots/pareto_frontier.py \
        --input "${ANALYSIS_DIR}/all_results.csv" \
        --output_dir "$FIGURES_DIR"

    python3 data-tools/plots/throughput_bar.py \
        --input "${ANALYSIS_DIR}/all_results.csv" \
        --output_dir "$FIGURES_DIR"

    python3 data-tools/plots/latency_cdf.py \
        --metrics_root "$SESSION_DIR" \
        --output_dir "$FIGURES_DIR"

    python3 data-tools/plots/latency_boxplot.py \
        --input "${ANALYSIS_DIR}/all_results.csv" \
        --output_dir "$FIGURES_DIR"

    python3 data-tools/plots/fairness.py \
        --input "${ANALYSIS_DIR}/all_results.csv" \
        --output_dir "$FIGURES_DIR"

    python3 data-tools/plots/cost_heatmap.py \
        --input "${ANALYSIS_DIR}/all_results.csv" \
        --output_dir "$FIGURES_DIR"

    python3 data-tools/plots/sensitivity.py \
        --input "${ANALYSIS_DIR}/all_results.csv" \
        --output_dir "$FIGURES_DIR"

    if [[ -f "${ANALYSIS_DIR}/all_batch_results.csv" ]]; then
        python3 data-tools/plots/batch_feasibility.py \
            --input "${ANALYSIS_DIR}/all_batch_results.csv" \
            --output_dir "$FIGURES_DIR"
    fi

    python3 data-tools/report/generate_md.py \
        --input "${ANALYSIS_DIR}/all_results.csv" \
        --stats "${ANALYSIS_DIR}/stats_summary.csv" \
        --output "${ANALYSIS_DIR}/thesis_summary.md" \
        --figures_dir "$FIGURES_DIR"
)

echo "[artifacts] generated:"
echo "  ${ANALYSIS_DIR}/all_results.csv"
echo "  ${ANALYSIS_DIR}/stats_summary.csv"
echo "  ${ANALYSIS_DIR}/thesis_summary.md"
echo "  ${FIGURES_DIR}/"
