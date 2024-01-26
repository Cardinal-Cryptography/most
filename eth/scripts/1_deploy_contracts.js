const fs = require("node:fs");
const { network, ethers, upgrades } = require("hardhat");

async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);
  const config = network.config.deploymentConfig;

  console.log("Using ", accounts[0], "as signer");

  const WETH9 = await ethers.getContractFactory("WETH9");
  console.log("Deploying WETH9...");
  const weth9 = await WETH9.deploy();
  console.log("WETH9 deployed to:", weth9.target);

  const Token = await ethers.getContractFactory("Token");
  console.log("Deploying USDT...");
  const usdt = await Token.deploy(
    "12000000000000000000000000",
    "Tether",
    "USDT",
  );
  console.log("USDT deployed to:", usdt.target);

  const Governance = await ethers.getContractFactory("Governance");
  const governance = await upgrades.deployProxy(
    Governance,
    [config.governanceIds, config.governanceThreshold],
    {
      initializer: "initialize",
      kind: "uups",
    },
  );
  await governance.waitForDeployment();
  console.log("Governance deployed to:", governance.target);

  const Most = await ethers.getContractFactory("Most");
  console.log("Deploying Most...");
  const most = await upgrades.deployProxy(
    Most,
    [config.guardianIds, config.threshold, accounts[0]],
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

  // --- spit addresses

  const addresses = {
    migrations: migrations.target,
    governance: governance.target,
    most: most.target,
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
