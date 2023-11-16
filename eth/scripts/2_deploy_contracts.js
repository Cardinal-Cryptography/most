const fs = require("node:fs");
const hre = require("hardhat");

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
  const membrane = await Membrane.deploy(guardians, QUORUM);
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
    wrappedEther: wrappedEtherAddress,
  };

  fs.writeFileSync("addresses.json", JSON.stringify(addresses));

  //await governance.setOwner(governance);
  //await membrane.setOwner(governance);
  //await membrane.addPair(hre.ethers.zeroPadValue(wrappedEtherAddress, 32), )
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
