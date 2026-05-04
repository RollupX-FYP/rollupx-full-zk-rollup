import { ethers } from "hardhat";

async function main() {
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

  console.log("MockVerifier:", await mockVerifier.getAddress());
  console.log("CalldataDA:", await calldataDA.getAddress());
  console.log("TestBlobDA:", await testBlobDA.getAddress());
  console.log("OffChainDA:", await offchainDA.getAddress());
  console.log("ZKRollupBridge:", await bridge.getAddress());
  console.log("GenesisRoot:", genesisRoot);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
