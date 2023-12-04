const fs = require("node:fs");
const hre = require("hardhat");

async function main() {
  const accounts = await hre.ethers.getSigners();

  // Membrane
  const Membrane = await hre.ethers.getContractFactory("Membrane");
  const membrane = await Membrane.deploy(accounts.slice(0, 1), 1);
  const membraneAddress = await membrane.getAddress();

  console.log("Membrane deployed to:",  membraneAddress);

  // Wrapped Ether
  const wrappedEtherFactory = await hre.ethers.getContractFactory("WETH9");
  const wrappedEther = await wrappedEtherFactory.deploy();
  const wrappedEtherAddress = await wrappedEther.getAddress();

  console.log("Wrapped Ether deployed to:",  wrappedEtherAddress);

  const addresses = {
    membrane: membraneAddress,
    wrappedEther: wrappedEtherAddress,
  };

  fs.writeFileSync("addresses.json", JSON.stringify(addresses));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
