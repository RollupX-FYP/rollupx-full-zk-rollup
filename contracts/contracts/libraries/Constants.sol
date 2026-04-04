// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

library Constants {
    /// @dev The scalar field of the BN254 curve (snark scalar field)
    uint256 internal constant SNARK_SCALAR_FIELD = 21888242871839275222246405745257275088548364400416034343698204186575808495617;

    /// @dev The base field of the BN254 curve (prime q)
    uint256 internal constant PRIME_Q = 21888242871839275222246405745257275088696311157297823662689037894645226208583;
}
