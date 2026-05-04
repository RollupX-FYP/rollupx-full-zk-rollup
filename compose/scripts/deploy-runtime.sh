#!/bin/sh
set -eu

RUNTIME_DIR="${RUNTIME_DIR:-/runtime}"
L1_RPC_HTTP="${L1_RPC_HTTP:-http://hardhat:8545}"
L1_RPC_WS="${L1_RPC_WS:-ws://hardhat:8545}"
L1_CHAIN_ID="${L1_CHAIN_ID:-31337}"
L1_START_BLOCK="${L1_START_BLOCK:-0}"
SEQUENCER_BATCH_MAX_SIZE="${SEQUENCER_BATCH_MAX_SIZE:-100}"
SEQUENCER_BATCH_TIMEOUT_MS="${SEQUENCER_BATCH_TIMEOUT_MS:-5000}"
SEQUENCER_BATCH_MIN_SIZE="${SEQUENCER_BATCH_MIN_SIZE:-10}"
SEQUENCER_BATCH_MAX_GAS_LIMIT="${SEQUENCER_BATCH_MAX_GAS_LIMIT:-30000000}"
SEQUENCER_POLICY="${SEQUENCER_POLICY:-FCFS}"
SUBMITTER_DA_MODE="${SUBMITTER_DA_MODE:-offchain}"
SUBMITTER_BLOB_BINDING="${SUBMITTER_BLOB_BINDING:-mock}"
SUBMITTER_BLOB_INDEX="${SUBMITTER_BLOB_INDEX:-0}"
SUBMITTER_ARCHIVER_URL="${SUBMITTER_ARCHIVER_URL:-http://archiver-service:3000}"
SUBMITTER_PROOF_BACKEND="${SUBMITTER_PROOF_BACKEND:-groth16}"
SUBMITTER_PROOF_VERIFICATION_MODE="${SUBMITTER_PROOF_VERIFICATION_MODE:-onchain}"
SUBMITTER_PROOF_VERIFIER_ID="${SUBMITTER_PROOF_VERIFIER_ID:-0}"

mkdir -p "${RUNTIME_DIR}"

DEPLOY_LOG="${RUNTIME_DIR}/contracts-deploy.log"
cd /app

echo "[contracts-deployer] deploying contracts against ${L1_RPC_HTTP}"
npx hardhat run scripts/deploy-local.ts --network host_docker | tee "${DEPLOY_LOG}"

MOCK_VERIFIER_ADDRESS="$(awk '/MockVerifier:/ {print $2}' "${DEPLOY_LOG}" | tail -n 1)"
CALLDATA_DA_ADDRESS="$(awk '/CalldataDA:/ {print $2}' "${DEPLOY_LOG}" | tail -n 1)"
TEST_BLOB_DA_ADDRESS="$(awk '/TestBlobDA:/ {print $2}' "${DEPLOY_LOG}" | tail -n 1)"
OFFCHAIN_DA_ADDRESS="$(awk '/OffChainDA:/ {print $2}' "${DEPLOY_LOG}" | tail -n 1)"
BRIDGE_ADDRESS="$(awk '/ZKRollupBridge:/ {print $2}' "${DEPLOY_LOG}" | tail -n 1)"
GENESIS_ROOT="$(awk '/GenesisRoot:/ {print $2}' "${DEPLOY_LOG}" | tail -n 1)"

if [ -z "${MOCK_VERIFIER_ADDRESS}" ] || [ -z "${BRIDGE_ADDRESS}" ]; then
    echo "[contracts-deployer] failed to parse deployment output" >&2
    exit 1
fi

cat > "${RUNTIME_DIR}/contracts.env" <<EOF
MOCK_VERIFIER_ADDRESS=${MOCK_VERIFIER_ADDRESS}
CALLDATA_DA_ADDRESS=${CALLDATA_DA_ADDRESS}
TEST_BLOB_DA_ADDRESS=${TEST_BLOB_DA_ADDRESS}
OFFCHAIN_DA_ADDRESS=${OFFCHAIN_DA_ADDRESS}
BRIDGE_ADDRESS=${BRIDGE_ADDRESS}
GENESIS_ROOT=${GENESIS_ROOT}
L1_RPC_HTTP=${L1_RPC_HTTP}
L1_RPC_WS=${L1_RPC_WS}
L1_CHAIN_ID=${L1_CHAIN_ID}
L1_START_BLOCK=${L1_START_BLOCK}
EOF

cat > "${RUNTIME_DIR}/contracts.json" <<EOF
{
  "mock_verifier": "${MOCK_VERIFIER_ADDRESS}",
  "calldata_da": "${CALLDATA_DA_ADDRESS}",
  "test_blob_da": "${TEST_BLOB_DA_ADDRESS}",
  "offchain_da": "${OFFCHAIN_DA_ADDRESS}",
  "bridge": "${BRIDGE_ADDRESS}",
  "genesis_root": "${GENESIS_ROOT}",
  "l1_rpc_http": "${L1_RPC_HTTP}",
  "l1_rpc_ws": "${L1_RPC_WS}",
  "chain_id": ${L1_CHAIN_ID},
  "start_block": ${L1_START_BLOCK}
}
EOF

cat > "${RUNTIME_DIR}/sequencer.default.toml" <<EOF
[batch]
max_batch_size = ${SEQUENCER_BATCH_MAX_SIZE}
timeout_interval_ms = ${SEQUENCER_BATCH_TIMEOUT_MS}
min_batch_size = ${SEQUENCER_BATCH_MIN_SIZE}
max_gas_limit = ${SEQUENCER_BATCH_MAX_GAS_LIMIT}

[scheduling]
policy_type = "${SEQUENCER_POLICY}"

[api]
host = "0.0.0.0"
port = 3000

[l1]
rpc_url = "${L1_RPC_WS}"
bridge_address = "${BRIDGE_ADDRESS}"
start_block = ${L1_START_BLOCK}

[database]
url = "sqlite:///var/lib/rollupx/sequencer/sequencer.db"

[executor]
grpc_url = "http://executor:50051"
EOF

cat > "${RUNTIME_DIR}/submitter.yaml" <<EOF
network:
  rpc_url: "${L1_RPC_HTTP}"
  chain_id: ${L1_CHAIN_ID}

contracts:
  bridge: "${BRIDGE_ADDRESS}"

da:
  mode: "${SUBMITTER_DA_MODE}"
  blob_binding: "${SUBMITTER_BLOB_BINDING}"
  blob_index: ${SUBMITTER_BLOB_INDEX}
  archiver_url: "${SUBMITTER_ARCHIVER_URL}"

batch:
  data_file: "dummy"
  new_root: "0x00"
  blob_versioned_hash: "0x0100000000000000000000000000000000000000000000000000000000000000"

proof:
  backend: "${SUBMITTER_PROOF_BACKEND}"
  verification_mode: "${SUBMITTER_PROOF_VERIFICATION_MODE}"
  verifier_id: ${SUBMITTER_PROOF_VERIFIER_ID}
EOF

echo "[contracts-deployer] runtime contracts: ${RUNTIME_DIR}/contracts.json"
echo "[contracts-deployer] runtime sequencer config: ${RUNTIME_DIR}/sequencer.default.toml"
echo "[contracts-deployer] runtime submitter config: ${RUNTIME_DIR}/submitter.yaml"
