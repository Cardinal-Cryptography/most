const fs = require("node:fs");
const { network, ethers, upgrades } = require("hardhat");

async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);
  const config = network.config.deploymentConfig;

  console.log("Using ", accounts[0], "as the transaction signer");

  addresses = {};

  if (network.name == "development" || network.name == "bridgenet") {
    const WETH = await ethers.getContractFactory("WETH9");
    console.log("Deploying WETH...");
    const weth = await WETH.deploy();
    console.log("WETH deployed to:", weth.target);
    addresses.weth = weth.target;

    const USDT = await ethers.getContractFactory("TetherToken");
    console.log("Deploying USDT...");
    const usdt = await USDT.deploy(
      "1000000000000000000",
      "Tether USD",
      "USDT",
      "6"
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
      accounts[0],
      addresses.weth,
    ],
    {
      initializer: "initialize",
      kind: "uups",
    },
  );
  await most.waitForDeployment();
  console.log("Most deployed to:", most.target);


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
