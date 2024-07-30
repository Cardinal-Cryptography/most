const { ethers } = require("hardhat");

async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);

  const WrappedToken = await ethers.getContractFactory("WrappedToken");
  const bazero = await WrappedToken.deploy(
    "Bridge Azero",
    "bazero",
    12,
    accounts[0],
  );
  await bazero.waitForDeployment();
  console.log("bazero deployed to:", bazero.target);

  // Replace later with native L2 token
  const testToken = await WrappedToken.deploy(
    "testToken",
    "ttoken",
    18,
    accounts[0],
  );
  await testToken.waitForDeployment();
  console.log("testToken deployed to:", testToken.target);

  const LP = await ethers.getContractFactory("StableSwapLP");
  const lp = await LP.deploy();
  await lp.waitForDeployment();

  console.log("LP deployed to:", lp.target);

  const Pool = await ethers.getContractFactory("StableSwapTwoPool");
  const pool = await Pool.deploy();
  await pool.waitForDeployment();
  console.log("Stable Swap deployed to:", pool.target);

  const setMinterTX = await lp.setMinter(pool.target);
  await setMinterTX.wait(1);

  const initializeTx = await pool.initialize(
    [bazero.target, testToken.target],
    1, // Todo set constants, fees etc
    1,
    1,
    accounts[0],
    lp.target,
  );
  await initializeTx.wait(1);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
