import { expect } from "chai";
import { ethers } from "hardhat";
import { MockVerifier, ZKRollupBridge, TestZKRollupBridge } from "../typechain-types";
import { SignerWithAddress } from "@nomicfoundation/hardhat-ethers/signers";

describe("ZKRollupBridge", function () {
  let bridge: ZKRollupBridge;
  let testBridge: TestZKRollupBridge;
  let verifier: MockVerifier;
  let owner: SignerWithAddress;
  let sequencer: SignerWithAddress;
  let otherAccount: SignerWithAddress;

  const genesisRoot = ethers.ZeroHash;
  const initialOwner = "0x0000000000000000000000000000000000000000"; // Placeholder, will be replaced by actual owner in test logic

  beforeEach(async function () {
    [owner, sequencer, otherAccount] = await ethers.getSigners();

    // Deploy Mock Verifier
    const Verifier = await ethers.getContractFactory("MockVerifier");
    verifier = await Verifier.deploy();

    // Deploy Bridge
    const Bridge = await ethers.getContractFactory("ZKRollupBridge");
    bridge = await Bridge.deploy(verifier.target, genesisRoot);

    // Deploy Test Bridge for mocking blobhash
    const TestBridge = await ethers.getContractFactory("TestZKRollupBridge");
    testBridge = await TestBridge.deploy(verifier.target, genesisRoot);
  });

  describe("Deployment", function () {
    it("Should set the right owner", async function () {
      expect(await bridge.owner()).to.equal(owner.address);
    });

    it("Should set the initial state", async function () {
      expect(await bridge.verifier()).to.equal(verifier.target);
      expect(await bridge.stateRoot()).to.equal(genesisRoot);
      expect(await bridge.nextBatchId()).to.equal(1);
    });
  });

  describe("Sequencer Management", function () {
    it("Should allow owner to set sequencer", async function () {
      await expect(bridge.setSequencer(sequencer.address))
        .to.emit(bridge, "SequencerUpdated")
        .withArgs(sequencer.address);
      expect(await bridge.sequencer()).to.equal(sequencer.address);
    });

    it("Should not allow non-owner to set sequencer", async function () {
      await expect(
        bridge.connect(otherAccount).setSequencer(sequencer.address)
      ).to.be.revertedWithCustomError(bridge, "OwnableUnauthorizedAccount");
    });
  });

  describe("Commit Batch Calldata", function () {
    const batchData = ethers.toUtf8Bytes("some batch data");
    const daCommitment = ethers.keccak256(batchData);
    const newRoot = ethers.hexlify(ethers.randomBytes(32));
    const proof: any = {
      a: [0, 0],
      b: [[0, 0], [0, 0]],
      c: [0, 0],
    };

    it("Should commit batch successfully by sequencer", async function () {
      await bridge.setSequencer(sequencer.address);

      await expect(
        bridge.connect(sequencer).commitBatchCalldata(batchData, newRoot, proof)
      )
        .to.emit(bridge, "BatchFinalized")
        .withArgs(1, daCommitment, genesisRoot, newRoot, 0);

      expect(await bridge.stateRoot()).to.equal(newRoot);
      expect(await bridge.batchCommitment(1)).to.equal(daCommitment);
      expect(await bridge.batchNewRoot(1)).to.equal(newRoot);
      expect(await bridge.nextBatchId()).to.equal(2);
    });

    it("Should commit batch successfully in permissionless mode", async function () {
      // sequencer is address(0) by default
      await expect(
        bridge.connect(otherAccount).commitBatchCalldata(batchData, newRoot, proof)
      )
        .to.emit(bridge, "BatchFinalized")
        .withArgs(1, daCommitment, genesisRoot, newRoot, 0);
    });

    it("Should revert if called by non-sequencer when sequencer is set", async function () {
      await bridge.setSequencer(sequencer.address);
      await expect(
        bridge.connect(otherAccount).commitBatchCalldata(batchData, newRoot, proof)
      ).to.be.revertedWithCustomError(bridge, "NotSequencer");
    });

    it("Should revert if newRoot is zero", async function () {
      await expect(
        bridge.commitBatchCalldata(batchData, ethers.ZeroHash, proof)
      ).to.be.revertedWithCustomError(bridge, "InvalidNewRoot");
    });

    it("Should revert if proof verification fails", async function () {
      await verifier.setShouldVerify(false);
      await expect(
        bridge.commitBatchCalldata(batchData, newRoot, proof)
      ).to.be.revertedWithCustomError(bridge, "InvalidProof");
    });
  });

  describe("Commit Batch Blob", function () {
    const expectedVersionedHash = ethers.hexlify(ethers.randomBytes(32));
    const newRoot = ethers.hexlify(ethers.randomBytes(32));
    const proof: any = {
      a: [0, 0],
      b: [[0, 0], [0, 0]],
      c: [0, 0],
    };

    it("Should commit blob batch successfully (mock blobhash)", async function () {
      // useOpcodeBlobhash = false for testing without actual blob
      await expect(
        bridge.commitBatchBlob(expectedVersionedHash, 0, false, newRoot, proof)
      )
        .to.emit(bridge, "BatchFinalized")
        .withArgs(1, expectedVersionedHash, genesisRoot, newRoot, 1);
      
       expect(await bridge.stateRoot()).to.equal(newRoot);
       expect(await bridge.batchCommitment(1)).to.equal(expectedVersionedHash);
    });

     it("Should revert if expectedVersionedHash is zero", async function () {
      await expect(
        bridge.commitBatchBlob(ethers.ZeroHash, 0, false, newRoot, proof)
      ).to.be.revertedWithCustomError(bridge, "InvalidDACommitment");
    });
    
    it("Should revert if newRoot is zero", async function () {
      await expect(
        bridge.commitBatchBlob(expectedVersionedHash, 0, false, ethers.ZeroHash, proof)
      ).to.be.revertedWithCustomError(bridge, "InvalidNewRoot");
    });

    it("Should revert if proof verification fails", async function () {
        await verifier.setShouldVerify(false);
        await expect(
            bridge.commitBatchBlob(expectedVersionedHash, 0, false, newRoot, proof)
        ).to.be.revertedWithCustomError(bridge, "InvalidProof");
    });

     // Testing the Cancun logic requires the environment to support blobhash. 
     // Hardhat's default network doesn't support blobhash yet in a way we can easily mock via opcode
     // unless we use specific hardfork settings and maybe hardhat primitives to inject blobs, 
     // which is complex.
     // However, we can try to test the logic by calling with useOpcodeBlobhash=true
     // and expecting it to fail with NoBlobAttached (since local hardhat block has no blobs).
     it("Should revert with NoBlobAttached if blob is missing when useOpcodeBlobhash is true", async function () {
         // The blobhash(0) returns 0 if no blob is attached.
         // Contract checks: if (actual == bytes32(0)) revert NoBlobAttached();
         // using testBridge to cover fallback to super._getBlobHash
         await expect(
             testBridge.commitBatchBlob(expectedVersionedHash, 0, true, newRoot, proof)
         ).to.be.revertedWithCustomError(testBridge, "NoBlobAttached");
     });

    it("Should revert with BlobHashMismatch if blob hash does not match", async function () {
        // Set up test bridge with mock blob hash
        const blobIndex = 0;
        const mockHash = ethers.hexlify(ethers.randomBytes(32));
        await testBridge.setMockBlobHash(blobIndex, mockHash);

        // useOpcodeBlobhash = true
        // expectedVersionedHash != mockHash
        await expect(
            testBridge.commitBatchBlob(expectedVersionedHash, blobIndex, true, newRoot, proof)
        ).to.be.revertedWithCustomError(testBridge, "BlobHashMismatch");
    });
  });
});
