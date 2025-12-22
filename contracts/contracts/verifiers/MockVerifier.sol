// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {IVerifier} from "../interfaces/IVerifier.sol";

/// @notice Mock verifier for local simulation.
/// Replace with real Groth16 verifier (snarkjs export) later.
contract MockVerifier is IVerifier {
    // Add a flag to simulate proof failure
    bool public shouldVerify = true;

    function setShouldVerify(bool newShouldVerify) external {
        shouldVerify = newShouldVerify;
    }

    function verifyProof(
        uint256[2] calldata,
        uint256[2][2] calldata,
        uint256[2] calldata,
        uint256[3] calldata
    ) external view override returns (bool) {
        return shouldVerify;
    }
}
