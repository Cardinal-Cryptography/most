const fs = require("node:fs");
const { network, ethers, upgrades } = require("hardhat");

async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);
  const config = network.config.deploymentConfig;

  console.log("Using ", accounts[0], "as the transaction signer");

  const Migrations = await ethers.getContractFactory("Migrations");
  const migrations = await Migrations.deploy();
  await migrations.deploymentTransaction().wait(1);
  console.log("Migrations deployed to:", migrations.target);

  // --- write addresses

  const addresses = {
    migrations: migrations.target,
  };

  console.log(addresses);
  fs.writeFileSync("addresses.json", JSON.stringify(addresses));

  console.log("Done");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
