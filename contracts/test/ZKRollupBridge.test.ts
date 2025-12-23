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
    
    it("Should revert if verifier is zero address", async function () {
        const Bridge = await ethers.getContractFactory("ZKRollupBridge");
        await expect(Bridge.deploy(ethers.ZeroAddress, genesisRoot))
            .to.be.revertedWithCustomError(Bridge, "InvalidVerifier");
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

    it("Should allow setting sequencer to address 0 (permissionless mode)", async function () {
      await expect(bridge.setSequencer(ethers.ZeroAddress))
        .to.emit(bridge, "SequencerUpdated")
        .withArgs(ethers.ZeroAddress);
      expect(await bridge.sequencer()).to.equal(ethers.ZeroAddress);
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

      it("Should prevent overwriting an enabled provider with different address", async function () {
        await bridge.setDAProvider(2, otherAccount.address, true);
        await expect(
            bridge.setDAProvider(2, sequencer.address, true)
        ).to.be.revertedWithCustomError(bridge, "DAProviderAlreadySet");
      });
    
      it("Should allow updating provider if disabled first (2-step)", async function () {
        await bridge.setDAProvider(2, otherAccount.address, true);
        // Disable (same address)
        await bridge.setDAProvider(2, otherAccount.address, false);
        // Update (new address)
        await bridge.setDAProvider(2, sequencer.address, true);
        
        expect(await bridge.daProviders(2)).to.equal(sequencer.address);
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
      // 2-step update: disable old, set new
      await bridge.setDAProvider(BLOB_DA_ID, blobDA.target, false);
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
       const zeroMeta = ethers.AbiCoder.defaultAbiCoder().encode(
            ["bytes32", "uint8"], 
            [ethers.ZeroHash, blobIndex]
        );
      
       await bridge.setDAProvider(BLOB_DA_ID, blobDA.target, false);
       await bridge.setDAProvider(BLOB_DA_ID, testBlobDA.target, true);
       await testBlobDA.setMockBlobHash(blobIndex, ethers.ZeroHash); 
       
       await expect(
         bridge.commitBatch(BLOB_DA_ID, "0x", zeroMeta, newRoot, proof)
       ).to.be.revertedWithCustomError(testBlobDA, "InvalidCommitment");
    });

     it("Should revert with NoBlobAttached if blob is missing (real opcode)", async function () {
         // Using real BlobDA (which calls blobhash(index) -> 0 in hardhat without blob)
         await bridge.setDAProvider(BLOB_DA_ID, blobDA.target, true);
         
         await expect(
             bridge.commitBatch(BLOB_DA_ID, "0x", daMeta, newRoot, proof)
         ).to.be.revertedWithCustomError(blobDA, "NoBlobAttached");
     });

    it("Should revert with BlobHashMismatch if blob hash does not match", async function () {
        await bridge.setDAProvider(BLOB_DA_ID, blobDA.target, false);
        await bridge.setDAProvider(BLOB_DA_ID, testBlobDA.target, true);
        
        const mockHash = ethers.hexlify(ethers.randomBytes(32));
        await testBlobDA.setMockBlobHash(blobIndex, mockHash);

        await expect(
            bridge.commitBatch(BLOB_DA_ID, "0x", daMeta, newRoot, proof)
        ).to.be.revertedWithCustomError(testBlobDA, "BlobHashMismatch");
    });
    
    it("Should revert InvalidCommitment if decoded metadata doesn't match commitment (tampered)", async function () {
         await expect(
             blobDA.validateDA(ethers.ZeroHash, daMeta)
         ).to.be.revertedWithCustomError(blobDA, "InvalidCommitment");
    });

    it("Should reduce inputs to field elements before verification", async function () {
        const bigVal = ethers.MaxUint256;
        const scalarField = 21888242871839275222246405745257275088548364400416034343698204186575808495617n;
        
        await bridge.setDAProvider(BLOB_DA_ID, blobDA.target, false);
        await bridge.setDAProvider(BLOB_DA_ID, testBlobDA.target, true);
        const bigHash = ethers.zeroPadValue("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", 32);
        
        await testBlobDA.setMockBlobHash(blobIndex, bigHash);
        
        const daMeta = ethers.AbiCoder.defaultAbiCoder().encode(
            ["bytes32", "uint8"], 
            [bigHash, blobIndex]
        );
        
        const reducedBig = bigVal % scalarField;
        const reducedZero = 0n;
        
        await verifier.setExpectedInput([reducedBig, reducedZero, reducedBig]);
        
        await bridge.commitBatch(BLOB_DA_ID, "0x", daMeta, bigHash, proof);
    });

    it("Should revert if reduced inputs do not match expected (MockVerifier check)", async function () {
        await bridge.setDAProvider(BLOB_DA_ID, blobDA.target, false);
        await bridge.setDAProvider(BLOB_DA_ID, testBlobDA.target, true);
        const bigHash = ethers.zeroPadValue("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", 32);
        
        await testBlobDA.setMockBlobHash(blobIndex, bigHash);
        
        const daMeta = ethers.AbiCoder.defaultAbiCoder().encode(
            ["bytes32", "uint8"], 
            [bigHash, blobIndex]
        );
        
        // Set WRONG expected input
        await verifier.setExpectedInput([0, 0, 0]);
        
        await expect(
            bridge.commitBatch(BLOB_DA_ID, "0x", daMeta, bigHash, proof)
        ).to.be.revertedWithCustomError(bridge, "InvalidProof");
    });

    it("Should revert if input[1] mismatch (MockVerifier branch check)", async function () {
        await bridge.setDAProvider(BLOB_DA_ID, blobDA.target, false);
        await bridge.setDAProvider(BLOB_DA_ID, testBlobDA.target, true);
        const bigHash = ethers.zeroPadValue("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", 32);
        const blobIndex = 0;
        
        await testBlobDA.setMockBlobHash(blobIndex, bigHash);
        
        const daMeta = ethers.AbiCoder.defaultAbiCoder().encode(
            ["bytes32", "uint8"], 
            [bigHash, blobIndex]
        );
        
        const scalarField = 21888242871839275222246405745257275088548364400416034343698204186575808495617n;
        const bigVal = ethers.MaxUint256;
        const reducedBig = bigVal % scalarField;
        
        // Expected actual: [reducedBig, 0, reducedBig]
        // Set mismatch at index 1
        await verifier.setExpectedInput([reducedBig, 1n, reducedBig]);
        
        await expect(
            bridge.commitBatch(BLOB_DA_ID, "0x", daMeta, bigHash, proof)
        ).to.be.revertedWithCustomError(bridge, "InvalidProof");
    });

    it("Should revert if input[2] mismatch (MockVerifier branch check)", async function () {
        await bridge.setDAProvider(BLOB_DA_ID, blobDA.target, false);
        await bridge.setDAProvider(BLOB_DA_ID, testBlobDA.target, true);
        const bigHash = ethers.zeroPadValue("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", 32);
        const blobIndex = 0;
        
        await testBlobDA.setMockBlobHash(blobIndex, bigHash);
        
        const daMeta = ethers.AbiCoder.defaultAbiCoder().encode(
            ["bytes32", "uint8"], 
            [bigHash, blobIndex]
        );
        
        const scalarField = 21888242871839275222246405745257275088548364400416034343698204186575808495617n;
        const bigVal = ethers.MaxUint256;
        const reducedBig = bigVal % scalarField;
        
        // Expected actual: [reducedBig, 0, reducedBig]
        // Set mismatch at index 2
        await verifier.setExpectedInput([reducedBig, 0n, 999n]);
        
        await expect(
            bridge.commitBatch(BLOB_DA_ID, "0x", daMeta, bigHash, proof)
        ).to.be.revertedWithCustomError(bridge, "InvalidProof");
    });

    it("Should use real blobhash if mock is not set in TestBlobDA", async function () {
        await bridge.setDAProvider(BLOB_DA_ID, blobDA.target, false);
        await bridge.setDAProvider(BLOB_DA_ID, testBlobDA.target, true);
        
        // Do NOT set mock hash.
        // It falls back to blobhash(0) -> 0.
        // Reverts NoBlobAttached.
        
        await expect(
            bridge.commitBatch(BLOB_DA_ID, "0x", daMeta, newRoot, proof)
        ).to.be.revertedWithCustomError(testBlobDA, "NoBlobAttached");
    });
  });
});
