const fs = require("node:fs");
const { ethers, network } = require("hardhat");
const { Keyring } = require("@polkadot/keyring");
const { u8aToHex } = require("@polkadot/util");

const azeroContracts = require("../../azero/addresses.json");

async function addTokenPair(
  ethTokenAddress,
  azeroTokenAddress,
  mostContract,
  ownerSigner,
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

  await mostContract.addPair(ethTokenAddressBytes, azeroTokenAddressBytes, {
    from: ownerSigner,
  });

  console.log(
    "Most now supports the token pair:",
    ethTokenAddressBytes,
    "=>",
    await mostContract.supportedPairs(ethTokenAddressBytes),
  );
}

async function unpauseMost(mostContract, ownerSigner) {
  console.log("Unpausing Most:");
  await mostContract.unpause({ from: ownerSigner });
  console.log("Most is now unpaused.");
}

async function main() {
  const signers = await ethers.getSigners();
  accounts = signers.map((s) => s.address);

  console.log("Using ", accounts[0], "as signer");

  // read addresses
  let addresses = JSON.parse(
    fs.readFileSync("addresses.json", { encoding: "utf8", flag: "r" }),
  );

  // --- setup
  const Most = await ethers.getContractFactory("Most");
  const most = await Most.attach(addresses.most);

  if (network.name == "development" || network.name == "bridgenet") {
    // NOTE : TEMPorary before devnet is fixed and uses proper genesis that seeds these accounts with funds
    for (const to of signers.slice(1, 4)) {
      await signers[0].sendTransaction({
        to: to.address,
        value: ethers.parseEther("1.0"), // Send 1.0 ether
      });
    }

    // --- provide some wETH and USDT to most contract
    const WETH = await ethers.getContractFactory("WETH9");
    const weth = await WETH.attach(addresses.weth);

    await weth.deposit({ value: 1000000000000000 });
    await weth.transfer(addresses.most, 1000000000000000);

    const USDT = await ethers.getContractFactory("TetherToken");
    const usdt = await USDT.attach(addresses.usdt);
    await usdt.transfer(addresses.most, 1000000000000000);

    for (let [_, ethAddress, azeroAddress] of azeroContracts.tokens) {
      await addTokenPair(ethAddress, azeroAddress, most, signers[0]);
    }

    // --- unpause most
    await unpauseMost(most, signers[0]);
  }

  console.log("Done");
  // NOTE: neccessary because script hangs in CI
  process.exit(0);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
