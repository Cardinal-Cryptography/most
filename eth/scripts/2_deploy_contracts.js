const fs = require("node:fs");
const hre = require("hardhat");

async function main() {
  const accounts = await hre.ethers.getSigners();

  // Membrane
  const Membrane = await hre.ethers.getContractFactory("Membrane");
  const membrane = await Membrane.deploy(accounts.slice(1, 9), 5);
  const membraneAddress = await membrane.getAddress();

  console.log("Membrane deployed to:",  membraneAddress);

  // Wrapped Ether
  const wrappedEtherFactory = await hre.ethers.getContractFactory("WrappedEther");
  const wrappedEther = await wrappedEtherFactory.deploy();
  const wrappedEtherAdress = await wrappedEther.getAddress();

  console.log("Wrapped Ether deployed to:",  wrappedEtherAdress);

  const addresses = {
    membrane: membraneAddress,
    wrappedEther: wrappedEtherAdress,
  };

  fs.writeFileSync("addresses.json", JSON.stringify(addresses));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
