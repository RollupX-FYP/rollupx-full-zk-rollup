import { expect } from "chai";
import { ethers } from "hardhat";
import { HardhatEthersSigner } from "@nomicfoundation/hardhat-ethers/signers";
import { ZKRollupBridge, MockVerifier, CalldataDA } from "../typechain-types";

describe("Full Coverage: Edge Cases", function () {
  let bridge: ZKRollupBridge;
  let verifier: MockVerifier;
  let calldataDA: CalldataDA;
  let owner: HardhatEthersSigner;
  let sequencer: HardhatEthersSigner;
  let user: HardhatEthersSigner;

  const FORCED_DELAY = 10;

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
  });

  describe("DA Provider Management (Branch Coverage)", function () {
    it("Should allow overwriting provider with SAME address if enabled", async function () {
      await bridge.setDAProvider(0, await calldataDA.getAddress(), true);
      // Overwrite with same
      await expect(bridge.setDAProvider(0, await calldataDA.getAddress(), true))
        .to.emit(bridge, "DAProviderSet");
    });

    it("Should allow overwriting provider with DIFFERENT address if DISABLED", async function () {
      await bridge.setDAProvider(0, await calldataDA.getAddress(), false);
      // Overwrite with diff (simulate by using user address as dummy)
      await expect(bridge.setDAProvider(0, user.address, true))
        .to.emit(bridge, "DAProviderSet");
    });
  });

  describe("Frozen State Logic (Branch Coverage)", function () {
    beforeEach(async function () {
        // Freeze the bridge manually
        const txHash = ethers.keccak256(ethers.toUtf8Bytes("censored"));
        await bridge.connect(user).forceTransaction(txHash);
        await ethers.provider.send("hardhat_mine", [ethers.toQuantity(FORCED_DELAY + 1)]);
        await bridge.freeze();
    });

    it("Should revert forceTransaction when frozen", async function () {
        const txHash = ethers.keccak256(ethers.toUtf8Bytes("new"));
        await expect(bridge.connect(user).forceTransaction(txHash))
            .to.be.revertedWithCustomError(bridge, "BridgeFrozenError");
    });

    it("Should revert commitBatch when frozen (via _requireSequencer)", async function () {
        const proof = { a: [0, 0], b: [[0, 0], [0, 0]], c: [0, 0] };
        await expect(
            bridge.connect(sequencer).commitBatch(0, "0x", "0x", ethers.ZeroHash, proof)
        ).to.be.revertedWithCustomError(bridge, "BridgeFrozenError");
    });
  });

  describe("Unfreeze Logic (Branch Coverage)", function () {
    it("Should revert unfreeze if not frozen", async function () {
        // Bridge starts unfrozen
        await expect(bridge.connect(owner).unfreeze())
            .to.be.reverted; // Revert without reason
    });
  });
});
