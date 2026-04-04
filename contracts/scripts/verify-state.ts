import { ethers } from "hardhat";
import * as fs from "fs";
import * as path from "path";

async function main() {
  console.log("Verifying On-Chain State...");

  // Load deployments
  const deploymentPath = path.join(__dirname, "../../deployments.json");
  if (!fs.existsSync(deploymentPath)) {
    throw new Error("Deployments not found.");
  }
  const deployments = JSON.parse(fs.readFileSync(deploymentPath, "utf8"));

  const Bridge = await ethers.getContractAt("ZKRollupBridge", deployments.ZKRollupBridge);

  // Check State
  const nextBatchId = await Bridge.nextBatchId();
  const stateRoot = await Bridge.stateRoot();

  console.log("Current State:");
  console.log(` - Next Batch ID: ${nextBatchId}`);
  console.log(` - State Root: ${stateRoot}`);

  // Load expected values from config
  const configPath = path.join(__dirname, "../../submitter/local_test.yaml");
  const configContent = fs.readFileSync(configPath, "utf8");
  const expectedRootMatch = configContent.match(/new_root: "(0x[0-9a-fA-F]+)"/);

  if (!expectedRootMatch) {
      throw new Error("Could not parse expected root from config");
  }
  const expectedRoot = expectedRootMatch[1];

  if (stateRoot === expectedRoot) {
      console.log("✅ SUCCESS: State Root matches expected value.");
  } else {
      console.error("❌ FAILURE: State Root does NOT match.");
      console.error(`   Expected: ${expectedRoot}`);
      console.error(`   Actual:   ${stateRoot}`);
      // If batch ID is still 1, it means no batch was committed
      if (nextBatchId == 1n) {
          console.error("   Reason: Batch was not committed (Next Batch ID is still 1)");
      }
      process.exit(1);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
