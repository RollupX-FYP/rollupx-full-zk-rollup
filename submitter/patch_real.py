import sys

with open('src/daemon.rs', 'r', encoding='utf-8') as f:
    data = f.read()

old_metrics = """                        // Save Metrics
                        let metrics = SubmitterMetrics {
                            submission_status: "submitted".to_string(),
                            error: None,
                            experiment_id: std::env::var("EXPERIMENT_ID").unwrap_or_else(|_| "default_experiment".to_string()),
                            batch_id: fetched.batch_id.clone(),
                            tx_hash: result.tx_hash.clone(),
                            submission_latency_ms: latency.as_millis() as u64,
                            l2_l1_latency_ms: result.latency_ms,
                            l1_block_number: result.block_number,
                            confirmation_blocks: confirmations,
                            da_mode: format!("{:?}", cfg.da.mode),
                            proof_metadata_hash: "mock_proof_meta_hash".to_string(),
                            tx_count,
                            batch_data_bytes,
                            proof_bytes: proof_bytes.len(),
                            compressed_bytes: result.compressed_bytes,
                            compression_time_ms: None,
                            compression_ratio: result.compression_ratio,
                            blob_count: if cfg.da.mode == DaMode::Blob {
                                ((result.compressed_bytes.unwrap_or(batch_data_bytes) + BLOB_SIZE_BYTES - 1)
                                    / BLOB_SIZE_BYTES) as u64
                            } else {
                                0
                            },
                            blob_utilization: if cfg.da.mode == DaMode::Blob {
                                let used = result.compressed_bytes.unwrap_or(batch_data_bytes);
                                let blobs = ((used + BLOB_SIZE_BYTES - 1) / BLOB_SIZE_BYTES).max(1);
                                used as f64 / (blobs * BLOB_SIZE_BYTES) as f64
                            } else {
                                0.0
                            },
                            l1_gas_used: result.gas_used,
                            fee_proxy_wei: fee_proxy_wei.to_string(),
                        };"""

new_metrics = """                        // Save Metrics
                        let bd = if cfg.da.mode == DaMode::Blob {
                            let blobs = ((result.compressed_bytes.unwrap_or(batch_data_bytes) + BLOB_SIZE_BYTES - 1) / BLOB_SIZE_BYTES).max(1) as u64;
                            CostBreakdown::estimate_blob(blobs, proof_bytes.len(), result.gas_used, result.blob_gas_used, result.blob_base_fee_wei)
                        } else {
                            CostBreakdown::estimate_calldata(batch_data_bytes, proof_bytes.len(), result.gas_used)
                        };

                        // Extract original gas price and bump info from record if we can
                        let mut gas_bumped = false;
                        let mut gas_bump_count = 0;
                        let mut original_gas_price_gwei = None;
                        if let Ok(Some(rec)) = outbox.get_record(&fetched.batch_id) {
                            gas_bump_count = rec.gas_bump_count;
                            gas_bumped = gas_bump_count > 0;
                            if let Some(og) = rec.original_gas_price {
                                if let Ok(og_wei) = og.parse::<u128>() {
                                    original_gas_price_gwei = Some(og_wei as f64 / 1e9);
                                }
                            }
                        }

                        // Retrieve batch_receive_ms if available
                        let mut batch_receive_ms = None;
                        if let Ok(Some(rec)) = outbox.get_record(&fetched.batch_id) {
                            if let Some(data_json) = rec.batch_data {
                                if let Ok(b) = serde_json::from_str::<Batch>(&data_json) {
                                    batch_receive_ms = b.batch_receive_ms;
                                }
                            }
                        }

                        let metrics = SubmitterMetrics {
                            submission_status: "submitted".to_string(),
                            error: None,
                            experiment_id: std::env::var("EXPERIMENT_ID").unwrap_or_else(|_| "default_experiment".to_string()),
                            batch_id: fetched.batch_id.clone(),
                            tx_hash: result.tx_hash.clone(),
                            submission_latency_ms: latency.as_millis() as u64,
                            l2_l1_latency_ms: result.latency_ms,
                            l1_block_number: result.block_number,
                            confirmation_blocks: confirmations,
                            da_mode: format!("{:?}", cfg.da.mode),
                            da_mode_is_simulated: false,
                            batch_receive_ms,
                            prover_rtt_ms: None, // Will add when Orchestrator runs prover
                            proof_generation_ms: None,
                            proof_metadata_hash: "mock_proof_meta_hash".to_string(),
                            tx_count,
                            batch_data_bytes,
                            proof_bytes: proof_bytes.len(),
                            compressed_bytes: result.compressed_bytes,
                            compression_time_ms: None,
                            compression_ratio: result.compression_ratio,
                            blob_count: if cfg.da.mode == DaMode::Blob {
                                ((result.compressed_bytes.unwrap_or(batch_data_bytes) + BLOB_SIZE_BYTES - 1)
                                    / BLOB_SIZE_BYTES) as u64
                            } else {
                                0
                            },
                            blob_utilization: if cfg.da.mode == DaMode::Blob {
                                let used = result.compressed_bytes.unwrap_or(batch_data_bytes);
                                let blobs = ((used + BLOB_SIZE_BYTES - 1) / BLOB_SIZE_BYTES).max(1);
                                used as f64 / (blobs * BLOB_SIZE_BYTES) as f64
                            } else {
                                0.0
                            },
                            l1_gas_used: result.gas_used,
                            fee_proxy_wei: fee_proxy_wei.to_string(),
                            blob_gas_used: result.blob_gas_used,
                            blob_base_fee_wei: result.blob_base_fee_wei,
                            blob_fee_total_wei: match (result.blob_gas_used, result.blob_base_fee_wei) {
                                (Some(g), Some(f)) => Some(g.saturating_mul(f)),
                                _ => None,
                            },
                            proof_verify_gas_estimate: bd.proof_verify_gas,
                            state_root_update_gas_estimate: bd.state_root_update_gas,
                            da_posting_gas_estimate: bd.da_posting_gas,
                            da_posting_blob_gas_estimate: bd.da_posting_blob_gas,
                            overhead_gas_estimate: bd.overhead_gas,
                            proof_verify_pct: bd.proof_verify_pct,
                            da_pct: bd.da_pct,
                            overhead_pct: bd.overhead_pct,
                            cost_breakdown_is_estimated: bd.is_estimated,
                            gas_bumped,
                            gas_bump_count,
                            original_gas_price_gwei,
                            final_gas_price_gwei: None, // Hard to capture on success immediately without extra API call, set to None for now
                        };"""

if old_metrics in data:
    data = data.replace(old_metrics, new_metrics)
    print('Replaced real metrics block.')
else:
    print('Could not find real metrics block.')

old_csv = """let _ = writeln!(file, "experiment_id,batch_id,batch_size,relay_latency_ms,e2e_latency_ms,da_mode,gas_used");
                                                }
                                            }
                                            let _ = writeln!(file, "{},{},{},{},{},{},{}", experiment_id, fetched.batch_id, batch_size, relay_latency_ms, e2e_latency_ms, da_mode, gas_used);"""

new_csv = """let _ = writeln!(file, "experiment_id,batch_id,batch_size,tx_count,da_mode,da_mode_is_simulated,gas_bumped,gas_bump_count,original_gas_price_gwei,final_gas_price_gwei,relay_latency_ms,e2e_latency_ms,prover_rtt_ms,proof_generation_ms,l1_gas_used,blob_gas_used,blob_base_fee_wei,blob_fee_total_wei,proof_verify_gas,state_root_update_gas,da_posting_gas,da_posting_blob_gas,overhead_gas,proof_verify_pct,da_pct,overhead_pct,cost_breakdown_is_estimated");
                                                }
                                            }
                                            
                                            // Get formatting helpers
                                            let og_gwei = original_gas_price_gwei.map(|v| v.to_string()).unwrap_or_else(|| "".to_string());
                                            let final_gwei = "".to_string();
                                            let prover_rtt = "".to_string(); // we don't have it here
                                            let proof_gen = "".to_string();
                                            let l1_gas = result.gas_used.map(|v| v.to_string()).unwrap_or_else(|| "".to_string());
                                            let blob_gas = result.blob_gas_used.map(|v| v.to_string()).unwrap_or_else(|| "".to_string());
                                            let blob_fee = result.blob_base_fee_wei.map(|v| v.to_string()).unwrap_or_else(|| "".to_string());
                                            let blob_tot = metrics.blob_fee_total_wei.map(|v| v.to_string()).unwrap_or_else(|| "".to_string());

                                            let _ = writeln!(file, "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}", 
                                                experiment_id,
                                                fetched.batch_id,
                                                batch_size,
                                                batch_size, // tx_count
                                                da_mode,
                                                false, // da_mode_is_simulated
                                                gas_bumped,
                                                gas_bump_count,
                                                og_gwei,
                                                final_gwei,
                                                relay_latency_ms,
                                                e2e_latency_ms,
                                                prover_rtt,
                                                proof_gen,
                                                l1_gas,
                                                blob_gas,
                                                blob_fee,
                                                blob_tot,
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

if old_csv in data:
    data = data.replace(old_csv, new_csv)
    print('Replaced real CSV block.')
else:
    print('Could not find real CSV block.')

with open('src/daemon.rs', 'w', encoding='utf-8') as f:
    f.write(data)
