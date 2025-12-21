import { expect } from "chai";
import { ethers } from "hardhat";

describe("ZKRollupBridge - calldata mode", function () {
  it("finalizes a batch with mock verifier", async function () {
    const [deployer] = await ethers.getSigners();

    const MockVerifier = await ethers.getContractFactory("MockVerifier");
    const mockVerifier = await MockVerifier.deploy();
    await mockVerifier.waitForDeployment();

    const Bridge = await ethers.getContractFactory("ZKRollupBridge");
    const bridge = await Bridge.deploy(
      await mockVerifier.getAddress(),
      ethers.ZeroHash
    );
    await bridge.waitForDeployment();

    // Optional: restrict to sequencer
    await bridge.setSequencer(deployer.address);

    const batchData = ethers.toUtf8Bytes("hello-batch");
    const newRoot = ethers.keccak256(ethers.toUtf8Bytes("new-root"));

    const proof = {
      a: [0, 0],
      b: [
        [0, 0],
        [0, 0],
      ],
      c: [0, 0],
    };

    await expect(bridge.commitBatchCalldata(batchData, newRoot, proof)).to.emit(
      bridge,
      "BatchFinalized"
    );

    expect(await bridge.stateRoot()).to.equal(newRoot);
  });
});
