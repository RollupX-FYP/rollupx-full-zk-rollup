// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {IDAProvider} from "../interfaces/IDAProvider.sol";

/// @notice DA Provider for Off-Chain Data Availability (Validium).
contract OffChainDA is IDAProvider {
    /// @notice Returns the commitment. For Off-Chain DA, the daMeta contains the commitment/pointer directly.
    function computeCommitment(
        bytes calldata /* batchData */,
        bytes calldata daMeta
    ) external pure override returns (bytes32) {
        require(daMeta.length == 32, "Invalid DA Meta length for OffChain");
        return bytes32(daMeta[0:32]);
    }

    /// @notice Validates that the DA is available. For Off-Chain, we trust the committee/sequencer.
    function validateDA(bytes32 /* commitment */, bytes calldata /* daMeta */) external pure override {
        // No-op
    }

    /// @notice Returns the mode identifier (2 for OffChain).
    function mode() external pure override returns (uint8) {
        return 2;
    }
}
