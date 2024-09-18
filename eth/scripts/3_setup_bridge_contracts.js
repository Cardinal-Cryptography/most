const fs = require("node:fs");
const { ethers, artifacts, network } = require("hardhat");
const { Keyring } = require("@polkadot/keyring");
const { u8aToHex } = require("@polkadot/util");

const azeroContracts = require("../../azero/addresses.json");

async function addTokenPair(
  ethTokenAddress,
  azeroTokenAddress,
  mostContract,
) {
  console.log(
    "Adding token pair to Most:",
    ethTokenAddress,
    "=>",
    azeroTokenAddress,
  );
  const ethTokenAddressBytes = ethers.zeroPadValue(
    ethers.getBytes(ethTokenAddress),
    32,
  );
  const azeroTokenAddressBytes = u8aToHex(
    new Keyring({ type: "sr25519" }).decodeAddress(azeroTokenAddress),
  );
  mostContract.addPair(
      ethTokenAddressBytes,
      azeroTokenAddressBytes,
  );

  console.log(
      "Most now supports the token pair:",
      ethTokenAddressBytes,
      "=>",
      await mostContract.supportedPairs(ethTokenAddressBytes),
  );
}

async function main() {
  const signers = await ethers.getSigners();
  const config = network.config;
  accounts = signers.map((s) => s.address);

  console.log("Using ", accounts[0], "as signer");

  // read addresses
  let addresses = JSON.parse(
    fs.readFileSync("addresses.json", { encoding: "utf8", flag: "r" }),
  );

  const Most = artifacts.require("Most");
  const most = await Most.at(addresses.most);

  if (config.dev) {
    // Provide some wETH to most contract
    const WETH = artifacts.require("WETH9");
    const weth = await WETH.at(addresses.weth);

    await weth.deposit({ value: 1000000000000000 });
    await weth.transfer(addresses.most, 1000000000000000);
  }

  for (let [_, ethAddress, azeroAddress] of azeroContracts.tokens) {
    await addTokenPair(ethAddress, azeroAddress, most);
  }

  console.log("Done");
  // NOTE: neccessary because script hangs in CI
  process.exit(0);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
