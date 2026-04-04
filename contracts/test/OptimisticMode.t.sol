// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../contracts/bridge/ZKRollupBridge.sol";
import "../contracts/interfaces/IDAProvider.sol";
import "../contracts/interfaces/IVerifier.sol";

contract MockDAProvider is IDAProvider {
    function mode() external pure override returns (uint8) {
        return 0;
    }
    
    function computeCommitment(bytes calldata batchData, bytes calldata daMeta) external pure override returns (bytes32) {
        return keccak256(abi.encode(batchData, daMeta));
    }
    
    function validateDA(bytes32 commitment, bytes calldata daMeta) external pure override {
        // dummy
    }
}

contract MockVerifier is IVerifier {
    function verifyProof(
        uint256[2] calldata a,
        uint256[2][2] calldata b,
        uint256[2] calldata c,
        uint256[4] calldata input
    ) external pure override returns (bool) {
        return true;
    }
}

contract OptimisticModeTest is Test {
    ZKRollupBridge public bridge;
    MockDAProvider public da;
    MockVerifier public verifier;

    address public owner = address(this);
    address public sequencer = address(0x456);

    function setUp() public {
        verifier = new MockVerifier();
        bridge = new ZKRollupBridge(address(verifier), bytes32(uint256(1)), 100);
        
        da = new MockDAProvider();
        bridge.setDAProvider(0, address(da), true);
        bridge.setSequencer(sequencer);
    }

    function test_ZKMode_DirectFinalization() public {
        bytes32 newRoot = bytes32(uint256(2));
        
        vm.prank(sequencer);
        bridge.commitBatch(0, 0, "0x", "0x", newRoot, new bytes(256));
        
        assertEq(bridge.latestStateRoot(), newRoot);
        assertEq(bridge.pendingStateRoot(), bytes32(0));
    }

    function test_OptimisticMode_Flow() public {
        bridge.setOptimisticMode(true);

        bytes32 newRoot = bytes32(uint256(2));
        
        vm.prank(sequencer);
        bridge.commitBatch(0, 0, "0x", "0x", newRoot, new bytes(256));
        
        // Root is pending, not finalized
        assertEq(bridge.latestStateRoot(), bytes32(uint256(1)));
        assertEq(bridge.pendingStateRoot(), newRoot);
        
        // Too early to finalize
        vm.expectRevert("Challenge period not elapsed");
        bridge.finalizeOptimisticRoot();

        // Warp to exactly the end of the challenge period
        vm.warp(block.timestamp + bridge.challengePeriod());

        // Finalize successfully
        bridge.finalizeOptimisticRoot();
        
        assertEq(bridge.latestStateRoot(), newRoot);
        assertEq(bridge.pendingStateRoot(), bytes32(0));
    }

    function test_VerifyFraudProof_Reverts() public {
        vm.expectRevert("FraudProofNotImplemented");
        bridge.verifyFraudProof("0xbad");
    }
}
