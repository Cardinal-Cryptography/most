const fs = require("node:fs");
const { ethers } = require("hardhat");

async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);

  console.log("Using", accounts[0], "as the transaction signer");

  const Most = await ethers.getContractFactory("Most");
  console.log("Deploying Most...");

  const most = await Most.deploy();
  console.log("Upgrade for Most deployed to:", most.target);

  addresses = { most_implementation: most.target };
  console.log("\n", addresses, "\n");
  fs.writeFileSync("upload.json", JSON.stringify(addresses));

  console.log("Done");
  // NOTE: neccessary because script hangs in CI
  process.exit(0);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
