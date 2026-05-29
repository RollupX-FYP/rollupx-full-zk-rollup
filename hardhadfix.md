Not fully. The necessary components **partly interact with Hardhat**, but based on the READMEs, I would not yet trust the Hardhat path as a clean benchmark backend without a verification pass.

## Current status

| Component        |                                                          Hardhat interaction status | Verdict                                                  |
| ---------------- | ----------------------------------------------------------------------------------: | -------------------------------------------------------- |
| Smart contracts  |         Hardhat compile, local deployment, local node, gas reporting are documented | Mostly OK                                                |
| Submitter        |       Requires Ethereum RPC and private key; records `gas_used` and `blob_gas_used` | Likely OK, but config must be checked                    |
| Sequencer        |          Talks to executor, not directly to Hardhat except L1 listener/forced queue | Partially relevant                                       |
| Executor/prover  |       Does not need Hardhat directly; produces proof/artifact consumed by submitter | OK                                                       |
| Benchmark suite  | Recreates Docker core stack, injects sequencer/submitter env vars, collects metrics | Good, but missing explicit L1/contract deployment wiring |
| Blob mode        |                       Contracts mention `BlobDA`; submitter mentions EIP-4844 blobs | Needs direct test                                        |
| Cost measurement |                      Submitter logs gas/blob gas; contracts have Hardhat gas report | Mostly OK, but needs joined validation                   |

The smart contract README clearly supports Hardhat for local deployment and simulation:

```bash
npx hardhat node
npx hardhat run scripts/deploy-local.ts --network localhost
npx hardhat test --network hardhat
```

It also says research-specific block times and reference gas prices are in `hardhat.config.ts`. 

The submitter is designed to talk to an Ethereum node such as Hardhat, Anvil, or Geth, using `SUBMITTER_PRIVATE_KEY`, `DATABASE_URL`, and gRPC connection to the executor.  It also treats `gas_used` and `blob_gas_used` as cost-analysis inputs. 

So the architecture is compatible with Hardhat. The risk is whether your **benchmark runner actually starts/deploys/configures Hardhat correctly for every run**.

---

# What is probably missing

## 1. Benchmark runner does not clearly deploy contracts per run

Your benchmark suite recreates the Docker `core` stack and injects:

```text
SEQUENCER_BATCH_MAX_SIZE
SEQUENCER_BATCH_TIMEOUT_MS
SEQUENCER_BATCH_MIN_SIZE
SEQUENCER_POLICY
SUBMITTER_DA_MODE
SUBMITTER_PROOF_BACKEND
EXPERIMENT_ID
EXPERIMENT_NAME
METRICS_DIR
```



But from the README snippet, I do **not** see these being injected:

```text
L1_RPC_URL
BRIDGE_ADDRESS
CALLDATA_DA_ADDRESS
BLOB_DA_ADDRESS
OFFCHAIN_DA_ADDRESS
VERIFIER_ADDRESS
SUBMITTER_PRIVATE_KEY
CHAIN_ID
CONFIRMATIONS
```

If those are not being set somewhere else, then the submitter may be running, but it may not be submitting to the correct Hardhat deployment.

### Required fix

Every benchmark run should have a setup step:

```text
1. Start/reset Hardhat node.
2. Deploy ZKRollupBridge, DA providers, verifier.
3. Write deployed addresses to l1_deployment.json.
4. Export those addresses into Docker Compose env.
5. Start sequencer/executor/submitter.
6. Run workload.
7. Verify submitter confirmed batches on Hardhat.
```

---

## 2. Hardhat must be reachable from Docker

Your pasted config includes a `host_docker` network using:

```ts
host_docker: {
  url: "http://l1-node:8545",
  chainId: 31337,
}
```



That is correct **only if** Docker Compose has a service named `l1-node` on the same network.

If Hardhat is running on your host machine instead, containers usually need:

```text
http://host.docker.internal:8545
```

or a Linux bridge workaround.

### Required check

From inside the submitter container:

```bash
docker exec -it rollupx-full-zk-rollup-submitter-1 sh -lc \
'curl -s -X POST http://l1-node:8545 \
-H "Content-Type: application/json" \
--data "{\"jsonrpc\":\"2.0\",\"method\":\"eth_chainId\",\"params\":[],\"id\":1}"'
```

Expected:

```json
{"jsonrpc":"2.0","id":1,"result":"0x7a69"}
```

`0x7a69` is chain ID `31337`.

---

## 3. Contract addresses must be visible to the submitter

The submitter can only submit batches if it knows the deployed bridge/DA/verifier addresses. The contracts README says deployment is done with:

```bash
npx hardhat run scripts/deploy-local.ts --network localhost
```



But the benchmark README does not show how the output of this deployment reaches the submitter.

### Required fix

Have deploy script write:

```json
{
  "chainId": 31337,
  "bridge": "0x...",
  "calldataDA": "0x...",
  "blobDA": "0x...",
  "offchainDA": "0x...",
  "verifier": "0x...",
  "deployer": "0x..."
}
```

to:

```text
benchmark-suite/metrics/<experiment>/<run>/l1_deployment.json
```

and also to a stable path like:

```text
contracts/deployments/hardhat-local.json
```

Then Docker Compose should inject:

```env
L1_RPC_URL=http://l1-node:8545
L1_CHAIN_ID=31337
ROLLUP_BRIDGE_ADDRESS=0x...
CALLDATA_DA_ADDRESS=0x...
BLOB_DA_ADDRESS=0x...
OFFCHAIN_DA_ADDRESS=0x...
VERIFIER_ADDRESS=0x...
```

---

## 4. Instant mining will distort latency results

The pasted Hardhat config has:

```ts
mining: {
  auto: true,
  interval: 0
}
```



That is fine for unit tests, but bad for latency benchmarking. With instant mining, L1 inclusion latency becomes almost zero, so your `submit_tx_duration_seconds` and `batch_e2e_duration_seconds` become unrealistically optimistic.

For research experiments, use a realistic block interval:

```ts
mining: {
  auto: true,
  interval: 12000
}
```

Claude’s pasted recommendation also says to use 12 seconds for latency experiments, not `interval: 0`. 

### Recommendation

Use two Hardhat profiles:

```text
hardhat_fast      -> interval 0, smoke tests only
hardhat_research  -> interval 12000, actual benchmark results
```

Never mix them in the same result table.

---

## 5. Blob mode needs a dedicated validation test

The contracts README lists `BlobDA` for EIP-4844 blob transactions.  The submitter README also says it supports EIP-4844 blobs and records `blob_gas_used`. 

But that does not prove blob transactions are actually being sent through Hardhat correctly.

### Required test

Run one forced blob-mode batch and assert:

```text
receipt.blobGasUsed > 0
receipt.blobGasPrice exists
submitter_metrics.da_mode == "blob"
submitter_metrics.blob_gas_used > 0
```

If `blob_gas_used` is empty or zero in blob mode, then your benchmark is not measuring real blob cost.

---

# Minimal end-to-end Hardhat verification checklist

Run this before trusting any benchmark result.

## Step 1: Start L1 node

```bash
npx hardhat node --hostname 0.0.0.0
```

or via Docker service `l1-node`.

## Step 2: Deploy contracts

```bash
npx hardhat run scripts/deploy-local.ts --network localhost
```

Expected output should include:

```text
ZKRollupBridge deployed at 0x...
CalldataDA deployed at 0x...
BlobDA deployed at 0x...
OffChainDA deployed at 0x...
Verifier deployed at 0x...
```

The contracts README confirms this is the intended Hardhat deployment path. 

## Step 3: Confirm submitter can see Hardhat

From submitter container:

```bash
curl -s -X POST $L1_RPC_URL \
-H "Content-Type: application/json" \
--data '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}'
```

Expected:

```text
0x7a69
```

## Step 4: Confirm submitter wallet is funded

```bash
curl -s -X POST $L1_RPC_URL \
-H "Content-Type: application/json" \
--data '{"jsonrpc":"2.0","method":"eth_getBalance","params":["<submitter_address>","latest"],"id":1}'
```

Expected: non-zero balance.

## Step 5: Run one smoke benchmark

```bash
bash benchmark-suite/scripts/run_matrix.sh --phase smoke
```

The benchmark suite already supports smoke runs and writes component metrics. 

## Step 6: Verify all three component metrics exist

```bash
RUN_DIR=benchmark-suite/metrics/<experiment>/<run>

ls -lah "$RUN_DIR"
wc -l "$RUN_DIR"/sequencer_batch_metrics.jsonl
wc -l "$RUN_DIR"/executor_batch_metrics.jsonl
wc -l "$RUN_DIR"/submitter_metrics.json
```

The benchmark README says these are the expected output files. 

## Step 7: Verify row parity

You need:

```text
sequencer sealed batches
≈ executor processed batches
≈ submitter confirmed/submitted batches
```

The benchmark suite already has a synchronization loop checking file stability and row parity between sequencer, executor, and submitter. 

## Step 8: Verify on-chain state changed

Call the bridge after the run:

```bash
npx hardhat console --network localhost
```

Then check something like:

```js
const bridge = await ethers.getContractAt("ZKRollupBridge", BRIDGE_ADDRESS);
await bridge.currentBatchId();
await bridge.stateRoot();
```

Expected:

```text
batch id increased
state root changed from genesis
```

---

# My judgment

## For calldata mode

Probably close to working, assuming:

```text
L1_RPC_URL is correct
bridge address is injected
submitter private key is funded
deployment happens before benchmark
```

## For blob mode

Not proven yet. You need a receipt-level test showing non-zero `blobGasUsed`.

## For off-chain DA mode

Not proven yet. You need to show that data is actually written to the archiver/off-chain store and that the on-chain commitment points to retrievable data.

## For cost benchmarking

Partly ready. The submitter logs `gas_used` and `blob_gas_used`, and Hardhat gas reporting is supported.  But you still need to ensure every benchmark row includes:

```text
gas_used
effective_gas_price
blob_gas_used
blob_gas_price
tx_count
da_mode
batch_id
```

Without those, cost-per-transaction plots will be incomplete.

---

# Required changes before final experiments

```text
1. Add Hardhat/L1 setup to run_experiment.sh.
2. Deploy contracts automatically per run or per experiment phase.
3. Write l1_deployment.json into each run folder.
4. Inject bridge/verifier/DA addresses into submitter container.
5. Ensure Docker containers can resolve the Hardhat RPC URL.
6. Use interval=12000 for research latency runs, interval=0 only for smoke tests.
7. Add receipt-level validation for calldata and blob submissions.
8. Fail the run if submitter confirms zero batches.
9. Fail blob-mode runs if blob_gas_used is missing or zero.
10. Add final on-chain bridge state check after each run.
```

So the answer is: **the components are architecturally compatible with Hardhat, but the current documentation does not prove the benchmark suite fully wires Hardhat into every run. Treat Hardhat integration as “partially implemented, needs validation and automation.”**
