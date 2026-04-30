const { ethers } = require("hardhat");

async function main() {
  const bridgeAddr = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512";
  const OffChainDA = await ethers.getContractFactory("OffChainDA");
  const offchain = await OffChainDA.deploy();
  await offchain.waitForDeployment();
  const offchainAddr = await offchain.getAddress();

  const bridge = await ethers.getContractAt("ZKRollupBridge", bridgeAddr);
  const tx = await bridge.setDAProvider(2, offchainAddr, true);
  await tx.wait();

  console.log("OffChainDA:", offchainAddr);
  console.log("Bridge setDAProvider tx:", tx.hash);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
