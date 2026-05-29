import pandas as pd
import json

df = pd.read_csv(r"c:\Lishan Dissanayake\4) Projects\FYP\rollupx-full-zk-rollup\benchmark-suite\metrics\final_stage4_da\analysis\all_results.csv")

cols = [
    "experiment_id",
    "da_mode",
    "policy",
    "tx_mix",
    "rate_tps",
    "timeout_ms",
    "blob_target_bytes",
    "blob_fill_target",
    "total_batches",
    "avg_batch_tx_count",
    "tps_committed",
    "avg_gas_per_tx",
    "avg_calldata_bytes",
    "avg_blob_utilization",
    "avg_queue_wait_ms",
    "avg_exec_ms",
    "avg_l2_l1_ms",
    "avg_total_cost_usd",
    "avg_cost_per_tx_usd"
]

sub_df = df[cols]
print(sub_df.to_string(index=False))
