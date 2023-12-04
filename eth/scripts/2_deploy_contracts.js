const fs = require("node:fs");
const hre = require("hardhat");

const COMMISSION_PER_DIX_MILLE = 30;
const MINIMUM_TRANSFER_AMOUNT_USD = 50;

async function main() {
  const accounts = await hre.ethers.getSigners();

  // Membrane
  const Membrane = await hre.ethers.getContractFactory("Membrane");
  const membrane = await Membrane.deploy(
    accounts.slice(0, 1),
    1,
    COMMISSION_PER_DIX_MILLE,
    MINIMUM_TRANSFER_AMOUNT_USD,
  );
  const membraneAddress = await membrane.getAddress();

  console.log("Membrane deployed to:", membraneAddress);

  // Wrapped Ether
  const wrappedEtherFactory = await hre.ethers.getContractFactory("WETH9");
  const wrappedEther = await wrappedEtherFactory.deploy();
  const wrappedEtherAddress = await wrappedEther.getAddress();

  console.log("Wrapped Ether deployed to:", wrappedEtherAddress);

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
