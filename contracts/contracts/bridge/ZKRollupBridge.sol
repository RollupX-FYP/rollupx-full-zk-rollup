// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Ownable2Step} from "@openzeppelin/contracts/access/Ownable2Step.sol";
import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";
import {IVerifier} from "../interfaces/IVerifier.sol";
import {IDAProvider} from "../interfaces/IDAProvider.sol";

/// @notice L1 rollup bridge core that supports modular DA providers via strategy pattern.
contract ZKRollupBridge is Ownable2Step {
    // -------------------------
    // Errors
    // -------------------------
    error NotSequencer();
    error InvalidNewRoot();
    error DAProviderNotEnabled();
    error InvalidProof();
    error DAProviderAlreadySet(); // Optional if we want to prevent overwrites, but setDAProvider allows updates usually.

    // -------------------------
    // Events
    // -------------------------
    event SequencerUpdated(address indexed newSequencer);
    event DAProviderSet(uint8 indexed daId, address provider, bool enabled);
    event BatchFinalized(
        uint256 indexed batchId,
        bytes32 indexed daCommitment,
        bytes32 oldRoot,
        bytes32 newRoot,
        uint8 daMode
    );

    // -------------------------
    // State
    // -------------------------
    IVerifier public immutable verifier;
    bytes32 public stateRoot;

    /// @notice If sequencer == address(0), anyone can submit (permissionless dev mode).
    address public sequencer;

    uint256 public nextBatchId;

    mapping(uint256 => bytes32) public batchCommitment;
    mapping(uint256 => bytes32) public batchNewRoot;

    mapping(uint8 => address) public daProviders;
    mapping(uint8 => bool) public daEnabled;

    // -------------------------
    // Types
    // -------------------------
    struct Groth16Proof {
        uint256[2] a;
        uint256[2][2] b;
        uint256[2] c;
    }

    // -------------------------
    // Constructor
    // -------------------------
    constructor(address _verifier, bytes32 _genesisRoot) Ownable(msg.sender) {
        verifier = IVerifier(_verifier);
        stateRoot = _genesisRoot;
        nextBatchId = 1;
    }

    // -------------------------
    // Admin
    // -------------------------
    function setSequencer(address _sequencer) external onlyOwner {
        sequencer = _sequencer;
        emit SequencerUpdated(_sequencer);
    }

    function setDAProvider(uint8 daId, address provider, bool enabled) external onlyOwner {
        daProviders[daId] = provider;
        daEnabled[daId] = enabled;
        emit DAProviderSet(daId, provider, enabled);
    }

    // -------------------------
    // Internal auth helper
    // -------------------------
    function _requireSequencer() internal view {
        if (sequencer != address(0) && msg.sender != sequencer) revert NotSequencer();
    }

    // -------------------------
    // Commit Batch (Strategy Pattern)
    // -------------------------
    function commitBatch(
        uint8 daId,
        bytes calldata batchData,
        bytes calldata daMeta,
        bytes32 newRoot,
        Groth16Proof calldata proof
    ) external {
        _requireSequencer();
        
        address providerAddr = daProviders[daId];
        if (providerAddr == address(0) || !daEnabled[daId]) revert DAProviderNotEnabled();

        IDAProvider provider = IDAProvider(providerAddr);

        // 1. Compute Commitment (Strategy)
        bytes32 daCommitment = provider.computeCommitment(batchData, daMeta);

        // 2. Validate DA (Strategy)
        provider.validateDA(daCommitment, daMeta);

        // 3. Verify Proof
        bytes32 oldRoot = stateRoot;
        
        // Public inputs: [DA_commitment, oldRoot, newRoot]
        uint256[3] memory input = [uint256(daCommitment), uint256(oldRoot), uint256(newRoot)];

        bool ok = verifier.verifyProof(proof.a, proof.b, proof.c, input);
        if (!ok) revert InvalidProof();

        // 4. Finalize
        _finalizeBatch(oldRoot, newRoot, daCommitment, provider.mode());
    }

    // -------------------------
    // Internal State Transition
    // -------------------------
    function _finalizeBatch(
        bytes32 oldRoot,
        bytes32 newRoot,
        bytes32 daCommitment,
        uint8 daMode
    ) internal {
        if (newRoot == bytes32(0)) revert InvalidNewRoot();
        
        stateRoot = newRoot;

        uint256 batchId = nextBatchId++;
        batchCommitment[batchId] = daCommitment;
        batchNewRoot[batchId] = newRoot;

        emit BatchFinalized(batchId, daCommitment, oldRoot, newRoot, daMode);
    }
}
