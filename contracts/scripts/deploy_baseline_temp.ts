import { ethers } from "hardhat";
import fs from "fs";

async function main() {
  const Token = await ethers.getContractFactory("L1BaselineToken");
  const token = await Token.deploy();
  await token.waitForDeployment();
  const address = await token.getAddress();
  console.log("BaselineToken:", address);
  fs.writeFileSync("../baseline_addr.txt", address);
}
main().catch((error) => { console.error(error); process.exitCode = 1; });
