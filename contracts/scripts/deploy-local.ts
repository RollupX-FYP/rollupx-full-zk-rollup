import { ethers } from "hardhat";

async function main() {
  // Genesis root (dummy)
  const genesisRoot = ethers.ZeroHash;

  const MockVerifier = await ethers.getContractFactory("MockVerifier");
  const mockVerifier = await MockVerifier.deploy();
  await mockVerifier.waitForDeployment();

  const Bridge = await ethers.getContractFactory("ZKRollupBridge");
  const bridge = await Bridge.deploy(await mockVerifier.getAddress(), genesisRoot);
  await bridge.waitForDeployment();

  console.log("MockVerifier:", await mockVerifier.getAddress());
  console.log("ZKRollupBridge:", await bridge.getAddress());
  console.log("GenesisRoot:", genesisRoot);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
