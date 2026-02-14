// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {IVerifier} from "../interfaces/IVerifier.sol";

/// @notice Stub implementation for Halo2 verifier (KZG/IPA).
/// @dev Always returns true for simulation interoperability.
contract Halo2Verifier is IVerifier {
    function verifyProof(
        uint256[2] calldata,
        uint256[2][2] calldata,
        uint256[2] calldata,
        uint256[4] calldata
    ) external pure override returns (bool) {
        return true;
    }
}
