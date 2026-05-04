import argparse
import os

import matplotlib.pyplot as plt
import pandas as pd


def _scatter(df: pd.DataFrame, x: str, y: str, out: str) -> None:
    if x not in df.columns or y not in df.columns:
        return
    clean = df[[x, y]].dropna()
    if clean.empty:
        return
    plt.figure(figsize=(8, 5))
    plt.scatter(clean[x], clean[y], s=12, alpha=0.7)
    plt.xlabel(x)
    plt.ylabel(y)
    plt.tight_layout()
    plt.savefig(out, dpi=150)
    plt.close()


def main() -> None:
    parser = argparse.ArgumentParser(description="Batch-level feasibility plots")
    parser.add_argument("--input", required=True, help="all_batch_results.csv")
    parser.add_argument("--output_dir", required=True, help="figure output directory")
    args = parser.parse_args()

    df = pd.read_csv(args.input)
    os.makedirs(args.output_dir, exist_ok=True)
    _scatter(df, "tx_count", "batch_data_bytes", os.path.join(args.output_dir, "batch_data_bytes_vs_tx_count.png"))
    _scatter(df, "tx_count", "state_diff_count", os.path.join(args.output_dir, "state_diff_count_vs_tx_count.png"))
    _scatter(df, "tx_count", "unique_touched_accounts", os.path.join(args.output_dir, "unique_touched_accounts_vs_tx_count.png"))
    _scatter(df, "tx_count", "execution_time_ms", os.path.join(args.output_dir, "execution_time_vs_tx_count.png"))
    _scatter(df, "tx_count", "proof_time_ms", os.path.join(args.output_dir, "proof_time_vs_tx_count.png"))
    _scatter(df, "tx_count", "l1_gas_used", os.path.join(args.output_dir, "l1_gas_used_vs_tx_count.png"))
    _scatter(df, "tx_count", "blob_utilization", os.path.join(args.output_dir, "blob_utilization_vs_tx_count.png"))
    _scatter(df, "tx_count", "l1_latency_ms", os.path.join(args.output_dir, "l1_latency_vs_tx_count.png"))


if __name__ == "__main__":
    main()
