// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {IDAProvider} from "../interfaces/IDAProvider.sol";

contract BlobDA is IDAProvider {
    error NoBlobAttached();
    error BlobHashMismatch();
    error InvalidCommitment();

    function computeCommitment(
        bytes calldata /* batchData */,
        bytes calldata daMeta
    ) external pure override returns (bytes32) {
        // Expect daMeta to be encoded as (bytes32 expectedVersionedHash, uint8 blobIndex)
        (bytes32 expectedVersionedHash, ) = abi.decode(daMeta, (bytes32, uint8));
        return expectedVersionedHash;
    }

    function validateDA(bytes32 commitment, bytes calldata daMeta) external view override {
        (bytes32 expectedVersionedHash, uint8 blobIndex) = abi.decode(daMeta, (bytes32, uint8));
        
        if (expectedVersionedHash == bytes32(0)) revert InvalidCommitment();

        // Ensure the commitment matches the expected hash in metadata
        if (commitment != expectedVersionedHash) revert InvalidCommitment();

        bytes32 actual = _getBlobHash(blobIndex);
        if (actual == bytes32(0)) revert NoBlobAttached();
        if (actual != expectedVersionedHash) revert BlobHashMismatch();
    }

    function _getBlobHash(uint8 index) internal view virtual returns (bytes32) {
        return blobhash(index);
    }

    function mode() external pure override returns (uint8) {
        return 1;
    }
}
