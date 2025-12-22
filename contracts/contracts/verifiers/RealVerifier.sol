// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Pairing} from "../libraries/Pairing.sol";
import {IVerifier} from "../interfaces/IVerifier.sol";

contract RealVerifier is IVerifier {
  uint256 constant SNARK_SCALAR_FIELD = 21888242871839275222246405745257275088548364400416034343698204186575808495617;
  using Pairing for *;

  struct VerifyingKey {
    Pairing.G1Point alfa1;
    Pairing.G2Point beta2;
    Pairing.G2Point gamma2;
    Pairing.G2Point delta2;
    Pairing.G1Point[4] IC;
  }

  struct Proof {
    Pairing.G1Point A;
    Pairing.G2Point B;
    Pairing.G1Point C;
  }

  function verifyingKey() internal pure returns (VerifyingKey memory vk) {
    vk.alfa1 = Pairing.G1Point(uint256(20692898189092739278193869274495556617788530808486270118371701516666252877969), uint256(11713062878292653967971378194351968039596396853904572879488166084231740557279));
    vk.beta2 = Pairing.G2Point([uint256(12168528810181263706895252315640534818222943348193302139358377162645029937006), uint256(281120578337195720357474965979947690431622127986816839208576358024608803542)], [uint256(16129176515713072042442734839012966563817890688785805090011011570989315559913), uint256(9011703453772030375124466642203641636825223906145908770308724549646909480510)]);
    vk.gamma2 = Pairing.G2Point([uint256(11559732032986387107991004021392285783925812861821192530917403151452391805634), uint256(10857046999023057135944570762232829481370756359578518086990519993285655852781)], [uint256(4082367875863433681332203403145435568316851327593401208105741076214120093531), uint256(8495653923123431417604973247489272438418190587263600148770280649306958101930)]);
    vk.delta2 = Pairing.G2Point([uint256(21280594949518992153305586783242820682644996932183186320680800072133486887432), uint256(150879136433974552800030963899771162647715069685890547489132178314736470662)], [uint256(1081836006956609894549771334721413187913047383331561601606260283167615953295), uint256(11434086686358152335540554643130007307617078324975981257823476472104616196090)]);
    
    vk.IC[0] = Pairing.G1Point(uint256(16225148364316337376768119297456868908427925829817748684139175309620217098814), uint256(5167268689450204162046084442581051565997733233062478317813755636162413164690));
    vk.IC[1] = Pairing.G1Point(uint256(12882377842072682264979317445365303375159828272423495088911985689463022094260), uint256(19488215856665173565526758360510125932214252767275816329232454875804474844786));
    vk.IC[2] = vk.IC[0];
    vk.IC[3] = vk.IC[1];
  }

  /*
   * @returns Whether the proof is valid given the hardcoded verifying key
   *          above and the public inputs
   */
  function verifyProof(
    uint256[2] calldata a,
    uint256[2][2] calldata b,
    uint256[2] calldata c,
    uint256[3] calldata input
  ) public view override returns (bool) {
    Proof memory _proof;
    _proof.A = Pairing.G1Point(a[0], a[1]);
    _proof.B = Pairing.G2Point([b[0][0], b[0][1]], [b[1][0], b[1][1]]);
    _proof.C = Pairing.G1Point(c[0], c[1]);

    VerifyingKey memory vk = verifyingKey();

    Pairing.G1Point memory vk_x = Pairing.G1Point(0, 0);
    vk_x = Pairing.plus(vk_x, vk.IC[0]);

    // Ensure we don't read past IC array or input array
    for (uint256 i = 0; i < input.length && i < vk.IC.length - 1; i++) {
      require(input[i] < SNARK_SCALAR_FIELD, "verifier-gte-snark-scalar-field");
      vk_x = Pairing.plus(vk_x, Pairing.scalar_mul(vk.IC[i + 1], input[i]));
    }

    return Pairing.pairing(
      Pairing.negate(_proof.A),
      _proof.B,
      vk.alfa1,
      vk.beta2,
      vk_x,
      vk.gamma2,
      _proof.C,
      vk.delta2
    );
  }
}
