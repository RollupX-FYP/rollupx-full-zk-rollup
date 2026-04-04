// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Constants} from "./Constants.sol";

library Pairing {
  error PairingAddFailed();
  error PairingMulFailed();
  error PairingOpcodeFailed();

  struct G1Point {
    uint256 X;
    uint256 Y;
  }

  // Encoding of field elements is: X[0] * z + X[1]
  struct G2Point {
    uint256[2] X;
    uint256[2] Y;
  }

  /*
   * @return The negation of p, i.e. p.plus(p.negate()) should be zero.
   */
  function negate(G1Point memory p) internal pure returns (G1Point memory) {
    // The prime q in the base field F_q for G1
    if (p.X == 0 && p.Y == 0) {
      return G1Point(0, 0);
    } else {
      return G1Point(p.X, Constants.PRIME_Q - (p.Y % Constants.PRIME_Q));
    }
  }

  /*
   * @return r the sum of two points of G1
   */
  function plus(
    G1Point memory p1,
    G1Point memory p2
  ) internal view returns (G1Point memory r) {
    uint256[4] memory input;
    input[0] = p1.X;
    input[1] = p1.Y;
    input[2] = p2.X;
    input[3] = p2.Y;
    bool success;

    // solium-disable-next-line security/no-inline-assembly
    // slither-disable-next-line assembly
    assembly {
      success := staticcall(sub(gas(), 2000), 6, input, 0x80, r, 0x40)
    }

    if (!success) revert PairingAddFailed();
  }

  /*
   * @return r the product of a point on G1 and a scalar, i.e.
   *         p == p.scalarMul(1) and p.plus(p) == p.scalarMul(2) for all
   *         points p.
   */
  function scalarMul(G1Point memory p, uint256 s) internal view returns (G1Point memory r) {
    uint256[3] memory input;
    input[0] = p.X;
    input[1] = p.Y;
    input[2] = s;
    bool success;
    // solium-disable-next-line security/no-inline-assembly
    // slither-disable-next-line assembly
    assembly {
      success := staticcall(sub(gas(), 2000), 7, input, 0x60, r, 0x40)
    }
    if (!success) revert PairingMulFailed();
  }

  /* @return The result of computing the pairing check
   *         e(p1[0], p2[0]) *  .... * e(p1[n], p2[n]) == 1
   *         For example,
   *         pairing([P1(), P1().negate()], [P2(), P2()]) should return true.
   */
  function pairing(
    G1Point memory a1,
    G2Point memory a2,
    G1Point memory b1,
    G2Point memory b2,
    G1Point memory c1,
    G2Point memory c2,
    G1Point memory d1,
    G2Point memory d2
  ) internal view returns (bool) {
    G1Point[4] memory p1 = [a1, b1, c1, d1];
    G2Point[4] memory p2 = [a2, b2, c2, d2];

    uint256 inputSize = 24;
    uint256[] memory input = new uint256[](inputSize);

    for (uint256 i = 0; i < 4; i++) {
      uint256 j = i * 6;
      input[j + 0] = p1[i].X;
      input[j + 1] = p1[i].Y;
      input[j + 2] = p2[i].X[0];
      input[j + 3] = p2[i].X[1];
      input[j + 4] = p2[i].Y[0];
      input[j + 5] = p2[i].Y[1];
    }

    uint256[1] memory out;
    bool success;

    // solium-disable-next-line security/no-inline-assembly
    // slither-disable-next-line assembly
    assembly {
      success := staticcall(sub(gas(), 2000), 8, add(input, 0x20), mul(inputSize, 0x20), out, 0x20)
    }

    if (!success) revert PairingOpcodeFailed();

    return out[0] != 0;
  }
}
