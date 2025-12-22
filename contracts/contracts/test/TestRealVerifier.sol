// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {RealVerifier} from "../verifiers/RealVerifier.sol";
import {Pairing} from "../libraries/Pairing.sol";

// Expose internal functions of RealVerifier and Pairing library for testing
contract TestRealVerifier is RealVerifier {
    using Pairing for *;

    // Helper to expose verifyProof directly (already public in RealVerifier, but included for completeness if we add more helpers)
    
    // Wrapper for library functions to test them in isolation
    function testPairingAdd(
        Pairing.G1Point memory p1,
        Pairing.G1Point memory p2
    ) public view returns (Pairing.G1Point memory) {
        return Pairing.plus(p1, p2);
    }
    
    function testPairingMul(
        Pairing.G1Point memory p,
        uint256 s
    ) public view returns (Pairing.G1Point memory) {
        return Pairing.scalar_mul(p, s);
    }
    
    function testPairingCheck(
        Pairing.G1Point memory a1,
        Pairing.G2Point memory a2,
        Pairing.G1Point memory b1,
        Pairing.G2Point memory b2,
        Pairing.G1Point memory c1,
        Pairing.G2Point memory c2,
        Pairing.G1Point memory d1,
        Pairing.G2Point memory d2
    ) public view returns (bool) {
        return Pairing.pairing(a1, a2, b1, b2, c1, c2, d1, d2);
    }
    
    function testNegate(Pairing.G1Point memory p) public pure returns (Pairing.G1Point memory) {
        return Pairing.negate(p);
    }
}
