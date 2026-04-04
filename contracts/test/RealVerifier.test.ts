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
      expect(result.X).to.not.equal(0);
      expect(result.Y).to.not.equal(0);
    });

    it("Should multiply G1 point", async function () {
      const result = await testVerifier.testPairingMul([G1x, G1y], 2);
      const added = await testVerifier.testPairingAdd([G1x, G1y], [G1x, G1y]);
      expect(result.X).to.equal(added.X);
      expect(result.Y).to.equal(added.Y);
    });

    it("Should fail addition for invalid points (out of field)", async function () {
        await expect(testVerifier.testPairingAdd([PRIME_Q, 1], [G1x, G1y]))
            .to.be.revertedWithCustomError(testVerifier, "PairingAddFailed");
    });
    
    it("Should fail multiplication for invalid points (out of field)", async function () {
        await expect(testVerifier.testPairingMul([PRIME_Q, 1], 2))
            .to.be.revertedWithCustomError(testVerifier, "PairingMulFailed");
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
        const invalidG1 = [PRIME_Q, 0];
        
        await expect(testVerifier.testPairingCheck(
           invalidG1, zeroG2,
           zeroG1, zeroG2,
           zeroG1, zeroG2,
           zeroG1, zeroG2
       )).to.be.revertedWithCustomError(testVerifier, "PairingOpcodeFailed");
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

        const invalidPoint = [0, 1];
        const res3 = await testVerifier.testNegate(invalidPoint);
        expect(res3.X).to.equal(0);
        expect(res3.Y).to.equal(PRIME_Q - 1n);
    });
  });

  describe("RealVerifier Contract", function () {
      it("Should verify proof (returning false for empty/invalid proof)", async function () {
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
          
          await expect(verifier.verifyProof(a, b, c, input)).to.be.revertedWithCustomError(verifier, "VerifierGteSnarkScalarField");
      });
      
      it("Should revert if proof element >= prime q", async function () {
           const primeQ = "21888242871839275222246405745257275088696311157297823662689037894645226208583";
           const a: [any, any] = [primeQ, 0];
           const b: [[any, any], [any, any]] = [[0, 0], [0, 0]];
           const c: [any, any] = [0, 0];
           const input: [any, any, any] = [0, 0, 0];
           
           await expect(verifier.verifyProof(a, b, c, input)).to.be.revertedWithCustomError(verifier, "PairingOpcodeFailed");
      });
  });
});
