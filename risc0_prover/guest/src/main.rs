use risc0_zkvm::guest::env;
use rollup_core::{BlockTrace, LightweightSMT};

fn main() {
    let trace: BlockTrace = env::read();

    let mut state = LightweightSMT::new(trace.initial_root);
    for diff in &trace.state_diffs {
        state.apply_diff(diff).expect("Invalid state transition");
    }

    assert_eq!(state.current_root(), trace.final_root);
    env::commit(&(trace.initial_root, trace.final_root));
}
