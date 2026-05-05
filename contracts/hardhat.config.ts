import { HardhatUserConfig } from "hardhat/config";
import "@nomicfoundation/hardhat-toolbox";
import "@nomicfoundation/hardhat-ignition-ethers";
import "hardhat-gas-reporter";
import "hardhat-contract-sizer";
import * as dotenv from "dotenv";

dotenv.config();

// ─── Reference constants for cost calculations ───────────────────────────────
// Fix these across ALL experiments. State them clearly in your paper.
const REFERENCE_GAS_PRICE_GWEI = 2;          // low-congestion baseline
const REFERENCE_GAS_PRICE_HIGH_GWEI = 50;    // high-congestion scenario
const REFERENCE_ETH_USD = 2500;              // fixed reference — not live price

const config: HardhatUserConfig = {
  solidity: {
    version: "0.8.24",
    settings: {
      viaIR: true,
      optimizer: {
        enabled: true,
        runs: 200,          // 200 = balanced deploy cost vs call cost
                            // use runs: 1 to measure unoptimized baseline
                            // use runs: 1000000 for call-heavy optimization
      },
      evmVersion: "cancun", // Required for blobhash + blob transactions
    },
  },

  networks: {
    // ── Primary experiment network: controlled, reproducible ─────────────────
    hardhat: {
      chainId: 31337,
      
      // Block time: match Ethereum's ~12s slot time for realistic conditions
      // Use interval: 0 only for unit tests, never for latency experiments
      mining: {
        auto: true,
        interval: process.env.HARDHAT_MINING_INTERVAL 
          ? parseInt(process.env.HARDHAT_MINING_INTERVAL) 
          : 12000,    // Default to 12s if not specified
      },

      // Gas configuration — must match what you state in your paper
      gasPrice: REFERENCE_GAS_PRICE_GWEI * 1e9,  // in wei
      gas: 30_000_000,       // 30M gas limit = current Ethereum block gas limit
      
      // EIP-1559 base fee configuration
      // initialBaseFeePerGas: 0 disables EIP-1559 — don't do this for research
      // Leave it at default (1 gwei) to simulate real fee market behavior
      initialBaseFeePerGas: 1_000_000_000,  // 1 gwei base fee starting point

      // Blob configuration for EIP-4844 experiments
      // Hardhat 2.22+ supports blob transactions natively
      // These match mainnet parameters from EIP-4844 spec
      // maxBlobsPerBlock: 6,     // uncomment if your hardhat version supports it
      
      // Account funding — enough for extensive experiments
      // Each account gets 10,000 ETH — never runs dry during experiments
      accounts: {
        mnemonic: process.env.TEST_MNEMONIC || 
          "test test test test test test test test test test test junk",
        count: 50,              // 50 accounts: deployer + sequencer + 48 test users
        accountsBalance: "10000000000000000000000", // 10,000 ETH each
      },

      // Allow unlimited contract sizes during development
      // Remove for final experiments to catch size limit violations
      allowUnlimitedContractSize: false,

      // Forking — enable for DA cost realism checks against real state
      // Comment out for primary experiments (determinism required)
      // forking: {
      //   url: process.env.MAINNET_RPC_URL || "",
      //   blockNumber: 21_000_000,  // pin to specific block for reproducibility
      // },
    },

    // ── High-congestion scenario network ─────────────────────────────────────
    // Run identical experiments here to show cost sensitivity to gas price
    hardhat_congested: {
      url: "http://127.0.0.1:8545",
      chainId: 31337,
      gasPrice: REFERENCE_GAS_PRICE_HIGH_GWEI * 1e9,
      gas: 30_000_000,
    },

    // ── Docker internal network (CI / reproducibility) ────────────────────────
    host_docker: {
      url: process.env.L1_NODE_URL || "http://l1-node:8545",
      chainId: 31337,
      // Don't hardcode gas here — let the network report it
      // so docker experiments use whatever the node is configured with
      timeout: 60_000,        // 60s timeout for slow docker networks
    },

    // ── Public testnet: realism checks only, not primary experiments ──────────
    sepolia: {
      url: process.env.SEPOLIA_RPC_URL || "",
      accounts: process.env.PRIVATE_KEY ? [process.env.PRIVATE_KEY] : [],
      
      // Don't set gasPrice here — let it float to capture real conditions
      // Your paper should note Sepolia results are indicative, not controlled
      timeout: 120_000,       // 2 min timeout — Sepolia can be slow
      confirmations: 2,       // wait for 2 confirmations for stability
    },
  },

  // ── Gas reporter: essential for your cost breakdown research ─────────────
  // Produces per-function gas measurements automatically on every test run
  gasReporter: {
    enabled: true,
    currency: "USD",
    gasPrice: REFERENCE_GAS_PRICE_GWEI,    // gwei
    ethPrice: REFERENCE_ETH_USD.toString(),
    outputFile: "reports/gas-report.txt",
    noColors: true,            // clean output for CI / file logging
    
    // Report format options
    showTimeSpent: true,       // shows wall clock time per test
    showMethodSig: true,       // shows full function signatures
    
    // Exclude test helpers from report
    excludeContracts: ["Mock", "Test", "Fixture"],
    
    // For your research: this reports gas per FUNCTION CALL
    // You still need your own scripts for per-BATCH breakdown
    // These complement each other — don't replace one with the other
  },

  // ── Contract sizer: catch size limit violations before deployment ─────────
  contractSizer: {
    alphaSort: true,
    runOnCompile: true,
    disambiguatePaths: false,
    strict: true,              // fail build if any contract exceeds 24KB limit
  },

  // ── Mocha test configuration ──────────────────────────────────────────────
  mocha: {
    timeout: 300_000,          // 5 min timeout for proof generation tests
    // For experiment scripts (not unit tests), you'll run these directly
    // with ts-node rather than through mocha
  },

  // ── Path configuration ────────────────────────────────────────────────────
  paths: {
    sources: "./contracts",
    tests: "./test",
    cache: "./cache",
    artifacts: "./artifacts",
  },
};

export default config;