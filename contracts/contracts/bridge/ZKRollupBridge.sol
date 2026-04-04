// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Ownable2Step} from "@openzeppelin/contracts/access/Ownable2Step.sol";
import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {MerkleProof} from "@openzeppelin/contracts/utils/cryptography/MerkleProof.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {IDAProvider} from "../interfaces/IDAProvider.sol";
import {IVerifier} from "../interfaces/IVerifier.sol";

/// @notice Minimal L1 rollup bridge for Gap Analysis fixes.
/// @dev Uses Strategy Pattern for Verifiers (Groth16, Plonky2, Halo2).
contract ZKRollupBridge is Ownable2Step, ReentrancyGuard {
    using SafeERC20 for IERC20;

    // -------------------------
    // Errors
    // -------------------------
    error ZeroDepositAmount();
    error TransferFailed();
    error AlreadyWithdrawn();
    error InvalidMerkleProof();
    error NotSequencer();
    error InvalidNewRoot();
    error DAProviderNotEnabled();
    error DAProviderAlreadySet();
    error BridgeFrozenError();
    error InvalidProof();
    error UnknownVerifier(uint8 verifierId);

    // -------------------------
    // Events
    // -------------------------
    event Deposit(address indexed from, address indexed to, uint256 amount);
    event DepositERC20(address indexed token, address indexed from, address indexed to, uint256 amount);
    event Withdrawal(address indexed to, uint256 amount, bytes32 indexed withdrawalId);

    event SequencerUpdated(address indexed newSequencer);
    event BridgeFrozen(string reason);
    event BridgeUnfrozen();
    event DAProviderSet(uint8 indexed daId, address provider, bool enabled);
    event VerifierSet(uint8 indexed verifierId, address verifierAddress);

    event BatchCommitted(
        uint256 indexed batchId,
        uint8 daId,
        uint8 verifierId,
        bytes32 daCommitment,
        bytes32 oldRoot,
        bytes32 newRoot
    );

    event BatchDataPointer(uint256 indexed batchId, bytes daMeta);

    event ForcedTransactionEnqueued(bytes32 indexed txHash, uint256 deadlineBlock);

    // -------------------------
    // State
    // -------------------------
    bytes32 public latestStateRoot;
    mapping(uint8 => IVerifier) public verifiers;

    /// @notice If sequencer == address(0), anyone can submit (permissionless dev mode).
    address public sequencer;

    uint256 public nextBatchId;

    mapping(uint256 => bytes32) public batchCommitment;
    mapping(uint256 => bytes32) public batchNewRoot;

    mapping(uint8 => address) public daProviders;
    mapping(uint8 => bool) public daEnabled;

    // Double-spend prevention for withdrawals
    mapping(bytes32 => bool) public nullifiers;

    // --- Censorship Resistance Configuration ---
    uint256 public immutable forcedInclusionDelay;
    mapping(bytes32 => uint256) public forcedTxTimestamps;
    bytes32[] public forcedTxQueue;
    uint256 public forcedHead;
    bool public isFrozen;

    // --- Optimistic Fallback State ---
    bool public optimisticMode;
    uint256 public challengePeriod = 7 days;
    bytes32 public pendingStateRoot;
    uint256 public pendingTimestamp;

    // -------------------------
    // Bridging (Deposit & Withdraw)
    // -------------------------

    /// @notice Lock ETH on L1 to mint on L2
    function deposit(address to) external payable {
        if (msg.value == 0) revert ZeroDepositAmount();
        emit Deposit(msg.sender, to, msg.value);
    }

    /// @notice Lock ERC20 on L1 to mint on L2
    function depositERC20(address token, address to, uint256 amount) external {
        if (amount == 0) revert ZeroDepositAmount();
        IERC20(token).safeTransferFrom(msg.sender, address(this), amount);
        emit DepositERC20(token, msg.sender, to, amount);
    }

    /// @notice Withdraw ETH using a Merkle proof of inclusion in the L2 state
    function withdraw(
        bytes32[] calldata merkleProof,
        address to,
        uint256 amount,
        bytes32 withdrawalId
    ) external nonReentrant {
        if (nullifiers[withdrawalId]) revert AlreadyWithdrawn();

        // 1. Recreate the leaf node (e.g. hash of withdrawal params)
        bytes32 leaf = keccak256(abi.encodePacked(to, amount, withdrawalId));

        // 2. Verify proof against latest L1 state root
        if (!MerkleProof.verify(merkleProof, latestStateRoot, leaf)) {
            revert InvalidMerkleProof();
        }

        // 3. Mark as withdrawn BEFORE transfer (CEI pattern)
        nullifiers[withdrawalId] = true;

        // 4. Transfer ETH
        (bool success, ) = to.call{value: amount}("");
        if (!success) revert TransferFailed();

        emit Withdrawal(to, amount, withdrawalId);
    }

    // -------------------------
    // Constructor
    // -------------------------
    constructor(
        address _verifier,
        bytes32 _genesisRoot,
        uint256 _forcedInclusionDelay
    ) Ownable(msg.sender) {
        // Default Groth16 verifier at index 0
        verifiers[0] = IVerifier(_verifier);
        latestStateRoot = _genesisRoot;
        nextBatchId = 1;
        forcedInclusionDelay = _forcedInclusionDelay;
    }

    // -------------------------
    // Admin
    // -------------------------

    function setOptimisticMode(bool _mode) external onlyOwner {
        optimisticMode = _mode;
    }

    function setChallengePeriod(uint256 _period) external onlyOwner {
        challengePeriod = _period;
    }

    function setSequencer(address newSequencer) external onlyOwner {
        sequencer = newSequencer;
        emit SequencerUpdated(newSequencer);
    }

    function setDAProvider(uint8 daId, address provider, bool enabled) external onlyOwner {
        if (daProviders[daId] != address(0) && daEnabled[daId] && daProviders[daId] != provider) {
            revert DAProviderAlreadySet();
        }
        daProviders[daId] = provider;
        daEnabled[daId] = enabled;
        emit DAProviderSet(daId, provider, enabled);
    }

    function setVerifier(uint8 verifierId, address verifierAddress) external onlyOwner {
        verifiers[verifierId] = IVerifier(verifierAddress);
        emit VerifierSet(verifierId, verifierAddress);
    }

    // -------------------------
    // Governance
    // -------------------------
    function unfreeze() external onlyOwner {
        if (!isFrozen) revert();
        isFrozen = false;
        emit BridgeUnfrozen();
    }

    function freeze() external {
        if (forcedTxQueue.length > forcedHead) {
            bytes32 oldestTxHash = forcedTxQueue[forcedHead];
            uint256 deadline = forcedTxTimestamps[oldestTxHash];
            if (block.number > deadline) {
                isFrozen = true;
                emit BridgeFrozen("Censorship proven via freeze()");
                return;
            }
        }
        revert("No censorship detected");
    }

    // -------------------------
    // Forced Inclusion
    // -------------------------
    function forceTransaction(bytes32 _txHash) external {
        if (isFrozen) revert BridgeFrozenError();
        uint256 deadline = block.number + forcedInclusionDelay;
        forcedTxTimestamps[_txHash] = deadline;
        forcedTxQueue.push(_txHash);
        emit ForcedTransactionEnqueued(_txHash, deadline);
    }

    function _requireSequencer() internal view {
        if (isFrozen) revert BridgeFrozenError();
        if (sequencer != address(0) && msg.sender != sequencer) revert NotSequencer();
    }

    // -------------------------
    // Commit Batch
    // -------------------------
    /// @notice Commits a new batch.
    function commitBatch(
        uint8 daId,
        uint8 verifierId,
        bytes calldata batchData,
        bytes calldata daMeta,
        bytes32 newRoot,
        bytes calldata proof
    ) external {
        _requireSequencer();

        // Censorship Check
        if (forcedTxQueue.length > forcedHead) {
            bytes32 oldestTxHash = forcedTxQueue[forcedHead];
            uint256 deadline = forcedTxTimestamps[oldestTxHash];
            if (block.number > deadline) {
                isFrozen = true;
                emit BridgeFrozen("Forced transaction deadline missed");
                revert BridgeFrozenError();
            }
        }
        
        address providerAddr = daProviders[daId];
        if (providerAddr == address(0) || !daEnabled[daId]) revert DAProviderNotEnabled();

        IDAProvider provider = IDAProvider(providerAddr);

        // 1. Compute Commitment
        bytes32 daCommitment = provider.computeCommitment(batchData, daMeta);

        // 2. Validate DA
        provider.validateDA(daCommitment, daMeta);

        // 3. Verify Proof
        bytes32 oldRoot = latestStateRoot;
        
        // Construct Inputs
        uint256[4] memory inputs;
        // Roots fit in scalar field (Poseidon)
        inputs[0] = uint256(oldRoot); 
        inputs[1] = uint256(newRoot);
        // DA Commitment is Keccak (256 bits), needs split
        inputs[2] = uint256(daCommitment) & type(uint128).max;
        inputs[3] = uint256(daCommitment) >> 128;

        IVerifier selectedVerifier = verifiers[verifierId];
        if (address(selectedVerifier) == address(0)) revert UnknownVerifier(verifierId);

        // Decode Proof (A, B, C)
        uint256[2] memory a;
        uint256[2][2] memory b;
        uint256[2] memory c;

        if (verifierId == 0) {
            // Groth16 Logic (BN254)
            require(proof.length == 256, "Invalid proof length");
            
            // Manually extract 32-byte chunks from bytes calldata
            a[0] = uint256(bytes32(proof[0:32]));
            a[1] = uint256(bytes32(proof[32:64]));
            
            // B: X0, X1, Y0, Y1 (flat from Rust)
            // Rust Arkworks serializes Fp2 as (c0, c1) -> (real, imaginary).
            // Ethereum precompile expects (c1, c0) -> (imaginary, real).
            // So we must swap the chunks for G2 points.
            
            // X (c0, c1) -> Want (c1, c0)
            b[0][1] = uint256(bytes32(proof[64:96]));  // c0 -> X[1]
            b[0][0] = uint256(bytes32(proof[96:128])); // c1 -> X[0]
            
            // Y (c0, c1) -> Want (c1, c0)
            b[1][1] = uint256(bytes32(proof[128:160])); // c0 -> Y[1]
            b[1][0] = uint256(bytes32(proof[160:192])); // c1 -> Y[0]

            c[0] = uint256(bytes32(proof[192:224]));
            c[1] = uint256(bytes32(proof[224:256]));
        } else {
            // Plonky2 / Halo2 / Mock
            // Pass zeroed A/B/C or handle decoding differently if needed.
            // The stubs ignore inputs anyway.
        }

        if (!optimisticMode) {
            if (!selectedVerifier.verifyProof(a, b, c, inputs)) {
                revert InvalidProof();
            }
            // 4. Finalize directly
            _finalizeBatch(oldRoot, newRoot, daCommitment, provider.mode(), verifierId, daMeta);
        } else {
            // Optimistic Mode
            if (newRoot == bytes32(0)) revert InvalidNewRoot();
            
            pendingStateRoot = newRoot;
            pendingTimestamp = block.timestamp;
            
            uint256 batchId = nextBatchId++;
            batchCommitment[batchId] = daCommitment;
            batchNewRoot[batchId] = newRoot;

            emit BatchDataPointer(batchId, daMeta);
            // We still emit BatchCommitted for DA tracking
            emit BatchCommitted(batchId, provider.mode(), verifierId, daCommitment, oldRoot, newRoot);
        }
    }

    // -------------------------
    // Optimistic Fallback (Fraud Proof Stub)
    // -------------------------

    function verifyFraudProof(bytes calldata /*proof*/) external pure {
        // TODO: integrate ZK fault proof or interactive bisection protocol
        revert("FraudProofNotImplemented");
    }

    event OptimisticRootFinalized(bytes32 root);

    function finalizeOptimisticRoot() external {
        require(optimisticMode, "Not in optimistic mode");
        require(pendingStateRoot != bytes32(0), "No pending root");
        require(block.timestamp >= pendingTimestamp + challengePeriod, "Challenge period not elapsed");

        latestStateRoot = pendingStateRoot;
        pendingStateRoot = bytes32(0); // clear it
        
        emit OptimisticRootFinalized(latestStateRoot);
    }

    function _finalizeBatch(
        bytes32 oldRoot,
        bytes32 newRoot,
        bytes32 daCommitment,
        uint8 daMode,
        uint8 verifierId,
        bytes calldata daMeta
    ) internal {
        if (newRoot == bytes32(0)) revert InvalidNewRoot();
        
        latestStateRoot = newRoot;

        uint256 batchId = nextBatchId++;
        batchCommitment[batchId] = daCommitment;
        batchNewRoot[batchId] = newRoot;

        emit BatchDataPointer(batchId, daMeta);
        emit BatchCommitted(batchId, daMode, verifierId, daCommitment, oldRoot, newRoot);
    }
}
