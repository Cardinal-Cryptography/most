const fs = require("node:fs");
const { network, ethers, upgrades } = require("hardhat");

async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);
  const config = network.config.deploymentConfig;

  console.log("Using ", accounts[0], "as the transaction signer");

  // read addresses
  let addresses = JSON.parse(
    fs.readFileSync("addresses.json", { encoding: "utf8", flag: "r" }),
  );

  const Migrations = artifacts.require("Migrations");
  const migrations = await Migrations.at(addresses.migrations);

  // check migratons
  let lastCompletedMigration = await migrations.last_completed_migration();
  lastCompletedMigration = lastCompletedMigration.toNumber();
  console.log("Last completed migration: ", lastCompletedMigration);
  if (lastCompletedMigration != 1) {
    console.error("Previous migration has not been completed");
    process.exit(-1);
  }

  if (network.name == "development" || network.name == "bridgenet") {
    const WETH = await ethers.getContractFactory("WETH9");
    console.log("Deploying WETH...");
    const weth = await WETH.deploy();
    console.log("WETH deployed to:", weth.target);
    addresses.weth = weth.target;

    const Token = await ethers.getContractFactory("Token");
    console.log("Deploying USDT...");
    const usdt = await Token.deploy(
      "12000000000000000000000000",
      "6",
      "Tether",
      "USDT",
    );
    console.log("USDT deployed to:", usdt.target);
    addresses.usdt = usdt.target;
  }

  const Most = await ethers.getContractFactory("Most");
  console.log("Deploying Most...");
  const most = await upgrades.deployProxy(
    Most,
    [
      config.guardianIds,
      config.threshold,
      addresses.gnosis.safe,
      addresses.weth,
    ],
    {
      initializer: "initialize",
      kind: "uups",
    },
  );
  await most.waitForDeployment();
  console.log("Most deployed to:", most.target);

  console.log("Updating migrations...");
  await migrations.setCompleted(2);

  // --- append addresses

  addresses = {
    ...addresses,
    most: most.target,
  };

  console.log(addresses);
  fs.writeFileSync("addresses.json", JSON.stringify(addresses));

  console.log("Done");
  // NOTE: neccessary because script hangs in CI
  process.exit(0);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
