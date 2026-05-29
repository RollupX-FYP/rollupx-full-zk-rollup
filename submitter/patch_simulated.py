import sys

with open('src/daemon.rs', 'r', encoding='utf-8') as f:
    data = f.read()

# Replace Simulated metrics
old_sim_metrics = """                    // Save Metrics (Simulated)
                    let metrics = SubmitterMetrics {
                        submission_status: "offchain_simulated".to_string(),
                        error: None,
                        experiment_id: std::env::var("EXPERIMENT_ID").unwrap_or_else(|_| "default_experiment".to_string()),
                        batch_id: fetched.batch_id.clone(),
                        tx_hash: "0x_offchain_simulated".to_string(),
                        submission_latency_ms: 0,
                        l2_l1_latency_ms: 0,
                        l1_block_number: 0,
                        confirmation_blocks: 0,
                        da_mode: format!("{:?}", cfg.da.mode),
                        proof_metadata_hash: "offchain".to_string(),
                        tx_count,
                        batch_data_bytes,
                        proof_bytes: proof_bytes.len(),
                        compressed_bytes: None,
                        compression_time_ms: None,
                        compression_ratio: None,
                        blob_count: 0,
                        blob_utilization: 0.0,
                        l1_gas_used: None,
                        fee_proxy_wei: fee_proxy_wei.to_string(),
                    };"""

new_sim_metrics = """                    // Save Metrics (Simulated)
                    let bd = CostBreakdown::estimate_calldata(batch_data_bytes, proof_bytes.len(), None);
                    let metrics = SubmitterMetrics {
                        submission_status: "offchain_simulated".to_string(),
                        error: None,
                        experiment_id: std::env::var("EXPERIMENT_ID").unwrap_or_else(|_| "default_experiment".to_string()),
                        batch_id: fetched.batch_id.clone(),
                        tx_hash: "0x_offchain_simulated".to_string(),
                        da_mode: format!("{:?}", cfg.da.mode),
                        da_mode_is_simulated: true,
                        submission_latency_ms: 0,
                        l2_l1_latency_ms: 0,
                        l1_block_number: 0,
                        confirmation_blocks: 0,
                        batch_receive_ms: None, // Simulated doesn't load it from record
                        prover_rtt_ms: None,
                        proof_generation_ms: None,
                        proof_metadata_hash: "offchain".to_string(),
                        tx_count,
                        batch_data_bytes,
                        proof_bytes: proof_bytes.len(),
                        compressed_bytes: None,
                        compression_time_ms: None,
                        compression_ratio: None,
                        blob_count: 0,
                        blob_utilization: 0.0,
                        l1_gas_used: None,
                        fee_proxy_wei: fee_proxy_wei.to_string(),
                        blob_gas_used: None,
                        blob_base_fee_wei: None,
                        blob_fee_total_wei: None,
                        proof_verify_gas_estimate: bd.proof_verify_gas,
                        state_root_update_gas_estimate: bd.state_root_update_gas,
                        da_posting_gas_estimate: bd.da_posting_gas,
                        da_posting_blob_gas_estimate: bd.da_posting_blob_gas,
                        overhead_gas_estimate: bd.overhead_gas,
                        proof_verify_pct: bd.proof_verify_pct,
                        da_pct: bd.da_pct,
                        overhead_pct: bd.overhead_pct,
                        cost_breakdown_is_estimated: bd.is_estimated,
                        gas_bumped: false,
                        gas_bump_count: 0,
                        original_gas_price_gwei: None,
                        final_gas_price_gwei: None,
                    };"""

if old_sim_metrics in data:
    data = data.replace(old_sim_metrics, new_sim_metrics)
    print('Replaced simulated metrics block.')
else:
    print('Could not find simulated metrics block.')

# Replace Simulated CSV header and row
old_sim_csv = """let _ = writeln!(file, "experiment_id,batch_id,batch_size,relay_latency_ms,e2e_latency_ms,da_mode,gas_used");
                                            }
                                        }
                                        let _ = writeln!(file, "{},{},{},{},{},{},{}", 
                                            experiment_id,
                                            record.batch_id,
                                            batch_size,
                                            relay_latency_ms,
                                            e2e_latency_ms,
                                            da_mode,
                                            gas_used
                                        );"""

new_sim_csv = """let _ = writeln!(file, "experiment_id,batch_id,batch_size,tx_count,da_mode,da_mode_is_simulated,gas_bumped,gas_bump_count,original_gas_price_gwei,final_gas_price_gwei,relay_latency_ms,e2e_latency_ms,prover_rtt_ms,proof_generation_ms,l1_gas_used,blob_gas_used,blob_base_fee_wei,blob_fee_total_wei,proof_verify_gas,state_root_update_gas,da_posting_gas,da_posting_blob_gas,overhead_gas,proof_verify_pct,da_pct,overhead_pct,cost_breakdown_is_estimated");
                                            }
                                        }
                                        let _ = writeln!(file, "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}", 
                                            experiment_id,
                                            record.batch_id,
                                            batch_size,
                                            batch_size, // tx_count
                                            da_mode,
                                            true, // da_mode_is_simulated
                                            false, // gas_bumped
                                            0, // gas_bump_count
                                            "", // original_gas_price_gwei
                                            "", // final_gas_price_gwei
                                            relay_latency_ms,
                                            e2e_latency_ms,
                                            "", // prover_rtt_ms
                                            "", // proof_generation_ms
                                            "", // l1_gas_used
                                            "", // blob_gas_used
                                            "", // blob_base_fee_wei
                                            "", // blob_fee_total_wei
                                            bd.proof_verify_gas,
                                            bd.state_root_update_gas,
                                            bd.da_posting_gas,
                                            bd.da_posting_blob_gas,
                                            bd.overhead_gas,
                                            bd.proof_verify_pct,
                                            bd.da_pct,
                                            bd.overhead_pct,
                                            bd.is_estimated
                                        );"""

if old_sim_csv in data:
    data = data.replace(old_sim_csv, new_sim_csv)
    print('Replaced simulated CSV write.')
else:
    print('Could not find simulated CSV write.')

with open('src/daemon.rs', 'w', encoding='utf-8') as f:
    f.write(data)
