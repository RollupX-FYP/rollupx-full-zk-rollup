import { expect } from "chai";
import { ethers } from "hardhat";
import { RealVerifier, TestRealVerifier } from "../typechain-types";

describe("RealVerifier", function () {
  let verifier: RealVerifier;
  let testVerifier: TestRealVerifier;

  // BN254 Prime Q
  const PRIME_Q = 21888242871839275222246405745257275088696311157297823662689037894645226208583n;

  beforeEach(async function () {
    const Verifier = await ethers.getContractFactory("RealVerifier");
    verifier = await Verifier.deploy();
    
    const TestVerifier = await ethers.getContractFactory("TestRealVerifier");
    testVerifier = await TestVerifier.deploy();
  });

  describe("Pairing Library Wrapper", function () {
    // Valid curve points (Generator G1)
    // x = 1, y = 2
    const G1x = 1;
    const G1y = 2;
    
    it("Should add two G1 points", async function () {
      const result = await testVerifier.testPairingAdd([G1x, G1y], [G1x, G1y]);
      // G1(1, 2) + G1(1, 2) should be on curve.
      expect(result.X).to.not.equal(0);
      expect(result.Y).to.not.equal(0);
    });

    it("Should multiply G1 point", async function () {
      const result = await testVerifier.testPairingMul([G1x, G1y], 2);
      // Should match addition result
      const added = await testVerifier.testPairingAdd([G1x, G1y], [G1x, G1y]);
      expect(result.X).to.equal(added.X);
      expect(result.Y).to.equal(added.Y);
    });

    it("Should fail addition for invalid points (out of field)", async function () {
        // Use PRIME_Q as coordinate to force failure (>= modulus)
        await expect(testVerifier.testPairingAdd([PRIME_Q, 1], [G1x, G1y]))
            .to.be.revertedWith("pairing-add-failed");
    });
    
    it("Should fail multiplication for invalid points (out of field)", async function () {
        await expect(testVerifier.testPairingMul([PRIME_Q, 1], 2))
            .to.be.revertedWith("pairing-mul-failed");
    });
    
    it("Should pass pairing check for trivial valid pairing", async function () {
       const zeroG1 = [0, 0];
       const zeroG2 = [[0, 0], [0, 0]];
       
       const result = await testVerifier.testPairingCheck(
           zeroG1, zeroG2,
           zeroG1, zeroG2,
           zeroG1, zeroG2,
           zeroG1, zeroG2
       );
       expect(result).to.be.true;
    });

    it("Should fail pairing check for invalid points (out of field)", async function () {
        const zeroG1 = [0, 0];
        const zeroG2 = [[0, 0], [0, 0]];
        // Invalid G1 point [PRIME_Q, 0]
        const invalidG1 = [PRIME_Q, 0];
        
        await expect(testVerifier.testPairingCheck(
           invalidG1, zeroG2,
           zeroG1, zeroG2,
           zeroG1, zeroG2,
           zeroG1, zeroG2
       )).to.be.revertedWith("pairing-opcode-failed");
    });
    
    it("Should test negate", async function () {
        const zeroG1 = [0, 0];
        const res = await testVerifier.testNegate(zeroG1);
        expect(res.X).to.equal(0);
        expect(res.Y).to.equal(0);
        
        const g1 = [1, 2];
        const res2 = await testVerifier.testNegate(g1);
        expect(res2.X).to.equal(1);
        expect(res2.Y).to.equal(PRIME_Q - 2n);

        // Test branch coverage for negate: p.X == 0 but p.Y != 0
        // This is technically an invalid point on G1 curve (since Y^2 = X^3 + 3 => Y^2 = 3),
        // but negate function logic doesn't check curve membership, just does arithmetic.
        // It hits the "else" branch of the (p.X == 0 && p.Y == 0) check.
        // If we pass (0, 1), it fails first part of AND? No, 0==0 is true. Fails second part 1==0.
        // So short-circuit evaluation proceeds to second check.
        const invalidPoint = [0, 1];
        const res3 = await testVerifier.testNegate(invalidPoint);
        // Should return (0, PRIME_Q - 1)
        expect(res3.X).to.equal(0);
        expect(res3.Y).to.equal(PRIME_Q - 1n);
    });
  });

  describe("RealVerifier Contract", function () {
      it("Should verify proof (returning false for empty/invalid proof)", async function () {
          // passing zeros
          const a = [0, 0];
          const b = [[0, 0], [0, 0]];
          const c = [0, 0];
          const input = [0, 0, 0];
          
          const ret = await verifier.verifyProof(a, b, c, input);
          expect(ret).to.be.false;
      });
      
      it("Should revert if input >= scalar field", async function () {
          const scalarField = "21888242871839275222246405745257275088548364400416034343698204186575808495617";
          const a = [0, 0];
          const b = [[0, 0], [0, 0]];
          const c = [0, 0];
          const input = [scalarField, 0, 0];
          
          await expect(verifier.verifyProof(a, b, c, input)).to.be.revertedWith("verifier-gte-snark-scalar-field");
      });
      
      it("Should revert if proof element >= prime q", async function () {
           const primeQ = "21888242871839275222246405745257275088696311157297823662689037894645226208583";
           const a: [any, any] = [primeQ, 0];
           const b: [[any, any], [any, any]] = [[0, 0], [0, 0]];
           const c: [any, any] = [0, 0];
           const input: [any, any, any] = [0, 0, 0];
           
           // This will likely revert in the library call or the struct unpacking?
           // Actually, verifyProof unpacks calldata to G1Point struct. 
           // If passed values are > uint256 max it's issue, but they fit in uint256.
           // The revert happens when verifyProof calls Pairing.pairing (via negate or pairing check).
           // Specifically, verifyProof computes vk_x using pairing add/mul.
           // It calls Pairing.pairing at the end.
           // If A is [PRIME_Q, 0], Pairing.negate(A) uses modulo? No, negate uses check X==0.
           // Negate returns (PRIME_Q, 0) -> (PRIME_Q, 0).
           // Then Pairing.pairing is called with that point.
           // Precompile fails => "pairing-opcode-failed".
           
           await expect(verifier.verifyProof(a, b, c, input)).to.be.revertedWith("pairing-opcode-failed");
      });
  });
});
