// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {ZKRollupBridge} from "../ZKRollupBridge.sol";

contract TestZKRollupBridge is ZKRollupBridge {
    mapping(uint8 => bytes32) public mockBlobHashes;

    constructor(address _verifier, bytes32 _genesisRoot) ZKRollupBridge(_verifier, _genesisRoot) {}

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
