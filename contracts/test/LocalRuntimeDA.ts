import { expect } from "chai";
import { ethers } from "hardhat";

describe("local runtime DA registration", function () {
  it("accepts calldata batches after CalldataDA is registered", async function () {
    const MockVerifier = await ethers.getContractFactory("MockVerifier");
    const verifier = await MockVerifier.deploy();
    await verifier.waitForDeployment();

    const Bridge = await ethers.getContractFactory("ZKRollupBridge");
    const bridge = await Bridge.deploy(await verifier.getAddress(), ethers.ZeroHash, 0);
    await bridge.waitForDeployment();

    const CalldataDA = await ethers.getContractFactory("CalldataDA");
    const calldataDA = await CalldataDA.deploy();
    await calldataDA.waitForDeployment();

    await (await bridge.setDAProvider(0, await calldataDA.getAddress(), true)).wait();

    const batchData = ethers.toUtf8Bytes("batch");
    const proof = new Uint8Array(256);
    const newRoot = "0x" + "11".repeat(32);

    await expect(bridge.commitBatch(0, 0, batchData, "0x", newRoot, proof))
      .to.emit(bridge, "BatchCommitted")
      .withArgs(1, 0, 0, ethers.keccak256(batchData), ethers.ZeroHash, newRoot);
  });

  it("rejects calldata batches when CalldataDA is not registered", async function () {
    const MockVerifier = await ethers.getContractFactory("MockVerifier");
    const verifier = await MockVerifier.deploy();
    await verifier.waitForDeployment();

    const Bridge = await ethers.getContractFactory("ZKRollupBridge");
    const bridge = await Bridge.deploy(await verifier.getAddress(), ethers.ZeroHash, 0);
    await bridge.waitForDeployment();

    const batchData = ethers.toUtf8Bytes("batch");
    const proof = new Uint8Array(256);
    const newRoot = "0x" + "11".repeat(32);

    await expect(bridge.commitBatch(0, 0, batchData, "0x", newRoot, proof))
      .to.be.revertedWithCustomError(bridge, "DAProviderNotEnabled");
  });
});
