const fs = require("node:fs");
const { network, ethers, upgrades } = require("hardhat");

async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);
  const config = network.config.deploymentConfig;

  console.log("Using ", accounts[0], "as signer");

  // read addresses
  const gnosis_contracts = JSON.parse(
    fs.readFileSync("addresses.json", { encoding: "utf8", flag: "r" }),
  );

  const WETH = await ethers.getContractFactory("WETH9");
  console.log("Deploying WETH...");
  const weth = await WETH.deploy();
  console.log("WETH deployed to:", weth.target);

  const Token = await ethers.getContractFactory("Token");
  console.log("Deploying USDT...");
  const usdt = await Token.deploy(
    "12000000000000000000000000",
    "Tether",
    "USDT",
  );
  console.log("USDT deployed to:", usdt.target);

  const Most = await ethers.getContractFactory("Most");
  console.log("Deploying Most...");
  const most = await upgrades.deployProxy(
    Most,
    [config.guardianIds, config.threshold, gnosis_contracts.safe, weth.target],
    {
      initializer: "initialize",
      kind: "uups",
    },
  );
  await most.waitForDeployment();
  console.log("Most deployed to:", most.target);

  const Migrations = await ethers.getContractFactory("Migrations");
  const migrations = await Migrations.deploy();
  console.log("migrations deployed to:", migrations.target);

  console.log("Updating migrations...");
  await migrations.setCompleted(1);

  // --- append addresses
  const addresses = Object.assign({}, gnosis_contracts, {
    migrations: migrations.target,
    most: most.target,
    weth: weth.target,
    usdt: usdt.target,
  });

  console.log(addresses);
  fs.writeFileSync("addresses.json", JSON.stringify(addresses));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
