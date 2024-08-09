const { ethers, upgrades, network } = require("hardhat");
const fs = require("node:fs");

const BAZERO_NAME = " Bridge Azero";
const BAZERO_SYMBOL = "bazero";

/// Deploys `StableSwapLP` and `StableSwapTwoPool`. Sets minter of the StableSwapLp tp the pool address.
async function deployStableSwap() {
  const LP = await ethers.getContractFactory("StableSwapLP");
  const lp = await LP.deploy();
  await lp.waitForDeployment();

  console.log("LP deployed to:", lp.target);
  const Pool = await ethers.getContractFactory("StableSwapTwoPool");
  const pool = await Pool.deploy();
  await pool.waitForDeployment();
  console.log("Stable Swap deployed to:", pool.target);

  return [lp, pool];
}

/// Deploys BAzero as `WrappedToken` contract. Sets owner of the token to the `newOwner`.
async function deployBAzero(config, newOwner) {
  const WrappedToken = await ethers.getContractFactory("WrappedToken");
  const bazero = await WrappedToken.deploy(
    BAZERO_NAME,
    BAZERO_SYMBOL,
    config.bazero_decimals,
    newOwner,
  );
  await bazero.waitForDeployment();
  console.log("bazero deployed to:", bazero.target);

  return bazero;
}

/// Deploys `MostL2` contract. Sets owner of the contract to the `mostOwner`.
async function deployMostL2(config, mostOwner, poolAddress, bazeroAddress) {
  const MostL2 = await ethers.getContractFactory("MostL2");
  console.log("Deploying MostL2...");
  const most = await upgrades.deployProxy(
    MostL2,
    [
      config.guardianIds,
      config.threshold,
      mostOwner,
      poolAddress,
      bazeroAddress,
    ],
    {
      initializer: "initialize",
      kind: "uups",
    },
  );
  await most.waitForDeployment();
  console.log("MostL2 deployed to:", most.target);

  return most;
}

async function saveContractAddresses(bazero, lp, pool, most) {
  const addresses = {
    pool: pool.target,
    lp: lp.target,
    most: most.target,
    bazero: bazero.target,
  };

  console.log(addresses);

  fs.writeFileSync("l2_addresses.json", JSON.stringify(addresses));
}

/// 1. Deploy bazero token
/// 2. Deploy stable swap pool with SwapLP
/// 3. Deploy most
/// 4. Save contracts addresses.
async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);
  const owner = accounts[0];

  const config = network.config.deploymentConfig;

  const bazero = await deployBAzero(config, owner);
  const [lp, pool] = await deployStableSwap();
  const most = await deployMostL2(config, owner, pool.target, bazero.target);

  await saveContractAddresses(bazero, lp, pool, most);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
