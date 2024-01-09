const fs = require("node:fs");
const { ethers, artifacts } = require("hardhat");
const contracts = require("../addresses.json");

async function main() {
  const signers = await ethers.getSigners();
  accounts = signers.map((s) => s.address);

  console.log("Using ", accounts[0], "as signer");

  // --- setup

  const Governance = artifacts.require("Governance");
  const governance = await Governance.at(contracts.governance);

  let initialGovernanceOwner = await governance.owner();
  console.log(
    "Transferring Governance ownership from ",
    initialGovernanceOwner,
    "to ",
    governance.address
  );
  await governance.transferOwnership(contracts.governance);
  console.log("Governance ownership transferred successfully");

  const Most = artifacts.require("Most");
  const most = await Most.at(contracts.most);

  const payload = ethers.zeroPadValue(ethers.getBytes(contracts.usdt), 32);
  console.log("Setting USDT address in Most to:", payload);
  await most.setUSDT(payload);

  let initialMostOwner = await most.owner();
  console.log(
    "Transferring Most ownership from ",
    initialMostOwner,
    "to",
    contracts.governance
  );
  await most.transferOwnership(contracts.governance);
  console.log("Most ownership transferred successfully");

  const Migrations = artifacts.require("Migrations");
  const migrations = await Migrations.at(contracts.migrations);

  let lastCompletedMigration = await migrations.last_completed_migration();
  console.log("Updating migrations from ", lastCompletedMigration, " to ", 2);
  await migrations.setCompleted(2);

  console.log("Done");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
