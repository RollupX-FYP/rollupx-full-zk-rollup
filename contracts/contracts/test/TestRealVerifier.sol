// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {RealVerifier, Pairing} from "../../contracts/RealVerifier.sol";

contract TestRealVerifier is RealVerifier {
    using Pairing for *;

    function testAdd(uint256 x1, uint256 y1, uint256 x2, uint256 y2) external view returns (uint256 rx, uint256 ry) {
        Pairing.G1Point memory p1 = Pairing.G1Point(x1, y1);
        Pairing.G1Point memory p2 = Pairing.G1Point(x2, y2);
        Pairing.G1Point memory r = Pairing.plus(p1, p2);
        return (r.X, r.Y);
    }

    function testMul(uint256 x, uint256 y, uint256 s) external view returns (uint256 rx, uint256 ry) {
        Pairing.G1Point memory p = Pairing.G1Point(x, y);
        Pairing.G1Point memory r = Pairing.scalar_mul(p, s);
        return (r.X, r.Y);
    }

    function testPairing(
        uint256[2] memory a1, uint256[2][2] memory a2,
        uint256[2] memory b1, uint256[2][2] memory b2,
        uint256[2] memory c1, uint256[2][2] memory c2,
        uint256[2] memory d1, uint256[2][2] memory d2
    ) external view returns (bool) {
        return Pairing.pairing(
            Pairing.G1Point(a1[0], a1[1]),
            Pairing.G2Point([a2[0][0], a2[0][1]], [a2[1][0], a2[1][1]]),
            Pairing.G1Point(b1[0], b1[1]),
            Pairing.G2Point([b2[0][0], b2[0][1]], [b2[1][0], b2[1][1]]),
            Pairing.G1Point(c1[0], c1[1]),
            Pairing.G2Point([c2[0][0], c2[0][1]], [c2[1][0], c2[1][1]]),
            Pairing.G1Point(d1[0], d1[1]),
            Pairing.G2Point([d2[0][0], d2[0][1]], [d2[1][0], d2[1][1]])
        );
    }
}
