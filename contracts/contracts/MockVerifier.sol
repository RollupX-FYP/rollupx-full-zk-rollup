// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @notice Mock verifier for local simulation.
/// Replace with real Groth16 verifier (snarkjs export) later.
contract MockVerifier {
    // Add a flag to simulate proof failure
    bool public shouldVerify = true;

    function setShouldVerify(bool _shouldVerify) external {
        shouldVerify = _shouldVerify;
    }

    function verifyProof(
        uint256[2] calldata,
        uint256[2][2] calldata,
        uint256[2] calldata,
        uint256[3] calldata
    ) external view returns (bool) {
        return shouldVerify;
    }
}
