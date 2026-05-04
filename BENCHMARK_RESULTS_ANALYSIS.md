# RollupX Benchmark Results Analysis

Source run: `benchmark-suite/metrics/run_20260504_025127`  
Experiment matrix: `benchmark-suite/config/experiments.toml`  
Analysis basis: raw `tx_log_*.csv`, `run_status.json`, `run_metadata.json`, `all_results.csv`, `stats_summary.csv`, `sensitivity_matrix.csv`, and generated figures in `figures/`.

## 1. Metrics Measured, Missing Metrics, and Useful Graphs

### Metrics actually present in this run

The aggregate file `all_results.csv` contains 20 experiment rows and 30 columns. All 20 runs passed and no transaction failures were recorded.

Measured or recorded fields:

| Category                               | Fields                                                                                                                                         |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Run identity                           | `experiment_id`, `run_id`, `run_status`, `git_commit`, `timestamp`, `seed`                                                                     |
| Config factors                         | `batch_size`, `timeout_ms`, `policy`, `da_mode`, `prover`, `rate_tps`, `tx_mix`                                                                |
| Workload outcome                       | `total_txs`, `success_txs`, `failed_txs`, `tx_count_A`, `tx_count_B`, `tx_count_C`, `duration_s_actual`                                        |
| Throughput                             | `tps_accepted`                                                                                                                                 |
| Per-class sequencer acceptance latency | `avg_latency_typeA_ms`, `p95_latency_typeA_ms`, `avg_latency_typeB_ms`, `p95_latency_typeB_ms`, `avg_latency_typeC_ms`, `p95_latency_typeC_ms` |
| Fairness                               | `jains_fairness`, `starvation_count`                                                                                                           |
| Raw transaction log fields             | `tx_id`, `tx_type`, `timestamp`, `latency`, `status`, `error`                                                                                  |

Important interpretation notes:

- The latency in `tx_log_*.csv` is submit-to-sequencer-response latency, not full L2-to-L1 or proof lifecycle latency.
- `tps_accepted` is the only usable throughput metric in this run.
- `tps_offered` exists but is `0` for every row, so it is not a usable measured value. Use `rate_tps` as the configured offered rate instead.
- `stats_summary.csv` only contains statistics for `tps_offered`, `tps_accepted`, Jain fairness, starvation, and per-class P95 latency.
- Each experiment has only one run row (`r01`, seed `42`). The config lists multiple seeds, but this run did not execute a repeated seed set. Statistical confidence is therefore weak.

### Missing or incomplete metrics

These were expected by the benchmark plan or generated reports, but are not present in this run:

| Missing area                | Missing metrics                                                                                    |
| --------------------------- | -------------------------------------------------------------------------------------------------- |
| Committed/sealed throughput | `tps_committed`, sealed batch count, proved tx count, published tx count                           |
| Batch lifecycle             | batch creation time, seal time, proof ready time, publish time, batch fullness, batch queue depth  |
| End-to-end latency          | submit-to-batch, batch-to-proof, proof-to-L1, L2-to-L1 confirmation, total submit-to-L1 latency    |
| Proof performance           | proof generation time, verification time, proof size, journal size, failed proof count             |
| L1 and DA cost              | gas per batch, gas per tx, calldata bytes, blob bytes, DA fee, gas saved, compression ratio        |
| Resource usage              | CPU, memory, disk, network, container-level resource saturation                                    |
| Robustness                  | retry count, timeout count, RPC errors, submitter receipt latency, executor lifecycle state counts |
| Experimental confidence     | repeated runs per configuration, confidence intervals, variance across seeds                       |
| Cross-factor design         | multi-factor combinations such as heavy workload plus non-FCFS policies                            |

Because these are missing, the run cannot support strong conclusions about gas cost, proof backend performance, DA cost, L2-to-L1 latency, or true committed rollup throughput.

### Graphs that can be generated from this run

Useful and valid from the available data:

| Graph                                 | Data source                    | What it shows                                           |
| ------------------------------------- | ------------------------------ | ------------------------------------------------------- |
| Accepted TPS by experiment            | `all_results.csv`              | Sequencer acceptance throughput per configuration       |
| Accepted TPS vs configured input rate | `rate_tps` + `tps_accepted`    | Scaling and saturation behavior                         |
| Acceptance ratio vs input rate        | `rate_tps` + `tps_accepted`    | How much of configured input rate was actually accepted |
| Success/failure count by experiment   | `success_txs`, `failed_txs`    | Reliability at the transaction submission layer         |
| Transaction mix distribution          | `tx_count_A/B/C`               | Whether generated workload matched intended mix         |
| Overall latency CDF / histogram       | raw `tx_log_*.csv`             | Submit-to-sequencer-response latency distribution       |
| P50/P95/P99 latency by experiment     | raw `tx_log_*.csv`             | Tail latency and outliers                               |
| Per-class latency bars                | per-class avg/P95 columns      | Whether A/B/C classes receive similar service           |
| Jain fairness by experiment           | `jains_fairness`               | Class-level fairness                                    |
| Starvation count by experiment        | `starvation_count`             | Outlier latency events over 3x mean                     |
| Delta vs baseline charts              | derived from `all_results.csv` | Sensitivity of accepted TPS, latency, and fairness      |

Graphs that should wait until missing metrics are instrumented:

| Graph                            | Why it is not valid yet                             |
| -------------------------------- | --------------------------------------------------- |
| Cost-vs-latency Pareto frontier  | gas and L2-to-L1 latency are absent                 |
| DA mode cost comparison          | gas/blob/calldata cost fields are absent            |
| Gas-per-tx heatmap               | gas fields are absent                               |
| Compression ratio heatmap        | compression ratio is absent                         |
| Proof backend comparison         | proof generation/verification metrics are absent    |
| True throughput-latency frontier | `tps_committed` and end-to-end latency are absent   |
| L2-to-L1 latency CDF             | submitter lifecycle latency is absent               |
| Executor lifecycle plots         | generated/proved/published lifecycle data is absent |

## 2. Insights from Raw Data Only

These insights come from the raw transaction logs and aggregate CSV values, not from the existing figures or thesis summary.

### Run-level health

- The benchmark submitted 23,420 successful transactions across 20 experiments.
- There were 0 failed transactions in the raw logs and `all_results.csv`.
- Only 3 starvation events were recorded across the entire run: `bs_010`, `pol_timeboost`, and `tps_050` each had 1 event.
- Across all successful transactions, submit-to-sequencer-response latency was low: average 1.884 ms, P50 1.913 ms, P95 2.445 ms, P99 2.808 ms.
- The maximum observed latency was 37.049 ms in `tps_050`, which is a clear outlier compared with the global P99 of 2.808 ms.

### Throughput behavior

| Experiment group | Raw-data observation                                                           |
| ---------------- | ------------------------------------------------------------------------------ |
| Baseline         | Accepted 1,018 txs over 120 s, or 8.483 TPS, under configured `rate_tps = 10`. |
| Input rate 5     | Accepted 537 txs, or 4.475 TPS, about 89.5% of configured rate.                |
| Input rate 20    | Accepted 1,867 txs, or 15.558 TPS, about 77.8% of configured rate.             |
| Input rate 50    | Accepted 3,643 txs, or 30.358 TPS, about 60.7% of configured rate.             |

The system scales upward as configured input rate increases, but not linearly. The acceptance ratio falls as the configured rate rises, which suggests the workload generator, sequencer, or local environment hits a throughput ceiling before reaching 50 TPS.

The high-rate run has lower P95 submit latency than baseline (`2.190 ms` vs `2.498 ms`) while accepting many more transactions. That does not mean end-to-end latency improves at higher load; it only means the measured sequencer response path stayed fast. Without batch, proof, and L1 publish timestamps, backlog after acceptance is invisible.

### Batch size

| Experiment | Batch size | Accepted TPS | Overall P95 latency | Jain fairness | Starvation |
| ---------- | ---------: | -----------: | ------------------: | ------------: | ---------: |
| baseline   |         50 |        8.483 |            2.498 ms |      0.999960 |          0 |
| `bs_010`   |         10 |        8.525 |            2.387 ms |      0.999957 |          1 |
| `bs_025`   |         25 |        8.508 |            2.424 ms |      0.999956 |          0 |
| `bs_100`   |        100 |        8.558 |            2.549 ms |      0.999796 |          0 |
| `bs_200`   |        200 |        8.475 |            2.430 ms |      0.999947 |          0 |

Batch size barely changed accepted TPS in this run. All batch-size variants stayed between 8.475 and 8.558 TPS. The largest batch size did not improve accepted throughput, and `bs_100` had the worst P95 latency among the batch-size variants. Since this run only measures sequencer acceptance latency, it cannot confirm the usual batch-size tradeoff between per-tx cost and finality latency.

### Timeout

| Experiment |  Timeout | Accepted TPS | Overall P95 latency | Jain fairness |
| ---------- | -------: | -----------: | ------------------: | ------------: |
| `to_0500`  |   500 ms |        8.533 |            2.447 ms |      0.999997 |
| `to_1000`  |  1000 ms |        8.475 |            2.411 ms |      0.999893 |
| `to_2500`  |  2500 ms |        8.542 |            2.449 ms |      0.999974 |
| baseline   |  5000 ms |        8.483 |            2.498 ms |      0.999960 |
| `to_10000` | 10000 ms |        8.525 |            2.511 ms |      0.999954 |

Timeout changes did not materially affect accepted TPS. Shorter timeouts look slightly better for fairness and latency in this run, but the differences are tiny and based on one sample per configuration.

### Scheduling policy

| Experiment      | Policy      | Accepted TPS | Overall P95 latency | Type C P95 | Jain fairness | Starvation |
| --------------- | ----------- | -----------: | ------------------: | ---------: | ------------: | ---------: |
| baseline        | FCFS        |        8.483 |            2.498 ms |   2.474 ms |      0.999960 |          0 |
| `pol_fee`       | FeePriority |        8.558 |            2.518 ms |   2.726 ms |      0.999802 |          0 |
| `pol_timeboost` | TimeBoost   |        8.583 |            2.546 ms |   2.807 ms |      0.999219 |          1 |
| `pol_fairbft`   | FairBFT     |        8.517 |            2.529 ms |   2.599 ms |      0.999706 |          0 |

The policy variants produced only small throughput differences. TimeBoost had the highest accepted TPS, but also the lowest fairness score and one starvation event. FeePriority improved accepted TPS slightly over baseline, but Type C P95 latency rose from 2.474 ms to 2.726 ms. This hints at a throughput/fairness tradeoff, but the effect size is very small and should not be treated as statistically proven.

### DA mode and prover setting

| Experiment    | Changed factor     | Accepted TPS | Overall P95 latency | Jain fairness |
| ------------- | ------------------ | -----------: | ------------------: | ------------: |
| baseline      | calldata + groth16 |        8.483 |            2.498 ms |      0.999960 |
| `da_blob`     | blob               |        8.508 |            2.445 ms |      0.999786 |
| `da_offchain` | offchain           |        8.483 |            2.425 ms |      0.999982 |
| `pv_plonk`    | plonk              |        8.492 |            2.426 ms |      0.999976 |

DA mode and prover setting had almost no visible effect on the measured sequencer acceptance path. That is expected if the benchmark does not capture batch publication, gas, DA bytes, proof generation, or proof verification. This run cannot support conclusions about blob DA savings or Groth16 vs Plonk proof performance.

### Transaction mix

| Experiment  | Mix      | A/B/C share           | Accepted TPS | Overall P95 latency | Jain fairness |
| ----------- | -------- | --------------------- | -----------: | ------------------: | ------------: |
| baseline    | balanced | 71.2% / 19.4% / 9.3%  |        8.483 |            2.498 ms |      0.999960 |
| `mix_light` | light    | 94.9% / 4.0% / 1.1%   |        8.483 |            2.421 ms |      0.999334 |
| `mix_heavy` | heavy    | 20.5% / 32.2% / 47.3% |        8.525 |            2.561 ms |      0.999929 |

The workload mix changed as intended. The heavy mix increased P95 latency relative to baseline, which is directionally plausible. The light mix had lower overall P95 latency but a lower Jain fairness score, likely because Type C had very few samples and a high P95 value. With only 11 Type C transactions in `mix_light`, its fairness result is fragile.

### Main raw-data conclusion

The benchmark successfully exercised sequencer acceptance under several one-factor-at-a-time configurations. The strongest raw result is that accepted throughput scales sublinearly with configured input rate, reaching about 30.36 TPS at configured 50 TPS. Most other factor changes produce small differences around the baseline, with fairness almost perfect in all cases.

The run is useful as a sequencer-acceptance smoke benchmark. It is not yet a full rollup lifecycle benchmark because it does not capture committed throughput, proof lifecycle, L1 publishing, gas, DA cost, or end-to-end finality latency.

## 3. What the Existing Generated Insights Say, and Whether They Match

### `thesis_summary.md`

The generated thesis summary says:

- 20 unique configurations were run.
- 20 total runs completed.
- 0 failed or excluded runs were observed.
- It lists each experiment with DA mode, policy, batch size, and fairness.
- It ranks top fairness as `to_0500`, `da_offchain`, and `pv_plonk`.
- It compares experiments against baseline only on Jain fairness and starvation count.
- It lists hypotheses about batch size, policy, blob DA, heavy tx instability, and transaction-aware policies, but leaves final supported/refuted/inconclusive decisions for manual analysis.

Match with raw-data analysis:

- Matches on run count, pass status, and no failed transactions.
- Matches on fairness ranking, but the summary rounds almost every fairness value to `1.00`, which hides the tiny differences.
- Partially matches the policy insight: TimeBoost and FeePriority have slightly higher accepted TPS but worse fairness than baseline.
- Does not provide the raw latency and input-rate insights above.
- Overreaches by referencing metrics such as `avg_gas_per_tx`, `avg_l2_l1_ms`, `tps_committed`, `avg_gas_saved`, and `failed_batches`; those metrics are not present in `all_results.csv`.
- It says several figures are "not yet generated", but figure files do exist in this run folder. The summary is therefore stale relative to the actual artifacts.

### `sensitivity_matrix.csv` and sensitivity figures

The sensitivity matrix has columns for:

- `TPS Committed`
- `Avg L2->L1 (ms)`
- `P95 L2->L1 (ms)`
- `Avg Prove (ms)`
- `Avg Gas/tx`
- `Compression Ratio`
- `Jain's Fairness`
- `Starvation Count`

However, only `Jain's Fairness` is populated. All other sensitivity columns are blank because the underlying metrics are missing from `all_results.csv`.

The populated fairness deltas say:

| Direction                     | Experiments                                                       |
| ----------------------------- | ----------------------------------------------------------------- |
| Slightly better than baseline | `to_0500`, `da_offchain`, `pv_plonk`, `to_2500`                   |
| Slightly worse than baseline  | most other experiments                                            |
| Largest fairness drops        | `pol_timeboost`, `mix_light`, `pol_fairbft`, `tps_050`, `da_blob` |

Match with raw-data analysis:

- Matches exactly for Jain fairness deltas.
- Does not cover throughput or latency, even though those are available in raw data.
- The heatmap visually implies a broad factor sensitivity report, but it is effectively a single-metric fairness heatmap with blank or unusable planned columns.

### Existing generated figures

Figure files present:

- `fairness_jains.png`
- `fairness_per_class.png`
- `pareto_cost_latency.png`
- `pareto_da_comparison.png`
- `sensitivity_heatmap.png`
- `sensitivity_jain's_fairness.png`
- `starvation.png`
- `throughput_by_batch_size.png`
- `throughput_by_da_mode.png`
- `throughput_by_policy.png`
- `throughput_by_rate.png`

What they say:

- `fairness_jains.png` shows all experiments near perfect fairness, with `pol_timeboost` and `mix_light` at the lower end.
- `starvation.png` shows one starvation event each for `bs_010`, `pol_timeboost`, and `tps_050`.
- `throughput_by_policy.png` shows TimeBoost and FeePriority slightly above baseline in accepted TPS.
- `pareto_cost_latency.png` is degenerate: points collapse around zero because cost and L2-to-L1 latency metrics are missing or defaulted.
- `sensitivity_heatmap.png` is mostly blank except Jain fairness.
- `throughput_by_rate.png` appears to show only baseline, so it misses the `tps_005`, `tps_020`, and `tps_050` experiments.
- `throughput_by_batch_size.png` also appears to show only baseline, so it misses the `bs_*` variants.

Match with raw-data analysis:

- Fairness and starvation figures match the raw-data findings.
- Policy throughput figure directionally matches the raw-data finding that TimeBoost and FeePriority are slightly higher than baseline.
- Pareto and DA/cost figures do not support meaningful conclusions because required metrics are absent.
- Throughput-by-rate and throughput-by-batch-size figures do not match the full raw-data analysis because they omit the relevant experiment IDs.

## 4. Recommended Next Steps

1. Fix `tps_offered` so it records configured or generated offered throughput instead of `0`.
2. Update throughput plots to group `tps_*` and `bs_*` experiment IDs correctly.
3. Add submitter/executor lifecycle logs to produce `tps_committed`, L2-to-L1 latency, proof time, gas, DA cost, and compression metrics.
4. Run all configured seeds or set `seeds = [42]` in the experiment config to avoid implying repeat coverage that was not executed.
5. Treat the current run as valid evidence for sequencer acceptance latency and basic fairness only.
