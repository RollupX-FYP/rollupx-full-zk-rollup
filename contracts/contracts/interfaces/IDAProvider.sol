// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @notice Interface for Data Availability (DA) Providers.
interface IDAProvider {
    /// @notice Computes the commitment for the given batch data and metadata.
    /// @param batchData The raw batch data.
    /// @param daMeta Metadata required for commitment computation (e.g., versioned hash, indices).
    /// @return The computed commitment (e.g., keccak256 or versioned hash).
    function computeCommitment(
        bytes calldata batchData,
        bytes calldata daMeta
    ) external view returns (bytes32);

    /// @notice Validates that the DA is available (e.g., blob is attached).
    /// @param commitment The commitment computed by computeCommitment.
    /// @param daMeta Metadata required for validation.
    function validateDA(bytes32 commitment, bytes calldata daMeta) external view;

    /// @notice Returns the mode identifier of the DA provider.
    /// @return 0 for Calldata, 1 for Blob, etc.
    function mode() external pure returns (uint8);
}
