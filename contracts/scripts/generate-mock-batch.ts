import { ethers } from "hardhat";
import * as fs from "fs";
import * as path from "path";

// Add specific interface for TestBlobDA to ensure TypeScript knows about setMockBlobHash
interface TestBlobDA {
  setMockBlobHash(index: number, hash: string): Promise<any>;
}

async function main() {
  console.log("Generating Mock Batch...");

  // Load deployments
  const deploymentPath = path.join(__dirname, "../../deployments.json");
  if (!fs.existsSync(deploymentPath)) {
    throw new Error("Deployments not found. Run deploy-test-net.ts first.");
  }
  const deployments = JSON.parse(fs.readFileSync(deploymentPath, "utf8"));
  console.log("Loaded deployments:", deployments);

  // Generate Data
  const blobIndex = 0;
  const batchData = ethers.toUtf8Bytes("Local Test Batch Data");

  // Calculate commitments
  // For BlobDA, we need a versioned hash. In real Cancun, this is SHA256 w/ version byte.
  // Here we just use random bytes 32.
  const blobVersionedHash = ethers.hexlify(ethers.randomBytes(32));
  const newRoot = ethers.hexlify(ethers.randomBytes(32));

  console.log("Generated Batch Data:");
  console.log(" - Versioned Hash:", blobVersionedHash);
  console.log(" - New Root:", newRoot);

  // Register the Mock Hash in TestBlobDA
  // This is CRITICAL: The contract will check blobhash(0) against this value.
  // Since we use TestBlobDA, we can inject it.
  const TestBlobDA = await ethers.getContractAt("TestBlobDA", deployments.TestBlobDA);
  const tx = await TestBlobDA.setMockBlobHash(blobIndex, blobVersionedHash);
  await tx.wait();
  console.log("Registered Mock Blob Hash on-chain.");

  // Generate Submitter Config (local_test.yaml)
  const submitterConfig = `
network:
  rpc_url: "http://127.0.0.1:8545"
  chain_id: 31337

contracts:
  bridge: "${deployments.ZKRollupBridge}"

da:
  mode: "blob"
  blob_binding: "mock" # We use mock binding because we don't have real blobs
  blob_index: ${blobIndex}

batch:
  data_file: "../submitter/test_data.txt"
  new_root: "${newRoot}"
  blob_versioned_hash: "${blobVersionedHash}"

resilience:
  max_retries: 1
  circuit_breaker_threshold: 1
`;

  // Write files
  const submitterDir = path.join(__dirname, "../../submitter");
  if (!fs.existsSync(submitterDir)) {
      console.warn("Submitter directory not found at ../../submitter");
  } else {
      fs.writeFileSync(path.join(submitterDir, "local_test.yaml"), submitterConfig);
      fs.writeFileSync(path.join(submitterDir, "test_data.txt"), "Local Test Batch Data");
      console.log(`Config written to ${path.join(submitterDir, "local_test.yaml")}`);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
