const fs = require("node:fs");
const { network, ethers, upgrades } = require("hardhat");

function getTokenAddressBySymbol(symbol, tokens) {
  for (let i = 0; i < tokens.length; i++) {
    if (tokens[i].symbol == symbol) {
      return tokens[i].address;
    }
  }
  return null;
}

async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);
  const deploymentConfig = network.config.deploymentConfig;
  const tokenConfigPath = deploymentConfig.tokenConfigPath;

  let tokenConfig = JSON.parse(
    fs.readFileSync(tokenConfigPath, { encoding: "utf8", flag: "r" }),
  );
  const alephTokens = tokenConfig.aleph;
  const ethTokens = tokenConfig.eth;

  var alephTokenAdresses = [];
  var ethTokenAdresses = [];

  console.log("Using ", accounts[0], "as the transaction signer");

  addresses = {};

  if (network.config.dev) {
    const WETH = await ethers.getContractFactory("WETH9");
    console.log("Deploying WETH...");
    const weth = await WETH.deploy();
    console.log("WETH deployed to:", weth.target);
    addresses.weth = weth.target;
    ethTokenAdresses.push({ symbol: "WETH", address: weth.target });

    const USDT = await ethers.getContractFactory("TetherToken");
    console.log("Deploying USDT...");
    const usdt = await USDT.deploy(
      "1000000000000000000",
      "Tether USD",
      "USDT",
      "6",
    );
    console.log("USDT deployed to:", usdt.target);
    ethTokenAdresses.push({ symbol: "USDT", address: usdt.target });
  }

  for (const token of ethTokens) {
    if (token.deployed == true) {
      ethTokenAdresses.push({ symbol: token.symbol, address: token.address });
    }
  }

  const Most = await ethers.getContractFactory("Most");
  console.log("Deploying Most...");
  const most = await upgrades.deployProxy(
    Most,
    [
      deploymentConfig.guardianIds,
      deploymentConfig.threshold,
      accounts[0],
      getTokenAddressBySymbol("WETH", ethTokenAdresses),
    ],
    {
      initializer: "initialize",
      kind: "uups",
    },
  );
  await most.waitForDeployment();
  console.log("Most deployed to:", most.target);

  for (const token of alephTokens) {
    const WrappedToken = await ethers.getContractFactory("WrappedToken");
    console.log("Deploying WrappedToken for", token.symbol, "...");
    const wrappedToken = await WrappedToken.deploy(
      token.name,
      token.symbol,
      token.decimals,
      most.target,
    );
    console.log("WrappedToken deployed to:", wrappedToken.target);
    alephTokenAdresses.push({
      symbol: token.symbol,
      address: wrappedToken.target,
    });
  }

  // --- append addresses
  addresses = {
    alephTokens: alephTokenAdresses,
    ethTokens: ethTokenAdresses,
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
