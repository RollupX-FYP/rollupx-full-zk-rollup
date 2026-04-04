import * as fs from "fs";
import * as path from "path";

const ARTIFACTS_DIR = path.join(__dirname, "../artifacts/contracts");
const OUTPUT_DIR = path.join(__dirname, "../docs/abis");

const TARGET_CONTRACTS = [
  "bridge/ZKRollupBridge.sol/ZKRollupBridge.json",
  "da/BlobDA.sol/BlobDA.json",
  "da/CalldataDA.sol/CalldataDA.json",
  "verifiers/RealVerifier.sol/RealVerifier.json",
  "interfaces/IDAProvider.sol/IDAProvider.json",
  "interfaces/IVerifier.sol/IVerifier.json"
];

async function main() {
  if (!fs.existsSync(OUTPUT_DIR)) {
    fs.mkdirSync(OUTPUT_DIR, { recursive: true });
  }

  for (const contractPath of TARGET_CONTRACTS) {
    const artifactPath = path.join(ARTIFACTS_DIR, contractPath);
    if (fs.existsSync(artifactPath)) {
      const artifact = JSON.parse(fs.readFileSync(artifactPath, "utf8"));
      // contractPath is like "bridge/ZKRollupBridge.sol/ZKRollupBridge.json"
      // we want "ZKRollupBridge"
      const fileName = path.basename(contractPath); // ZKRollupBridge.json
      const contractName = path.basename(fileName, ".json"); // ZKRollupBridge
      
      const abi = artifact.abi;
      const outputPath = path.join(OUTPUT_DIR, `${contractName}.json`);
      
      fs.writeFileSync(outputPath, JSON.stringify(abi, null, 2));
      console.log(`Exported ABI for ${contractName} to ${outputPath}`);
    } else {
      console.warn(`Artifact not found: ${artifactPath}`);
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
