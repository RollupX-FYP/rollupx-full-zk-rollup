import { expect } from "chai";
import { ethers } from "hardhat";

describe("ZKRollupBridge - Deposit & Withdrawal", function () {
  let bridge: any;
  let owner: any, user: any;

  beforeEach(async function () {
    [owner, user] = await ethers.getSigners();
    const Bridge = await ethers.getContractFactory("ZKRollupBridge");
    const mockVerifierAddress = user.address; // Doesn't matter for this test
    const genesisRoot = ethers.ZeroHash;
    const forcedDelay = 100;
    bridge = await Bridge.deploy(mockVerifierAddress, genesisRoot, forcedDelay);
  });

  it("should successfully deposit ETH", async function () {
    const amount = ethers.parseEther("1.0");
    await expect(bridge.connect(user).deposit(user.address, { value: amount }))
      .to.emit(bridge, "Deposit")
      .withArgs(user.address, user.address, amount);

    const bridgeBalance = await ethers.provider.getBalance(await bridge.getAddress());
    expect(bridgeBalance).to.equal(amount);
  });

  it("should revert deposit with 0 value", async function () {
    await expect(
      bridge.connect(user).deposit(user.address, { value: 0 })
    ).to.be.revertedWithCustomError(bridge, "ZeroDepositAmount");
  });

  describe("Withdrawal", function () {
    let root: string;
    let amount: bigint;
    let withdrawalId: string;
    let leafHash: string;
    let proof: string[];

    beforeEach(async function () {
      amount = ethers.parseEther("1.0");
      withdrawalId = ethers.id("test-withdrawal");
      
      // Fund bridge
      await bridge.connect(owner).deposit(owner.address, { value: ethers.parseEther("10.0") });

      // Generate leaf hash using encodePacked
      const types = ["address", "uint256", "bytes32"];
      const values = [user.address, amount, withdrawalId];
      leafHash = ethers.solidityPackedKeccak256(types, values);

      // Create a simple Merkle Tree with 2 leaves
      const otherLeaf = ethers.id("some-other-leaf");
      
      // Sort leaves for OpenZeppelin MerkleProof standard (optional, but good practice to know ordering)
      const leaves = [leafHash, otherLeaf].sort();
      
      // Combine nodes
      root = ethers.keccak256(
          ethers.solidityPacked(
              ["bytes32", "bytes32"],
              leaves[0] < leaves[1] ? [leaves[0], leaves[1]] : [leaves[1], leaves[0]]
          )
      );

      // The proof for our leaf is just the other leaf
      proof = [otherLeaf];

      // We need to set the state root on the bridge contract
      // The only way to set latestStateRoot currently is via commitBatch
      // But for testing purposes, we can write a mock or exploit the fact that we can set it.
      // Wait, let's create a TestBridge that exposes state root setter for this test, or use hardhat's setStorageAt.
    });

    it("should process withdrawal and prevent double spend", async function () {
      // 1. Force set the state root via setStorageAt
      // Find slot for latestStateRoot
      // Looking at the contract layout: latestStateRoot is slot 0 (after Ownable2Step)
      // Actually Ownable has _owner (slot 0). Ownable2Step has _pendingOwner (slot 1).
      // ReentrancyGuard has _status (slot 2).
      // latestStateRoot would be slot 3.
      const bridgeAddress = await bridge.getAddress();
      await ethers.provider.send("hardhat_setStorageAt", [
        bridgeAddress,
        "0x3", 
        root
      ]);

      const initialUserBalance = await ethers.provider.getBalance(user.address);

      // 2. Successful withdrawal
      const tx = await bridge.connect(user).withdraw(proof, user.address, amount, withdrawalId);
      const receipt = await tx.wait();
      
      // Calculate gas costs
      const gasUsed = receipt.gasUsed * receipt.gasPrice;

      // Assert balance change
      const finalUserBalance = await ethers.provider.getBalance(user.address);
      expect(finalUserBalance).to.equal(initialUserBalance + amount - gasUsed);

      // Assert event emitted
      await expect(tx).to.emit(bridge, "Withdrawal").withArgs(user.address, amount, withdrawalId);

      // 3. Prevent Double Spend
      await expect(
        bridge.connect(user).withdraw(proof, user.address, amount, withdrawalId)
      ).to.be.revertedWithCustomError(bridge, "AlreadyWithdrawn");
    });

    it("should revert with invalid merkle proof", async function () {
      const bridgeAddress = await bridge.getAddress();
      await ethers.provider.send("hardhat_setStorageAt", [
        bridgeAddress,
        "0x3", 
        ethers.ZeroHash // wrong root
      ]);

      await expect(
        bridge.connect(user).withdraw(proof, user.address, amount, withdrawalId)
      ).to.be.revertedWithCustomError(bridge, "InvalidMerkleProof");
    });
  });
});
