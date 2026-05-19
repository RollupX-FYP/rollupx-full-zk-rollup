import csv

with open(r'c:\Lishan Dissanayake\4) Projects\FYP\rollupx-full-zk-rollup\benchmark-suite\metrics\final_stage3_policy\analysis\all_results.csv', 'r') as f:
    reader = csv.reader(f)
    header = next(reader)
    rows = list(reader)

for row in rows:
    exp_id = row[0]
    if exp_id in ['s3_pol_fcfs', 's3_pol_feepriority', 's3_pol_timeboost', 's3_pol_fairbft', 's3_pol_blobpacking']:
        print(f"\n=== Experiment: {exp_id} ===")
        for col_name, val in zip(header, row):
            # Print columns that are relevant to performance, gas, and latency
            if any(x in col_name for x in ['policy', 'tps', 'gas', 'latency', 'wait', 'exec', 'prove', 'finality', 'count', 'cost']):
                print(f"  {col_name}: {val}")
