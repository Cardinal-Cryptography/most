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

  console.log("Transferring Governance ownership to self...");
  await governance.transferOwnership(governance.address);
  console.log("Governance ownership successfully Transferred");

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

  const WETH9 = await ethers.getContractFactory("WETH9");
  console.log("Deploying WETH9...");
  const weth9 = await WETH9.deploy();
  // const wrappedEtherAddress = await wrappedEther.getAddress();
  console.log("WETH9 deployed to:",  weth9.address);

  const Token = await ethers.getContractFactory("Token");
  console.log("Deploying USDT...");
  const usdt = await wrappedEtherFactory.deploy("Tether", "USDT", 12000000e18); // 12 mill
  console.log("USDT deployed to:",  usdt.address);

  const addresses = {
    governance: governance.addresses,
    membrane: membrane.address,
    weth9: weth9.address,
    usdt: usdt.address,
  };

  fs.writeFileSync("addresses.json", JSON.stringify(addresses));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
