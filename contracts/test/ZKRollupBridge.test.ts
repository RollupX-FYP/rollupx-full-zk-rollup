import { expect } from "chai";
import { ethers } from "hardhat";
import { 
  MockVerifier, 
  ZKRollupBridge, 
  CalldataDA, 
  BlobDA, 
  TestBlobDA 
} from "../typechain-types";
import { SignerWithAddress } from "@nomicfoundation/hardhat-ethers/signers";

describe("ZKRollupBridge", function () {
  let bridge: ZKRollupBridge;
  let verifier: MockVerifier;
  let calldataDA: CalldataDA;
  let blobDA: BlobDA;
  let testBlobDA: TestBlobDA;
  let owner: SignerWithAddress;
  let sequencer: SignerWithAddress;
  let otherAccount: SignerWithAddress;

  const genesisRoot = ethers.ZeroHash;
  const initialOwner = "0x0000000000000000000000000000000000000000"; 

  // DA Constants
  const CALLDATA_DA_ID = 0;
  const BLOB_DA_ID = 1;

  beforeEach(async function () {
    [owner, sequencer, otherAccount] = await ethers.getSigners();

    // Deploy Mock Verifier
    const Verifier = await ethers.getContractFactory("MockVerifier");
    verifier = await Verifier.deploy();

    // Deploy DA Providers
    const CalldataDA = await ethers.getContractFactory("CalldataDA");
    calldataDA = await CalldataDA.deploy();

    const BlobDA = await ethers.getContractFactory("BlobDA");
    blobDA = await BlobDA.deploy();

    const TestBlobDA = await ethers.getContractFactory("TestBlobDA");
    testBlobDA = await TestBlobDA.deploy();

    // Deploy Bridge
    const Bridge = await ethers.getContractFactory("ZKRollupBridge");
    bridge = await Bridge.deploy(verifier.target, genesisRoot);

    // Register DA Providers
    await bridge.setDAProvider(CALLDATA_DA_ID, calldataDA.target, true);
    await bridge.setDAProvider(BLOB_DA_ID, blobDA.target, true);
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

    it("Should revert if new sequencer is address 0", async function () {
      await expect(bridge.setSequencer(ethers.ZeroAddress))
        .to.be.revertedWithCustomError(bridge, "InvalidSequencerAddress");
    });
  });

  describe("DA Provider Management", function () {
      it("Should allow owner to set DA provider", async function () {
          await expect(bridge.setDAProvider(2, otherAccount.address, true))
            .to.emit(bridge, "DAProviderSet")
            .withArgs(2, otherAccount.address, true);
          
          expect(await bridge.daProviders(2)).to.equal(otherAccount.address);
          expect(await bridge.daEnabled(2)).to.be.true;
      });

       it("Should not allow non-owner to set DA provider", async function () {
          await expect(
            bridge.connect(otherAccount).setDAProvider(2, otherAccount.address, true)
          ).to.be.revertedWithCustomError(bridge, "OwnableUnauthorizedAccount");
      });
  });

  describe("Commit Batch Calldata", function () {
    const batchData = ethers.toUtf8Bytes("some batch data");
    const daMeta = "0x";
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
        bridge.connect(sequencer).commitBatch(CALLDATA_DA_ID, batchData, daMeta, newRoot, proof)
      )
        .to.emit(bridge, "BatchFinalized")
        .withArgs(1, daCommitment, genesisRoot, newRoot, 0); // 0 is calldata mode

      expect(await bridge.stateRoot()).to.equal(newRoot);
      expect(await bridge.batchCommitment(1)).to.equal(daCommitment);
      expect(await bridge.batchNewRoot(1)).to.equal(newRoot);
      expect(await bridge.nextBatchId()).to.equal(2);
    });

    it("Should commit batch successfully in permissionless mode", async function () {
      // sequencer is address(0) by default
      await expect(
        bridge.connect(otherAccount).commitBatch(CALLDATA_DA_ID, batchData, daMeta, newRoot, proof)
      )
        .to.emit(bridge, "BatchFinalized")
        .withArgs(1, daCommitment, genesisRoot, newRoot, 0);
    });

    it("Should revert if called by non-sequencer when sequencer is set", async function () {
      await bridge.setSequencer(sequencer.address);
      await expect(
        bridge.connect(otherAccount).commitBatch(CALLDATA_DA_ID, batchData, daMeta, newRoot, proof)
      ).to.be.revertedWithCustomError(bridge, "NotSequencer");
    });
    
    it("Should revert if provider is not enabled/found", async function () {
        await expect(
             bridge.commitBatch(99, batchData, daMeta, newRoot, proof)
        ).to.be.revertedWithCustomError(bridge, "DAProviderNotEnabled");
    });
    
    it("Should revert if provider is explicitly disabled", async function () {
        await bridge.setDAProvider(CALLDATA_DA_ID, calldataDA.target, false);
        await expect(
             bridge.commitBatch(CALLDATA_DA_ID, batchData, daMeta, newRoot, proof)
        ).to.be.revertedWithCustomError(bridge, "DAProviderNotEnabled");
    });

    it("Should revert if newRoot is zero", async function () {
      await expect(
        bridge.commitBatch(CALLDATA_DA_ID, batchData, daMeta, ethers.ZeroHash, proof)
      ).to.be.revertedWithCustomError(bridge, "InvalidNewRoot");
    });

    it("Should revert if proof verification fails", async function () {
      await verifier.setShouldVerify(false);
      await expect(
        bridge.commitBatch(CALLDATA_DA_ID, batchData, daMeta, newRoot, proof)
      ).to.be.revertedWithCustomError(bridge, "InvalidProof");
    });
  });

  describe("Commit Batch Blob", function () {
    const expectedVersionedHash = ethers.hexlify(ethers.randomBytes(32));
    const blobIndex = 0;
    // encode daMeta as (bytes32, uint8)
    const daMeta = ethers.AbiCoder.defaultAbiCoder().encode(
        ["bytes32", "uint8"], 
        [expectedVersionedHash, blobIndex]
    );

    const newRoot = ethers.hexlify(ethers.randomBytes(32));
    const proof: any = {
      a: [0, 0],
      b: [[0, 0], [0, 0]],
      c: [0, 0],
    };

    it("Should commit blob batch successfully (mock blobhash)", async function () {
      // We must use TestBlobDA to mock the blobhash
      // Update registry to point to testBlobDA
      await bridge.setDAProvider(BLOB_DA_ID, testBlobDA.target, true);
      
      // Setup mock hash in testBlobDA
      await testBlobDA.setMockBlobHash(blobIndex, expectedVersionedHash);
      
      await expect(
        bridge.commitBatch(BLOB_DA_ID, "0x", daMeta, newRoot, proof)
      )
        .to.emit(bridge, "BatchFinalized")
        .withArgs(1, expectedVersionedHash, genesisRoot, newRoot, 1); // 1 is blob mode
      
       expect(await bridge.stateRoot()).to.equal(newRoot);
       expect(await bridge.batchCommitment(1)).to.equal(expectedVersionedHash);
    });

    it("Should revert if expectedVersionedHash is zero", async function () {
       // Just pass a meta with 0 hash
       const zeroMeta = ethers.AbiCoder.defaultAbiCoder().encode(
            ["bytes32", "uint8"], 
            [ethers.ZeroHash, blobIndex]
        );
      
       // Using regular blobDA (not test) which uses actual blobhash(0) -> 0
       // It will fail, but we want to check where it fails.
       // The BlobDA logic:
       // computeCommitment returns 0.
       // validateDA checks commitment != expected. If we pass 0 expected, commitment is 0.
       // validateDA checks actual blobhash. 
       // If actual blobhash is 0, it reverts NoBlobAttached.
       
       // Wait, does BlobDA check for zero commitment explicitly? No.
       // But Bridge usually validates something? No, Bridge relies on provider.
       
       // Actually, if we use TestBlobDA we can control return.
       await bridge.setDAProvider(BLOB_DA_ID, testBlobDA.target, true);
       await testBlobDA.setMockBlobHash(blobIndex, ethers.ZeroHash); 
       
       // computeCommitment returns 0
       // validateDA: actual is 0. Reverts NoBlobAttached.
       
       await expect(
         bridge.commitBatch(BLOB_DA_ID, "0x", zeroMeta, newRoot, proof)
       ).to.be.revertedWithCustomError(testBlobDA, "NoBlobAttached");
    });

     it("Should revert with NoBlobAttached if blob is missing (real opcode)", async function () {
         // Using real BlobDA (which calls blobhash(index) -> 0 in hardhat without blob)
         await bridge.setDAProvider(BLOB_DA_ID, blobDA.target, true);
         
         await expect(
             bridge.commitBatch(BLOB_DA_ID, "0x", daMeta, newRoot, proof)
         ).to.be.revertedWithCustomError(blobDA, "NoBlobAttached");
     });

    it("Should revert with BlobHashMismatch if blob hash does not match", async function () {
        await bridge.setDAProvider(BLOB_DA_ID, testBlobDA.target, true);
        
        const mockHash = ethers.hexlify(ethers.randomBytes(32));
        await testBlobDA.setMockBlobHash(blobIndex, mockHash);

        // expectedVersionedHash != mockHash
        await expect(
            bridge.commitBatch(BLOB_DA_ID, "0x", daMeta, newRoot, proof)
        ).to.be.revertedWithCustomError(testBlobDA, "BlobHashMismatch");
    });
    
    it("Should revert InvalidCommitment if decoded metadata doesn't match commitment (tampered)", async function () {
         // This is a bit tricky to test via Bridge because Bridge calls computeCommitment then validateDA with same meta.
         // computeCommitment -> extract hash X.
         // validateDA -> extract hash X.
         // they will always match if the code is correct.
         
         // To test this specific revert in BlobDA, we might need to call BlobDA directly or find a way to make them differ?
         // They only differ if `computeCommitment` and `validateDA` decode differently.
         // Since they use same code, it's consistent.
         // But maybe we can pass malformed meta?
         
         // Actually, if we call `validateDA` directly with a mismatched commitment we can trigger it.
         await expect(
             blobDA.validateDA(ethers.ZeroHash, daMeta)
         ).to.be.revertedWithCustomError(blobDA, "InvalidCommitment");
    });
  });
});
