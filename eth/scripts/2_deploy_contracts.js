const fs = require("node:fs");
const hre = require("hardhat");

const COMMISSION_PER_DIX_MILLE=30
const MINIMUM_TRANSFER_AMOUNT_USD=50

async function main() {
  const QUORUM = 5;

  const accounts = await hre.ethers.getSigners();
  const guardians = accounts.slice(1, 9);

  // Governance
  const Governance = await hre.ethers.getContractFactory("Governance");
  const governance = await Governance.deploy(guardians, QUORUM);
  const governanceAddress = await governance.getAddress();

  console.log("Governance deployed to:",  governanceAddress);

  // Membrane
  const Membrane = await hre.ethers.getContractFactory("Membrane");
  const membrane = await Membrane.deploy(guardians,
                                         QUORUM,
                                         COMMISSION_PER_DIX_MILLE,
                                         MINIMUM_TRANSFER_AMOUNT_USD,);
  const membraneAddress = await membrane.getAddress();

  console.log("Membrane deployed to:",  membraneAddress);

  // Wrapped Ether
  const wrappedEtherFactory = await hre.ethers.getContractFactory("WETH9");
  const wrappedEther = await wrappedEtherFactory.deploy();
  const wrappedEtherAddress = await wrappedEther.getAddress();

  console.log("Wrapped Ether deployed to:",  wrappedEtherAddress);

  const addresses = {
    governance: governanceAddress,
    membrane: membraneAddress,
    weth: wrappedEtherAddress,
  };

  fs.writeFileSync("addresses.json", JSON.stringify(addresses));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
