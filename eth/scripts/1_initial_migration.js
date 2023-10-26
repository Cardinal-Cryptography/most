const hre = require("hardhat");

async function main() {
  const Migrations = await hre.ethers.getContractFactory("Migrations");
  const migrations = await Migrations.deploy();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
