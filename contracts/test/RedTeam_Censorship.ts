import { expect } from "chai";
import { ethers } from "hardhat";
import { HardhatEthersSigner } from "@nomicfoundation/hardhat-ethers/signers";
import { ZKRollupBridge, MockVerifier, CalldataDA } from "../typechain-types";

describe("Red Team: Censorship Resistance Bypass", function () {
  let bridge: ZKRollupBridge;
  let verifier: MockVerifier;
  let calldataDA: CalldataDA;
  let owner: HardhatEthersSigner;
  let sequencer: HardhatEthersSigner;
  let user: HardhatEthersSigner;

  const FORCED_DELAY = 10; // 10 blocks

  beforeEach(async function () {
    [owner, sequencer, user] = await ethers.getSigners();
    const VerifierFactory = await ethers.getContractFactory("MockVerifier");
    verifier = (await VerifierFactory.deploy()) as MockVerifier;
    const CalldataFactory = await ethers.getContractFactory("CalldataDA");
    calldataDA = (await CalldataFactory.deploy()) as CalldataDA;
    const BridgeFactory = await ethers.getContractFactory("ZKRollupBridge");
    bridge = (await BridgeFactory.deploy(
      await verifier.getAddress(),
      ethers.ZeroHash,
      FORCED_DELAY
    )) as ZKRollupBridge;
    await bridge.setSequencer(sequencer.address);
    await bridge.setDAProvider(0, await calldataDA.getAddress(), true);
  });

  it("PASS: Sequencer is BLOCKED (Revert) if forcing deadline is missed", async function () {
    const txHash = ethers.keccak256(ethers.toUtf8Bytes("censored transaction"));

    // 1. Force Transaction
    await bridge.connect(user).forceTransaction(txHash);

    const deadline = (await ethers.provider.getBlockNumber()) + FORCED_DELAY;

    // 2. Mine past deadline
    await ethers.provider.send("hardhat_mine", [ethers.toQuantity(FORCED_DELAY + 1)]);
    expect(await ethers.provider.getBlockNumber()).to.be.gt(deadline);

    // 3. Submit Batch -> Should REVERT with BridgeFrozenError
    const proof = "0x" + "00".repeat(256);
    const newRoot = ethers.keccak256(ethers.toUtf8Bytes("new root"));

    await expect(
        bridge.connect(sequencer).commitBatch(0, 0, "0x1234", "0x", newRoot, proof)
    ).to.be.revertedWithCustomError(bridge, "BridgeFrozenError");

    console.log("SUCCESS: Sequencer was correctly blocked from committing due to censorship check.");
  });
});
