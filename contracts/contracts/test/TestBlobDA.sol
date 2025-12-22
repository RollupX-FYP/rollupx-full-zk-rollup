// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {BlobDA} from "../da/BlobDA.sol";

contract TestBlobDA is BlobDA {
    mapping(uint8 => bytes32) public mockBlobHashes;

    function setMockBlobHash(uint8 index, bytes32 hash) external {
        mockBlobHashes[index] = hash;
    }

    function _getBlobHash(uint8 index) internal view override returns (bytes32) {
        if (mockBlobHashes[index] != bytes32(0)) {
            return mockBlobHashes[index];
        }
        return super._getBlobHash(index);
    }
}
