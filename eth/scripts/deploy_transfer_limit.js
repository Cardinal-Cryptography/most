const { ethers, upgrades } = require("hardhat");

async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);
  console.log("Using ", accounts[0], "as the transaction signer");

  const TransferLimit = await ethers.getContractFactory("TransferLimit");
  const limit = await upgrades.deployProxy(TransferLimit, [accounts[0]], {
    initializer: "initialize",
    kind: "uups",
  });

  await limit.waitForDeployment();
  console.log("TransferLimit deployed to:", limit.target);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
