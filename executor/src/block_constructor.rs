use crate::proto::rollup::BatchPayload;
use crate::types::ExecutionTraceV1;

pub fn build_enriched_payload(
    input: BatchPayload,
    trace: &ExecutionTraceV1,
    da_commitment: Vec<u8>,
    proof: Vec<u8>,
) -> BatchPayload {
    let normalized_batch_data = serde_json::to_vec(&trace.executed_transactions).unwrap_or_else(|_| input.batch_data.clone());

    BatchPayload {
        batch_id: input.batch_id,
        batch_data: normalized_batch_data,
        pre_state_root: trace.public_inputs.initial_root.to_vec(),
        post_state_root: trace.public_inputs.final_root.to_vec(),
        da_commitment,
        proof,
        experiment_id: input.experiment_id,
    }
}
