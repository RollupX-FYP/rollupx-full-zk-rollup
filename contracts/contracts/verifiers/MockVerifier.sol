// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {IVerifier} from "../interfaces/IVerifier.sol";

/// @notice Mock verifier for local simulation.
/// Replace with real Groth16 verifier (snarkjs export) later.
contract MockVerifier is IVerifier {
    // Add a flag to simulate proof failure
    bool public shouldVerify = true;
    
    // Add input assertion
    bool public checkInput;
    uint256[4] public expectedInput;

    function setShouldVerify(bool newShouldVerify) external {
        shouldVerify = newShouldVerify;
    }

    function setExpectedInput(uint256[4] calldata newInput) external {
        expectedInput = newInput;
        checkInput = true;
    }

    function verifyProof(
        uint256[2] calldata,
        uint256[2][2] calldata,
        uint256[2] calldata,
        uint256[4] calldata input
    ) external view override returns (bool) {
        if (!shouldVerify) return false;
        
        if (checkInput) {
             for (uint i = 0; i < 4; i++) {
                 if (input[i] != expectedInput[i]) return false;
             }
        }
        
        return true;
    }
}
