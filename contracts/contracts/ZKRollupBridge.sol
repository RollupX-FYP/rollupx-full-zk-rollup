// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Ownable2Step} from "@openzeppelin/contracts/access/Ownable2Step.sol";
import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";

/// -----------------------------------------------------------------------
/// REAL VERIFIER (COMMENTED FOR NOW)
/// Replace MockVerifier with your snarkjs-generated verifier and use this.
/// Typical snarkjs Groth16 verifier signature is:
///
/// interface IVerifier {
///   function verifyProof(
///     uint256[2] calldata a,
///     uint256[2][2] calldata b,
///     uint256[2] calldata c,
///     uint256[3] calldata input
///   ) external view returns (bool);
/// }
/// -----------------------------------------------------------------------

interface IVerifierLike {
    function verifyProof(
        uint256[2] calldata a,
        uint256[2][2] calldata b,
        uint256[2] calldata c,
        uint256[3] calldata input
    ) external view returns (bool);
}

/// @notice L1 rollup bridge core that supports:
/// - Calldata DA: commitment = keccak256(batchData)
/// - Blob DA: commitment = EIP-4844 versioned hash (optionally checked via blobhash opcode)
///
/// NOTE: Blobs are not directly readable by the EVM; only commitments are bound on-chain:contentReference[oaicite:3]{index=3}.
contract ZKRollupBridge is Ownable2Step {
    // -------------------------
    // Errors (cheaper than revert strings)
    // -------------------------
    error NotSequencer();
    error InvalidNewRoot();
    error InvalidDACommitment();
    error NoBlobAttached();
    error BlobHashMismatch();
    error InvalidProof();

    // -------------------------
    // Events
    // -------------------------
    event SequencerUpdated(address indexed newSequencer);
    event BatchFinalized(
        uint256 indexed batchId,
        bytes32 indexed daCommitment,
        bytes32 oldRoot,
        bytes32 newRoot,
        uint8 daMode // 0=calldata, 1=blob
    );

    // -------------------------
    // State
    // -------------------------
    IVerifierLike public immutable verifier;
    bytes32 public stateRoot;

    /// @notice If sequencer == address(0), anyone can submit (permissionless dev mode).
    address public sequencer;

    uint256 public nextBatchId;

    mapping(uint256 => bytes32) public batchCommitment;
    mapping(uint256 => bytes32) public batchNewRoot;

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
        verifier = IVerifierLike(_verifier);
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

    // -------------------------
    // Internal auth helper
    // -------------------------
    function _requireSequencer() internal view {
        if (sequencer != address(0) && msg.sender != sequencer) revert NotSequencer();
    }

    // -------------------------
    // Calldata DA path
    // -------------------------
    /// @notice Submit a batch where DA lives in calldata (batchData).
    /// commitment = keccak256(batchData) binds data to proof.
    function commitBatchCalldata(
        bytes calldata batchData,
        bytes32 newRoot,
        Groth16Proof calldata proof
    ) external {
        _requireSequencer();
        if (newRoot == bytes32(0)) revert InvalidNewRoot();

        bytes32 oldRoot = stateRoot;
        bytes32 daCommitment = keccak256(batchData);

        // Public inputs: [DA_commitment, oldRoot, newRoot]
        uint256[3] memory input = [uint256(daCommitment), uint256(oldRoot), uint256(newRoot)];

        bool ok = verifier.verifyProof(proof.a, proof.b, proof.c, input);
        if (!ok) revert InvalidProof();

        // finalize
        stateRoot = newRoot;

        uint256 batchId = nextBatchId++;
        batchCommitment[batchId] = daCommitment;
        batchNewRoot[batchId] = newRoot;

        emit BatchFinalized(batchId, daCommitment, oldRoot, newRoot, 0);
    }

    // -------------------------
    // Blob DA path
    // -------------------------
    /// @notice Submit a batch where DA is a blob commitment (versioned hash).
    /// If useOpcodeBlobhash=true, contract checks blobhash(blobIndex) == expectedVersionedHash.
    ///
    /// For local simulation (hardhat), useOpcodeBlobhash should be false (since no Cancun blob opcode).
    function commitBatchBlob(
        bytes32 expectedVersionedHash,
        uint8 blobIndex,
        bool useOpcodeBlobhash,
        bytes32 newRoot,
        Groth16Proof calldata proof
    ) external {
        _requireSequencer();
        if (newRoot == bytes32(0)) revert InvalidNewRoot();
        if (expectedVersionedHash == bytes32(0)) revert InvalidDACommitment();

        bytes32 oldRoot = stateRoot;

        if (useOpcodeBlobhash) {
            // Cancun+ only
            bytes32 actual = _getBlobHash(blobIndex);
            if (actual == bytes32(0)) revert NoBlobAttached();
            if (actual != expectedVersionedHash) revert BlobHashMismatch();
        }

        uint256[3] memory input = [
            uint256(expectedVersionedHash),
            uint256(oldRoot),
            uint256(newRoot)
        ];

        bool ok = verifier.verifyProof(proof.a, proof.b, proof.c, input);
        if (!ok) revert InvalidProof();

        stateRoot = newRoot;

        uint256 batchId = nextBatchId++;
        batchCommitment[batchId] = expectedVersionedHash;
        batchNewRoot[batchId] = newRoot;

        emit BatchFinalized(batchId, expectedVersionedHash, oldRoot, newRoot, 1);
    }

    function _getBlobHash(uint8 index) internal view virtual returns (bytes32) {
        return blobhash(index);
    }
}
