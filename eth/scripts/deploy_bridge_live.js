const fs = require("node:fs");
const { u8aToHex } = require("@polkadot/util");
const { Keyring } = require("@polkadot/keyring");
const { network, ethers, upgrades } = require("hardhat");

const azeroContracts = require("../../azero/addresses.json");

async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);
  const config = network.config.deploymentConfig;

  console.log("Using", accounts[0], "as the transaction signer");

  const Most = await ethers.getContractFactory("Most");
  console.log("Deploying Most...");

  let weth = ethers.getAddress(config.weth);

  const most = await upgrades.deployProxy(
    Most,
    [config.guardianIds, config.threshold, accounts[0], weth],
    {
      initializer: "initialize",
      kind: "uups",
    },
  );
  await most.waitForDeployment();
  console.log("Most deployed to:", most.target);

  for (let [ethAddress, azeroAddress] of azeroContracts.tokens) {
    console.log("Adding pair", ethAddress, azeroAddress);

    let ethAddressBytes = ethers.zeroPadValue(ethers.getBytes(ethAddress), 32);

    let azeroAddressBytes = u8aToHex(
      new Keyring({ type: "sr25519" }).decodeAddress(azeroAddress),
    );

    let tx = await most.addPair(ethAddressBytes, azeroAddressBytes);
    await tx.wait();
  }

  addresses = {
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
  process.exitCode = 1;
});
