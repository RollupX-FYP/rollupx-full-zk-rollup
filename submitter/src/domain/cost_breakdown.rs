/// Component-level gas cost breakdown for a single batch submission.
///
/// Gas cost on L1 is not a monolithic number — it has structurally distinct
/// components that your research claims to measure independently.
/// This module provides both estimation (from known constants) and population
/// from actual receipt data.
///
/// EIP-4844 blob gas is priced on a *separate* fee market from EIP-1559.
/// `l1_gas_used` from a receipt does NOT include blob gas. Always record both.
use serde::{Deserialize, Serialize};

/// Constants for gas estimation (Ethereum mainnet/testnet values)
/// Calldata costs: non-zero byte = 16 gas, zero byte = 4 gas
const CALLDATA_NON_ZERO_GAS: u64 = 16;
const CALLDATA_ZERO_GAS: u64 = 4;
/// Base transaction cost (no data)
const BASE_TX_GAS: u64 = 21_000;
/// EIP-4844: gas per blob (128 KB = 131_072 bytes)
pub const BLOB_GAS_PER_BLOB: u64 = 131_072;
/// Approximate on-chain Groth16 verify cost (pairing-heavy)
const GROTH16_VERIFY_GAS: u64 = 250_000;
/// SSTORE (warm) × 2 for state root update
const STATE_ROOT_UPDATE_GAS: u64 = 10_000;
/// ABI decode + event emit + misc overhead above base tx
const ABI_OVERHEAD_GAS: u64 = 5_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    /// On-chain ZK proof verification (pairing operations)
    pub proof_verify_gas: u64,
    /// State root SSTORE updates on the bridge contract
    pub state_root_update_gas: u64,
    /// Calldata posting cost (non-zero bytes × 16 + zero bytes × 4)
    pub da_posting_gas: u64,
    /// EIP-4844 blob gas units consumed (separate from EIP-1559 gas)
    pub da_posting_blob_gas: u64,
    /// Base tx (21000) + ABI overhead
    pub overhead_gas: u64,
    /// Sum of all EIP-1559 gas components (does NOT include blob gas)
    pub total_eip1559_gas: u64,

    // --- EIP-4844 blob fee market (only populated for Blob DA mode) ---
    /// Blob gas actually used (from receipt, if available)
    pub blob_gas_used: Option<u64>,
    /// Blob base fee in wei at time of inclusion
    pub blob_base_fee_wei: Option<u64>,
    /// Total blob fee = blob_gas_used × blob_base_fee_wei
    pub blob_fee_total_wei: Option<u64>,

    // --- Derived percentages (of total_eip1559_gas) ---
    pub proof_verify_pct: f64,
    pub da_pct: f64,
    pub overhead_pct: f64,

    /// True if these are estimates (no receipt data), false if from actual receipt
    pub is_estimated: bool,
}

impl CostBreakdown {
    /// Estimate cost breakdown for a **Calldata** submission.
    ///
    /// `calldata_bytes`: raw batch payload bytes (post-compression if any).
    /// `proof_bytes`: serialized proof bytes sent in calldata.
    /// `actual_gas`: actual gas used from receipt (if available; used only to
    ///   scale overhead to match reality).
    pub fn estimate_calldata(
        calldata_bytes: usize,
        proof_bytes: usize,
        actual_gas: Option<u64>,
    ) -> Self {
        // Count zero vs non-zero bytes — for estimation assume worst-case (all non-zero)
        let da_gas = calldata_bytes as u64 * CALLDATA_NON_ZERO_GAS;
        let proof_calldata_gas = proof_bytes as u64 * CALLDATA_NON_ZERO_GAS;
        let proof_verify_gas = GROTH16_VERIFY_GAS + proof_calldata_gas;
        let state_root_gas = STATE_ROOT_UPDATE_GAS;
        let overhead_gas = BASE_TX_GAS + ABI_OVERHEAD_GAS;

        let estimated_total = proof_verify_gas + state_root_gas + da_gas + overhead_gas;

        // If we have actual gas, scale overhead to account for the difference
        let (total_eip1559_gas, overhead_final) = if let Some(actual) = actual_gas {
            let diff = actual.saturating_sub(estimated_total);
            (actual, overhead_gas + diff)
        } else {
            (estimated_total, overhead_gas)
        };

        Self::build(
            proof_verify_gas,
            state_root_gas,
            da_gas,
            0,
            overhead_final,
            total_eip1559_gas,
            None,
            None,
            None,
            actual_gas.is_none(),
        )
    }

    /// Estimate cost breakdown for a **Blob** submission.
    ///
    /// `blob_count`: number of EIP-4844 blobs used.
    /// `proof_bytes`: serialized proof bytes in the EIP-1559 calldata portion.
    /// `actual_eip1559_gas`: gas used (EIP-1559 portion) from receipt.
    /// `blob_gas_used`: blob gas units from receipt (field `blobGasUsed`).
    /// `blob_base_fee_wei`: blob base fee per gas unit at inclusion time.
    pub fn estimate_blob(
        blob_count: u64,
        proof_bytes: usize,
        actual_eip1559_gas: Option<u64>,
        blob_gas_used: Option<u64>,
        blob_base_fee_wei: Option<u64>,
    ) -> Self {
        let blob_gas = blob_count * BLOB_GAS_PER_BLOB;
        // For blob tx: calldata only carries proof + ABI-encoded meta, not batch data
        let proof_calldata_gas = proof_bytes as u64 * CALLDATA_NON_ZERO_GAS;
        let proof_verify_gas = GROTH16_VERIFY_GAS + proof_calldata_gas;
        let state_root_gas = STATE_ROOT_UPDATE_GAS;
        // No batch data in calldata for blob mode
        let da_posting_gas: u64 = 0;
        let overhead_gas = BASE_TX_GAS + ABI_OVERHEAD_GAS;

        let estimated_total = proof_verify_gas + state_root_gas + da_posting_gas + overhead_gas;
        let (total_eip1559_gas, overhead_final) = if let Some(actual) = actual_eip1559_gas {
            let diff = actual.saturating_sub(estimated_total);
            (actual, overhead_gas + diff)
        } else {
            (estimated_total, overhead_gas)
        };

        let blob_fee_total = match (blob_gas_used, blob_base_fee_wei) {
            (Some(g), Some(f)) => Some(g.saturating_mul(f)),
            _ => None,
        };

        Self::build(
            proof_verify_gas,
            state_root_gas,
            da_posting_gas,
            blob_gas,
            overhead_final,
            total_eip1559_gas,
            blob_gas_used,
            blob_base_fee_wei,
            blob_fee_total,
            actual_eip1559_gas.is_none() || blob_gas_used.is_none() || blob_base_fee_wei.is_none(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        proof_verify_gas: u64,
        state_root_update_gas: u64,
        da_posting_gas: u64,
        da_posting_blob_gas: u64,
        overhead_gas: u64,
        total_eip1559_gas: u64,
        blob_gas_used: Option<u64>,
        blob_base_fee_wei: Option<u64>,
        blob_fee_total_wei: Option<u64>,
        is_estimated: bool,
    ) -> Self {
        let total_f = total_eip1559_gas as f64;
        let (proof_verify_pct, da_pct, overhead_pct) = if total_f > 0.0 {
            (
                proof_verify_gas as f64 / total_f * 100.0,
                da_posting_gas as f64 / total_f * 100.0,
                overhead_gas as f64 / total_f * 100.0,
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        Self {
            proof_verify_gas,
            state_root_update_gas,
            da_posting_gas,
            da_posting_blob_gas,
            overhead_gas,
            total_eip1559_gas,
            blob_gas_used,
            blob_base_fee_wei,
            blob_fee_total_wei,
            proof_verify_pct,
            da_pct,
            overhead_pct,
            is_estimated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calldata_breakdown_percentages_sum_near_100() {
        let bd = CostBreakdown::estimate_calldata(50_000, 256, None);
        let total_pct = bd.proof_verify_pct
            + bd.da_pct
            + bd.overhead_pct
            + (bd.state_root_update_gas as f64 / bd.total_eip1559_gas as f64 * 100.0);
        assert!(
            (total_pct - 100.0).abs() < 0.1,
            "percentages should sum to ~100, got {:.2}",
            total_pct
        );
    }

    #[test]
    fn test_actual_gas_overrides_estimate_total() {
        let bd = CostBreakdown::estimate_calldata(1_000, 128, Some(400_000));
        assert_eq!(bd.total_eip1559_gas, 400_000);
        assert!(!bd.is_estimated);
    }

    #[test]
    fn test_blob_fee_computed_correctly() {
        let bd = CostBreakdown::estimate_blob(2, 256, None, Some(262_144), Some(1_000));
        assert_eq!(bd.blob_fee_total_wei, Some(262_144 * 1_000));
    }

    #[test]
    fn test_blob_gas_separate_from_eip1559() {
        let bd = CostBreakdown::estimate_blob(1, 128, Some(280_000), Some(131_072), None);
        assert_eq!(bd.total_eip1559_gas, 280_000);
        assert_eq!(bd.da_posting_blob_gas, BLOB_GAS_PER_BLOB);
        assert!(bd.is_estimated);
        // blob gas is NOT in total_eip1559_gas
        assert_ne!(bd.total_eip1559_gas, 280_000 + BLOB_GAS_PER_BLOB);
    }

    #[test]
    fn test_blob_breakdown_is_measured_only_with_receipt_blob_fields() {
        let bd = CostBreakdown::estimate_blob(1, 128, Some(280_000), Some(131_072), Some(7));
        assert_eq!(bd.blob_fee_total_wei, Some(131_072 * 7));
        assert!(!bd.is_estimated);
    }

    #[test]
    fn test_zero_gas_no_panic() {
        let bd = CostBreakdown::estimate_calldata(0, 0, Some(0));
        assert_eq!(bd.proof_verify_pct, 0.0);
        assert_eq!(bd.da_pct, 0.0);
    }
}
