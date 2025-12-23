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
    uint256[3] public expectedInput;

    function setShouldVerify(bool newShouldVerify) external {
        shouldVerify = newShouldVerify;
    }

    function setExpectedInput(uint256[3] calldata _input) external {
        expectedInput = _input;
        checkInput = true;
    }

    function verifyProof(
        uint256[2] calldata,
        uint256[2][2] calldata,
        uint256[2] calldata,
        uint256[3] calldata input
    ) external view override returns (bool) {
        if (!shouldVerify) return false;
        
        if (checkInput) {
             if (input[0] != expectedInput[0] || input[1] != expectedInput[1] || input[2] != expectedInput[2]) {
                 return false;
             }
        }
        
        return true;
    }
}
