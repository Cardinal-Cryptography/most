const fs = require("node:fs");
const { ethers, upgrades } = require("hardhat");

const COMMISSION_PER_DIX_MILLE=30
const MINIMUM_TRANSFER_AMOUNT_USD=50

async function main() {
  const accounts = await ethers.getSigners();

  const Governance = await ethers.getContractFactory("Governance");
  const governance = await upgrades.deployProxy(Governance,
                                                [
                                                    ['0x05501355922a6529670DB49158676D98D6c34245'], // accounts.slice(1, 9),
                                                    1 //5
                                                ],
                                                {
                                                  initializer: "initialize",
                                                  kind: 'uups'
                                                });
  await governance.waitForDeployment();
  console.log("Governance deployed to:", governance.target);

  console.log("Transferring Governance ownership to self...");
  await governance.transferOwnership(governance.target);
  console.log("Governance ownership transferred successfully");

  const Membrane = await ethers.getContractFactory("Membrane");
  console.log("Deploying Membrane...");
  const membrane = await upgrades.deployProxy(Membrane,
                                              [
                                                  ['0x05501355922a6529670DB49158676D98D6c34245'], // accounts.slice(1, 9),
                                                  1, // 5
                                                  COMMISSION_PER_DIX_MILLE,
                                                  MINIMUM_TRANSFER_AMOUNT_USD,
                                                  governance.target
                                              ],
                                              {
                                                initializer: "initialize",
                                                kind: 'uups'
                                              });
  await membrane.waitForDeployment();
  console.log("Membrane deployed to:",  membrane.target);

  const WETH9 = await ethers.getContractFactory("WETH9");
  console.log("Deploying WETH9...");
  const weth9 = await WETH9.deploy();
  console.log("WETH9 deployed to:",  weth9.target);

  const Token = await ethers.getContractFactory("Token");
  console.log("Deploying USDT...");
  const usdt = await Token.deploy("12000000000000000000000000", "Tether", "USDT");
  console.log("USDT deployed to:",  usdt.target);

  const addresses = {
    governance: governance.target,
    membrane: membrane.target,
    weth9: weth9.target,
    usdt: usdt.target,
  };

  console.log(addresses);
  fs.writeFileSync("addresses.json", JSON.stringify(addresses));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
