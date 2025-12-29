import { expect } from "chai";
import { ethers } from "hardhat";
import { HardhatEthersSigner } from "@nomicfoundation/hardhat-ethers/signers";
import { ZKRollupBridge, BlobDA, MockVerifier } from "../typechain-types";

describe("Red Team: Blob DA Failure", function () {
  let bridge: ZKRollupBridge;
  let blobDA: BlobDA;
  let verifier: MockVerifier;
  let owner: HardhatEthersSigner;
  let sequencer: HardhatEthersSigner;

  beforeEach(async function () {
    [owner, sequencer] = await ethers.getSigners();
    const VerifierFactory = await ethers.getContractFactory("MockVerifier");
    verifier = (await VerifierFactory.deploy()) as MockVerifier;
    const BlobDAFactory = await ethers.getContractFactory("BlobDA");
    blobDA = (await BlobDAFactory.deploy()) as BlobDA;
    const BridgeFactory = await ethers.getContractFactory("ZKRollupBridge");
    bridge = (await BridgeFactory.deploy(
      await verifier.getAddress(),
      ethers.ZeroHash,
      0
    )) as ZKRollupBridge;
    await bridge.setSequencer(sequencer.address);
    await bridge.setDAProvider(1, await blobDA.getAddress(), true);
  });

  it("FAIL: Real BlobDA reverts when no blob is attached", async function () {
    const newRoot = ethers.keccak256(ethers.toUtf8Bytes("root"));
    const dummyHash = ethers.hexlify(ethers.randomBytes(32));
    const blobIndex = 0;

    // Simulate Submitter: Sending (Hash, Index) in metadata, but NO sidecar
    const daMeta = ethers.AbiCoder.defaultAbiCoder().encode(
        ["bytes32", "uint8"],
        [dummyHash, blobIndex]
    );

    const proof = { a: [0, 0], b: [[0, 0], [0, 0]], c: [0, 0] };

    // Should revert because blobhash(0) == 0 (since tx has no blobs)
    // We verify it reverts with ANY reason to be robust against "NoBlobAttached" vs "BlobHashMismatch"
    await expect(
        bridge.connect(sequencer).commitBatch(1, "0x", daMeta, newRoot, proof)
    ).to.be.reverted;

    console.log("CRITICAL: Confirmed that the Submitter's transaction structure fails on the Real BlobDA contract.");
  });
});
