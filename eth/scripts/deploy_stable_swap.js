const { ethers } = require("hardhat");

async function main() {
  const WrappedToken = await ethers.getContractFactory("WrappedToken");
  const wrappedToken = await WrappedToken.deploy(
    "Bridge Azero",
    "BAzero",
    12,
    "0x929b4B89021Fa9cfA4317CF5a0C6A9821ed9295b", // Todo: set to MostL2
  );
  wrappedToken.waitForDeployment();

  console.log("Bazero deployed to:", wrappedToken.target);

  const StableSwapLPFactory = await ethers.getContractFactory(
    "StableSwapLPFactory",
  );
  const stableSwapLPFactory = await StableSwapLPFactory.deploy();
  stableSwapLPFactory.waitForDeployment();

  console.log("StableSwapLPFactory deployed to:", stableSwapLPFactory.target);

  const StableSwapTwoPoolDeployer = await ethers.getContractFactory(
    "StableSwapTwoPoolDeployer",
  );
  const stableSwapTwoPoolDeployer = await StableSwapTwoPoolDeployer.deploy();
  stableSwapTwoPoolDeployer.waitForDeployment();

  console.log(
    "StableSwapTwoPoolDeployer deployed to:",
    stableSwapTwoPoolDeployer.target,
  );

  const StableSwapFactory =
    await ethers.getContractFactory("StableSwapFactory");
  const stableSwapFactory = await StableSwapFactory.deploy(
    stableSwapLPFactory.target,
    stableSwapTwoPoolDeployer.target,
  );
  await stableSwapFactory.waitForDeployment();

  console.log("StableSwapFactory deployed to:", stableSwapFactory.target);

  await stableSwapLPFactory.transferOwnership(stableSwapFactory.target);
  await (
    await stableSwapTwoPoolDeployer.transferOwnership(stableSwapFactory.target)
  ).wait(1);

  const tx = await stableSwapFactory.createSwapPair(
    wrappedToken.target,
    "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE",
    1,
    1,
    1,
  );
  await tx.wait(1);

  console.log(tx);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
