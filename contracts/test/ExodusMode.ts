import { expect } from "chai";
import { ethers } from "hardhat";
import { HardhatEthersSigner } from "@nomicfoundation/hardhat-ethers/signers";
import { ZKRollupBridge, MockVerifier, CalldataDA } from "../typechain-types";

describe("Exodus Mode: Governance Unfreeze", function () {
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
    await bridge.setDAProvider(0, await calldataDA.getAddress(), true);
  });

  it("Should allow freezing and then unfreezing via governance", async function () {
    const txHash = ethers.keccak256(ethers.toUtf8Bytes("censored"));
    await bridge.connect(user).forceTransaction(txHash);
    await ethers.provider.send("hardhat_mine", [ethers.toQuantity(FORCED_DELAY + 1)]);

    // 1. Prove Censorship (Freeze)
    await expect(bridge.connect(user).freeze())
        .to.emit(bridge, "BridgeFrozen")
        .withArgs("Censorship proven via freeze()");

    expect(await bridge.isFrozen()).to.equal(true);

    // 2. Commit should fail
    const proof = "0x" + "00".repeat(256);
    await expect(
        bridge.connect(sequencer).commitBatch(0, 0, "0x", "0x", ethers.ZeroHash, proof)
    ).to.be.revertedWithCustomError(bridge, "BridgeFrozenError");

    // 3. Unfreeze
    await expect(bridge.connect(owner).unfreeze())
        .to.emit(bridge, "BridgeUnfrozen");

    expect(await bridge.isFrozen()).to.equal(false);
  });

  it("Should revert freeze() if no censorship is detected", async function () {
    // Queue is empty or deadline not passed
    await expect(bridge.connect(user).freeze())
        .to.be.revertedWith("No censorship detected");
  });
});
