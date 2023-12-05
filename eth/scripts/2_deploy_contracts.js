const fs = require("node:fs");
const { ethers, upgrades } = require("hardhat");


const COMMISSION_PER_DIX_MILLE=30
const MINIMUM_TRANSFER_AMOUNT_USD=50

async function main() {
  const accounts = await ethers.getSigners();

  const Governance = await ethers.getContractFactory("Governance");
  console.log("Deploying Governance...");
  const governance = await upgrades.deployProxy(Governance,
                                                accounts.slice(1, 9),
                                                5,
                                                {
                                                  initializer: "initialize",
                                                });
  await governance.deployed();

  console.log("Governance deployed to:", governance.address);

  // TODO : transfer ownership to self

  const Membrane = await ethers.getContractFactory("Membrane");

  console.log("Deploying Membrane...");

  const membrane = await upgrades.deployProxy(Membrane,
                                              accounts.slice(1, 9),
                                              5,
                                              COMMISSION_PER_DIX_MILLE,
                                              MINIMUM_TRANSFER_AMOUNT_USD,
                                              governance.address,
                                              {
                                                initializer: "initialize",
                                              });

   await membrane.deployed();

  console.log("Membrane deployed to:",  membrane.address);

  // Wrapped Ether
  const WrappedEther = await ethers.getContractFactory("WETH9");
  const wrappedEther = await wrappedEtherFactory.deploy();
  // const wrappedEtherAddress = await wrappedEther.getAddress();

  console.log("Wrapped Ether deployed to:",  wrappedEtherAddress);

  const addresses = {
    governance: governance.addresses,
    membrane: membrane.address,
    wrappedEther: wrappedEther.address,
  };

  fs.writeFileSync("addresses.json", JSON.stringify(addresses));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
