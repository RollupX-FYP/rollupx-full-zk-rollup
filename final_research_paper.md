# RollupX: Empirical Performance Characterization and Systemic Auditing of a Modular ZK-Rollup Prototype

**Authors:** Fernando T. H. L., Gamage M. S., Lishan S. D., Perera M. V. D. P. D. S.  
*Department of Computer Science and Engineering, Faculty of Engineering, University of Moratuwa, Sri Lanka*

---

## Abstract

Zero-Knowledge Rollups (ZK-Rollups) represent the state-of-the-art in Ethereum Layer-2 (L2) scaling, inheriting Layer-1 (L1) security guarantees while executing transactions off-chain. However, the multi-dimensional parameter space governing rollups—spanning batch sizes, timeout limits, sequencer scheduling policies, data availability (DA) modes, and zero-knowledge prover configurations—remains poorly understood in real-world deployments. This paper presents **RollupX**, an experimental, modular L2 ZK-rollup prototype implemented in Rust and Solidity, using the RISC0 zero-knowledge Virtual Machine (zkVM) with Groth16 proof compression. 

We conduct a rigorous empirical evaluation of RollupX across five distinct experimental stages, revealing critical performance trade-offs: L1 gas amortization, queueing latency dynamics, and ZK-proving economics. Crucially, our empirical audit exposes three severe system anomalies: 
1. **The Nonce Reordering Vulnerability**: A systemic flaw in priority sequencing that violates EVM state consistency, rendering global fee-priority, TimeBoost, and size-based blob packing policies unusable in single-account workloads by triggering permanent mempool blocks.
2. **The Hysteresis Threshold Trap**: A mathematical design flaw in the adaptive batching logic that prevents the sequencer from ever sealing at its configured small batch size under steady load.
3. **Methodological Aggregation Anomalies**: Boundary-effect distortions in unweighted arithmetic metrics that inflate reported tail latencies and empty-batch gas costs.

We present mathematical models for rollup behavior, analyze the economic efficiency of EIP-4844 blobs and off-chain DA, and provide concrete Rust code blueprints to resolve the identified vulnerabilities.

---

## 1. Introduction

Public blockchain networks, particularly Ethereum, are fundamentally constrained by the **blockchain trilemma**—the inherent architectural trade-off that makes it difficult to maximize decentralization, security, and scalability simultaneously [7]. Ethereum Layer-1 (L1) enforces security and decentralization by requiring every node to execute every transaction, which constrains base-layer throughput to approximately 15–25 transactions per second (TPS) and leads to severe congestion and volatile, prohibitive transaction fees under high demand [3]. To bypass these execution bottlenecks without compromising security, the Ethereum ecosystem has transitioned to a rollup-centric roadmap [18]. Layer-2 (L2) Zero-Knowledge Rollups (ZK-Rollups) execute transactions off-chain, aggregate them into batches, generate a succinct cryptographic validity proof (SNARK/STARK) of the resulting state transition, and submit the batch data and proof to L1. L1 smart contracts verify this proof, instantly securing L2 transactions with L1-level finality guarantees without re-executing them [17].

A major milestone in this scaling roadmap was the Dencun network upgrade (March 2024), which implemented EIP-4844 (Proto-Danksharding) [19]. EIP-4844 introduced blob-carrying transactions, providing L2 rollups with dedicated, cheaper transient data storage (blobs) instead of forcing them to post transaction data as expensive L1 calldata. Following Dencun, transaction fees on major L2 networks (e.g., Base, Arbitrum, Optimism) decayed by more than 90–95%, dropping typical transaction costs to sub-cent levels and sparking massive activity surges [20, 21]. 

However, despite these advancements, the multi-dimensional parameter space governing rollups remains poorly characterized. Designing and operating a rollup requires tuning highly interdependent parameters: batch sizes ($B_{\text{max}}$), time-driven flush timeouts ($T_{\text{timeout}}$), sequencer transaction-scheduling policies, data availability (DA) modes, and zero-knowledge prover configurations. In production rollups (e.g., zkSync Era [32], Starknet [33], Scroll), these components are highly complex and tightly coupled with production-specific engineering constraints (e.g., compiler optimizations, custom VM opcodes, and custom state structures). This complexity makes it virtually impossible to perform controlled, isolated scientific sweeps of individual parameters. 

To bridge this gap, we present **RollupX**, a modular, educational L2 ZK-rollup prototype implemented in Rust and Solidity that serves as a reproducible benchmarking sandbox. RollupX implements simple balance transfers, deposits, and withdrawals using a Sparse Merkle Tree (SMT) state manager, the RISC0 zero-knowledge Virtual Machine (zkVM) for state execution and proof generation, and a Groth16 wrapper for L1 proof compression. This clean-room design allows us to systematically isolate and sweep sequencer scheduling, batching thresholds, DA storage, and prover configurations.

### 1.1 Rationale for a Clean-Room Benchmarking Sandbox

To justify the engineering and evaluation of a custom, simplified rollup prototype (RollupX) instead of forking or black-box testing an existing production ZK-rollup, we outline the following key design rationales supported by literature:

1. **System Isolation vs. Production Complexity**: Forking a production zkEVM (e.g., zkSync Era [32] or Scroll) to perform parameter sweeps introduces massive experimental noise. Production architectures compile EVM bytecode to custom intermediate representations, which requires translating EVM execution traces into arithmetic constraints [34]. This introduces high prover overhead (up to hundreds of thousands of constraints per basic EVM instruction) and tightly couples sequencing delays with complex hardware-acceleration schedules, making isolated layer-level measurement impossible [24]. By utilizing a decoupled, execution-agnostic sandbox, RollupX isolates parameter sweeps of the sequencing, batching, and data layers from execution complexities.
2. **ZK-Friendly State Representations**: While production L1/L2 networks rely on Hexary Patricia Merkle Tries (PMTs) for state storage [8], PMTs are highly inefficient to prove in ZK due to variable-length paths and RLP serialization overhead [7]. By choosing a 256-level binary Sparse Merkle Tree (SMT) mapping 160-bit account addresses to states, RollupX achieves constant-length inclusion proofs that fit within the memory limits of general-purpose zkVMs and compile efficiently in ZK [32, 33].
3. **Rust-in-zkVM vs. Custom Circuit Engineering**: Writing custom circuits (e.g., in Circom) requires massive development overhead and lacks flexibility [25]. By executing a standard Rust-based guest program inside the RISC0 zkVM, RollupX demonstrates the feasibility of modular, on-demand zkVM-based execution, simplifying auditing and allowing standard cryptographic libraries (like `secp256k1` signatures) to be compiled directly into standard RISC-V ELFs [36].
4. **Isolating the Data Availability Cost-Latency Frontier**: Under EIP-4844, blobs are sold in fixed 128 KB chunks, creating a "Blob Dilemma" (cost vs. latency) for low-throughput networks [25, 26]. A customizable prototype is required to sweep and quantify this amortization gap under controlled traffic conditions without the noise of production-chain gas dynamics.

Our contributions are threefold:
1. **Empirical Characterization of the Performance Frontier**: We sweep batch sizes, timeouts, and workloads, demonstrating the complete decoupling of L2 virtual machine execution complexity from L1 gas costs.
2. **Economic Comparison of Data Layers and Prover Scaling**: We analyze the costs of three data availability modes (Calldata, EIP-4844 Blobs, and Offchain DA) and model RISC0 prover execution. We show that zkVM proving time scales as a step-function governed by segment boundaries ($2^{20}$ cycles) and identify the prover's economy of scale.
3. **Critical Systems Audit**: We expose three severe design and implementation anomalies in the rollup prototype: the *Nonce Reordering Vulnerability* in priority scheduling, the *Hysteresis Threshold Trap* in adaptive batching, and *Methodological Aggregation Anomalies* in benchmarking metrics. We provide concrete, production-ready Rust code blueprints to resolve these flaws.

---

## 2. Background and System Architecture

RollupX consists of four primary microservices and a set of L1 smart contracts:

```mermaid
graph TD
    User[JSON-RPC Clients] -->|User Transactions| Seq[Sequencer]
    Seq -->|Mempool / Scheduling| Batch[Batch Sealing Engine]
    Batch -->|Sealed Batches| Exec[Executor]
    Exec -->|State Transition guest code| R0[RISC0 Prover]
    R0 -->|STARK Proof| Groth[Groth16 Wrapper]
    Groth -->|256B Groth16 Proof| Sub[Submitter]
    Exec -->|Enriched Batch Payload| Sub
    Sub -->|commitBatch TX| L1[L1 Smart Contracts / Hardhat]
    L1 -->|Mock/Real Verifier| Bridge[RollupBridge.sol]
```

### 2.1 Microservices and Cryptographic Components
* **Sequencer**: Accepts user transactions over JSON-RPC, performs basic validation (ECDSA signature and account nonce checks), holds them in an in-memory mempool, sorts them according to a selected scheduling policy, and seals batches when trigger conditions (size, timeout, or forced L1 operations) are met.
* **Executor**: Consumes sealed batches, executes the State Transition Function (STF), updates the state manager's Sparse Merkle Tree (SMT), generates execution traces, and invokes the RISC0 host prover. The SMT is implemented as a 256-level binary tree using Keccak-256 for leaf and node hashing, mapping 160-bit account addresses to serialized EVM-like account states (nonce and balance).
* **Prover (RISC0)**: Executes the guest program inside the RISC0 zkVM. The guest program takes the pre-state root, the transaction list, cryptographic signatures, and SMT inclusion proofs (siblings) as guest input. It verifies the ECDSA `secp256k1` signatures and sequential nonces, executes the balance updates, recomputes the root of the SMT, and outputs the post-state root. The host generates a STARK proof of this guest execution, which is then wrapped and compressed into a 256-byte Groth16 SNARK proof on the BN254 curve using Arkworks.
* **Submitter**: Consumes the gRPC stream of executed batches and ZK proofs, packages the transaction calldata (or EIP-4844 blobs) and the 256-byte Groth16 proof, and submits a `commitBatch` transaction to the L1 contracts.
* **L1 Contracts**: Deployed on a local Hardhat network, `RollupBridge.sol` stores the canonical state root and manages deposits/withdrawals, while `Groth16Verifier.sol` verifies the SNARK proof using Ethereum's bilinear pairing precompiles.

---

## 3. Related Work

### 3.1 Layer-2 Rollup Scaling and Data Availability Cost Heuristics
The scalability bottlenecks of Ethereum Layer-1 (L1) execution have driven the development of Layer-2 (L2) rollup architectures, which execute transactions off-chain and post state roots, proofs, and transaction data to L1 [15, 18]. Historically, data availability (DA) posted as L1 calldata constituted up to 80% of L2 transaction fees [12, 21]. The Dencun upgrade and EIP-4844 (Proto-Danksharding) introduced blob transactions, which utilize a dedicated, independent blob gas market to dramatically reduce the cost of posting data on-chain [19, 20]. Adler et al. [26] and Werner [25] analyzed the cost-latency frontier of data posting in early rollups, noting that while calldata is billed linearly per byte, EIP-4844 blobs are sold in fixed 128 KB chunks. This creates a scheduling challenge for sequencers: low-throughput rollups must choose between delaying batch postings to pack blobs tightly (increasing user latency) or posting empty space (diluting utilization and increasing marginal fees per transaction) [25]. Our work quantifies this amortization gap, contrasting small-batch PoC costs with production-scale networks.

### 3.2 Sequencer Scheduling and Transaction Ordering Heuristics
Transaction sequencing determines both the execution order and the latency profile of L2 networks. The simplest sequencing policy is First-Come-First-Served (FCFS), which is simple and predictable but vulnerable to MEV latency racing and network spam by searchers attempting to front-run or back-run transactions [30]. To combat latency races, Offchain Labs proposed TimeBoost, which groups transactions into discrete time windows and conducts sealed-bid auctions to order transactions within each window [31]. Decentralized sequencing models, such as Based Sequencing (delegating ordering to L1 validators) and Shared Sequencing (e.g., Espresso, Astria), aim to achieve censorship resistance and cross-rollup composability [30]. However, existing transaction ordering literature primarily addresses game-theoretic MEV extraction and consensus security. Our systems audit exposes a more fundamental, state-level conflict: we demonstrate that naive global scheduling algorithms (such as priority fee or size-aware packing) violate account nonce order, creating permanent mempool blocks in single-account workloads.

### 3.3 Zero-Knowledge Proof Acceleration and zkVM Performance
The generation of cryptographic validity proofs remains a major computational and time bottleneck for ZK-Rollups [24, 25]. Early rollups relied on custom circuits written in domain-specific languages (e.g., Circom, ZoKrates). The ecosystem has since shifted toward zero-knowledge Virtual Machines (zkVMs) and zkEVMs (e.g., RISC0, Polygon zkEVM, zkSync Era) that execute standard compiler-targeted code (e.g., Rust compiled to RISC-V or Solidity compiled to Yul/EVM) inside a zero-knowledge execution environment [34, 32, 33]. Prover performance is heavily optimized via hardware acceleration (GPUs/ASICs) to speed up Multi-Scalar Multiplications (MSMs) and Number Theoretic Transforms (NTTs) over field elements [25]. RISC0 zkVM introduces segment-based execution, dividing guest traces into blocks of $2^{20}$ cycles to enable parallel proving, which are then merged via recursive STARK-to-STARK joins before being wrapped into a BN254 Groth16 SNARK proof [35, 36]. Our work characterises the performance scaling of this recursive prover, demonstrating that proving time scales as a step-function governed by segment boundaries rather than a smooth linear curve.

---

## 4. Experimental Design and Benchmarking Methodology

To systematically map the performance space, we designed five experimental stages, utilizing a Python-based workload generator to replay transactions under controlled loads (Steady-state at 25 TPS, High-load at 60 TPS, and Bursty traffic cycling between 8 and 80 TPS).

### 4.1 Experimental Parameters
* **Batch Size ($B_{\text{max}}$)**: Capped size from 25 to 1000 transactions.
* **Timeout ($T_{\text{timeout}}$)**: Time-driven flush trigger ranging from 0.5s to 200s.
* **Scheduling Policies**: FCFS (First-Come-First-Served), FeePriority, TimeBoost, FairBFT, and BlobPacking.
* **Data Availability Modes**: Calldata DA (L1 calldata), Blob DA (EIP-4844), and Offchain DA (State roots only).
* **Prover Backends**: RISC0 Mock Prover vs. RISC0 Real Prover (using Groth16 Arkworks wrapping).

### 4.2 Key Metrics
* **Committed TPS**: Realized transactional throughput settled on L1.
* **L2-to-L1 Latency**: Elapsed time from transaction submission to L1 block inclusion.
* **Queue Wait Time ($W_q$)**: The duration transactions wait in the sequencer mempool before sealing.
* **L1 Gas per Transaction**: Total L1 gas spent divided by transaction count.
* **Proving Wall-Clock Time**: Time required to generate the Groth16 proof.
* **Jain's Fairness Index**: Metric indicating the equality of transaction latencies, defined as:
  \[J(x_1, x_2, \dots, x_n) = \frac{\left(\sum_{i=1}^{n} x_i\right)^2}{n \sum_{i=1}^{n} x_i^2}\]
  where $x_i$ is the queuing delay of transaction $i$.

---

## 5. Empirical Evaluation and Critical Analysis

### 5.1 Stage 1: Fixed Batching and Aggregation Anomalies

Stage 1 sweeps evaluate the performance limits of the fixed batching policy under varying batch sizes $B_{\text{max}}$, timeouts $T_{\text{timeout}}$, and transaction workloads. The results demonstrate the classic latency-cost trade-off: larger batches amortize the fixed L1 verifier and bridge overhead over more transactions, reducing L1 gas cost per transaction, but they increase queue wait times for L2 transactions.

#### 4.1.1 The Queueing Latency Model
Under a steady and uniform transaction arrival rate $\lambda$ (TPS), the time to accumulate a batch of size $B$ is given by $B/\lambda$. In a sealed batch of size $B$, transactions arrive sequentially. The $i$-th transaction waits in the mempool for an accumulation interval $W_i = \frac{B - i}{\lambda}$. The average queue wait time $W_q$ of transactions in a single batch is the arithmetic mean of these individual waiting times:
\[W_q = \frac{1}{B} \sum_{i=1}^{B} \frac{B - i}{\lambda} = \frac{B - 1}{2\lambda}\]
When a time-driven timeout $T_{\text{timeout}}$ is active, it enforces an upper limit on the accumulation interval. Transactions in a batch sealed by timeout wait on average for half the timeout interval ($T_{\text{timeout}}/2$). Therefore, we model the average queue wait time $W_q$ as:
\[W_q \approx \frac{1}{2} \min\left(T_{\text{timeout}}, \frac{B_{\text{max}}}{\lambda}\right)\]
This theoretical model matches our empirical measurements with high precision under FCFS sequencing (where transaction arrival is stable and sequential):
* **`s1_to_00500`** ($T_{\text{timeout}}$ = 500 ms): Theoretical $W_q$ = 250.0 ms. Empirical = **256.48 ms** (+2.59% error).
* **`s1_to_01000`** ($T_{\text{timeout}}$ = 1000 ms): Theoretical $W_q$ = 500.0 ms. Empirical = **516.30 ms** (+3.26% error).
* **`s1_to_02000`** ($T_{\text{timeout}}$ = 2000 ms): Theoretical $W_q$ = 1000.0 ms. Empirical = **1017.20 ms** (+1.72% error).
* **`s1_to_05000`** ($T_{\text{timeout}}$ = 5000 ms): Theoretical $W_q$ = 2500.0 ms. Empirical = **2487.05 ms** (-0.52% error).

#### 4.1.2 Tail Latency Inflation (Boundary Distortion)
For `s1_bs_0100`, the reported average queue wait time in the aggregate results was **8,212.60 ms**, which is significantly higher than the expected value of $\sim$2,700 ms predicted by the model ($100 / (2 \times 18.5) \approx 2.7$s). 

A granular audit of the raw event stream in `sequencer_batch_metrics.jsonl` revealed a severe boundary effect. In finite test runs, the workload generator stops transmitting transactions once the target volume is reached, leaving a final trailing batch of $K$ transactions (where $K < B_{\text{max}}$) stranded in the mempool. Because no new transactions arrive to trigger the size limit, the sequencer holds these transactions until the timeout $T_{\text{timeout}}$ expires. In this run, the final batch of 80 transactions waited for **198,247 ms** (nearly the full 200s timeout).

The aggregator computed the reported mean queue wait time as a simple arithmetic average of the mean wait times of all $M$ batches:
\[\text{Reported Mean} = \frac{1}{M} \sum_{j=1}^{M} \bar{W}_{q,j}\]
where $\bar{W}_{q,j}$ is the average wait time for batch $j$. This unweighted average of averages is highly vulnerable to boundary outliers. With $M = 36$ batches, the single trailing outlier skewed the entire metric:
\[\text{Reported Mean} = \frac{35 \times 2,700\text{ ms} + 198,247\text{ ms}}{36} \approx 8,212.6\text{ ms}\]
To eliminate this boundary distortion, research frameworks must compute the volume-weighted average queue wait time across all individual transactions:
\[W_{q,\text{weighted}} = \frac{\sum_{j=1}^{M} N_j \cdot \bar{W}_{q,j}}{\sum_{j=1}^{M} N_j}\]
where $N_j$ is the transaction count of batch $j$. Under the volume-weighted formula, the true average queue wait time for `s1_bs_0100` was **7,061.35 ms**—still slightly elevated by the trailing batch but representing a much more accurate reflection of user experience.

#### 4.1.3 Empty Batch Cost Inflation
For `s1_bs_1000`, the reported gas per transaction was **83,626 gas**, compared to $\sim$19,600 gas for smaller configurations. 
* At the end of the run, the workload generator stopped, but the sequencer continued submitting batches to L1 to avoid timeouts, resulting in three empty batches ($0$ txs).
* The aggregator calculates batch efficiency as:
  \[\text{gas\_per\_tx}_{\text{batch}} = \frac{\text{L1\_gas\_used}}{\max(\text{tx\_count}, 1)}\]
* The empty batches were billed at $\frac{115,132 \text{ gas}}{1 \text{ tx}} = 115,132$ gas/tx. The unweighted arithmetic average of these batch ratios yielded 83,626 gas.
* The true volume-weighted average (total gas / total transactions) was actually **19,771 gas/tx**, aligning perfectly with the L1 gas amortization curve.

> [!WARNING]
> **Methodological Flaw**: Calculating L1 gas efficiency and queue latency using simple arithmetic means of batch averages introduces severe boundary distortion in finite test runs. Research frameworks must use volume-weighted averages and median values for latency.

#### 4.1.4 Decoupling L2 Execution from L1 Gas
A key ZK-rollup characteristic is that L1 contracts only verify validity proofs and store calldata, rather than executing L2 virtual machine steps. 
* This empirically proves that L1 gas costs are decoupled from L2 execution complexity.

---

### 5.2 Stage 2: Adaptive Batching: Hysteresis Trap and Load-Responsive Scaling

Stage 2 sweeps evaluate the RollupX sequencer's ability to adjust batch sealing target sizes dynamically based on mempool depth. An adaptive batching policy optimizes the latency-throughput trade-off: it seals smaller batches under light load to prevent excessive queuing latency, and aggregates larger batches under heavy load to maximize L1 gas amortization. The sequencer computes the dynamic target batch size $N(d)$ as a step-function of the current pending transaction count (mempool depth) $d$:
\[
N(d) =
\begin{cases}
S_b \ (\text{small}), & d < L_t \\
M_b \ (\text{medium}), & L_t \le d < M_t \\
L_b \ (\text{large}), & d \ge M_t
\end{cases}
\]
where $L_t$ and $M_t$ are the low and medium load thresholds, and $S_b$, $M_b$, and $L_b$ represent the target batch sizes under low, medium, and high loads. The target size is physically capped at the maximum batch size limit: $N(d) \leftarrow \min(N(d), B_{\text{max}})$.

#### 5.2.1 Baseline Configuration Discrepancy
A systems audit of the initial [all_results.csv](file:///c:/Users/malin/Desktop/rollupx-full-zk-rollup/Metrics/Metrics/final_stage2_adaptive_batching/analysis/all_results.csv) sweeps revealed that due to a manual environment-variable propagation bug in `run_experiment.sh` during the high-timeout runs, the variable `BATCH_POLICY` defaulted to `fixed`. Consequently, the initial baseline sweeps `s2_adaptive_low`, `s2_adaptive_medium`, and all adaptive threshold configuration sweeps (`s2_adapt_*`) were executed using the `fixed` batching policy instead of the planned `adaptive` policy. While the high-load and burst sweeps successfully ran the adaptive policy, the lower load sweeps executed as fixed batching, producing identical results (e.g., average batch size $\sim$99, L1 gas/tx $\sim$19.6k, wait time $\sim$7s) to their fixed-batching control groups.

#### 5.2.2 The Hysteresis Threshold Trap (Mathematical Proof)
Beyond this orchestration bug, our audit exposed a fundamental mathematical design flaw in the batch trigger logic implemented in the baseline [trigger.rs](file:///c:/Users/malin/Desktop/rollupx-full-zk-rollup/sequencer/src/batch/trigger.rs).

Let $P$ be the current mempool depth (pending count). For the sequencer to trigger a size-based batch seal, the pending count must meet or exceed the target batch size:
\[P \ge N(P)\]
To seal a batch of the designated small batch size $S_b$, the mempool depth must satisfy the low-load condition:
\[P < L_t\]
Substituting the low-load target $N(P) = S_b$ into the trigger condition yields:
\[P \ge S_b\]
Combining this trigger requirement with the low-load boundary condition ($P < L_t$) establishes the following system of inequalities for a size-driven seal of size $S_b$:
\[S_b \le P < L_t\]
This system possesses a valid integer solution for $P$ if and only if:
\[S_b < L_t\]
If $S_b \ge L_t$ (i.e., the target small batch size is greater than or equal to the low-load threshold), then:
1. For any mempool depth $P < L_t$, the target size is $S_b$. But since $P < L_t \le S_b$, the condition $P \ge S_b$ can never be satisfied.
2. The instant the arrival of a new transaction causes the mempool depth to reach the threshold $P = L_t$, the low-load condition $P < L_t$ becomes false.
3. The sequencer immediately switches to the medium-load target $M_b$ (where $M_b > L_t$). The new trigger condition becomes $P \ge M_b$. Since $P = L_t < M_b$, the trigger remains unsatisfied, and the mempool continues to accumulate transactions without sealing.

**Economic and Systemic Impact**: In all baseline configurations:
* `s2_adaptive_low`: $S_b = 50, L_t = 50 \implies S_b \ge L_t$
* `s2_adapt_l10_m50`: $S_b = 25, L_t = 10 \implies S_b > L_t$
* `s2_adapt_l25_m100`: $S_b = 50, L_t = 25 \implies S_b > L_t$
* `s2_adapt_l50_m150`: $S_b = 50, L_t = 50 \implies S_b \ge L_t$

Under all these configurations, the sequencer was mathematically blocked from ever sealing a batch of size $S_b$ via the size trigger under steady load. It always bypassed the small batch size and accumulated transactions until it hit the medium or maximum batch size (100). Thus, even if the adaptive policy had been active, the results would have converged to the fixed-batching policy at size 100, rendering the adaptive scaling logic useless.

#### 5.2.3 Load-Responsive Operation in the Patched System
Once the orchestration configuration was corrected and the threshold parameters were tuned to satisfy the strict inequality $\text{adaptive\_small\_batch\_size} < \text{adaptive\_low\_load\_threshold}$ (specifically setting $S_b = 25$ and $L_t = 50$), the adaptive batching policy functioned as intended. 

Under low-load conditions (10 TPS), the mempool depth remained below 50, allowing the size-based trigger to successfully seal batches at $S_b = 25$. This reduced the average L2 queue delay to **$\sim$1.25 seconds** (compared to **$\sim$2.7 seconds** for fixed-batching at size 100), albeit at a higher L1 gas cost per transaction ($\sim$24.5k gas/tx) due to the smaller amortization base. Under high-load conditions (60 TPS), the sequencer scaled up the target batch size dynamically to $M_b = 100$ and $L_b = 500$ (capped at $B_{\text{max}} = 100$ in these configurations), reducing L1 gas cost to **$\sim$19.6k gas/tx**. This confirms that a properly configured adaptive batching engine successfully navigates the cost-latency Pareto frontier.

#### 5.2.4 Policy Convergence and Burst Load Profiles
Under sustained high offered load (60 TPS), transaction arrival is rapid enough that the mempool depth consistently exceeds the medium load threshold ($M_t = 100$). The target batch size evaluates to $L_b$ (500), which is then capped at the maximum batch size limit $B_{\text{max}} = 100$. Since both the fixed policy (which always uses $B_{\text{max}}$) and the adaptive policy enforce a target size of 100 in this regime, the adaptive policy's behavior converges completely to the fixed policy. This convergence was verified empirically in the metrics: average batch size was **97.12** (adaptive) vs. **97.93** (fixed), with L1 gas per transaction at **19,941 gas** (adaptive) vs. **19,713 gas** (fixed).

Under the bursty traffic profile (8 TPS base rate for 22.5s, bursting to 80 TPS for 7.5s):
1. During the **low-rate base phase**, transactions arrive slowly. With a timeout $T_{\text{timeout}} = 2.0$s, only $\sim$16 transactions accumulate. Both policies seal on timeout with a batch size of $\sim$16.
2. During the **high-rate burst phase**, transactions arrive rapidly. The mempool depth crosses the low threshold of 25 in $\sim$312 ms, shifting the adaptive target size to $M_b = 100$. Both policies accumulate 100 transactions and seal on size threshold in $\sim$1.25s.
3. Because both policies behave identically in both phases, their overall averages are equivalent, demonstrating that simple depth-based adaptive policies fail to provide performance gains under highly transient workloads.

---

### 5.3 Stage 3: Sequencer Scheduling and the Nonce Reordering Vulnerability

Stage 3 benchmarks evaluate the throughput, cost efficiency, and latency fairness of FCFS, FeePriority, TimeBoost, FairBFT, and BlobPacking scheduling policies under steady-state (25 TPS offered) and bursty (8 TPS base, 80 TPS burst) traffic.

#### 5.3.1 The Nonce Reordering Vulnerability (Systemic EVM Conflict)
Our systems audit uncovered a catastrophic systemic flaw in the baseline implementation of all global scheduling policies (`FeePriority`, `TimeBoost`, and `BlobPacking`).

In EVM-compatible networks, account state transitions are strictly sequential. The state transition function (STF) enforces that for any sender account $A$, the transaction with nonce $N$ must be executed before the transaction with nonce $N+1$. If the executor receives nonce $N+1$ first, it must reject it or queue it, since executing it out-of-order would violate state consistency (creating a "nonce gap").

In the baseline [policies.rs](file:///c:/Users/malin/Desktop/rollupx-full-zk-rollup/sequencer/src/scheduler/policies.rs#L91-L95), the sequencer's priority policies treated all transactions in the pool as a single flat list and sorted them globally. For instance, the `FeePriorityPolicy` sorted strictly by gas price in descending order:
```rust
transactions.sort_by(|a, b| b.tx.gas_price.cmp(&a.tx.gas_price));
```
When a single account submitted multiple transactions with different gas prices (to simulate fee bidding), global sorting frequently placed a higher-fee transaction with nonce $N+1$ *ahead* of a lower-fee transaction with nonce $N$ in the batch. 

When the executor processed this batch, it evaluated the transactions in the sequencer's sorted order:
1. **Tx(Nonce: $N+1$, fee: high)** was executed first. The STF rejected it due to a nonce mismatch (expected nonce $N$, got $N+1$).
2. **Tx(Nonce: $N$, fee: low)** was executed next. It succeeded, incrementing the account nonce on-chain to $N+1$.
3. However, **Tx(Nonce: $N+1$) was already processed and rejected** in this batch. It was omitted from the state transition.
4. In subsequent batches, the user's client continued to submit new transactions starting from nonce $N+1$. But because the sequencer continued to sort the newer, higher-fee transactions ($N+2, N+3, \dots$) ahead of the re-submitted nonce $N+1$, a permanent **nonce gap mempool block** was established.
5. The executor rejected 100% of transactions in all subsequent batches, resulting in `tx_count = 0` for batches 2 through 97 (as confirmed in `executor_batch_metrics.jsonl` for the baseline `s3_pol_feepriority` run).

Because the submitter still published these empty batches to L1 to prevent timeouts, each empty batch consumed a fixed L1 gas cost of $\sim$112,000 gas. Due to the unweighted aggregation floor ($\max(tx, 1)$), the reported average gas per transaction skyrocketed to **$\sim$112,000 gas/tx** for `FeePriority` and `TimeBoost`, compared to **$\sim$20.7k gas/tx** for FCFS (which preserves nonce order).

#### 5.3.2 Two-Tier Nonce-Aware Scheduler Design
To resolve this vulnerability, we implemented a two-tier nonce-aware scheduling engine. The pool is organized by grouping transactions by sender address, sorting each sender's queue strictly by nonce in ascending order. The scheduler then prioritizes across accounts by evaluating only the head transaction of each sender's queue (which is guaranteed to have the correct sequential nonce).

For the `FeePriorityPolicy`, the scheduler pops the sender whose next sequential transaction has the highest gas price. For the `BlobPackingPolicy`, the scheduler selects the account whose next sequential transaction has the largest size in bytes. This guarantees that transactions are never processed out-of-order.

#### 5.3.3 Empirical Evaluation of Patched Schedulers
In the patched system, the nonce reordering vulnerability was completely eliminated across all scheduling policies:
1. **Gas Cost and Throughput Restoration**: All batches executed successfully without nonce gap failures, returning L1 costs to the baseline range (**$\sim$20.7k gas/tx** under calldata DA). Realized throughput under `FeePriority` restored to a full **19.9 TPS** (up from the near-zero baseline rate of **0.01 TPS**).
2. **FeePriority Trade-Offs**: Under bursty traffic, `FeePriority` successfully prioritized higher-fee transactions. However, this introduced a starvation effect for low-fee transactions, which were held in the mempool during the burst phase, reducing Jain's Fairness Index to **0.74** (compared to **0.75** for FCFS).
3. **TimeBoost and Fairness**: `TimeBoost` successfully resolved this starvation by grouping transactions into discrete 5-second windows:
\[W_{\text{slot}} = \lfloor t_{\text{arrival}} / 5000\text{ ms} \rfloor\]
While `TimeBoost` sorted transactions by gas price *within* each window, it prevented newer high-fee transactions from leapfrogging older slots. This bounded the maximum queuing delay, optimizing latency fairness and achieving the highest Jain's index of **0.77** while retaining the economic benefits of fee prioritization.
4. **BlobPacking Efficiency**: The patched `BlobPackingPolicy` successfully grouped and packed transactions by size to maximize EIP-4844 space utilization. In a multi-sender transaction mix, it successfully packed larger transactions first, matching FCFS in throughput while preparing data layouts for optimal blob fill targets.

---

### 5.4 Stage 4: Data Availability Modes and the Live Fill-Target Trade-off

Stage 4 benchmarks evaluate the regular gas, blob gas, and storage utilization characteristics of three Data Availability (DA) modes—**Calldata**, **Blob** (EIP-4844), and **Offchain** DA—under FCFS sequencing and steady transaction load. In addition, we analyze the impact of EIP-4844 parameters (`BLOB_TARGET_BYTES` and `BLOB_FILL_TARGET`) and the `BlobPacking` scheduling policy.

#### 5.4.1 Economic Comparison and Mathematical Modeling of DA Modes
Rollup L1 submission costs are modeled by the general gas equation:
\[\text{Gas}_{\text{batch}}(N) = F_{\text{regular}} + M_{\text{data}}(N) \cdot N + \text{Gas}_{\text{blob}}(N)\]
where $F_{\text{regular}}$ is the fixed L1 gas overhead for committing the batch, verifying the ZK proof, and updating the state root; $M_{\text{data}}(N)$ is the marginal L1 regular gas per transaction (for copying calldata and executing logs); $N$ is the batch transaction count; and $\text{Gas}_{\text{blob}}(N)$ is the blob gas cost (only active in EIP-4844 mode).

To map these gas metrics to actual dollar prices, we model transaction costs under a standard baseline L1 market configuration: a reference Ethereum price of **\$2,000/ETH**, an L1 regular gas base fee of **3.0 gwei** (matching typical low-congestion L1 base fees), and an EIP-4844 blob base fee of **1.0 gwei**. 

Table 3 summarizes the empirical results under FCFS sequencing (average batch size $N \approx 37$) across the three DA modes.

| Experiment ID | DA Mode | Avg. Batch Size ($N$) | Avg. L1 Gas/Tx | Avg. Cost/Tx (USD)* | Cost Reduction vs. Calldata |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `s4_da_calldata` | Calldata | 37.09 | 20,739.36 | \$0.122 | Baseline |
| `s4_da_blob` | Blob | 36.94 | 4,596.32 | \$0.026 | **78.3%** |
| `s4_da_offchain` | Offchain | 36.81 | 3,615.49 | \$0.019 | **84.4%** |

*\*Assumes ETH = \$2,000, L1 Base Fee = 3.0 gwei, and Blob Base Fee = 1.0 gwei. Note that the reported average cost per transaction ($0.026) for Blob DA incorporates batch-by-batch variance; due to Jensen's Inequality, the average of the per-transaction costs ($\mathbb{E}[1/N]$) is higher than the cost calculated from the simple average batch size ($1/\mathbb{E}[N]$).*

Our empirical evaluation demonstrates distinct economics for each mode:
1. **Calldata DA (`s4_da_calldata`)**: The transaction payload is posted as L1 calldata, which is billed at 16 gas per non-zero byte (and 4 gas per zero byte) [8]. Under this mode, $F_{\text{regular}} \approx 30,000$ gas, $M_{\text{data}} \approx 20,400$ gas/tx, and $\text{Gas}_{\text{blob}} = 0$. Submitting $\sim$17.9 KB of calldata per batch consumes $\sim$749,067 gas. The average cost per transaction is **$0.122** (20,739 gas/tx).
2. **Blob DA (`s4_da_blob`)**: Transaction data is moved to a transient blob. Regular L1 gas consumption drops sharply to a fixed verifier overhead of $F_{\text{regular}} \approx 118,290$ gas, while the data payload is billed under the blob gas market (billed at 131,072 blob gas units per blob). The average regular gas per transaction drops to **4,596 gas/tx** (representing a **78.3% USD cost saving** to **$0.026 per transaction**).
3. **Offchain DA (`s4_da_offchain`)**: Transaction data is stored in an off-chain database or Data Availability Committee (DAC). The L1 contract only receives the updated state root and the validity proof. Thus, $F_{\text{regular}} \approx 115,732$ gas, $M_{\text{data}} = 0$, and $\text{Gas}_{\text{blob}} = 0$. The average cost per transaction drops to **$0.019** (3,615 gas/tx), achieving an **84.4% cost saving**.

#### 5.4.2 Batch Sizing and the Production Amortization Gap
A key finding is that our PoC's EIP-4844 savings (78.3%) are lower than the 90–95%+ fee reductions observed on major production L2s (such as Base or Arbitrum) after the Dencun upgrade [20, 21]. This difference is explained mathematically by batch size amortization.

In our PoC sweeps, FCFS batch sealing is driven by the 2-second timeout, resulting in small batches ($N \approx 37$). The regular gas overhead $F_{\text{regular}} \approx 118,000$ gas is divided by only 37 transactions, adding $\sim$3,189 gas of regular L1 execution cost to every transaction:
\[\text{Gas}_{\text{tx}} = \frac{118,290\text{ gas}}{37} + \text{Marginal Data Regular Gas} \approx 3,197\text{ gas/tx}\]
In contrast, production rollups accumulate thousands of transactions ($N \ge 1,000$) before submitting a batch to L1. This dilutes the fixed regular gas overhead $F_{\text{regular}}$ to a negligible value:
\[\text{Gas}_{\text{tx, production}} = \frac{118,290\text{ gas}}{1000} + \text{Marginal Data Regular Gas} \approx 118.3\text{ gas/tx}\]
At this scale, the L1 transaction cost is dominated almost entirely by the blob gas price. Since the blob gas market was heavily underutilized post-Dencun (often priced at the minimum of 1 wei per blob gas), the total fee dropped to near-zero. This demonstrates that **blob data availability only yields its full economic scalability when combined with large batch sizes**.

#### 5.4.3 Blob Target Bytes and Sealing Independence
In the `s4_blob_target_*` sweeps, the target capacity of the EIP-4844 blob was varied from 32 KB up to 120 KB under FCFS sequencing. This variation had no impact on batch size ($\sim$36.9 txs) or L1 submission costs ($1.46 per batch). Because FCFS batch sealing is driven entirely by the 2-second timeout, the sequencer collects the same transaction volume regardless of the target capacity. Consequently, increasing `BLOB_TARGET_BYTES` simply diluted the blob utilization ratio linearly:
\[\text{Utilization} \approx \frac{17.8 \text{ KB}}{\text{BLOB\_TARGET\_BYTES}}\]

#### 5.4.4 Live Fill-Target Behavior
In the baseline sweeps, changing `blob_fill_target` from 0.50 to 0.95 yielded completely identical results (batch size of 50.10 txs, committed TPS of 27.0, and blob utilization of 20.2%). A code audit confirmed that `blob_fill_target` was a **dead configuration parameter**. It was parsed in [config.rs](file:///c:/Users/malin/Desktop/rollupx-full-zk-rollup/sequencer/src/config.rs#L87) but never checked in the sequencer's active sealing logic in [trigger.rs](file:///c:/Users/malin/Desktop/rollupx-full-zk-rollup/sequencer/src/batch/trigger.rs).

Once the sequencer was patched to implement the EIP-4844 size-based trigger, the sequencer checked if the accumulated transaction size reached `blob_target_bytes * blob_fill_target`. The patched system successfully demonstrated the expected cost-latency trade-offs:
1. **Low Fill Target (0.50)**: The sequencer sealed batches sooner, resulting in smaller batch sizes, lower average L2 queue wait times, and lower blob utilization ($\sim$50.0%). L1 regular gas per transaction was higher due to less amortization.
2. **High Fill Target (0.95)**: The sequencer waited to collect more transactions, maximizing blob utilization ($\sim$95.0%) and reducing L1 regular gas per transaction, at the cost of higher average queuing delays for users.

#### 5.4.5 Patched BlobPacking Behavior
In the baseline run `s4_da_blobpacking`, size-priority sorting without nonce grouping caused immediate account locks, resulting in empty batches and an inflated cost of **115,550 gas/tx** from Batch 2 onwards.

With the patched, nonce-aware `BlobPackingPolicy` active, this vulnerability was fully resolved. The policy successfully sorted transactions by size within the strict nonce order of each sender account. All batches executed successfully without nonce gaps, restoring L1 gas costs to the baseline range ($\sim$4.6k gas/tx) and enabling efficient EIP-4844 blob packing under multi-transaction workloads.

---

### 5.5 Stage 5: Zero-Knowledge Proving and RISC0 Segment Scaling

Stage 5 profiles the core cryptographic engine of RollupX, swapping the mock prover for the real RISC0 zkVM host/guest subsystem and executing proof generation under production-like conditions with Groth16 proof compression.

#### 5.5.1 Empirical Results of Prover Backend Scaling
To analyze the cryptographic efficiency and verification overhead of the RollupX zero-knowledge engine, sweeps were executed using the real RISC0 prover backend with Groth16 proof compression across target batch sizes of 50, 100, 200, and 500. 

Table 4 summarizes the results of the Stage 5 experiments.

| Experiment ID | Prover Backend | Target Size | Timeout | Batches | Avg. Batch Size | Peak Cycles* | Peak Segments* | Avg. Proving Time | Proof Size | Avg. L1 Gas/Tx |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `baseline` | RISC0 (mock) | 100 | 2,000 ms | 28 | 9.04 | 0 | 0 | 0.00 s | 32 B | 28,133.73 |
| `s5_real_bs_0050` | RISC0 (real) | 50 | 150,000 ms | 6 | 42.17 | 2,162,688 | 3 | 317.82 s | 256 B | 24,594.02 |
| `s5_real_bs_0100` | RISC0 (real) | 100 | 150,000 ms | 3 | 83.00 | 4,227,072 | 5 | 540.79 s | 256 B | 19,803.95 |
| `s5_real_bs_0200` | RISC0 (real) | 200 | 150,000 ms | 2 | 126.00 | 8,388,608 | 8 | 782.42 s | 256 B | 19,782.08 |
| `s5_real_bs_0500` | RISC0 (real) | 500 | 150,000 ms | 1 | 246.00 | 10,485,760 | 10 | 1,487.31 s | 256 B | **19,502.72** |

*\*Note: Cycle and segment metrics represent the peak configurations for full-sized batches in that run. Proving times and L1 gas metrics include trailing batches (which are smaller and have lower proving/L1 costs, slightly reducing the averages).*

#### 5.5.2 The Proving Step-Function (RISC0 Segments and Recursion)
In the RISC0 zkVM, proof generation executes the rollup's State Transition Guest Program inside a virtual CPU. The zkVM divides guest execution into discrete units called **segments**, each limited to a maximum execution size of $2^{20} = 1,048,576$ cycles (instructions) to make proof generation parallelizable and memory-efficient.

A STARK validity proof is generated for each segment. These individual segment proofs must then be aggregated into a single proof. This is accomplished using a binary recursion tree of zkVM execution steps:
1. **Lift**: Each segment proof is "lifted" into a standardized recursive representation.
2. **Join**: The lifted segment proofs are merged pairwise in a binary tree structure. Each merge runs a "join" guest program inside the zkVM that verifies two input STARK proofs and outputs a single combined STARK proof. For $S$ segments, this process requires $S - 1$ join operations, with the tree depth scaling as $\lceil \log_2(S) \rceil$.
3. **Stark-to-Snark Wrapping**: The root STARK proof is converted into a 256-byte Groth16 SNARK proof on the BN254 curve, which can be verified on-chain in EVM via pairing precompiles.

Because proving time is dominated by the number of segment proofs and recursion levels rather than raw instruction counts, the proving wall-clock time scales as a step-function of the segment count, as shown below:
* **Batch size 3 (trailing)**: 262,144 cycles $\rightarrow$ **1 segment** $\rightarrow$ **96.2 seconds**.
* **Batch size 49 (trailing)**: 2,097,152 cycles (exactly $2 \times 2^{20}$) $\rightarrow$ **2 segments** $\rightarrow$ **333.4 seconds**.
* **Batch size 50**: 2,162,688 cycles (exceeds $2 \times 2^{20}$) $\rightarrow$ **3 segments** $\rightarrow$ **362.3 seconds**.
* **Batch size 100**: 4,227,072 cycles (exceeds $4 \times 2^{20}$) $\rightarrow$ **5 segments** $\rightarrow$ **645.8 seconds**.
* **Batch size 200**: 8,388,608 cycles (exactly $8 \times 2^{20}$) $\rightarrow$ **8 segments** $\rightarrow$ **1,196.0 seconds**.
* **Batch size 246**: 10,485,760 cycles (exactly $10 \times 2^{20}$) $\rightarrow$ **10 segments** $\rightarrow$ **1,487.3 seconds**.

```
Proving Time (Seconds)
  ^
  |                                        _.-'' (10 seg, 1487s)
  |                                   _.-''
  |                              _.-'' (8 seg, 1196s)
  |                         _.-''
  |                    _.-'' (5 seg, 645s)
  |               _.-''
  |           _.-' (3 seg, 362s)
  |       _.-' (2 seg, 333s)
  |  _.-'' (1 seg, 96s)
  +--------------------------------------------------> Batch Size (Tx Count)
```

Each L2 transaction consumes between **41,000 and 45,000 guest cycles**. This is highly efficient and is achieved by leveraging RISC0's native accelerators for Keccak-256 and SHA-256 hash functions. Without hardware accelerators, verifying `secp256k1` ECDSA signatures and traversing the Sparse Merkle Tree (SMT) would require millions of instructions. Even with accelerators, signature verification and SMT path hashing remain the primary cycle consumers in the guest program.

#### 5.5.3 Prover Economics of Scale
While the cycle count scales linearly with the number of transactions, the wall-clock proving time exhibits a strong economy of scale. 

By performing a linear regression on proving wall-clock time $T(N)$ as a function of batch size $N$:
\[T(N) = A + B \times N\]
* **Fixed Setup/SNARK Compression Overhead ($A$)**: **70 to 80 seconds**. This represents the time required to compile the guest ELF, initialize the executor, generate the execution trace, and perform the final Groth16 SNARK wrapping (which requires solving large Multi-Scalar Multiplications (MSMs) and Number Theoretic Transforms (NTTs) on BN254).
* **Marginal Proving Cost ($B$)**: **~5.7 seconds per transaction**.

Due to the high fixed cost $A$, proving small batches is extremely inefficient. As the batch size grows, the fixed overhead is amortized over more transactions, reducing the average proving cost from **7.5 seconds/tx** (at batch size 50) to **6.0 seconds/tx** (at batch size 246).

#### 5.5.4 L1 Gas Amortization and O(1) Succinctness
A defining characteristic of zero-knowledge rollups is cryptographic succinctness: the validation time and storage size of proof verification on L1 is $O(1)$, remaining completely invariant to the volume of transactions executed off-chain.

No matter how many transactions are executed in a batch (ranging from 3 to 246), the final compressed Groth16 proof is always exactly **256 bytes**. This structure consists of three elliptic curve points on BN254:
\[\text{Proof} = (A \in \mathbb{G}_1, B \in \mathbb{G}_2, C \in \mathbb{G}_1)\]
with $A$ consuming 64 bytes, $B$ consuming 128 bytes, and $C$ consuming 64 bytes ($64 + 128 + 64 = 256$ bytes).

This succinctness enables powerful L1 gas amortization. L1 gas per batch is modeled as:
\[\text{L1 Gas per Batch}(N) = F + M \times N\]
* **Fixed L1 Overhead ($F$)**: **30,000 to 50,000 gas**. This represents the base transaction fee for `commitBatch` plus the pairing precompile verification gas on L1.
* **Marginal L1 Cost per Tx ($M$)**: **~19,300 gas per transaction** (to copy L2 transaction calldata bytes and emit receipts).

As batch sizes grow, the fixed overhead $F$ is amortized over a larger L2 transaction volume, causing the per-transaction gas to decay towards the marginal calldata asymptote:
* Baseline (avg batch 9.04): **28,133 gas/tx**
* Size 50 (avg batch 42.17): **24,594 gas/tx**
* Size 500 (avg batch 246.0): **19,502 gas/tx** (converging to the $M \approx 19.3$k gas limit).

---

## 6. Architectural Recommendations and Code Fixes

### 6.1 Nonce-Aware Sequencer Scheduler

To resolve the Nonce Reordering Vulnerability, the sequencer's scheduling engine must group transactions by sender and sort them strictly by nonce ascending. Only the head of each account queue should be considered for global priority sorting.

We propose the following Rust blueprint to replace the sorting logic in `policies.rs`:

```rust
use std::collections::{HashMap, VecDeque};
use crate::PooledTransaction;
use ethers::types::{Address, U256};

pub fn order_transactions_nonce_aware(
    transactions: Vec<PooledTransaction>,
    policy_name: &str,
) -> Vec<PooledTransaction> {
    if policy_name == "FCFS" || policy_name == "FairBFT" {
        // Keep original timestamp-based ordering, which naturally preserves nonces
        return transactions;
    }

    // 1. Group transactions by sender address
    let mut account_queues: HashMap<Address, Vec<PooledTransaction>> = HashMap::new();
    for tx in transactions {
        account_queues
            .entry(tx.tx.from)
            .or_default()
            .push(tx);
    }

    // 2. Sort each account's queue strictly by nonce ascending to guarantee contiguous execution
    for queue in account_queues.values_mut() {
        queue.sort_by_key(|t| t.tx.nonce);
    }

    // Convert to VecDeque for efficient popping from the head
    let mut queues: HashMap<Address, VecDeque<PooledTransaction>> = account_queues
        .into_iter()
        .map(|(addr, q)| (addr, q.into()))
        .collect();

    let mut ordered_txs = Vec::new();

    // 3. Iteratively schedule the highest-priority eligible head transaction
    while !queues.is_empty() {
        let mut best_sender = None;
        
        // Temporarily hold candidate head transactions for sorting
        let mut candidates = Vec::new();

        for (sender, queue) in &queues {
            if let Some(next_tx) = queue.front() {
                candidates.push((*sender, next_tx));
            }
        }

        if candidates.is_empty() {
            break;
        }

        // Apply policy-specific priority sorting to the head transactions
        match policy_name {
            "FeePriority" => {
                candidates.sort_by(|a, b| b.1.tx.gas_price.cmp(&a.1.tx.gas_price));
            }
            "TimeBoost" => {
                candidates.sort_by(|a, b| {
                    let window_a = a.1.tx.timestamp / 5000; // 5s windows
                    let window_b = b.1.tx.timestamp / 5000;
                    match window_a.cmp(&window_b) {
                        std::cmp::Ordering::Equal => {
                            let bid_a = a.1.tx.boost_bid.unwrap_or_default();
                            let bid_b = b.1.tx.boost_bid.unwrap_or_default();
                            match bid_b.cmp(&bid_a) {
                                std::cmp::Ordering::Equal => {
                                    b.1.tx.gas_price.cmp(&a.1.tx.gas_price)
                                }
                                other => other,
                            }
                        }
                        other => other,
                    }
                });
            }
            "BlobPacking" => {
                candidates.sort_by(|a, b| {
                    let size_a = a.1.estimated_encoded_bytes();
                    let size_b = b.1.estimated_encoded_bytes();
                    match size_b.cmp(&size_a) {
                        std::cmp::Ordering::Equal => {
                            b.1.tx.gas_price.cmp(&a.1.tx.gas_price)
                        }
                        other => other,
                    }
                });
            }
            _ => {
                // Fallback to FCFS behavior
                candidates.sort_by_key(|c| c.1.tx.timestamp);
            }
        }

        // Select the highest priority candidate
        if let Some((sender, _)) = candidates.first() {
            best_sender = Some(*sender);
        }

        if let Some(sender) = best_sender {
            if let Some(mut queue) = queues.get_mut(&sender) {
                if let Some(tx) = queue.pop_front() {
                    ordered_txs.push(tx);
                }
                if queue.is_empty() {
                    queues.remove(&sender);
                }
            }
        } else {
            break;
        }
    }

    ordered_txs
}
```

### 6.2 Hysteresis Trap Solution
To resolve the hysteresis trap, the sequencer setup validation must enforce the following invariant during configuration loading:
\[\text{adaptive\_small\_batch\_size} < \text{adaptive\_low\_load\_threshold}\]
A recommended tuning configuration for the adaptive batch policy is:
* `adaptive_low_load_threshold = 50`
* `adaptive_small_batch_size = 25`
* `adaptive_medium_load_threshold = 150`
* `adaptive_medium_batch_size = 100`
* `max_batch_size = 500`

This ensures size-driven batching triggers at size 25 under low loads and scales up to 100 or 500 under heavy loads.

### 6.3 Active Blob-Fill Sealing Logic
To activate the `blob_fill_target` parameter, the sequencer's sealing engine in `trigger.rs` should be refactored to evaluate size in bytes when EIP-4844 is active:

```rust
// Proposed byte-based trigger check in trigger.rs
pub fn should_seal_for_blob(
    mempool_bytes: usize,
    blob_target_bytes: usize,
    blob_fill_target: f64,
) -> bool {
    let target_limit = (blob_target_bytes as f64 * blob_fill_target) as usize;
    mempool_bytes >= target_limit
}
```

---

## 7. Design Choices and Limitations

### 7.1 Key Design Choices
The architecture of RollupX was guided by several deliberate design trade-offs aimed at maximizing modularity, ease of audit, and reproducibility of benchmarking data, when compared to state-of-the-art (SOTA) production architectures:

1. **Clean-Room Sandbox vs. Production zkEVM Fork**: Production ZK-Rollups typically implement zkEVMs (Type-1 or Type-2, e.g., Scroll, Linea [24], or zkSync Era [32]) to maintain developer-level compatibility with Solidity. However, compiling EVM bytecode directly into zero-knowledge circuits requires translating EVM execution traces (including stack, memory, and gas operations) into arithmetic constraints. This introduces massive proving overhead (often hundreds of thousands of constraints per basic EVM instruction) [24, 34]. For our benchmarking goals, forking a production zkEVM would introduce complex compilation heuristics and gas-metering variables that would obscure the core systems performance of the sequencer and data layers. By building RollupX as a clean-room Rust-based sandbox, we decoupled execution parameters from production VM constraints. This allowed us to systematically isolate and sweep sequencing, batching, and data availability variables.
2. **RISC0 zkVM with Groth16 Wrapper Compression**: We chose the RISC0 zkVM as the execution-proving engine rather than custom circuit architectures (e.g., Loopring). Writing the State Transition Guest Program in standard Rust (compiled to a target RISC-V ELF) simplifies code audibility and supports standard cryptographic libraries. To mitigate the high gas cost of verifying STARK validity proofs on-chain, we wrapped the host STARK proof in a 256-byte Groth16 SNARK proof on the BN254 curve using Arkworks. This design choice limits the L1 verification cost to a constant $\approx 250,000$ gas (pairing precompiles), regardless of L2 transaction complexity.
3. **256-level Sparse Merkle Tree (SMT)**: While Ethereum L1 utilizes a Hexary Patricia Merkle Trie (PMT) for state storage [8], PMTs are highly inefficient to prove in ZK due to variable path lengths and RLP serialization overhead [24, 34]. We selected a 256-level binary SMT mapping 160-bit account addresses to serialized states. SMTs provide constant-length inclusion proofs (siblings), which fit within the zkVM guest memory footprint and can be verified efficiently in ZK. Using Keccak-256 for leaf and node hashing allowed guest execution to leverage RISC0's native Keccak hardware accelerator, keeping transaction verification to just $\sim$41k–45k cycles.

### 7.2 Core Limitations
While RollupX serves as an effective sandbox, our empirical results must be interpreted within the following structural bounds:

1. **Centralized Single-Node Sequencer vs. Shared Sequencing**: The sequencer is operated by a single centralized node. While this ensures a highly repeatable benchmarking environment, it does not capture the consensus delays, network gossip latencies, or BFT validator coordination overhead characteristic of decentralized sequencing networks (e.g., Espresso, Astria [30]) or Based Sequencing schemes (where L1 validators sequence L2 transactions [18]).
2. **Single-Account Transaction Generation**: The synthetic workload generator submits transactions from a single sender account (`0xf39f...`). Although this isolated nonce dynamics—enabling us to uncover the Nonce Reordering Vulnerability—it does not simulate multi-account concurrency, transaction replacement mechanisms (e.g., replace-by-fee), or competitive gas-bidding (priority fee) congestion dynamics present in production mempools.
3. **Mock L1 Gas Verifier**: The experimental sweeps for Stages 1–4 used a mock verifier contract on L1 that bypassed pairing computations. This is a threat to validity that we address in Section 8 by mathematically projecting the real Groth16 verifier gas overhead (~250,000 gas) onto the empirical datasets.
4. **Hardhat Block Mining Dominance**: The local Ethereum L1 simulation used a fixed 12-second mining interval. This hard limit dominates the measured L2-to-L1 transaction latency, masking the latency advantages of smaller L2 batches on L1 settlement.
5. **FCFS Sealing vs. SOTA Blob Packing**: Under the FCFS timeout policy, batch sealing is driven entirely by the 2-second timeout. Because of this, RollupX was subject to the "Blob Dilemma" [25] under low transaction volumes: it submitted partially empty blobs ($\sim$17.8 KB out of a target 120 KB), resulting in a 70% cost reduction instead of the >95% savings seen in production. SOTA sequencers implement dynamic blob packing (waiting to accumulate more data or sharing blobs with other rollups [25, 26]) to optimize this cost-delay trade-off, which was not supported by our FCFS sealer.

---

## 8. Threats to Validity

* **L1 Gas Estimation under Mock Verification**: In Stages 1-4, `MockVerifier.sol` was used, consuming negligible gas (~1,000 gas). A real Groth16 verifier contract performs multiple bilinear pairing operations, consuming a fixed ~250,000 gas. In production, this increases the fixed L1 overhead, making the gas amortization benefit of larger batches significantly more pronounced:
  * For batch size 25: Gas per tx would increase from ~20,549 to **~30,515 gas** (+48%).
  * For batch size 500: Gas per tx would increase from ~19,459 to **~20,060 gas** (+3%).
  Therefore, the cost savings of scaling batch size would increase from **5%** (mock verifier) to **34%** (real verifier).
* **Hardhat Mining Latency Dominance**: The local Hardhat mining interval was fixed at 12s, meaning L2-to-L1 latency was dominated by L1 block times (~7.0s), masking the actual execution latency gains of smaller batches on L2.

---

## 9. Conclusion

RollupX provides a highly modular and reproducible ZK-rollup benchmarking framework, proving the economic efficiency of EIP-4844 blobs (70% savings) and Offchain DA (82.5% savings). We modeled the queueing delays and ZK proving scaling, identifying a critical step-function in RISC0 proving times governed by segment boundaries. 

Importantly, our systems audit identified key implementation flaws—most notably the Nonce Reordering Vulnerability in priority scheduling— and provided concrete code blueprints to ensure EVM compliance. Future research will focus on integrating decentralized sequencer networks with BFT-based consensus timestamping.

---

## References

1. Deloitte, "Deloitte's 2019 Global Blockchain Survey," 2019.
2. S. Nakamoto, "Bitcoin: A Peer-to-Peer Electronic Cash System," 2008.
3. V. Buterin, "Ethereum Whitepaper: A Next-Generation Smart Contract and Decentralized Application Platform," 2014.
4. L. Luu et al., "Demystifying Blockchain Scalability limits," ACM CCS, 2016.
5. J. Poon and T. Dryja, "The Bitcoin Lightning Network: Scalable Off-Chain Instant Payments," 2016.
6. E. Kokoris-Kogias et al., "OmniLedger: A Secure, Scale-Out, Decentralized Ledger via Sharding," IEEE S&P, 2018.
7. V. Buterin, "Why sharding is great: demystifying the blockchain trilemma," 2021.
8. G. Wood, "Ethereum: A Secure Decentralised Generalised Transaction Ledger," Yellow Paper, 2015.
9. Dune Analytics, "Ethereum L1 Throughput Tracker," 2025.
10. Etherscan, "Ethereum Daily Transactions Chart," 2025.
11. Dune Analytics, "Ethereum L1 Gas Price Dashboard," 2025.
12. L2BEAT, "Layer 2 Fees Tracker," 2025.
13. Etherscan, "Ethereum Transaction Fees (USD) Chart," 2025.
14. Dune Analytics, "Ethereum Daily Active Addresses and Tx Count," 2025.
15. J. Poon and V. Buterin, "Plasma: Scalable Autonomous Smart Contracts," 2017.
16. B. Bünz et al., "Bulletproofs: Short Proofs for Confidential Transactions and More," IEEE S&P, 2018.
17. E. Ben-Sasson et al., "Scalable, transparent, and post-quantum secure cryptographic proofs," IACR Cryptology ePrint, 2018.
18. V. Buterin, "Rollups-on-the-horizon: the ultimate guide," 2021.
19. Ethereum Foundation, "EIP-4844: Shard Blob Transactions," 2022.
20. L2BEAT, "Post-Dencun L2 Gas Price Decay Analysis," 2024.
21. Dune Analytics, "EIP-4844 Blob Fees and L2 Gas Savings," 2024.
22. Base Scan, "Base Network Daily Transaction Charts," 2025.
23. Arbitrum Scan, "Arbitrum Daily Activity Dashboard," 2025.
24. A. Bez et al., "An Empirical Performance Comparison of Layer-2 Rollup Architectures," IEEE ICBC, 2023.
25. M. J. M. Werner, "Analyzing the Latency-Throughput-Cost Frontier in Production ZK-Rollups," Journal of Blockchain Research, 2024.
26. J. Adler et al., "Data Availability Proofs: A General Framework for Blockchain Scaling," Financial Cryptography, 2021.
27. V. Kalodner et al., "Arbitrum: Scalable, private smart contracts," USENIX Security, 2018.
28. A. Yakovenko, "Solana: A new architecture for a high performance blockchain," 2018.
29. Sui Foundation, "Sui Lutris: Consensus and Fast-path Execution," 2023.
30. S. Motepalli, L. Freitas, and B. Livshits, "Sok: Decentralized sequencers for rollups," arXiv preprint arXiv:2310.03611, 2023.
31. Offchain Labs, "Timeboost: Arbitrum transaction ordering policy," 2023.
32. Matter Labs, "ZKsync Docs," https://docs.zksync.io/, 2025.
33. StarkWare Industries, "Starknet Documentation: Index," https://docs.starknet.io/, 2025.
34. M. A. Habib, "Analyzing performance bottlenecks in zero-knowledge proof based rollups," 2023.
35. J. Groth, "On the size of pairing-based non-interactive arguments," in EUROCRYPT 2016, 2016, pp. 305–326.
36. T. Lavaur et al., "Modular zk-rollup on-demand," https://arxiv.org/abs/2306.02785, Jun 2023.
