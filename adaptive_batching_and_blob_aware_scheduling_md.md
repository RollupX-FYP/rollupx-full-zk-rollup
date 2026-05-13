# Full Explanation of the Adaptive Batching and Blob-Aware Scheduling Equations

This document explains two algorithms used in a rollup sequencer:

1. **Adaptive batching**
2. **Blob-aware scheduling**

The explanation assumes **zero prior knowledge**. It defines every symbol, every variable, the logic behind each equation, how to compute the values step by step, why the algorithm exists, what trade-offs it creates, and how a coding agent should implement it safely.

---

# 1. Background: what problem these algorithms are solving

A rollup receives many user transactions over time. These transactions wait in a queue called the **mempool** until the sequencer puts them into a **batch**.

That batch is then:

1. executed,
2. proved,
3. submitted to Layer 1,
4. and its data is made available through some DA mechanism such as calldata or blobs.

Two major problems appear.

## Problem A: fixed batch size is bad under changing traffic

If the batch size is always large, users wait too long when traffic is low.

If the batch size is always small, the system creates too many small batches when traffic is high, which increases cost per transaction and reduces throughput.

So batch size should adapt to current load.

This leads to the **adaptive batching** equation.

## Problem B: not all valid batches use DA space efficiently

When using blob-based data availability, transactions occupy bytes inside a blob.

If the sequencer selects transactions in a naive order, it may leave a lot of blob space unused.

That means the rollup pays for blob space that it does not fully use.

So transaction selection should consider:

- transaction fee,
- transaction waiting time,
- and how well each transaction fits the remaining blob space.

This leads to the **blob-aware scheduling** equation.

---

# 2. Adaptive batching: the full idea

Adaptive batching decides:

> **How many transactions should be included in the next batch right now?**

Instead of using one fixed batch size for all situations, it chooses a target batch size based on the current mempool depth.

---

# 3. Adaptive batching equation

A common threshold-based adaptive batching rule is:

\[
N(t)=
\begin{cases}
N_{\min}, & d < L \\
N_{\text{mid}}, & L \le d < H \\
N_{\max}, & d \ge H
\end{cases}
\]

This equation means:

- if the mempool is small, use a small batch,
- if the mempool is medium, use a medium batch,
- if the mempool is large, use a large batch.

---

# 4. Meaning of every symbol in the adaptive batching equation

## \(N(t)\)

This means the **target batch size at time \(t\)**.

It is the number of transactions the sequencer *wants* to include in the next batch.

Example:

- \(N(t)=20\) means the sequencer is targeting a batch of 20 transactions.
- \(N(t)=500\) means the sequencer is targeting a batch of 500 transactions.

This is the **output** of the adaptive batching equation.

## \(t\)

This is the current time or the current decision moment.

The algorithm may run every time:

- a new transaction arrives,
- the timeout is checked,
- a batch is submitted,
- or on a periodic timer.

The exact time representation does not matter to the math. It just means:

> “At the current moment, calculate the appropriate batch size.”

## \(d\)

This is the **current mempool depth**.

It means:

\[
d = \text{number of pending transactions currently waiting in the mempool}
\]

Example:

- if 12 transactions are waiting, then \(d=12\)
- if 340 transactions are waiting, then \(d=340\)

This is the main **input** to the adaptive batching equation.

## \(L\)

This is the **low-load threshold**.

If mempool depth is below \(L\), traffic is considered low.

Example:

\[
L=50
\]

Then:

- if \(d<50\), the system treats the load as low.

## \(H\)

This is the **high-load threshold**.

If mempool depth is at least \(H\), traffic is considered high.

Example:

\[
H=200
\]

Then:

- if \(d \ge 200\), the system treats the load as high.

The interval between \(L\) and \(H\) is the medium-load region.

## \(N_{\min}\)

This is the **small batch size** used when traffic is low.

Example:

\[
N_{\min}=20
\]

Why use a small batch under low load?

Because if very few users are transacting, they should not wait a long time for a large batch to fill.

A small batch reduces waiting time.

## \(N_{\text{mid}}\)

This is the **medium batch size** used when traffic is moderate.

Example:

\[
N_{\text{mid}}=100
\]

This tries to balance:

- latency,
- throughput,
- proof cost amortization,
- and DA efficiency.

## \(N_{\max}\)

This is the **large batch size** used when traffic is high.

Example:

\[
N_{\max}=500
\]

Why use a large batch under high load?

Because when many transactions are arriving, batches fill quickly anyway, so the system can use larger batches to:

- amortize fixed costs,
- improve throughput,
- and reduce cost per transaction.

---

# 5. How to compute the adaptive batching equation step by step

Assume the configuration is:

\[
L=50,\quad H=200,\quad N_{\min}=20,\quad N_{\text{mid}}=100,\quad N_{\max}=500
\]

Now suppose the current mempool depth is:

\[
d=35
\]

Since:

\[
35 < 50
\]

the first case applies, so:

\[
N(t)=N_{\min}=20
\]

That means the target batch size is 20.

Now suppose:

\[
d=120
\]

Since:

\[
50 \le 120 < 200
\]

the second case applies, so:

\[
N(t)=N_{\text{mid}}=100
\]

Now suppose:

\[
d=450
\]

Since:

\[
450 \ge 200
\]

the third case applies, so:

\[
N(t)=N_{\max}=500
\]

---

# 6. Why the adaptive batching equation is only part of the algorithm

The equation gives the **target batch size**, but it does **not** tell you when to flush a batch if the target size has not been reached.

That is why adaptive batching almost always needs a **timeout rule**.

---

# 7. Timeout in adaptive batching

Define:

\[
T_{\max} = \text{maximum allowed waiting time for the oldest transaction}
\]

Example:

\[
T_{\max}=5\text{ seconds}
\]

Also define:

\[
oldestWait = now - arrivalTime(\text{oldest transaction in mempool})
\]

If:

\[
oldestWait \ge T_{\max}
\]

then the batch is flushed even if the target batch size has not been reached.

This prevents a user from waiting forever during low traffic.

---

# 8. Full adaptive batching rule with timeout

The practical algorithm is not only:

\[
N(t)=
\begin{cases}
N_{\min}, & d < L \\
N_{\text{mid}}, & L \le d < H \\
N_{\max}, & d \ge H
\end{cases}
\]

It is really:

1. compute target batch size \(N(t)\),
2. if enough transactions are available, build a batch of size \(N(t)\),
3. otherwise wait,
4. unless timeout is reached,
5. in which case flush the available transactions.

In implementation terms:

- **size-triggered flush**: when `mempool_size >= target_batch_size`
- **time-triggered flush**: when `oldest_wait >= T_max`

---

# 9. Practical formula for actual batch size

The target batch size may be larger than the number of transactions available.

So define:

\[
N_{\text{actual}} = \min(d, N(t))
\]

This means:

- if 500 is the target but only 120 transactions exist, the largest batch you can actually build is 120.

But normally you only flush that partial batch when timeout or another flush condition is met.

---

# 10. Adaptive batching trade-offs

Adaptive batching exists because of a cost-latency-throughput trade-off.

## Larger batches

Larger batches usually improve:

- throughput,
- proof amortization,
- DA efficiency,
- gas cost per transaction.

But they usually worsen:

- queueing delay,
- batch formation latency,
- proof generation time.

## Smaller batches

Smaller batches usually improve:

- responsiveness,
- user waiting time,
- low-load latency.

But they usually worsen:

- cost per transaction,
- DA efficiency,
- throughput under heavy traffic.

## Therefore

Adaptive batching tries to get the best of both worlds:

- small batches when traffic is low,
- large batches when traffic is high.

---

# 11. A coding-agent-friendly adaptive batching specification

## Inputs

A coding agent implementing adaptive batching needs:

- `mempool_size`
- `low_threshold`
- `high_threshold`
- `batch_size_min`
- `batch_size_mid`
- `batch_size_max`
- `timeout_ms`
- `oldest_wait_ms`

## Output

The algorithm should produce:

- `target_batch_size`
- `should_flush_now`
- `flush_reason`

## Reference implementation logic

```text
if mempool_size < low_threshold:
    target_batch_size = batch_size_min
elif mempool_size < high_threshold:
    target_batch_size = batch_size_mid
else:
    target_batch_size = batch_size_max

should_flush_now = False
flush_reason = None

if mempool_size >= target_batch_size:
    should_flush_now = True
    flush_reason = "size_reached"
elif oldest_wait_ms >= timeout_ms and mempool_size > 0:
    should_flush_now = True
    flush_reason = "timeout"
```

Then actual batch size is:

```text
actual_batch_size = min(mempool_size, target_batch_size)
```

if `should_flush_now` is true.

---

# 12. Smooth adaptive batching variant

The threshold-based version is simple, but it changes abruptly.

For example:

- if `d = 199`, batch size may be 100
- if `d = 200`, batch size may jump to 500

That is a hard discontinuity.

A smoother alternative is:

\[
N(t)=\min\left(N_{\max}, \max\left(N_{\min}, k \cdot d\right)\right)
\]

Where:

- \(k\) is a scaling factor,
- \(d\) is mempool depth.

Example:

\[
N_{\min}=20,\quad N_{\max}=500,\quad k=0.8
\]

If:

\[
d=50
\]

then:

\[
k \cdot d = 0.8 \cdot 50 = 40
\]

So:

\[
N(t)=40
\]

If:

\[
d=1000
\]

then:

\[
k \cdot d = 800
\]

but since \(N_{\max}=500\), the result is capped:

\[
N(t)=500
\]

This version is more flexible but a little harder to reason about and tune.

For most research prototypes, the threshold-based version is easier to explain and validate.

---

# 13. Blob-aware scheduling: the full idea

Adaptive batching decides **how many transactions** to include.

Blob-aware scheduling decides **which transactions** to include.

This matters when not all transactions have the same encoded size.

If all transactions were exactly the same size, then any reasonable ordering would pack blobs similarly.

But in reality, transactions may vary in encoded size because of:

- different transaction types,
- metadata size,
- number of outputs,
- number of touched accounts,
- proof-related auxiliary data,
- or compressed state diff size.

So the scheduler should consider how well transactions fit into the remaining blob space.

---

# 14. Blob-aware scheduling score equation

A practical blob-aware scheduling score is:

\[
Score(tx_i)=
\alpha \cdot feeScore(tx_i)
+
\beta \cdot fit(tx_i, r)
+
\gamma \cdot waitScore(tx_i)
\]

This equation assigns a priority score to each candidate transaction.

The sequencer repeatedly selects the transaction with the highest score among transactions that can fit.

---

# 15. Meaning of every symbol in the blob-aware scheduling equation

## \(tx_i\)

This is the **i-th transaction** in the mempool.

Examples:

- \(tx_1\)
- \(tx_2\)
- \(tx_3\)

Each transaction has properties such as:

- fee,
- arrival time,
- encoded size,
- transaction type.

## \(Score(tx_i)\)

This is the **priority score** assigned to transaction \(tx_i\).

Higher score means the transaction is more attractive to include in the current batch.

This is the **output** of the blob-aware scheduling equation.

## \(\alpha\), \(\beta\), \(\gamma\)

These are non-negative **weights** controlling the importance of each scoring term.

- \(\alpha\) controls fee preference.
- \(\beta\) controls blob packing preference.
- \(\gamma\) controls fairness through waiting time.

These weights are usually chosen so that:

\[
\alpha + \beta + \gamma = 1
\]

That is not mathematically required, but it is a good practice because it makes the score easier to interpret.

Example:

\[
\alpha=0.25,\quad \beta=0.50,\quad \gamma=0.25
\]

This means:

- 25% fee importance,
- 50% blob-fit importance,
- 25% waiting-time importance.

## \(feeScore(tx_i)\)

This is the normalized fee score of transaction \(tx_i\).

It represents the relative economic priority of the transaction.

A common normalization is:

\[
feeScore(tx_i)=
\frac{fee_i - fee_{\min}}{fee_{\max} - fee_{\min}}
\]

Where:

- \(fee_i\) is the fee of the current transaction,
- \(fee_{\min}\) is the minimum fee among candidate transactions,
- \(fee_{\max}\) is the maximum fee among candidate transactions.

This scales the fee score to the range \([0,1]\).

## \(fit(tx_i, r)\)

This measures how well transaction \(tx_i\) fits into the **remaining blob space** \(r\).

This is the most important term for DA-aware scheduling.

A simple fit function is:

\[
fit(tx_i,r)=
\begin{cases}
\frac{b_i}{r}, & b_i \le r \\
0, & b_i > r
\end{cases}
\]

Where:

- \(b_i\) is the encoded blob size of transaction \(tx_i\),
- \(r\) is the currently remaining blob space.

Interpretation:

- if the transaction does not fit, its fit score is 0,
- if it fits and uses most of the remaining space, the score is close to 1,
- if it fits but uses only a tiny fraction of the remaining space, the score is small.

## \(waitScore(tx_i)\)

This is the normalized waiting-time score of transaction \(tx_i\).

It is used to reduce starvation.

A common definition is:

\[
waitScore(tx_i)=\min\left(\frac{wait_i}{W_{\max}},1\right)
\]

Where:

- \(wait_i\) is the current waiting time of the transaction,
- \(W_{\max}\) is the maximum waiting-time scale.

Interpretation:

- newly arrived transactions have small wait score,
- old transactions have large wait score,
- after a certain point, the score saturates at 1.

## \(r\)

This is the **remaining blob space**.

It changes during batch construction.

If the blob capacity is \(B_{blob}\) and the selected transactions currently use total bytes \(\sum b_{tx}\), then:

\[
r = B_{blob} - \sum b_{tx}
\]

This value must be recomputed every time a new transaction is added to the batch.

## \(b_i\)

This is the **blob byte size** of transaction \(tx_i\).

It is the number of bytes the transaction contributes to the DA payload.

In an implementation:

```text
b_i = len(encoded_transaction_bytes)
```

This must be estimated from the actual encoding that will be posted to DA.

## \(B_{blob}\)

This is the total usable capacity of one blob.

In experiments, this is often treated as a fixed constant.

Example:

\[
B_{blob}=131072 \text{ bytes}
\]

which is 128 KiB.

---

# 16. Why raw size is not enough

A common mistake is to score transactions using raw size directly:

\[
Score(tx_i)=\alpha \cdot feeScore(tx_i)+\beta \cdot sizeScore(tx_i)+\gamma \cdot waitScore(tx_i)
\]

This is not ideal.

Why?

Because a larger transaction is not always better.

Suppose remaining space is only 10 KB.

- A 9 KB transaction is a very good fit.
- A 30 KB transaction is impossible to include.
- A 2 KB transaction fits, but does not fill the blob well.

So what matters is not raw size alone.

What matters is:

> **How well the transaction fits the current remaining space.**

That is why the algorithm uses \(fit(tx_i, r)\), not just \(size(tx_i)\).

---

# 17. How to calculate feeScore step by step

Assume three candidate transactions have fees:

- \(tx_A = 5\)
- \(tx_B = 10\)
- \(tx_C = 20\)

Then:

\[
fee_{\min}=5,\quad fee_{\max}=20
\]

For \(tx_A\):

\[
feeScore(tx_A)=\frac{5-5}{20-5}=0
\]

For \(tx_B\):

\[
feeScore(tx_B)=\frac{10-5}{20-5}=\frac{5}{15}=0.333
\]

For \(tx_C\):

\[
feeScore(tx_C)=\frac{20-5}{20-5}=1
\]

So the normalized fee scores are:

- \(tx_A = 0\)
- \(tx_B = 0.333\)
- \(tx_C = 1\)

---

# 18. How to calculate waitScore step by step

Assume:

\[
W_{\max}=10 \text{ seconds}
\]

and the transactions have waited:

- \(tx_A = 2\) s
- \(tx_B = 1\) s
- \(tx_C = 8\) s

Then:

For \(tx_A\):

\[
waitScore(tx_A)=\min(2/10,1)=0.2
\]

For \(tx_B\):

\[
waitScore(tx_B)=\min(1/10,1)=0.1
\]

For \(tx_C\):

\[
waitScore(tx_C)=\min(8/10,1)=0.8
\]

---

# 19. How to calculate fit step by step

Assume remaining blob space is:

\[
r=30\text{ KB}
\]

and transaction sizes are:

- \(tx_A = 25\) KB
- \(tx_B = 10\) KB
- \(tx_C = 40\) KB

Then using:

\[
fit(tx_i,r)=
\begin{cases}
\frac{b_i}{r}, & b_i \le r \\
0, & b_i > r
\end{cases}
\]

For \(tx_A\):

\[
fit(tx_A,r)=25/30=0.833
\]

For \(tx_B\):

\[
fit(tx_B,r)=10/30=0.333
\]

For \(tx_C\):

since \(40 > 30\),

\[
fit(tx_C,r)=0
\]

So:

- \(tx_A\) is a very good fit,
- \(tx_B\) is a weaker fit,
- \(tx_C\) cannot fit at all.

---

# 20. How to calculate final blob-aware score step by step

Assume the weights are:

\[
\alpha=0.25,\quad \beta=0.50,\quad \gamma=0.25
\]

Now combine the previously computed values.

## Transaction A

\[
feeScore=0,\quad fit=0.833,\quad waitScore=0.2
\]

So:

\[
Score(tx_A)=
0.25(0)+0.50(0.833)+0.25(0.2)
\]

\[
Score(tx_A)=0+0.4165+0.05=0.4665
\]

## Transaction B

\[
feeScore=0.333,\quad fit=0.333,\quad waitScore=0.1
\]

So:

\[
Score(tx_B)=
0.25(0.333)+0.50(0.333)+0.25(0.1)
\]

\[
Score(tx_B)=0.08325+0.1665+0.025=0.27475
\]

## Transaction C

\[
feeScore=1,\quad fit=0,\quad waitScore=0.8
\]

So:

\[
Score(tx_C)=
0.25(1)+0.50(0)+0.25(0.8)
\]

\[
Score(tx_C)=0.25+0+0.2=0.45
\]

## Final ordering

The scores are:

- \(tx_A = 0.4665\)
- \(tx_C = 0.45\)
- \(tx_B = 0.27475\)

So the scheduler chooses \(tx_A\) first.

Even though \(tx_C\) has the highest fee and a long waiting time, it does not fit in the remaining space, so its fit score is zero.

---

# 21. Blob utilization equation

A central metric in blob-aware scheduling is blob utilization.

\[
Utilisation = \frac{\sum b_{tx}}{B_{blob}}
\]

Where:

- \(\sum b_{tx}\) is the total blob bytes used by selected transactions,
- \(B_{blob}\) is total blob capacity.

This tells you how full the blob is.

---

# 22. How to calculate blob utilization

Suppose:

\[
B_{blob}=131072 \text{ bytes}
\]

and selected transactions use:

\[
\sum b_{tx}=120000 \text{ bytes}
\]

Then:

\[
Utilisation = \frac{120000}{131072} \approx 0.9155
\]

As a percentage:

\[
Utilisation \approx 91.55\%
\]

This means the blob is about 91.55% full.

---

# 23. Why blob utilization matters

Blob-aware scheduling tries to maximize utilization because the rollup typically pays per blob unit, not only per used byte.

If the blob is half empty, the system still paid for the blob, but the cost is spread across fewer useful bytes and usually fewer transactions.

So higher utilization usually means lower effective DA cost per transaction.

---

# 24. Full blob-aware scheduling procedure

The score equation alone does not define the full algorithm.

The full procedure is iterative.

## Step 1

Start with an empty batch.

So initially:

\[
\sum b_{tx}=0
\]

and therefore:

\[
r=B_{blob}
\]

## Step 2

For each candidate transaction in the mempool:

1. estimate its encoded size \(b_i\),
2. compute \(feeScore(tx_i)\),
3. compute \(waitScore(tx_i)\),
4. compute \(fit(tx_i,r)\),
5. compute \(Score(tx_i)\).

## Step 3

Choose the highest-scoring transaction that fits.

That means:

- it has a high total score,
- and \(b_i \le r\).

## Step 4

Add it to the batch.

Then update:

\[
\sum b_{tx} \leftarrow \sum b_{tx} + b_i
\]

and recompute:

\[
r = B_{blob} - \sum b_{tx}
\]

## Step 5

Recompute fit scores for remaining transactions.

This is important because \(r\) changed.

A transaction that was a weak fit before may become a strong fit now.

## Step 6

Stop when one of these conditions is true:

1. target utilization reached,
2. timeout reached,
3. no transaction can fit,
4. max transaction count reached,
5. max proof-cost estimate reached.

---

# 25. Target utilization threshold

Define:

\[
U_{\text{target}} = \text{minimum utilization required before normal flush}
\]

Example:

\[
U_{\text{target}}=0.90
\]

Then the batch may be flushed when:

\[
\frac{\sum b_{tx}}{B_{blob}} \ge U_{\text{target}}
\]

This means:

> “If the blob is at least 90% full, it is good enough to submit.”

---

# 26. Timeout in blob-aware scheduling

Just like adaptive batching needs timeout, blob-aware scheduling also needs timeout.

Why?

Because otherwise the algorithm may keep waiting for the perfect fit and delay users too long.

Define:

\[
T_{\max} = \text{maximum allowed wait time before forced flush}
\]

If the oldest transaction has waited too long, flush even if utilization is below target.

This creates the trade-off:

- waiting longer improves blob fill,
- waiting longer also worsens latency.

---

# 27. Maximum waiting-time fairness cap

Define:

\[
W_{\max}
\]

as the waiting-time cap used in the wait score normalization.

A transaction older than \(W_{\max}\) gets maximum fairness score:

\[
waitScore(tx_i)=1
\]

Some systems go further and force-include such transactions in the next batch to guarantee no starvation.

---

# 28. Maximum batch count and proof-cost constraints

Blob packing alone is not enough.

A batch may fit the blob well but still be too expensive to prove.

So practical implementations also enforce:

## Max transaction count

\[
|S| \le N_{\max}
\]

Where \(S\) is the set of selected transactions.

## Max estimated proof cost

Define each transaction's estimated proving cost as \(c_i\), then:

\[
C_{batch}=\sum c_i
\]

Require:

\[
C_{batch} \le C_{\max}
\]

This prevents creation of a DA-efficient but proof-heavy batch.

---

# 29. A complete coding-agent-friendly blob-aware specification

## Inputs

A coding agent needs:

- `candidate_transactions`
- `blob_capacity_bytes`
- `target_utilization`
- `alpha`
- `beta`
- `gamma`
- `wait_cap_ms`
- `flush_timeout_ms`
- `max_batch_tx_count`
- optional `max_estimated_proof_cost`

Each transaction should provide:

- `fee`
- `arrival_time_ms`
- `encoded_size_bytes`
- optional `estimated_proof_cost`

## Output

The algorithm should produce:

- `selected_transactions`
- `used_blob_bytes`
- `remaining_blob_bytes`
- `blob_utilization`
- `flush_reason`

---

# 30. Reference implementation logic for blob-aware scheduling

```text
selected = []
used_bytes = 0

while True:
    remaining = blob_capacity_bytes - used_bytes

    best_tx = None
    best_score = -infinity

    for tx in candidate_transactions not already selected:
        if tx.encoded_size_bytes > remaining:
            fit_score = 0
        else:
            fit_score = tx.encoded_size_bytes / remaining

        fee_score = normalize(tx.fee, min_fee, max_fee)

        wait_ms = now_ms - tx.arrival_time_ms
        wait_score = min(wait_ms / wait_cap_ms, 1.0)

        score = alpha * fee_score + beta * fit_score + gamma * wait_score

        if tx.encoded_size_bytes <= remaining and score > best_score:
            best_score = score
            best_tx = tx

    if best_tx is None:
        break

    selected.append(best_tx)
    used_bytes += best_tx.encoded_size_bytes

    utilization = used_bytes / blob_capacity_bytes

    if utilization >= target_utilization:
        flush_reason = "target_utilization_reached"
        break

    if len(selected) >= max_batch_tx_count:
        flush_reason = "max_batch_tx_count"
        break

    if oldest_wait_ms >= flush_timeout_ms:
        flush_reason = "timeout"
        break
```

---

# 31. Important normalization rules

A coding agent should **never** combine raw fee, raw byte size, and raw waiting time directly.

Bad:

\[
Score = fee + bytes + wait
\]

This is bad because the scales are different.

Example:

- fee might be 100000,
- bytes might be 200,
- wait might be 4.3.

The fee would dominate completely.

So each term should be normalized to a comparable range, typically \([0,1]\).

That is why the score is built from:

- `feeScore`
- `fit`
- `waitScore`

not raw values.

---

# 32. Practical implementation edge cases

## Case A: all fees are equal

Then:

\[
fee_{\max}=fee_{\min}
\]

The standard normalization would divide by zero.

So implementation should handle this safely.

Recommended rule:

```text
if fee_max == fee_min:
    fee_score = 1.0
```

or sometimes `0.0`, depending on desired semantics.

Using `1.0` means fees provide no differentiation, so all transactions are equally fine on the fee dimension.

## Case B: remaining space is zero

If:

\[
r=0
\]

then no more transactions can fit.

Stop batch construction.

## Case C: no transaction fits

If every remaining transaction has:

\[
b_i > r
\]

then all fit scores are zero and no candidate can be added.

Stop and flush if the batch is non-empty.

## Case D: empty mempool

If no transactions are pending, do not build or flush a batch.

## Case E: exact fit

If a transaction exactly matches the remaining space:

\[
b_i = r
\]

then:

\[
fit(tx_i,r)=1
\]

This is the ideal fit.

---

# 33. Relationship between adaptive batching and blob-aware scheduling

These two algorithms are related but solve different decisions.

## Adaptive batching decides quantity

It answers:

> “How many transactions should the next batch aim for?”

Its main input is:

- mempool depth.

Its main output is:

- target batch size.

## Blob-aware scheduling decides composition

It answers:

> “Which transactions should go into that batch?”

Its main inputs are:

- transaction fee,
- transaction size,
- transaction waiting time,
- current remaining blob space.

Its main output is:

- selected transaction set.

## In practice they are combined

A practical sequencer might work like this:

1. use adaptive batching to compute target batch size,
2. use blob-aware scheduling to choose the best transactions up to that batch size and blob capacity,
3. flush when utilization target, count target, or timeout condition is met.

So one algorithm controls **batch size policy**, and the other controls **transaction selection policy**.

---

# 34. Minimal end-to-end combined model

A good combined model is:

## Adaptive batching target

\[
N(t)=
\begin{cases}
N_{\min}, & d < L \\
N_{\text{mid}}, & L \le d < H \\
N_{\max}, & d \ge H
\end{cases}
\]

## Remaining blob space

\[
r = B_{blob} - \sum b_{tx}
\]

## Fit score

\[
fit(tx_i,r)=
\begin{cases}
\frac{b_i}{r}, & b_i \le r \\
0, & b_i > r
\end{cases}
\]

## Fee score

\[
feeScore(tx_i)=
\frac{fee_i-fee_{\min}}{fee_{\max}-fee_{\min}}
\]

with safe handling for equal min and max.

## Wait score

\[
waitScore(tx_i)=\min\left(\frac{wait_i}{W_{\max}},1\right)
\]

## Blob-aware selection score

\[
Score(tx_i)=
\alpha \cdot feeScore(tx_i)
+
\beta \cdot fit(tx_i,r)
+
\gamma \cdot waitScore(tx_i)
\]

## Blob utilization

\[
Utilisation = \frac{\sum b_{tx}}{B_{blob}}
\]

---

# 35. What a coding agent must remember

A coding agent implementing these equations should remember the following rules.

1. **Adaptive batching computes target size, not automatic flush by itself.**  
   Timeout logic is required.

2. **Blob-aware scheduling is iterative.**  
   Fit scores must be recomputed after each selected transaction because remaining blob space changes.

3. **Normalize score components.**  
   Never combine raw fee, raw bytes, and raw wait directly.

4. **Handle division-by-zero cases safely.**  
   This especially matters for fee normalization.

5. **Enforce hard constraints in addition to score optimization.**  
   Examples:
   - max batch size,
   - max blob capacity,
   - max proof cost,
   - timeout.

6. **Packing efficiency is not the only goal.**  
   Fairness and latency must also be protected.

---

# 36. Final plain-language summary

Adaptive batching changes the target batch size based on current mempool load. When few transactions are waiting, it uses smaller batches so users do not wait too long. When many transactions are waiting, it uses larger batches to improve throughput and reduce cost per transaction.

Blob-aware scheduling chooses which transactions go into the batch by scoring each one using three ideas: how much fee it offers, how long it has been waiting, and how well it fits the remaining blob space. The scheduler repeatedly picks the best-fitting high-score transaction until the blob is full enough, the timeout is reached, or another hard limit is hit.

Together, these algorithms let the sequencer adapt to traffic conditions and reduce wasted DA space while still keeping latency and fairness under control.

