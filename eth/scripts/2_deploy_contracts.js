const fs = require("node:fs");
const hre = require("hardhat");

async function main() {
  const accounts = await hre.ethers.getSigners();

  const Membrane = await hre.ethers.getContractFactory("Membrane");
  const membrane = await Membrane.deploy(accounts.slice(1, 9), 5);
  const membraneAddress = await membrane.getAddress();

  // Log address to console
  console.log("Membrane deployed to:",  membraneAddress);

  const addresses = {
    membrane: membraneAddress,
  };

  fs.writeFileSync("addresses.json", JSON.stringify(addresses));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
