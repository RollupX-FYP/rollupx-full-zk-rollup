import { ethers } from "hardhat";
import * as fs from "fs";
import * as path from "path";

async function main() {
  const [deployer] = await ethers.getSigners();
  const network = await ethers.provider.getNetwork();

  // Genesis root (dummy)
  const genesisRoot = ethers.ZeroHash;

  const MockVerifier = await ethers.getContractFactory("MockVerifier");
  const mockVerifier = await MockVerifier.deploy();
  await mockVerifier.waitForDeployment();

  const Bridge = await ethers.getContractFactory("ZKRollupBridge");
  const bridge = await Bridge.deploy(await mockVerifier.getAddress(), genesisRoot, 0);
  await bridge.waitForDeployment();

  const CalldataDA = await ethers.getContractFactory("CalldataDA");
  const calldataDA = await CalldataDA.deploy();
  await calldataDA.waitForDeployment();

  const TestBlobDA = await ethers.getContractFactory("TestBlobDA");
  const testBlobDA = await TestBlobDA.deploy();
  await testBlobDA.waitForDeployment();

  const OffChainDA = await ethers.getContractFactory("OffChainDA");
  const offchainDA = await OffChainDA.deploy();
  await offchainDA.waitForDeployment();

  let tx = await bridge.setDAProvider(0, await calldataDA.getAddress(), true);
  await tx.wait();
  tx = await bridge.setDAProvider(1, await testBlobDA.getAddress(), true);
  await tx.wait();
  tx = await bridge.setDAProvider(2, await offchainDA.getAddress(), true);
  await tx.wait();

  const mockVerifierAddress = await mockVerifier.getAddress();
  const calldataDAAddress = await calldataDA.getAddress();
  const testBlobDAAddress = await testBlobDA.getAddress();
  const offchainDAAddress = await offchainDA.getAddress();
  const bridgeAddress = await bridge.getAddress();

  const deployment = {
    network: network.name,
    chainId: Number(network.chainId),
    deployer: deployer.address,
    bridge: bridgeAddress,
    verifier: mockVerifierAddress,
    mockVerifier: mockVerifierAddress,
    calldataDA: calldataDAAddress,
    blobDA: testBlobDAAddress,
    testBlobDA: testBlobDAAddress,
    offchainDA: offchainDAAddress,
    genesisRoot,
    daProviders: {
      calldata: { id: 0, address: calldataDAAddress },
      blob: { id: 1, address: testBlobDAAddress },
      offchain: { id: 2, address: offchainDAAddress },
    },

    // Legacy aliases consumed by older shell tooling.
    mock_verifier: mockVerifierAddress,
    calldata_da: calldataDAAddress,
    test_blob_da: testBlobDAAddress,
    offchain_da: offchainDAAddress,
    genesis_root: genesisRoot,
  };

  const outFile = path.resolve(
    process.env.DEPLOYMENT_OUT || "deployments/hardhat-local.json",
  );
  fs.mkdirSync(path.dirname(outFile), { recursive: true });
  fs.writeFileSync(outFile, `${JSON.stringify(deployment, null, 2)}\n`);

  console.log("MockVerifier:", mockVerifierAddress);
  console.log("CalldataDA:", calldataDAAddress);
  console.log("TestBlobDA:", testBlobDAAddress);
  console.log("OffChainDA:", offchainDAAddress);
  console.log("ZKRollupBridge:", bridgeAddress);
  console.log("GenesisRoot:", genesisRoot);
  console.log("DeploymentOut:", outFile);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
