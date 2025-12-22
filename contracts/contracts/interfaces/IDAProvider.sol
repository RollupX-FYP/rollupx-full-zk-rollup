// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

interface IDAProvider {
    /// @notice Returns the commitment that must be used as public input (daCommitment)
    function computeCommitment(
        bytes calldata batchData,
        bytes calldata daMeta
    ) external view returns (bytes32);

    /// @notice Validates that DA is actually available/attached (e.g., blobhash check)
    function validateDA(bytes32 commitment, bytes calldata daMeta) external view;

    /// @notice Returns the mode of the DA provider (0 = calldata, 1 = blob)
    function mode() external pure returns (uint8);
}
