const { ethers, artifacts, network } = require("hardhat");
const fs = require("node:fs");
const { u8aToHex } = require("@polkadot/util");
const { Keyring } = require("@polkadot/keyring");

// bridges (sends) 1 unit of wETH from Ethereum to AlephZero
async function main() {
  const signers = await ethers.getSigners();
  accounts = signers.map((s) => s.address);

  console.log("Using ", accounts[0], "as signer");

  let addresses = JSON.parse(
    fs.readFileSync("addresses.json", { encoding: "utf8", flag: "r" }),
  );

  const amount = 1000000000000;
  const receiver = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";

  const WETH = artifacts.require("WETH9");
  const weth = await WETH.at(addresses.weth);

  // check senders balance
  let query = await weth.balanceOf(accounts[0]);
  let balance = query.toNumber();

  console.log(
    "wETH [",
    addresses.weth,
    "]",
    "balance of:",
    accounts[0],
    ":",
    balance,
  );

  if (balance < amount) {
    // if not enough balance, deposit some
    console.log("Wrapping", amount, "of pETH", "as", addresses.weth);
    await weth.deposit({ value: amount });
  }

  const Most = artifacts.require("Most");
  const most = await Most.at(addresses.most);

  // TODO check approval
  // approve Most
  await weth.approve(addresses.most, amount);

  let srcTokenAddress = ethers.zeroPadValue(
    ethers.getBytes(addresses.weth),
    32,
  );

  const destReceiverAddress = u8aToHex(
    new Keyring({ type: "sr25519" }).decodeAddress(receiver),
  );

  console.log(
    "Requesting transfer of",
    amount,
    "units of",
    addresses.weth,
    "to",
    receiver,
  );
  let tx_response = await most.sendRequest(
    srcTokenAddress,
    amount,
    destReceiverAddress,
  );
  console.log(
    "Tx hash:",
    tx_response.tx,
    ", block number:",
    tx_response.receipt.blockNumber,
  );

  console.log("Done");
  // NOTE: neccessary because script hangs in CI
  process.exit(0);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
