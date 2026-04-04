import { ethers } from "hardhat";
import * as fs from "fs";
import * as path from "path";

async function main() {
  const [deployer] = await ethers.getSigners();
  console.log("Deploying contracts with the account:", deployer.address);

  // Configuration from Env Vars
  const genesisRoot = process.env.GENESIS_ROOT || ethers.ZeroHash;
  const forceInclusionDelay = process.env.FORCE_INCLUSION_DELAY || "50";

  console.log("Configuration:");
  console.log(` - Genesis Root: ${genesisRoot}`);
  console.log(` - Force Inclusion Delay: ${forceInclusionDelay} blocks`);

  // 1. Deploy Verifier (Real or Mock)
  let verifierAddr;
  if (process.env.USE_MOCK_VERIFIER === "true") {
      console.log("Using MockVerifier for simulation...");
      const MockVerifier = await ethers.getContractFactory("MockVerifier");
      const mockVerifier = await MockVerifier.deploy();
      await mockVerifier.waitForDeployment();
      verifierAddr = await mockVerifier.getAddress();
      console.log("MockVerifier deployed to:", verifierAddr);
  } else {
      const Groth16Verifier = await ethers.getContractFactory("Groth16Verifier");
      const realVerifier = await Groth16Verifier.deploy();
      await realVerifier.waitForDeployment();
      verifierAddr = await realVerifier.getAddress();
      console.log("Groth16Verifier deployed to:", verifierAddr);
  }

  // Deploy Plonky2 Stub
  const Plonky2Verifier = await ethers.getContractFactory("Plonky2Verifier");
  const plonky2Verifier = await Plonky2Verifier.deploy();
  await plonky2Verifier.waitForDeployment();
  const plonky2Addr = await plonky2Verifier.getAddress();
  console.log("Plonky2Verifier deployed to:", plonky2Addr);

  // Deploy Halo2 Stub
  const Halo2Verifier = await ethers.getContractFactory("Halo2Verifier");
  const halo2Verifier = await Halo2Verifier.deploy();
  await halo2Verifier.waitForDeployment();
  const halo2Addr = await halo2Verifier.getAddress();
  console.log("Halo2Verifier deployed to:", halo2Addr);

  // 2. Deploy DA Providers
  const CalldataDA = await ethers.getContractFactory("CalldataDA");
  const calldataDA = await CalldataDA.deploy();
  await calldataDA.waitForDeployment();
  const calldataAddr = await calldataDA.getAddress();
  console.log("CalldataDA deployed to:", calldataAddr);

  const BlobDA = await ethers.getContractFactory("BlobDA");
  const blobDA = await BlobDA.deploy();
  await blobDA.waitForDeployment();
  const blobAddr = await blobDA.getAddress();
  console.log("BlobDA deployed to:", blobAddr);

  const TestBlobDA = await ethers.getContractFactory("TestBlobDA");
  const testBlobDA = await TestBlobDA.deploy();
  await testBlobDA.waitForDeployment();
  const testBlobAddr = await testBlobDA.getAddress();
  console.log("TestBlobDA deployed to:", testBlobAddr);

  // 3. Deploy Bridge
  const Bridge = await ethers.getContractFactory("ZKRollupBridge");
  // Updated constructor to accept forceInclusionDelay
  const bridge = await Bridge.deploy(verifierAddr, genesisRoot, forceInclusionDelay);
  await bridge.waitForDeployment();
  const bridgeAddr = await bridge.getAddress();
  console.log("ZKRollupBridge deployed to:", bridgeAddr);

  // 4. Register DA Providers & Verifiers

  // Register Verifiers
  // ID 0: Groth16 (Set in constructor, but good to be explicit or if we want to change)
  // ID 1: Plonky2
  let tx = await bridge.setVerifier(1, plonky2Addr);
  await tx.wait();
  console.log("Registered Plonky2Verifier (ID 1)");

  // ID 2: Halo2
  tx = await bridge.setVerifier(2, halo2Addr);
  await tx.wait();
  console.log("Registered Halo2Verifier (ID 2)");

  // ID 0: Calldata
  tx = await bridge.setDAProvider(0, calldataAddr, true);
  await tx.wait();
  console.log("Registered CalldataDA (ID 0)");

  // ID 1: Blob (We use TestBlobDA for local testing to mock hashes)
  // Note: In a real scenario we'd use BlobDA, but for local Hardhat without Blob support, we need TestBlobDA
  tx = await bridge.setDAProvider(1, testBlobAddr, true);
  await tx.wait();
  console.log("Registered TestBlobDA (ID 1)");

  // Save deployments to file
  const deployments = {
    RealVerifier: verifierAddr,
    CalldataDA: calldataAddr,
    BlobDA: blobAddr,
    TestBlobDA: testBlobAddr,
    ZKRollupBridge: bridgeAddr,
    Config: {
        GenesisRoot: genesisRoot,
        ForceInclusionDelay: forceInclusionDelay
    }
  };

  const deploymentPath = path.join(__dirname, "../../deployments.json");
  fs.writeFileSync(deploymentPath, JSON.stringify(deployments, null, 2));
  console.log(`Deployments saved to ${deploymentPath}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
