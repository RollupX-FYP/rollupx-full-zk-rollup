import { ethers } from "hardhat";

async function main() {
  const errorData = "0x90679ba2"; 

  const artifact = await ethers.getContractFactory("ZKRollupBridge");
  const iface = artifact.interface;

  console.log("Checking errors...");
  
  if (!iface) {
      console.log("Interface is undefined");
      return;
  }

  // Iterate over fragments
  iface.forEachError((error) => {
      // ethers v6
      const selector = error.selector;
      if (selector === errorData) {
          console.log("Match found:", error.name, "=>", selector);
      }
  });
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
