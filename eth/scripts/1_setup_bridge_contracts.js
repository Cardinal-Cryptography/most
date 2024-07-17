const fs = require("node:fs");
const { ethers, network } = require("hardhat");
const { Keyring } = require("@polkadot/keyring");
const { u8aToHex } = require("@polkadot/util");

function getTokenAddressBySymbol(symbol, tokens) {
  for (let i = 0; i < tokens.length; i++) {
    if (tokens[i].symbol == symbol) {
      return tokens[i].address;
    }
  }
  return null;
}

async function addTokenPair(
  ethTokenAddress,
  azeroTokenAddress,
  isLocal,
  mostContract,
  ownerSigner,
) {
  const ethTokenAddressBytes = ethers.zeroPadValue(
    ethers.getBytes(ethTokenAddress),
    32,
  );
  const azeroTokenAddressBytes = u8aToHex(
    new Keyring({ type: "sr25519" }).decodeAddress(azeroTokenAddress),
  );

  console.log(
    "Adding token pair to Most:",
    ethTokenAddress,
    "=>",
    azeroTokenAddress,
    "( direction:",
    isLocal ? "ETH -> Aleph" : "Aleph -> ETH",
    ")",
  );

  const addPairTx = await mostContract.addPair(
    ethTokenAddressBytes,
    azeroTokenAddressBytes,
    isLocal,
    {
      from: ownerSigner,
    },
  );
  await addPairTx.wait(1);

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

  // read aleph addresses
  let alephAddresses = JSON.parse(
    fs.readFileSync("../azero/addresses.json", { encoding: "utf8", flag: "r" }),
  );

  // --- setup
  const Most = await ethers.getContractFactory("Most");
  const most = Most.attach(addresses.most);

  // --- add Ethereum -> Aleph token pairs
  for (let token of addresses.ethTokens) {
    await addTokenPair(
      token.address,
      getTokenAddressBySymbol(token.symbol, alephAddresses.ethTokens),
      true,
      most,
      signers[0],
    );
  }

  // --- add Aleph -> Ethereum token pairs
  for (let token of addresses.alephTokens) {
    await addTokenPair(
      token.address,
      getTokenAddressBySymbol(token.symbol, alephAddresses.alephTokens),
      false,
      most,
      signers[0],
    );
  }

  // Set wrapped azero address
  const setWrappedAzeroTx = await most.setWrappedAzeroAddress(
    getTokenAddressBySymbol("wAZERO", addresses.alephTokens),
    { from: signers[0] },
  );
  await setWrappedAzeroTx.wait(1);
  console.log("Wrapped azero address set to", await most.wrappedAzeroAddress());

  if (network.name == "development" || network.name == "bridgenet") {
    // NOTE : TEMPorary before devnet is fixed and uses proper genesis that seeds these accounts with funds
    for (const to of signers.slice(1, 4)) {
      await signers[0].sendTransaction({
        to: to.address,
        value: ethers.parseEther("1.0"), // Send 1.0 ether
      });
    }

    // --- provide some wETH and USDT to most contract
    const wethAddress = getTokenAddressBySymbol("WETH", addresses.ethTokens);
    const WETH = await ethers.getContractFactory("WETH9");
    const weth = WETH.attach(wethAddress);

    await weth.deposit({ value: 1000000000000000 });
    /// Gas estimation fails here 1_000_000 is more than enough to make this work.
    await weth.transfer(addresses.most, 100000000000, { gasLimit: 1000000 });

    const usdtAddress = getTokenAddressBySymbol("USDT", addresses.ethTokens);
    const USDT = await ethers.getContractFactory("TetherToken");
    const usdt = USDT.attach(usdtAddress);
    await usdt.transfer(addresses.most, 1000000000000000);

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
