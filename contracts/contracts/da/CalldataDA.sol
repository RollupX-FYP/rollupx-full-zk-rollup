// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {IDAProvider} from "../interfaces/IDAProvider.sol";

contract CalldataDA is IDAProvider {
    function computeCommitment(
        bytes calldata batchData,
        bytes calldata /* daMeta */
    ) external pure override returns (bytes32) {
        return keccak256(batchData);
    }

    function validateDA(bytes32 /* commitment */, bytes calldata /* daMeta */) external pure override {
        // No-op for Calldata DA
    }

    function mode() external pure override returns (uint8) {
        return 0;
    }
}
