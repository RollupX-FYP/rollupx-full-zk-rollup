import { ethers } from "hardhat";
import * as fs from "fs";
import * as path from "path";

type Deployment = {
  chainId?: number;
  bridge?: string;
  verifier?: string;
  mockVerifier?: string;
  mock_verifier?: string;
  calldataDA?: string;
  calldata_da?: string;
  blobDA?: string;
  testBlobDA?: string;
  test_blob_da?: string;
  offchainDA?: string;
  offchain_da?: string;
  genesisRoot?: string;
  genesis_root?: string;
};

function requiredAddress(deployment: Deployment, keys: Array<keyof Deployment>): string {
  for (const key of keys) {
    const value = deployment[key];
    if (typeof value === "string" && ethers.isAddress(value)) {
      return value;
    }
  }
  throw new Error(`Missing deployment address for ${keys.join("/")}`);
}

async function requireCode(label: string, address: string) {
  const code = await ethers.provider.getCode(address);
  if (!code || code === "0x") {
    throw new Error(`${label} has no contract code at ${address}`);
  }
}

async function main() {
  const deploymentFile = path.resolve(
    process.env.DEPLOYMENT_FILE || "deployments/hardhat-local.json",
  );
  const raw = fs.readFileSync(deploymentFile, "utf8");
  const deployment = JSON.parse(raw) as Deployment;

  const network = await ethers.provider.getNetwork();
  const chainId = Number(network.chainId);
  if (deployment.chainId !== undefined && deployment.chainId !== chainId) {
    throw new Error(
      `Chain id mismatch: deployment=${deployment.chainId}, provider=${chainId}`,
    );
  }

  const bridgeAddress = requiredAddress(deployment, ["bridge"]);
  const verifierAddress = requiredAddress(deployment, [
    "verifier",
    "mockVerifier",
    "mock_verifier",
  ]);
  const calldataDAAddress = requiredAddress(deployment, ["calldataDA", "calldata_da"]);
  const blobDAAddress = requiredAddress(deployment, ["blobDA", "testBlobDA", "test_blob_da"]);
  const offchainDAAddress = requiredAddress(deployment, ["offchainDA", "offchain_da"]);

  await requireCode("ZKRollupBridge", bridgeAddress);
  await requireCode("Verifier", verifierAddress);
  await requireCode("CalldataDA", calldataDAAddress);
  await requireCode("BlobDA", blobDAAddress);
  await requireCode("OffChainDA", offchainDAAddress);

  const bridge = await ethers.getContractAt("ZKRollupBridge", bridgeAddress);
  const latestStateRoot: string = await bridge.latestStateRoot();
  const nextBatchId: bigint = await bridge.nextBatchId();

  const providerChecks = [
    { id: 0, label: "calldata", expected: calldataDAAddress },
    { id: 1, label: "blob", expected: blobDAAddress },
    { id: 2, label: "offchain", expected: offchainDAAddress },
  ];

  for (const check of providerChecks) {
    const actual: string = await bridge.daProviders(check.id);
    const enabled: boolean = await bridge.daEnabled(check.id);
    if (actual.toLowerCase() !== check.expected.toLowerCase() || !enabled) {
      throw new Error(
        `DA provider ${check.label} not wired: actual=${actual}, expected=${check.expected}, enabled=${enabled}`,
      );
    }
  }

  const verifierOnBridge: string = await bridge.verifiers(0);
  if (verifierOnBridge.toLowerCase() !== verifierAddress.toLowerCase()) {
    throw new Error(
      `Verifier 0 not wired: actual=${verifierOnBridge}, expected=${verifierAddress}`,
    );
  }

  const minNextBatchId = BigInt(process.env.EXPECT_MIN_NEXT_BATCH_ID || "1");
  if (nextBatchId < minNextBatchId) {
    throw new Error(`Expected nextBatchId >= ${minNextBatchId}, got ${nextBatchId}`);
  }

  const expectRootChanged = process.env.EXPECT_STATE_ROOT_CHANGED === "1";
  const genesisRoot = deployment.genesisRoot || deployment.genesis_root || ethers.ZeroHash;
  if (expectRootChanged && latestStateRoot.toLowerCase() === genesisRoot.toLowerCase()) {
    throw new Error(`State root did not change from genesis ${genesisRoot}`);
  }

  const result = {
    chainId,
    bridge: bridgeAddress,
    latestStateRoot,
    genesisRoot,
    nextBatchId: nextBatchId.toString(),
    daProviders: {
      calldata: calldataDAAddress,
      blob: blobDAAddress,
      offchain: offchainDAAddress,
    },
    verifier: verifierAddress,
    stateRootChanged: latestStateRoot.toLowerCase() !== genesisRoot.toLowerCase(),
  };

  const outFile = process.env.RUNTIME_VALIDATION_OUT;
  if (outFile) {
    const resolved = path.resolve(outFile);
    fs.mkdirSync(path.dirname(resolved), { recursive: true });
    fs.writeFileSync(resolved, `${JSON.stringify(result, null, 2)}\n`);
  }

  console.log(JSON.stringify(result, null, 2));
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
