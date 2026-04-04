// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../contracts/bridge/ZKRollupBridge.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

// Dummy ERC20 for testing
contract MockERC20 is ERC20 {
    constructor() ERC20("Mock", "MCK") {
        _mint(msg.sender, 1000000 * 10**18);
    }
    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

contract ZKRollupBridgeTest is Test {
    ZKRollupBridge public bridge;
    MockERC20 public token;

    address public owner = address(this);
    address public user = address(0x123);
    address public sequencer = address(0x456);

    // Provide a dummy verifier address
    address public mockVerifier = address(0x789);

    function setUp() public {
        // Deploy the bridge with some initial parameters
        // constructor(address _verifier, bytes32 _genesisRoot, uint256 _forcedInclusionDelay)
        bridge = new ZKRollupBridge(mockVerifier, bytes32(0), 100);
        
        // Deploy mock token
        token = new MockERC20();

        // Give the user some ETH and tokens
        vm.deal(user, 100 ether);
        token.mint(user, 1000 ether);
    }

    // --- Deposit Tests ---

    function test_DepositETH_Success() public {
        vm.startPrank(user);
        
        // Expect the Deposit event
        vm.expectEmit(true, true, false, true);
        emit ZKRollupBridge.Deposit(user, user, 1 ether);
        
        bridge.deposit{value: 1 ether}(user);
        
        assertEq(address(bridge).balance, 1 ether);
        vm.stopPrank();
    }

    function test_DepositETH_RevertZeroAmount() public {
        vm.startPrank(user);
        vm.expectRevert(ZKRollupBridge.ZeroDepositAmount.selector);
        bridge.deposit{value: 0}(user);
        vm.stopPrank();
    }

    function test_DepositERC20_Success() public {
        vm.startPrank(user);
        
        // Approve bridge to spend tokens
        token.approve(address(bridge), 10 ether);

        // Expect the DepositERC20 event
        vm.expectEmit(true, true, true, true);
        emit ZKRollupBridge.DepositERC20(address(token), user, user, 10 ether);
        
        bridge.depositERC20(address(token), user, 10 ether);
        
        assertEq(token.balanceOf(address(bridge)), 10 ether);
        vm.stopPrank();
    }

    function test_DepositERC20_RevertZeroAmount() public {
        vm.startPrank(user);
        vm.expectRevert(ZKRollupBridge.ZeroDepositAmount.selector);
        bridge.depositERC20(address(token), user, 0);
        vm.stopPrank();
    }

    // --- Withdrawal Tests ---

    function test_Withdraw_Success() public {
        // 1. Fund the bridge with some ETH
        vm.deal(address(bridge), 10 ether);

        // 2. We need to construct a valid Merkle tree manually to pass the verify check.
        // Let's say we have 2 leaves:
        // Leaf 1: our valid withdrawal
        uint256 amount = 1 ether;
        bytes32 withdrawalId = keccak256("withdrawal-1");
        bytes32 leaf1 = keccak256(abi.encodePacked(user, amount, withdrawalId));

        // Leaf 2: some random other withdrawal
        bytes32 leaf2 = keccak256(abi.encodePacked(address(0x999), uint256(2 ether), keccak256("withdrawal-2")));

        // Construct root. Sort leaves as OpenZeppelin MerkleProof requires for pairs
        bytes32 left = leaf1 < leaf2 ? leaf1 : leaf2;
        bytes32 right = leaf1 < leaf2 ? leaf2 : leaf1;
        bytes32 root = keccak256(abi.encodePacked(left, right));

        // The proof for leaf1 is just leaf2 (since it's a tree of height 1)
        bytes32[] memory proof = new bytes32[](1);
        proof[0] = leaf2;

        // Force the contract's state root to match our constructed root
        // Using Forge's store cheatcode
        // latestStateRoot is slot 3 (Ownable has 1 slot, Ownable2Step has 1 slot, ReentrancyGuard has 1 slot)
        vm.store(address(bridge), bytes32(uint256(3)), root);

        // Record balance before
        uint256 balanceBefore = user.balance;

        // 3. Execute the withdrawal
        vm.startPrank(user);
        
        vm.expectEmit(true, false, false, true);
        emit ZKRollupBridge.Withdrawal(user, amount, withdrawalId);
        
        bridge.withdraw(proof, user, amount, withdrawalId);
        
        vm.stopPrank();

        // 4. Assertions
        assertEq(user.balance, balanceBefore + amount);
        assertTrue(bridge.nullifiers(withdrawalId));
    }

    function test_Withdraw_RevertDoubleSpend() public {
        // Setup same as above
        vm.deal(address(bridge), 10 ether);

        uint256 amount = 1 ether;
        bytes32 withdrawalId = keccak256("withdrawal-1");
        bytes32 leaf1 = keccak256(abi.encodePacked(user, amount, withdrawalId));
        bytes32 leaf2 = keccak256(abi.encodePacked(address(0x999), uint256(2 ether), keccak256("withdrawal-2")));

        bytes32 left = leaf1 < leaf2 ? leaf1 : leaf2;
        bytes32 right = leaf1 < leaf2 ? leaf2 : leaf1;
        bytes32 root = keccak256(abi.encodePacked(left, right));

        bytes32[] memory proof = new bytes32[](1);
        proof[0] = leaf2;

        vm.store(address(bridge), bytes32(uint256(3)), root);

        vm.startPrank(user);
        
        // First withdrawal succeeds
        bridge.withdraw(proof, user, amount, withdrawalId);
        
        // Second withdrawal with same ID fails
        vm.expectRevert(ZKRollupBridge.AlreadyWithdrawn.selector);
        bridge.withdraw(proof, user, amount, withdrawalId);
        
        vm.stopPrank();
    }

    function test_Withdraw_RevertInvalidProof() public {
        vm.deal(address(bridge), 10 ether);

        uint256 amount = 1 ether;
        bytes32 withdrawalId = keccak256("withdrawal-1");
        
        // We supply an invalid proof (empty proof) against a zero root
        bytes32[] memory proof = new bytes32[](0);

        vm.startPrank(user);
        vm.expectRevert(ZKRollupBridge.InvalidMerkleProof.selector);
        bridge.withdraw(proof, user, amount, withdrawalId);
        vm.stopPrank();
    }
}
